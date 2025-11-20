//! HTTP API client for poker server.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// API client for communicating with poker server
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
    access_token: Option<String>,
    #[allow(dead_code)]
    refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoginRequest {
    username: String,
    password: String,
    totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegisterRequest {
    username: String,
    password: String,
    display_name: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
    #[allow(dead_code)]
    user_id: i64,
    #[allow(dead_code)]
    username: String,
}

#[derive(Debug, Deserialize)]
pub struct TableInfo {
    pub id: i64,
    pub name: String,
    pub max_players: i32,
    pub player_count: usize,
    pub small_blind: i64,
    pub big_blind: i64,
    pub is_private: bool,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            access_token: None,
            refresh_token: None,
        }
    }

    /// Register a new user
    pub async fn register(
        &mut self,
        username: String,
        password: String,
        display_name: String,
    ) -> Result<()> {
        let request = RegisterRequest {
            username,
            password,
            display_name,
            email: None,
        };

        let response = self
            .client
            .post(format!("{}/api/auth/register", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send register request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            anyhow::bail!("Registration failed: {}", error_text);
        }

        let auth_response: AuthResponse = response
            .json()
            .await
            .context("Failed to parse register response")?;

        self.access_token = Some(auth_response.access_token);
        self.refresh_token = Some(auth_response.refresh_token);

        Ok(())
    }

    /// Login with username and password
    pub async fn login(&mut self, username: String, password: String) -> Result<()> {
        let request = LoginRequest {
            username,
            password,
            totp_code: None,
        };

        let response = self
            .client
            .post(format!("{}/api/auth/login", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            anyhow::bail!("Login failed: {}", error_text);
        }

        let auth_response: AuthResponse = response
            .json()
            .await
            .context("Failed to parse login response")?;

        self.access_token = Some(auth_response.access_token);
        self.refresh_token = Some(auth_response.refresh_token);

        Ok(())
    }

    /// List all available tables
    pub async fn list_tables(&self) -> Result<Vec<TableInfo>> {
        let response = self
            .client
            .get(format!("{}/api/tables", self.base_url))
            .send()
            .await
            .context("Failed to list tables")?;

        let tables: Vec<TableInfo> = response
            .json()
            .await
            .context("Failed to parse table list")?;

        Ok(tables)
    }

    /// Get access token for WebSocket authentication
    #[allow(dead_code)]
    pub fn get_access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    /// Join a table with specified buy-in amount
    pub async fn join_table(&self, table_id: i64, buy_in: i64) -> Result<()> {
        let token = self.access_token.as_ref().context("Not authenticated")?;

        #[derive(Serialize)]
        struct JoinRequest {
            buy_in_amount: i64,
        }

        let request = JoinRequest {
            buy_in_amount: buy_in,
        };

        let response = self
            .client
            .post(format!("{}/api/tables/{}/join", self.base_url, table_id))
            .header("Authorization", format!("Bearer {}", token))
            .json(&request)
            .send()
            .await
            .context("Failed to send join request")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            anyhow::bail!("Join table failed: {}", error_text);
        }

        Ok(())
    }

    /// Get WebSocket URL for a table
    pub fn get_websocket_url(&self, table_id: i64) -> Result<String> {
        let token = self.access_token.as_ref().context("Not authenticated")?;

        let ws_url = self
            .base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        Ok(format!("{}/ws/{}?token={}", ws_url, table_id, token))
    }
}
