//! Agent execution with retry logic.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{AgentRunConfig, AgentRunResult, LogCallback, RunOutcome};
use super::cancel::CancelHandle;
use super::config::{INITIAL_BACKOFF_MS, MAX_TRANSIENT_RETRIES};
use super::error::SpawnError;
use super::process::AgentProcess;
use super::utils::is_transient_error;
use crate::agents::provider::AgentProvider;
use crate::agents::worktree::{get_git_user_email, get_git_user_name};

pub type OnSpawnCallback = Box<dyn FnOnce(CancelHandle) + Send>;

pub fn run_agent_via_provider(
    provider: &dyn AgentProvider,
    config: &AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
) -> Result<AgentRunResult, SpawnError> {
    run_agent_via_provider_with_cancel(provider, config, on_log, None)
}

/// Run an agent using a provider, with cancel callback support.
pub fn run_agent_via_provider_with_cancel(
    provider: &dyn AgentProvider,
    config: &AgentRunConfig,
    on_log: Option<Arc<LogCallback>>,
    on_spawn: Option<OnSpawnCallback>,
) -> Result<AgentRunResult, SpawnError> {
    tracing::debug!(
        "run_agent_via_provider: agent={} run_id={}",
        provider.id(),
        config.run_id
    );

    let (command, args) = provider.build_command(config);

    let mut env_vars = provider.build_env_vars(config);

    // Override git identity so commits are authored/committed by the user,
    // not by the agent CLI. The Co-authored-by trailer in the commit message
    // (added via the add-and-commit command instructions) credits Bored.
    if let Some(name) = get_git_user_name(&config.repo_path) {
        env_vars.push(("GIT_AUTHOR_NAME".to_string(), name.clone()));
        env_vars.push(("GIT_COMMITTER_NAME".to_string(), name));
    }
    if let Some(email) = get_git_user_email(&config.repo_path) {
        env_vars.push(("GIT_AUTHOR_EMAIL".to_string(), email.clone()));
        env_vars.push(("GIT_COMMITTER_EMAIL".to_string(), email));
    }

    run_agent_inner(
        command,
        args,
        env_vars,
        &config.repo_path,
        config.run_id.clone(),
        config.timeout_secs,
        on_log,
        on_spawn,
    )
}

// ── Shared execution core ───────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn run_agent_inner(
    command: String,
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    repo_path: &std::path::Path,
    run_id: String,
    timeout_secs: Option<u64>,
    on_log: Option<Arc<LogCallback>>,
    on_spawn: Option<OnSpawnCallback>,
) -> Result<AgentRunResult, SpawnError> {
    let start_time = Instant::now();
    let env_refs: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let idle_timeout = timeout_secs.map(Duration::from_secs);
    let mut attempt = 0;
    let mut on_spawn = on_spawn;

    loop {
        attempt += 1;

        if attempt > 1 {
            let backoff_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 2);
            tracing::debug!("Retry {} for run {} after {}ms", attempt, run_id, backoff_ms);
            thread::sleep(Duration::from_millis(backoff_ms));
        }

        let process = AgentProcess::spawn(
            &command,
            &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            repo_path,
            &env_refs,
        )?;

        if let Some(callback) = on_spawn.take() {
            callback(process.cancel_handle());
        }

        let result = process.wait_with_capture(idle_timeout, on_log.clone(), true);

        match result {
            Ok((exit_code, outcome, captured_stdout, captured_stderr)) => {
                if outcome == RunOutcome::Error {
                    if let Some(ref stderr) = captured_stderr {
                        if !stderr.is_empty() {
                            tracing::error!("Run {} stderr: {}", run_id, stderr);
                        }
                    }
                }

                if outcome == RunOutcome::Error && attempt < MAX_TRANSIENT_RETRIES {
                    if let Some(ref stderr) = captured_stderr {
                        if is_transient_error(stderr) {
                            tracing::warn!(
                                "Transient error on attempt {} for {}",
                                attempt,
                                run_id
                            );
                            continue;
                        }
                    }
                }

                let duration_secs = start_time.elapsed().as_secs_f64();

                return Ok(AgentRunResult {
                    run_id,
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
                    run_id,
                    exit_code: None,
                    status: RunOutcome::Timeout,
                    summary: Some(format!(
                        "Process idle timed out after {} seconds of inactivity",
                        secs
                    )),
                    duration_secs,
                    captured_stdout: None,
                });
            }
            Err(SpawnError::Cancelled) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                return Ok(AgentRunResult {
                    run_id,
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
