//! Repository trait definitions for testability and dependency injection.
//!
//! This module provides trait-based abstractions over database operations,
//! enabling better testing through mock implementations and dependency injection.

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::auth::{AuthResult, Session, User};
use crate::wallet::{WalletResult, Wallet, WalletEntry, TableEscrow, FaucetClaim};

/// Trait for user/authentication repository operations
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        display_name: &str,
    ) -> AuthResult<i64>;

    /// Find user by username
    async fn find_by_username(&self, username: &str) -> AuthResult<Option<User>>;

    /// Find user by ID
    async fn find_by_id(&self, user_id: i64) -> AuthResult<Option<User>>;

    /// Update user's last login timestamp
    async fn update_last_login(&self, user_id: i64) -> AuthResult<()>;

    /// Deactivate user account
    async fn deactivate_user(&self, user_id: i64) -> AuthResult<()>;
}

/// Trait for session repository operations
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Create a new session
    async fn create_session(
        &self,
        user_id: i64,
        access_token: &str,
        refresh_token: &str,
        device_fingerprint: Option<&str>,
    ) -> AuthResult<Session>;

    /// Find session by access token
    async fn find_by_access_token(&self, token: &str) -> AuthResult<Option<Session>>;

    /// Find session by refresh token
    async fn find_by_refresh_token(&self, token: &str) -> AuthResult<Option<Session>>;

    /// Invalidate session
    async fn invalidate_session(&self, session_id: i64) -> AuthResult<()>;

    /// Invalidate all sessions for a user
    async fn invalidate_all_user_sessions(&self, user_id: i64) -> AuthResult<()>;
}

/// Trait for wallet repository operations
#[async_trait]
pub trait WalletRepository: Send + Sync {
    /// Get wallet for user
    async fn get_wallet(&self, user_id: i64) -> WalletResult<Wallet>;

    /// Get or create wallet for user
    async fn get_or_create_wallet(&self, user_id: i64, initial_balance: i64) -> WalletResult<Wallet>;

    /// Update wallet balance
    async fn update_balance(&self, user_id: i64, new_balance: i64) -> WalletResult<()>;

    /// Get wallet entries (transaction history)
    async fn get_entries(&self, user_id: i64, limit: i64) -> WalletResult<Vec<WalletEntry>>;

    /// Create wallet entry
    async fn create_entry(&self, entry: &WalletEntry) -> WalletResult<i64>;

    /// Get table escrow
    async fn get_escrow(&self, table_id: i64) -> WalletResult<TableEscrow>;

    /// Update escrow balance
    async fn update_escrow(&self, table_id: i64, new_balance: i64) -> WalletResult<()>;

    /// Get last faucet claim for user
    async fn get_last_faucet_claim(&self, user_id: i64) -> WalletResult<Option<FaucetClaim>>;

    /// Create faucet claim
    async fn create_faucet_claim(&self, claim: &FaucetClaim) -> WalletResult<i64>;
}

/// Default PostgreSQL implementation of UserRepository
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        display_name: &str,
    ) -> AuthResult<i64> {
        let row = sqlx::query(
            "INSERT INTO users (username, password_hash, display_name) VALUES ($1, $2, $3) RETURNING id"
        )
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    async fn find_by_username(&self, username: &str) -> AuthResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, display_name, avatar_url, email, country, timezone,
                    tos_version, privacy_version, is_active, is_admin, created_at, last_login
             FROM users WHERE username = $1"
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| User {
            id: r.get("id"),
            username: r.get("username"),
            display_name: r.get("display_name"),
            avatar_url: r.get("avatar_url"),
            email: r.get("email"),
            country: r.get("country"),
            timezone: r.get("timezone"),
            tos_version: r.get("tos_version"),
            privacy_version: r.get("privacy_version"),
            is_active: r.get("is_active"),
            is_admin: r.get("is_admin"),
            created_at: r.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            last_login: r.get::<Option<chrono::NaiveDateTime>, _>("last_login").map(|dt| dt.and_utc()),
        }))
    }

    async fn find_by_id(&self, user_id: i64) -> AuthResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id, username, display_name, avatar_url, email, country, timezone,
                    tos_version, privacy_version, is_active, is_admin, created_at, last_login
             FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| User {
            id: r.get("id"),
            username: r.get("username"),
            display_name: r.get("display_name"),
            avatar_url: r.get("avatar_url"),
            email: r.get("email"),
            country: r.get("country"),
            timezone: r.get("timezone"),
            tos_version: r.get("tos_version"),
            privacy_version: r.get("privacy_version"),
            is_active: r.get("is_active"),
            is_admin: r.get("is_admin"),
            created_at: r.get::<chrono::NaiveDateTime, _>("created_at").and_utc(),
            last_login: r.get::<Option<chrono::NaiveDateTime>, _>("last_login").map(|dt| dt.and_utc()),
        }))
    }

    async fn update_last_login(&self, user_id: i64) -> AuthResult<()> {
        sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn deactivate_user(&self, user_id: i64) -> AuthResult<()> {
        sqlx::query("UPDATE users SET is_active = FALSE WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Mock implementation for testing
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    pub struct MockUserRepository {
        users: Arc<Mutex<HashMap<i64, User>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl MockUserRepository {
        pub fn new() -> Self {
            Self {
                users: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(Mutex::new(1)),
            }
        }

        pub fn with_user(self, user: User) -> Self {
            self.users.lock().unwrap().insert(user.id, user);
            self
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create_user(
            &self,
            username: &str,
            password_hash: &str,
            display_name: &str,
        ) -> AuthResult<i64> {
            let mut next_id = self.next_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;

            let user = User {
                id,
                username: username.to_string(),
                display_name: display_name.to_string(),
                avatar_url: None,
                email: None,
                country: None,
                timezone: None,
                tos_version: 1,
                privacy_version: 1,
                is_active: true,
                is_admin: false,
                created_at: chrono::Utc::now(),
                last_login: None,
            };

            self.users.lock().unwrap().insert(id, user);
            Ok(id)
        }

        async fn find_by_username(&self, username: &str) -> AuthResult<Option<User>> {
            let users = self.users.lock().unwrap();
            Ok(users.values().find(|u| u.username == username).cloned())
        }

        async fn find_by_id(&self, user_id: i64) -> AuthResult<Option<User>> {
            Ok(self.users.lock().unwrap().get(&user_id).cloned())
        }

        async fn update_last_login(&self, _user_id: i64) -> AuthResult<()> {
            Ok(())
        }

        async fn deactivate_user(&self, user_id: i64) -> AuthResult<()> {
            if let Some(user) = self.users.lock().unwrap().get_mut(&user_id) {
                user.is_active = false;
            }
            Ok(())
        }
    }
}
