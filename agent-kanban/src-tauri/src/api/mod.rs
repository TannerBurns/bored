pub mod auth;
pub mod error;
pub mod events;
pub mod handlers;
pub mod routes;
pub mod spool;
pub mod state;
pub mod types;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::db::Database;

pub use auth::generate_token;
pub use state::AppState;
pub use error::{ApiError, AppError, ApiResult};
pub use spool::{start_spool_processor, get_default_spool_dir};

/// API server configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub port: u16,
    pub token: String,
    pub host: [u8; 4],
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            port: 7432,
            token: generate_token(),
            host: [127, 0, 0, 1],
        }
    }
}

/// Server handle for managing the running server
pub struct ServerHandle {
    pub addr: SocketAddr,
    pub shutdown_tx: oneshot::Sender<()>,
}

impl ServerHandle {
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Start the API server
pub async fn start_server(
    db: Arc<Database>,
    config: ApiConfig,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let state = AppState::new(db, config.token.clone());
    let router = routes::create_router(state);

    let addr = SocketAddr::from((config.host, config.port));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    tracing::info!("API server listening on http://{}", actual_addr);
    tracing::info!("API token: {}", config.token);

    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
                tracing::info!("API server shutting down");
            })
            .await
            .expect("API server error");
    });

    Ok(ServerHandle {
        addr: actual_addr,
        shutdown_tx,
    })
}
