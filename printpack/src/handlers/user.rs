use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use askama::Template;
use bcrypt::{hash, verify, DEFAULT_COST};
use serde_json::json;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPageTemplate {
    pub lang: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterPageTemplate {
    pub lang: String,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Page handlers
// ---------------------------------------------------------------------------

pub async fn login_page(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    LoginPageTemplate {
        lang: "zh".to_string(),
        error: params.get("error").cloned(),
    }
}

pub async fn register_page(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    RegisterPageTemplate {
        lang: "zh".to_string(),
        error: params.get("error").cloned(),
    }
}

// ---------------------------------------------------------------------------
// API handlers
// ---------------------------------------------------------------------------

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), StatusCode> {
    // Validate
    if req.email.is_empty() || req.password.len() < 6 || req.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if email exists
    let existing = sqlx::query!("SELECT id FROM users WHERE email = ?", req.email)
        .fetch_optional(&state.pool)
        .await;

    if existing.is_ok() && existing.as_ref().is_some_and(|r| r.is_some()) {
        return Err(StatusCode::CONFLICT);
    }

    // Hash password
    let password_hash =
        hash(&req.password, DEFAULT_COST).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let uuid = uuid::Uuid::new_v4().to_string();

    // Insert user
    let result = sqlx::query!(
        "INSERT INTO users (uuid, email, password_hash, name, role, phone) VALUES (?, ?, ?, ?, 'customer', ?)",
        uuid, req.email, password_hash, req.name, req.phone
    )
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let user = UserResponse {
                id: 0,
                uuid,
                email: req.email,
                name: req.name,
                role: "customer".to_string(),
                phone: req.phone,
                address: None,
            };
            Ok((StatusCode::CREATED, Json(user)))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, uuid, email, password_hash, name, role, phone, address, created_at, updated_at FROM users WHERE email = ?",
        req.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = user.ok_or(StatusCode::UNAUTHORIZED)?;

    if !verify(&req.password, &user.password_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = crate::handlers::create_token(&user, &state.jwt_secret)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse::from(user),
    }))
}

pub async fn get_profile(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
) -> Result<Json<UserResponse>, StatusCode> {
    let claims = claims.0;
    let user = sqlx::query_as!(
        User,
        "SELECT id, uuid, email, password_hash, name, role, phone, address, created_at, updated_at FROM users WHERE uuid = ?",
        claims.sub
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match user {
        Some(u) => Ok(Json(UserResponse::from(u))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn update_profile(
    State(state): State<AppState>,
    claims: axum::extract::Extension<crate::handlers::Claims>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UserResponse>, StatusCode> {
    let claims = claims.0;
    sqlx::query!(
        "UPDATE users SET name = ?, phone = ?, address = ?, updated_at = CURRENT_TIMESTAMP WHERE uuid = ?",
        req.name, req.phone, req.address, claims.sub
    )
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = sqlx::query_as!(
        User,
        "SELECT id, uuid, email, password_hash, name, role, phone, address, created_at, updated_at FROM users WHERE uuid = ?",
        claims.sub
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(UserResponse::from(user)))
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
}
