pub mod auth;
pub mod cleanup;
pub mod error;
pub mod events;
pub mod handlers;
pub mod routes;
pub mod state;

use crate::db::Database;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

pub use auth::generate_token;
pub use cleanup::{start_cleanup_service, CleanupConfig};
pub use error::{ApiError, ApiResult, AppError};
pub use state::{AppState, LiveEvent};

/// Create a new event broadcaster channel
pub fn create_event_channel() -> broadcast::Sender<LiveEvent> {
    let (tx, _) = broadcast::channel(1024);
    tx
}

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

/// Start the API server
pub async fn start_server(
    db: Arc<Database>,
    config: ApiConfig,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let state = AppState::new(db.clone(), config.token.clone());
    start_server_with_state(db, config, state).await
}

/// Start the API server with an externally provided event channel
pub async fn start_server_with_event_tx(
    db: Arc<Database>,
    config: ApiConfig,
    event_tx: broadcast::Sender<LiveEvent>,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let state = AppState::with_event_tx(db.clone(), config.token.clone(), event_tx);
    start_server_with_state(db, config, state).await
}

/// Bind a TCP listener with retries. During Tauri updater restarts the old
/// process may still hold the port briefly; back off and retry so the new
/// process doesn't silently lose its API server.
async fn bind_with_retry(
    addr: SocketAddr,
) -> Result<tokio::net::TcpListener, Box<dyn std::error::Error + Send + Sync>> {
    const MAX_RETRIES: u32 = 10;
    const BASE_DELAY_MS: u64 = 150;
    const MAX_DELAY_MS: u64 = 2000;

    let mut last_err = None;
    for attempt in 0..MAX_RETRIES {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if attempt > 0 {
                    tracing::info!(
                        "API server bound to {} after {} retries",
                        addr,
                        attempt
                    );
                }
                return Ok(listener);
            }
            Err(e) => {
                let delay = Duration::from_millis(
                    (BASE_DELAY_MS * 2u64.pow(attempt)).min(MAX_DELAY_MS),
                );
                tracing::warn!(
                    "Port {} in use (attempt {}/{}), retrying in {:?}: {}",
                    addr.port(),
                    attempt + 1,
                    MAX_RETRIES,
                    delay,
                    e,
                );
                tokio::time::sleep(delay).await;
                last_err = Some(e);
            }
        }
    }

    Err(Box::new(last_err.unwrap()))
}

/// Start the API server with a pre-configured AppState
async fn start_server_with_state(
    db: Arc<Database>,
    config: ApiConfig,
    state: AppState,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let router = routes::create_router(state);

    let addr = SocketAddr::from((config.host, config.port));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let listener = bind_with_retry(addr).await?;
    let actual_addr = listener.local_addr()?;

    tracing::info!("API server listening on http://{}", actual_addr);

    start_cleanup_service(db, CleanupConfig::default());

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[tokio::test]
    async fn bind_with_retry_succeeds_on_free_port() {
        let listener = bind_with_retry(localhost(0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn bind_with_retry_succeeds_after_port_released() {
        let blocker = tokio::net::TcpListener::bind(localhost(0)).await.unwrap();
        let port = blocker.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            bind_with_retry(localhost(port)).await
        });

        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(blocker);

        let result = handle.await.unwrap();
        assert!(result.is_ok(), "should bind after blocker is dropped");
        assert_eq!(result.unwrap().local_addr().unwrap().port(), port);
    }

    #[test]
    fn backoff_delay_caps_at_max() {
        const BASE_DELAY_MS: u64 = 150;
        const MAX_DELAY_MS: u64 = 2000;

        for attempt in 0..10u32 {
            let delay = (BASE_DELAY_MS * 2u64.pow(attempt)).min(MAX_DELAY_MS);
            assert!(delay <= MAX_DELAY_MS, "attempt {attempt}: {delay} > {MAX_DELAY_MS}");
        }
        let at_cap = (BASE_DELAY_MS * 2u64.pow(4)).min(MAX_DELAY_MS);
        assert_eq!(at_cap, MAX_DELAY_MS);
    }
}
