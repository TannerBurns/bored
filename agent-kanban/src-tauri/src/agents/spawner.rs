use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::{AgentKind, AgentRunConfig, AgentRunResult, LogCallback, LogLine, LogStream, RunOutcome};

/// Errors that can occur during agent execution
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Process timed out after {0} seconds")]
    Timeout(u64),

    #[error("Process was cancelled")]
    Cancelled,

    #[error("CLI not found: {0}")]
    CliNotFound(String),
}

/// Handle to a running agent process
pub struct AgentProcess {
    child: Child,
    cancelled: Arc<AtomicBool>,
}

impl AgentProcess {
    /// Start a new agent process
    pub fn spawn(
        command: &str,
        args: &[&str],
        working_dir: &std::path::Path,
        env_vars: &[(&str, &str)],
    ) -> Result<Self, SpawnError> {
        let mut cmd = Command::new(command);

        cmd.args(args)
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SpawnError::CliNotFound(command.to_string())
            } else {
                SpawnError::SpawnFailed(e)
            }
        })?;

        Ok(Self {
            child,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get a handle to cancel this process
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle {
            cancelled: self.cancelled.clone(),
        }
    }

    /// Wait for the process to complete, streaming output
    pub fn wait_with_output(
        mut self,
        timeout: Option<Duration>,
        on_log: Option<Arc<LogCallback>>,
    ) -> Result<(Option<i32>, RunOutcome), SpawnError> {
        let stdout = self.child.stdout.take();
        let stderr = self.child.stderr.take();
        let cancelled = self.cancelled.clone();

        let on_log_stdout = on_log.clone();
        let stdout_handle = stdout.map(|out| {
            thread::spawn(move || read_stream(out, LogStream::Stdout, on_log_stdout))
        });

        let on_log_stderr = on_log;
        let stderr_handle = stderr.map(|err| {
            thread::spawn(move || read_stream(err, LogStream::Stderr, on_log_stderr))
        });

        let deadline = timeout.map(|t| Instant::now() + t);

        loop {
            if cancelled.load(Ordering::Relaxed) {
                let _ = self.child.kill();
                // Wait for reader threads to finish before returning
                if let Some(h) = stdout_handle {
                    let _ = h.join();
                }
                if let Some(h) = stderr_handle {
                    let _ = h.join();
                }
                return Err(SpawnError::Cancelled);
            }

            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    let _ = self.child.kill();
                    // Wait for reader threads to finish before returning
                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }
                    return Err(SpawnError::Timeout(timeout.unwrap().as_secs()));
                }
            }

            match self.child.try_wait() {
                Ok(Some(status)) => {
                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }

                    let exit_code = status.code();
                    let outcome = if exit_code == Some(0) {
                        RunOutcome::Success
                    } else {
                        RunOutcome::Error
                    };

                    return Ok((exit_code, outcome));
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    return Err(SpawnError::SpawnFailed(e));
                }
            }
        }
    }

}

/// Handle to cancel a running process
#[derive(Clone)]
pub struct CancelHandle {
    cancelled: Arc<AtomicBool>,
}

