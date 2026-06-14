use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::Cookie;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TOKEN_DURATION_HOURS: i64 = 24 * 7; // 7 days

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user id
    pub email: String,
    pub role: String,
    pub exp: usize,
}

#[derive(Clone)]
pub struct AuthService {
    pub jwt_secret: Arc<String>,
}

impl AuthService {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret: Arc::new(jwt_secret),
        }
    }

    pub fn generate_token(&self, user_id: &str, email: &str, role: &str) -> anyhow::Result<String> {
        let exp = Utc::now()
            .checked_add_signed(Duration::hours(TOKEN_DURATION_HOURS))
            .unwrap()
            .timestamp() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            exp,
        };

        Ok(encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?)
    }

    pub fn verify_token(&self, token: &str) -> anyhow::Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// Extract token from cookie jar
    pub fn extract_token(&self, jar: &CookieJar) -> Option<String> {
        jar.get("auth_token")
            .map(|cookie| cookie.value().to_string())
    }

    /// Create auth cookie
    pub fn create_auth_cookie(&self, token: &str) -> Cookie<'static> {
        Cookie::build("auth_token", token.to_string())
            .path("/")
            .http_only(true)
            .max_age(time::Duration::hours(TOKEN_DURATION_HOURS))
            .finish()
    }

    /// Remove auth cookie
    pub fn remove_auth_cookie() -> Cookie<'static> {
        Cookie::build("auth_token", "")
            .path("/")
            .http_only(true)
            .max_age(time::Duration::seconds(0))
            .finish()
    }
}
