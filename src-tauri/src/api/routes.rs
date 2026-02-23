use axum::{
    middleware,
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::auth::auth_middleware;
use super::events::{sse_filtered, sse_handler};
use super::handlers::*;
use super::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/health/detailed", get(health_detailed));

    let protected_routes = Router::new()
        .route("/v1/stream", get(sse_handler))
        .route("/v1/stream/filtered", get(sse_filtered))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    app.layer(cors)
}
