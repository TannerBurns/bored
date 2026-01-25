use axum::{
    middleware,
    routing::{get, post, patch, delete},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::auth::auth_middleware;
use super::handlers::*;
use super::events::{sse_handler, sse_filtered};
use super::state::AppState;

pub fn create_router(state: AppState) -> Router {
    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/health/detailed", get(health_detailed));

    // Protected routes (auth required)
    let protected_routes = Router::new()
        // Boards
        .route("/v1/boards", get(list_boards))
        .route("/v1/boards/:board_id", get(get_board))
        .route("/v1/boards/:board_id/columns", get(list_columns))
        .route("/v1/boards/:board_id/tickets", get(list_tickets))
        
        // Tickets
        .route("/v1/tickets", post(create_ticket))
        .route("/v1/tickets/:ticket_id", get(get_ticket))
        .route("/v1/tickets/:ticket_id", patch(update_ticket))
        .route("/v1/tickets/:ticket_id", delete(delete_ticket))
        .route("/v1/tickets/:ticket_id/move", post(move_ticket))
        .route("/v1/tickets/:ticket_id/reserve", post(reserve_ticket))
        .route("/v1/tickets/:ticket_id/comments", get(list_comments))
        .route("/v1/tickets/:ticket_id/comments", post(create_comment))
        .route("/v1/tickets/:ticket_id/runs", get(list_runs))
        
        // Runs
        .route("/v1/runs", post(create_run))
        .route("/v1/runs/:run_id", get(get_run))
        .route("/v1/runs/:run_id", patch(update_run))
        .route("/v1/runs/:run_id/heartbeat", post(heartbeat))
        .route("/v1/runs/:run_id/release", post(release_run))
        .route("/v1/runs/:run_id/events", get(list_events))
        .route("/v1/runs/:run_id/events", post(create_event))
        
        // Queue
        .route("/v1/queue/next", post(queue_next))
        .route("/v1/queue/status", get(queue_status))
        
        // Real-time updates (SSE)
        .route("/v1/stream", get(sse_handler))
        .route("/v1/stream/filtered", get(sse_filtered))
        
        // Apply auth middleware
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Combine routes with state
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state);

    // Add CORS for local development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    app.layer(cors)
}
