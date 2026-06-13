use crate::models::*;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use serde_json::json;
use sqlx::SqlitePool;
use tracing::error;

// Error type
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Internal error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: String,
}

// JWT claims
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Duration, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
}

pub fn create_token(user: &User, secret: &str) -> Result<String, AppError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.uuid.clone(),
        email: user.email.clone(),
        role: user.role.clone(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal)
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized("Invalid token".into()))
}

// Auth middleware
pub async fn auth_middleware(
    jar: CookieJar,
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar
        .get("auth_token")
        .map(|c| c.value().to_string())
        .or_else(|| {
            request
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer ").map(String::from))
        });

    let token = token.ok_or_else(|| AppError::Unauthorized("No token provided".into()))?;
    let claims = decode_token(&token, &state.jwt_secret)?;

    let mut request = request;
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

// Optional auth middleware (doesn't fail if no token)
pub async fn optional_auth_middleware(
    jar: CookieJar,
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = jar
        .get("auth_token")
        .map(|c| c.value().to_string())
        .or_else(|| {
            request
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer ").map(String::from))
        });

    if let Some(token) = token {
        if let Ok(claims) = decode_token(&token, &state.jwt_secret) {
            request.extensions_mut().insert(claims);
        }
    }

    next.run(request).await
}
