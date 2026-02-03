//! Agent execution with retry logic.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{AgentRunConfig, AgentRunResult, LogCallback, RunOutcome};
use super::cancel::CancelHandle;
use super::config::{INITIAL_BACKOFF_MS, MAX_TRANSIENT_RETRIES};
use super::error::SpawnError;
use super::process::AgentProcess;
use super::utils::{build_env_vars, is_transient_error};

pub type OnSpawnCallback = Box<dyn FnOnce(CancelHandle) + Send>;

pub fn run_agent(
    config: AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
) -> Result<AgentRunResult, SpawnError> {
    run_agent_with_cancel_callback(config, on_log, None)
}

/// Retries on transient network errors with exponential backoff.
pub fn run_agent_with_cancel_callback(
    config: AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
    on_spawn: Option<OnSpawnCallback>,
) -> Result<AgentRunResult, SpawnError> {
    tracing::debug!("run_agent: {:?} run_id={}", config.kind, config.run_id);
    let start_time = Instant::now();

    let (command, args) = match config.kind {
        super::super::AgentKind::Cursor => super::super::cursor::build_command(&config),
        super::super::AgentKind::Claude => super::super::claude::build_command(&config),
    };

    let env_vars = build_env_vars(&config);
    let env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let global_deadline = config
        .timeout_secs
        .map(|secs| Instant::now() + Duration::from_secs(secs));
    let mut attempt = 0;
    let mut on_spawn = on_spawn;

    loop {
        attempt += 1;

        if attempt > 1 {
            let backoff_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 2);
            tracing::debug!("Retry {} for run {} after {}ms", attempt, config.run_id, backoff_ms);
            thread::sleep(Duration::from_millis(backoff_ms));
        }

        let remaining_timeout = global_deadline.map(|deadline| {
            let now = Instant::now();
            if now >= deadline {
                Duration::ZERO
            } else {
                deadline - now
            }
        });

        if let Some(remaining) = remaining_timeout {
            if remaining.is_zero() {
                let duration_secs = start_time.elapsed().as_secs_f64();
                tracing::warn!("Timeout before attempt {} for run {}", attempt, config.run_id);
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Timeout,
                    summary: Some(format!(
                        "Process timed out after {} seconds",
                        config.timeout_secs.unwrap_or(0)
                    )),
                    duration_secs,
                    captured_stdout: None,
                });
            }
        }

        let process = AgentProcess::spawn(
            &command,
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            &config.repo_path,
            &env_refs,
        )?;

        if let Some(callback) = on_spawn.take() {
            callback(process.cancel_handle());
        }

        let result = process.wait_with_capture(remaining_timeout, on_log.clone(), true);

        match result {
            Ok((exit_code, outcome, captured_stdout, captured_stderr)) => {
                if outcome == RunOutcome::Error {
                    if let Some(ref stderr) = captured_stderr {
                        if !stderr.is_empty() {
                            tracing::error!("Run {} stderr: {}", config.run_id, stderr);
                        }
                    }
                }

                if outcome == RunOutcome::Error && attempt < MAX_TRANSIENT_RETRIES {
                    if let Some(ref stderr) = captured_stderr {
                        if is_transient_error(stderr) {
                            tracing::warn!("Transient error on attempt {} for {}", attempt, config.run_id);
                            continue;
                        }
                    }
                }

                let duration_secs = start_time.elapsed().as_secs_f64();

                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code,
                    status: outcome,
                    summary: None,
                    duration_secs,
                    captured_stdout,
                });
            }
            Err(SpawnError::Timeout(secs)) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Timeout,
                    summary: Some(format!("Process timed out after {} seconds", secs)),
                    duration_secs,
                    captured_stdout: None,
                });
            }
            Err(SpawnError::Cancelled) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Cancelled,
                    summary: Some("Process was cancelled".to_string()),
                    duration_secs,
                    captured_stdout: None,
                });
            }
            Err(e) => return Err(e),
        }
    }
}

/// Captures stdout for multi-stage workflows. Retries on transient errors.
pub fn run_agent_with_capture(
    config: AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
    on_spawn: Option<OnSpawnCallback>,
) -> Result<AgentRunResult, SpawnError> {
    tracing::debug!("run_agent_with_capture: {:?} run_id={}", config.kind, config.run_id);
    let start_time = Instant::now();

    let (command, args) = match config.kind {
        super::super::AgentKind::Cursor => super::super::cursor::build_command(&config),
        super::super::AgentKind::Claude => super::super::claude::build_command(&config),
    };

    let env_vars = build_env_vars(&config);
    let env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let global_deadline = config
        .timeout_secs
        .map(|secs| Instant::now() + Duration::from_secs(secs));
    let mut attempt = 0;
    let mut on_spawn = on_spawn;

    loop {
        attempt += 1;

        if attempt > 1 {
            let backoff_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 2);
            tracing::debug!("Retry {} for run {} after {}ms", attempt, config.run_id, backoff_ms);
            thread::sleep(Duration::from_millis(backoff_ms));
        }

        let remaining_timeout = global_deadline.map(|deadline| {
            let now = Instant::now();
            if now >= deadline {
                Duration::ZERO
            } else {
                deadline - now
            }
        });

        if let Some(remaining) = remaining_timeout {
            if remaining.is_zero() {
                let duration_secs = start_time.elapsed().as_secs_f64();
                tracing::warn!("Timeout before attempt {} for {}", attempt, config.run_id);
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Timeout,
                    summary: Some(format!(
                        "Process timed out after {} seconds",
                        config.timeout_secs.unwrap_or(0)
                    )),
                    duration_secs,
                    captured_stdout: None,
                });
            }
        }

        let process = AgentProcess::spawn(
            &command,
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            &config.repo_path,
            &env_refs,
        )?;

        if let Some(callback) = on_spawn.take() {
            callback(process.cancel_handle());
        }

        let result = process.wait_with_capture(remaining_timeout, on_log.clone(), true);

        match result {
            Ok((exit_code, outcome, captured_stdout, captured_stderr)) => {
                if outcome == RunOutcome::Error {
                    if let Some(ref stderr) = captured_stderr {
                        if !stderr.is_empty() {
                            tracing::error!("Run {} stderr: {}", config.run_id, stderr);
                        }
                    }
                }

                if outcome == RunOutcome::Error && attempt < MAX_TRANSIENT_RETRIES {
                    if let Some(ref stderr) = captured_stderr {
                        if is_transient_error(stderr) {
                            tracing::warn!("Transient error on attempt {} for {}", attempt, config.run_id);
                            continue;
                        }
                    }
                }

                let duration_secs = start_time.elapsed().as_secs_f64();

                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code,
                    status: outcome,
                    summary: None,
                    duration_secs,
                    captured_stdout,
                });
            }
            Err(SpawnError::Timeout(secs)) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Timeout,
                    summary: Some(format!("Process timed out after {} seconds", secs)),
                    duration_secs,
                    captured_stdout: None,
                });
            }
            Err(SpawnError::Cancelled) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                return Ok(AgentRunResult {
                    run_id: config.run_id,
                    exit_code: None,
                    status: RunOutcome::Cancelled,
                    summary: Some("Process was cancelled".to_string()),
                    duration_secs,
                    captured_stdout: None,
                });
            }
            Err(e) => return Err(e),
        }
    }
}
