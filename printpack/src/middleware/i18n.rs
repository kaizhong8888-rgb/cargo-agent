use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;

pub async fn i18n_middleware(
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Response {
    let lang = jar
        .get("lang")
        .map(|c| c.value().to_string())
        .or_else(|| {
            request
                .headers()
                .get("Accept-Language")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| {
                    if h.starts_with("zh") {
                        Some("zh".to_string())
                    } else if h.starts_with("en") {
                        Some("en".to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "zh".to_string());

    request.extensions_mut().insert(lang);
    next.run(request).await
}
