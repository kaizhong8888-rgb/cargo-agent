use crate::{AppState, auth::AuthService};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

// Show register page
pub async fn show_register() -> impl IntoResponse {
    Html(crate::routes::page::RegisterTemplate { error: None }.render().unwrap())
}

// Handle registration
pub async fn handle_register(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> impl IntoResponse {
    // Validate
    if form.email.is_empty() || form.password.len() < 6 || form.name.is_empty() {
        return Html(
            crate::routes::page::RegisterTemplate {
                error: Some("请填写完整信息，密码至少6位".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Check if email exists
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = ?")
            .bind(&form.email)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    if existing.is_some() {
        return Html(
            crate::routes::page::RegisterTemplate {
                error: Some("该邮箱已被注册".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(form.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => {
            return Html(
                crate::routes::page::RegisterTemplate {
                    error: Some("注册失败，请重试".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Insert user
    let user_id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO users (id, email, password_hash, name, role) VALUES (?, ?, ?, ?, 'customer')"
    )
    .bind(&user_id)
    .bind(&form.email)
    .bind(&password_hash)
    .bind(&form.name)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            // Auto login
            let token = match state.auth.generate_token(&user_id, &form.email, "customer") {
                Ok(t) => t,
                Err(_) => return Redirect::to("/login").into_response(),
            };

            let cookie = state.auth.create_auth_cookie(&token);
            (
                [("Set-Cookie", cookie.encoded().to_string())],
                Redirect::to("/"),
            )
                .into_response()
        }
        Err(_) => Html(
            crate::routes::page::RegisterTemplate {
                error: Some("注册失败，请重试".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

// Show login page
pub async fn show_login() -> impl IntoResponse {
    Html(crate::routes::page::LoginTemplate { error: None }.render().unwrap())
}

// Handle login
pub async fn handle_login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Fetch user
    let user: Option<(String, String, String)> =
        sqlx::query_as("SELECT id, email, password_hash FROM users WHERE email = ?")
            .bind(&form.email)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let (user_id, email, password_hash) = match user {
        Some(u) => u,
        None => {
            return Html(
                crate::routes::page::LoginTemplate {
                    error: Some("邮箱或密码错误".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    // Verify password
    let argon2 = Argon2::default();
    let parsed_hash = match password_hash.parse() {
        Ok(h) => h,
        Err(_) => {
            return Html(
                crate::routes::page::LoginTemplate {
                    error: Some("登录失败，请重试".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    if argon2
        .verify_password(form.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Html(
            crate::routes::page::LoginTemplate {
                error: Some("邮箱或密码错误".to_string()),
            }
            .render()
            .unwrap(),
        )
        .into_response();
    }

    // Get user role
    let role: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(("customer".to_string(),));

    // Generate token
    let token = match state.auth.generate_token(&user_id, &email, &role.0) {
        Ok(t) => t,
        Err(_) => {
            return Html(
                crate::routes::page::LoginTemplate {
                    error: Some("登录失败，请重试".to_string()),
                }
                .render()
                .unwrap(),
            )
            .into_response();
        }
    };

    let cookie = state.auth.create_auth_cookie(&token);
    (
        [("Set-Cookie", cookie.encoded().to_string())],
        Redirect::to("/"),
    )
        .into_response()
}

// Handle logout
pub async fn handle_logout() -> impl IntoResponse {
    let cookie = AuthService::remove_auth_cookie();
    (
        [("Set-Cookie", cookie.encoded().to_string())],
        Redirect::to("/"),
    )
        .into_response()
}
