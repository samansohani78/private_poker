//! Authentication manager implementation.

use super::{
    errors::{AuthError, AuthResult},
    models::{
        AccessTokenClaims, LoginRequest, RegisterRequest, SessionTokens, User, UserId,
    },
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

/// Authentication manager
#[derive(Clone)]
pub struct AuthManager {
    pool: Arc<PgPool>,
    pepper: String,
    jwt_secret: String,
    access_token_duration: Duration,
    refresh_token_duration: Duration,
}

impl AuthManager {
    /// Create a new authentication manager
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    /// * `pepper` - Server-side pepper for password hashing
    /// * `jwt_secret` - Secret key for JWT signing
    ///
    /// # Returns
    ///
    /// * `AuthManager` - New authentication manager instance
    pub fn new(pool: Arc<PgPool>, pepper: String, jwt_secret: String) -> Self {
        Self {
            pool,
            pepper,
            jwt_secret,
            access_token_duration: Duration::minutes(15),  // 15 minutes
            refresh_token_duration: Duration::days(7),     // 7 days
        }
    }

    /// Register a new user
    ///
    /// # Arguments
    ///
    /// * `request` - Registration request with username, password, etc.
    ///
    /// # Returns
    ///
    /// * `AuthResult<User>` - Created user or error
    ///
    /// # Errors
    ///
    /// * `AuthError::UsernameTaken` - Username already exists
    /// * `AuthError::EmailTaken` - Email already exists
    /// * `AuthError::InvalidUsername` - Username format invalid
    /// * `AuthError::WeakPassword` - Password too weak
    pub async fn register(&self, request: RegisterRequest) -> AuthResult<User> {
        // Validate username
        self.validate_username(&request.username)?;

        // Validate password strength
        self.validate_password(&request.password)?;

        // Check if username exists
        let existing_user = sqlx::query("SELECT id FROM users WHERE username = $1")
            .bind(&request.username)
            .fetch_optional(self.pool.as_ref())
            .await?;

        if existing_user.is_some() {
            return Err(AuthError::UsernameTaken);
        }

        // Check if email exists (if provided)
        if let Some(ref email) = request.email {
            let existing_email = sqlx::query("SELECT id FROM users WHERE email = $1")
                .bind(email)
                .fetch_optional(self.pool.as_ref())
                .await?;

            if existing_email.is_some() {
                return Err(AuthError::EmailTaken);
            }
        }

        // Hash password with Argon2id + pepper
        let password_hash = self.hash_password(&request.password)?;

        // Insert user
        let row = sqlx::query(
            r#"
            INSERT INTO users (username, password_hash, display_name, email)
            VALUES ($1, $2, $3, $4)
            RETURNING id, username, display_name, avatar_url, email, country, timezone,
                      tos_version, privacy_version, is_active, is_admin, created_at, last_login
            "#,
        )
        .bind(&request.username)
        .bind(&password_hash)
        .bind(&request.display_name)
        .bind(&request.email)
        .fetch_one(self.pool.as_ref())
        .await?;

        let user = User {
            id: row.get("id"),
            username: row.get("username"),
            display_name: row.get("display_name"),
            avatar_url: row.get("avatar_url"),
            email: row.get("email"),
            country: row.get("country"),
            timezone: row.get("timezone"),
            tos_version: row.get("tos_version"),
            privacy_version: row.get("privacy_version"),
            is_active: row.get("is_active"),
            is_admin: row.get("is_admin"),
            created_at: row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            last_login: row.get::<Option<chrono::NaiveDateTime>, _>("last_login").map(|dt| dt.and_utc()),
        };

        // Create wallet for new user
        sqlx::query("INSERT INTO wallets (user_id, balance) VALUES ($1, 10000)")
            .bind(user.id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(user)
    }

    /// Login a user
    ///
    /// # Arguments
    ///
    /// * `request` - Login request with username and password
    /// * `device_fingerprint` - Device fingerprint (User-Agent + IP hash)
    ///
    /// # Returns
    ///
    /// * `AuthResult<(User, SessionTokens)>` - User and session tokens or error
    ///
    /// # Errors
    ///
    /// * `AuthError::UserNotFound` - User doesn't exist
    /// * `AuthError::InvalidPassword` - Incorrect password
    /// * `AuthError::TwoFactorRequired` - 2FA code required but not provided
    /// * `AuthError::InvalidTwoFactorCode` - Invalid 2FA code
    pub async fn login(
        &self,
        request: LoginRequest,
        device_fingerprint: String,
    ) -> AuthResult<(User, SessionTokens)> {
        // Fetch user with password hash
        let user_row = sqlx::query(
            r#"
            SELECT id, username, password_hash, display_name, avatar_url, email, country, timezone,
                   tos_version, privacy_version, is_active, is_admin, created_at, last_login
            FROM users
            WHERE username = $1
            "#,
        )
        .bind(&request.username)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(AuthError::UserNotFound)?;

        // Verify password
        let password_hash: String = user_row.get("password_hash");
        self.verify_password(&request.password, &password_hash)?;

        // Check if 2FA is enabled
        let two_factor = sqlx::query("SELECT secret, enabled FROM two_factor_auth WHERE user_id = $1")
            .bind(user_row.get::<i64, _>("id"))
            .fetch_optional(self.pool.as_ref())
            .await?;

        if let Some(two_factor_row) = two_factor {
            let enabled: bool = two_factor_row.get("enabled");
            if enabled {
                // 2FA is enabled, verify code
                let totp_code = request.totp_code.ok_or(AuthError::TwoFactorRequired)?;
                let secret: String = two_factor_row.get("secret");
                self.verify_totp(&secret, &totp_code)?;
            }
        }

        // Create user object
        let user = User {
            id: user_row.get("id"),
            username: user_row.get("username"),
            display_name: user_row.get("display_name"),
            avatar_url: user_row.get("avatar_url"),
            email: user_row.get("email"),
            country: user_row.get("country"),
            timezone: user_row.get("timezone"),
            tos_version: user_row.get("tos_version"),
            privacy_version: user_row.get("privacy_version"),
            is_active: user_row.get("is_active"),
            is_admin: user_row.get("is_admin"),
            created_at: user_row.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            last_login: user_row.get::<Option<chrono::NaiveDateTime>, _>("last_login").map(|dt| dt.and_utc()),
        };

        // Update last login
        sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
            .bind(user.id)
            .execute(self.pool.as_ref())
            .await?;

        // Generate tokens
        let tokens = self.create_session(user.id, &user.username, user.is_admin, device_fingerprint).await?;

        Ok((user, tokens))
    }

    /// Create a new session with access and refresh tokens
    async fn create_session(
        &self,
        user_id: UserId,
        username: &str,
        is_admin: bool,
        device_fingerprint: String,
    ) -> AuthResult<SessionTokens> {
        // Generate access token (JWT)
        let access_token = self.generate_access_token(user_id, username, is_admin)?;

        // Generate refresh token (UUID)
        let refresh_token = Uuid::new_v4().to_string();

        // Store refresh token in database
        let expires_at = Utc::now() + self.refresh_token_duration;
        sqlx::query(
            r#"
            INSERT INTO sessions (token, user_id, device_fingerprint, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&refresh_token)
        .bind(user_id)
        .bind(&device_fingerprint)
        .bind(expires_at.naive_utc())
        .execute(self.pool.as_ref())
        .await?;

        Ok(SessionTokens {
            access_token,
            refresh_token,
        })
    }

    /// Refresh access token using refresh token
    ///
    /// # Arguments
    ///
    /// * `refresh_token` - Refresh token
    /// * `device_fingerprint` - Device fingerprint
    ///
    /// # Returns
    ///
    /// * `AuthResult<SessionTokens>` - New access and refresh tokens
    ///
    /// # Errors
    ///
    /// * `AuthError::InvalidRefreshToken` - Refresh token not found
    /// * `AuthError::SessionExpired` - Refresh token expired
    pub async fn refresh_token(
        &self,
        refresh_token: String,
        device_fingerprint: String,
    ) -> AuthResult<SessionTokens> {
        // Fetch session
        let session_row = sqlx::query(
            r#"
            SELECT token, user_id, device_fingerprint, created_at, expires_at, last_used
            FROM sessions
            WHERE token = $1
            "#,
        )
        .bind(&refresh_token)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(AuthError::InvalidRefreshToken)?;

        // Check if expired
        let expires_at = session_row.get::<chrono::NaiveDateTime, _>("expires_at").and_utc();
        if expires_at < Utc::now() {
            // Delete expired session
            sqlx::query("DELETE FROM sessions WHERE token = $1")
                .bind(&refresh_token)
                .execute(self.pool.as_ref())
                .await?;
            return Err(AuthError::SessionExpired);
        }

        // Verify device fingerprint matches
        let stored_fingerprint: String = session_row.get("device_fingerprint");
        if stored_fingerprint != device_fingerprint {
            return Err(AuthError::InvalidRefreshToken);
        }

        // Fetch user
        let user_id: i64 = session_row.get("user_id");
        let user_row = sqlx::query(
            r#"
            SELECT id, username, display_name, avatar_url, email, country, timezone,
                   tos_version, privacy_version, is_active, is_admin, created_at, last_login
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or(AuthError::UserNotFound)?;

        let username: String = user_row.get("username");
        let is_admin: bool = user_row.get("is_admin");

        // Delete old refresh token (rotation)
        sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(&refresh_token)
            .execute(self.pool.as_ref())
            .await?;

        // Create new session with rotated tokens
        let new_tokens = self
            .create_session(user_id, &username, is_admin, device_fingerprint)
            .await?;

        Ok(new_tokens)
    }

    /// Logout user by invalidating refresh token
    ///
    /// # Arguments
    ///
    /// * `refresh_token` - Refresh token to invalidate
    ///
    /// # Returns
    ///
    /// * `AuthResult<()>` - Success or error
    pub async fn logout(&self, refresh_token: String) -> AuthResult<()> {
        sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(&refresh_token)
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    /// Verify an access token
    ///
    /// # Arguments
    ///
    /// * `token` - JWT access token
    ///
    /// # Returns
    ///
    /// * `AuthResult<AccessTokenClaims>` - Decoded claims or error
    pub fn verify_access_token(&self, token: &str) -> AuthResult<AccessTokenClaims> {
        let token_data = decode::<AccessTokenClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// Hash password with Argon2id + pepper
    fn hash_password(&self, password: &str) -> AuthResult<String> {
        // Add pepper to password
        let peppered = format!("{}{}", password, self.pepper);
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        Ok(argon2
            .hash_password(peppered.as_bytes(), &salt)
            .map_err(|_| AuthError::HashingFailed)?
            .to_string())
    }

    /// Verify password against hash
    fn verify_password(&self, password: &str, hash: &str) -> AuthResult<()> {
        let peppered = format!("{}{}", password, self.pepper);
        let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::InvalidPassword)?;
        let argon2 = Argon2::default();

        argon2
            .verify_password(peppered.as_bytes(), &parsed_hash)
            .map_err(|_| AuthError::InvalidPassword)
    }

    /// Generate JWT access token
    fn generate_access_token(
        &self,
        user_id: UserId,
        username: &str,
        is_admin: bool,
    ) -> AuthResult<String> {
        let now = Utc::now();
        let claims = AccessTokenClaims {
            sub: user_id,
            username: username.to_string(),
            is_admin,
            exp: (now + self.access_token_duration).timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// Verify TOTP code
    fn verify_totp(&self, secret: &str, code: &str) -> AuthResult<()> {
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            Secret::Encoded(secret.to_string())
                .to_bytes()
                .map_err(|_| AuthError::InvalidTwoFactorCode)?,
        )
        .map_err(|_| AuthError::InvalidTwoFactorCode)?;

        if totp.check_current(code).map_err(|_| AuthError::InvalidTwoFactorCode)? {
            Ok(())
        } else {
            Err(AuthError::InvalidTwoFactorCode)
        }
    }

    /// Validate username format
    fn validate_username(&self, username: &str) -> AuthResult<()> {
        let len = username.len();
        if len < 3 || len > 20 {
            return Err(AuthError::InvalidUsername(
                "Username must be 3-20 characters".to_string(),
            ));
        }

        if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(AuthError::InvalidUsername(
                "Username can only contain letters, numbers, and underscores".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate password strength
    fn validate_password(&self, password: &str) -> AuthResult<()> {
        if password.len() < 8 {
            return Err(AuthError::WeakPassword(
                "Password must be at least 8 characters".to_string(),
            ));
        }

        // Check for at least one number, one uppercase, one lowercase
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());

        if !has_digit || !has_uppercase || !has_lowercase {
            return Err(AuthError::WeakPassword(
                "Password must contain at least one number, one uppercase and one lowercase letter"
                    .to_string(),
            ));
        }

        Ok(())
    }
}