impl CancelHandle {
    /// Signal the process to cancel
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Read a stream line by line, calling the callback for each line
fn read_stream<R: std::io::Read>(reader: R, stream: LogStream, on_log: Option<Arc<LogCallback>>) {
    let reader = BufReader::new(reader);

    for line in reader.lines() {
        match line {
            Ok(content) => {
                if let Some(ref callback) = on_log {
                    callback(LogLine {
                        stream,
                        content,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            Err(_) => break,
        }
    }
}

/// Callback for receiving the cancel handle after spawn
pub type OnSpawnCallback = Box<dyn FnOnce(CancelHandle) + Send>;

/// Run an agent with the given configuration
pub fn run_agent(
    config: AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
) -> Result<AgentRunResult, SpawnError> {
    run_agent_with_cancel_callback(config, on_log, None)
}

/// Run an agent with the given configuration, providing a callback to receive the cancel handle
pub fn run_agent_with_cancel_callback(
    config: AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
    on_spawn: Option<OnSpawnCallback>,
) -> Result<AgentRunResult, SpawnError> {
    let start_time = Instant::now();

    let (command, args) = match config.kind {
        AgentKind::Cursor => super::cursor::build_command(&config),
        AgentKind::Claude => super::claude::build_command(&config),
    };

    let env_vars = build_env_vars(&config);
    let env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let process = AgentProcess::spawn(
        &command,
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &config.repo_path,
        &env_refs,
    )?;

    // Provide the cancel handle to the caller before we start waiting
    if let Some(callback) = on_spawn {
        callback(process.cancel_handle());
    }

    let timeout = config.timeout_secs.map(Duration::from_secs);
    let result = process.wait_with_output(timeout, on_log);

    let duration_secs = start_time.elapsed().as_secs_f64();

    match result {
        Ok((exit_code, outcome)) => Ok(AgentRunResult {
            run_id: config.run_id,
            exit_code,
            status: outcome,
            summary: None, // Will be filled in by hooks
            duration_secs,
        }),
        Err(SpawnError::Timeout(secs)) => Ok(AgentRunResult {
            run_id: config.run_id,
            exit_code: None,
            status: RunOutcome::Timeout,
            summary: Some(format!("Process timed out after {} seconds", secs)),
            duration_secs,
        }),
        Err(SpawnError::Cancelled) => Ok(AgentRunResult {
            run_id: config.run_id,
            exit_code: None,
            status: RunOutcome::Cancelled,
            summary: Some("Process was cancelled".to_string()),
            duration_secs,
        }),
        Err(e) => Err(e),
    }
}

/// Build environment variables for the agent process
fn build_env_vars(config: &AgentRunConfig) -> Vec<(String, String)> {
    vec![
        (
            "AGENT_KANBAN_TICKET_ID".to_string(),
            config.ticket_id.clone(),
        ),
        ("AGENT_KANBAN_RUN_ID".to_string(), config.run_id.clone()),
        ("AGENT_KANBAN_API_URL".to_string(), config.api_url.clone()),
        (
            "AGENT_KANBAN_API_TOKEN".to_string(),
            config.api_token.clone(),
        ),
        (
            "AGENT_KANBAN_REPO_PATH".to_string(),
            config.repo_path.to_string_lossy().to_string(),
        ),
    ]
}

use crate::db::{Database, RunArtifacts, RunStatus};
use crate::lifecycle::{TransitionExecutor, LifecycleOutcome};

pub fn finalize_run(
    db: &Database,
    ticket_id: &str,
    run_id: &str,
    outcome: super::RunOutcome,
    artifacts: Option<RunArtifacts>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lifecycle_outcome = LifecycleOutcome::from_run_outcome(outcome);

    let status = match outcome {
        super::RunOutcome::Success => RunStatus::Finished,
        super::RunOutcome::Error | super::RunOutcome::Timeout => RunStatus::Error,
        super::RunOutcome::Cancelled => RunStatus::Aborted,
    };
    
    db.update_run_status(run_id, status, None, None)?;

    if let Some(arts) = artifacts {
        db.update_run_artifacts(run_id, &arts)?;
    }

    db.release_lock(ticket_id, run_id)?;

    let executor = TransitionExecutor::new(db);
    let result = executor.handle_run_completion(ticket_id, lifecycle_outcome)?;

    tracing::info!("Run {} finalized for ticket {}: {:?}", run_id, ticket_id, result);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_env_vars_includes_all_fields() {
        let config = AgentRunConfig {
            kind: AgentKind::Cursor,
            ticket_id: "ticket-123".to_string(),
            run_id: "run-456".to_string(),
            repo_path: PathBuf::from("/tmp/repo"),
            prompt: "test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "test-token".to_string(),
        };

        let env_vars = build_env_vars(&config);

        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_TICKET_ID" && v == "ticket-123"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_RUN_ID" && v == "run-456"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_API_URL" && v == "http://localhost:7432"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_API_TOKEN" && v == "test-token"));
        assert!(env_vars
            .iter()
            .any(|(k, v)| k == "AGENT_KANBAN_REPO_PATH" && v == "/tmp/repo"));
    }

    #[test]
    fn cancel_handle_sets_flag() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle = CancelHandle {
            cancelled: cancelled.clone(),
        };

        assert!(!cancelled.load(Ordering::Relaxed));
        handle.cancel();
        assert!(cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn spawn_error_cli_not_found_message() {
        let err = SpawnError::CliNotFound("nonexistent-cli".to_string());
        assert_eq!(err.to_string(), "CLI not found: nonexistent-cli");
    }

    #[test]
    fn spawn_error_timeout_message() {
        let err = SpawnError::Timeout(300);
        assert_eq!(err.to_string(), "Process timed out after 300 seconds");
    }

    #[test]
    fn spawn_error_cancelled_message() {
        let err = SpawnError::Cancelled;
        assert_eq!(err.to_string(), "Process was cancelled");
    }

    #[test]
    fn spawn_error_spawn_failed_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let err = SpawnError::SpawnFailed(io_err);
        assert!(err.to_string().contains("Failed to spawn process"));
    }

    #[test]
    fn env_vars_count() {
        let config = AgentRunConfig {
            kind: AgentKind::Claude,
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/"),
            prompt: "p".to_string(),
            timeout_secs: None,
            api_url: "http://x".to_string(),
            api_token: "tok".to_string(),
        };
        let env_vars = build_env_vars(&config);
        assert_eq!(env_vars.len(), 5);
    }
}
