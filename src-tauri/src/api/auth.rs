use axum::{
    body::Body,
    extract::{Query, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::Deserialize;

use super::state::AppState;

pub const AUTH_HEADER: &str = "X-AgentKanban-Token";

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = request
        .headers()
        .get(AUTH_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or(query.token);

    match token {
        Some(t) if t == state.api_token => Ok(next.run(request).await),
        Some(t) => {
            // Log at debug level with full details, warn level with just the path
            // This happens when Cursor IDE has cached stale hooks.json
            tracing::debug!(
                "Invalid API token: received '{}' expected '{}'",
                t,
                state.api_token
            );
            tracing::warn!(
                "Invalid API token for {} {} (see docs/guides/06-cursor-integration.md for troubleshooting)",
                request.method(),
                request.uri().path()
            );
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            tracing::warn!(
                "Missing API token for {} {}",
                request.method(),
                request.uri().path()
            );
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

pub fn generate_token() -> String {
    use rand::Rng;
    
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const TOKEN_LENGTH: usize = 32;
    
    let mut rng = rand::thread_rng();
    (0..TOKEN_LENGTH)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token1 = generate_token();
        let token2 = generate_token();
        
        assert_eq!(token1.len(), 32);
        assert_eq!(token2.len(), 32);
        assert_ne!(token1, token2);
        assert!(token1.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
