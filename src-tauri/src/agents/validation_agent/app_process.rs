//! Manages the app subprocess (e.g. `npm run dev`) for a validation session.
//! Streams stdout/stderr to the frontend via ValidationAppLog SSE events.
//! On Unix, spawns in a new process group so stop() kills the entire tree.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::api::state::LiveEvent;

/// Which event kind to emit for app subprocess logs.
#[derive(Debug, Clone, Copy)]
pub enum AppLogEventKind {
    Validation,
    Chat,
}

fn make_app_log_event(
    kind: AppLogEventKind,
    id: &str,
    stream: &str,
    message: String,
    timestamp: String,
) -> LiveEvent {
    match kind {
        AppLogEventKind::Validation => LiveEvent::ValidationAppLog {
            session_id: id.to_string(),
            stream: stream.to_string(),
            message,
            timestamp,
        },
        AppLogEventKind::Chat => LiveEvent::ChatAppLog {
            chat_id: id.to_string(),
            stream: stream.to_string(),
            message,
            timestamp,
        },
    }
}

/// Result of starting the app subprocess
pub enum StartResult {
    /// Process is still running after the initial check
    Running,
    /// Process exited within the first few seconds
    ExitedEarly { exit_code: i32, output: String },
}

struct AppProcessHandle {
    child: Child,
    /// PID used for process-group kill on Unix
    pid: u32,
    /// Worktree path to clean up on stop (if we created one)
    worktree_path: Option<PathBuf>,
    /// Repo path for worktree cleanup
    repo_path: Option<PathBuf>,
}

/// Manages one app subprocess per validation session.
pub struct AppProcessManager {
    processes: Mutex<HashMap<String, AppProcessHandle>>,
}

impl Default for AppProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AppProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
        }
    }

    /// Start the app command in the given working directory. Streams stdout/stderr to event_tx.
    /// If a process is already running for this session, it is stopped first.
    /// `worktree_path` and `repo_path` are stored so stop() can clean up the worktree.
    pub fn start(
        &self,
        session_id: String,
        command: String,
        working_dir: &Path,
        event_tx: broadcast::Sender<LiveEvent>,
        worktree_path: Option<PathBuf>,
        repo_path: Option<PathBuf>,
        event_kind: AppLogEventKind,
    ) -> Result<StartResult, String> {
        self.kill_process(&session_id);

        let (program, args) = parse_shell_command(&command)?;

        let mut cmd = Command::new(&program);
        cmd.args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn app: {}", e))?;
        let pid = child.id();

        let log_path = working_dir.join(".validation-app.log");
        if let Err(e) = File::create(&log_path) {
            tracing::warn!("Could not create app log file at {:?}: {}", log_path, e);
        }

        let stdout = child.stdout.take().ok_or("Failed to take stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to take stderr")?;

        let session_id_stdout = session_id.clone();
        let session_id_stderr = session_id.clone();
        let tx_stdout = event_tx.clone();
        let tx_stderr = event_tx.clone();
        let log_path_stdout = log_path.clone();
        let log_path_stderr = log_path.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stdout.send(make_app_log_event(
                    event_kind,
                    &session_id_stdout,
                    "stdout",
                    line.clone(),
                    Utc::now().to_rfc3339(),
                ));
                append_to_log(&log_path_stdout, "stdout", &line);
            }
        });

        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stderr.send(make_app_log_event(
                    event_kind,
                    &session_id_stderr,
                    "stderr",
                    line.clone(),
                    Utc::now().to_rfc3339(),
                ));
                append_to_log(&log_path_stderr, "stderr", &line);
            }
        });

        self.processes.lock().map_err(|e| e.to_string())?.insert(
            session_id.clone(),
            AppProcessHandle { child, pid, worktree_path, repo_path },
        );

        // Wait briefly and check if the process exited immediately (bad command, missing binary, etc.)
        thread::sleep(Duration::from_secs(3));

        if let Ok(mut guard) = self.processes.lock() {
            if let Some(handle) = guard.get_mut(&session_id) {
                if let Ok(Some(status)) = handle.child.try_wait() {
                    let exit_code = status.code().unwrap_or(-1);
                    // Read last lines from the log file for context
                    let output = std::fs::read_to_string(&log_path)
                        .unwrap_or_default()
                        .lines()
                        .rev()
                        .take(50)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("\n");
                    // Remove the dead process from the map but do NOT clean up the
                    // worktree yet — the caller's retry loop may need the working
                    // directory for subsequent commands or start_app attempts.
                    // Worktree cleanup is handled by stop() or by the caller after
                    // the retry loop finishes.
                    guard.remove(&session_id);
                    return Ok(StartResult::ExitedEarly { exit_code, output });
                }
            }
        }

        Ok(StartResult::Running)
    }

    /// Kill the app process for the given session without cleaning up the
    /// worktree.  Use this when the session is still active and may need the
    /// working directory for subsequent commands (e.g. inside the command loop
    /// or when `start()` restarts the process).
    pub fn kill_process(&self, session_id: &str) -> bool {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(mut handle) = guard.remove(session_id) {
                kill_process_tree(&mut handle);
                return true;
            }
        }
        false
    }

    /// Stop the app process AND clean up the associated git worktree.
    /// Use this for final cleanup (stop button, app exit).
    pub fn stop(&self, session_id: &str) -> bool {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(mut handle) = guard.remove(session_id) {
                kill_process_tree(&mut handle);
                cleanup_worktree(handle.worktree_path, handle.repo_path);
                return true;
            }
        }
        false
    }

    /// Return true if an app process is running for this session.
    pub fn is_running(&self, session_id: &str) -> bool {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(handle) = guard.get_mut(session_id) {
                return handle.child.try_wait().map(|s| s.is_none()).unwrap_or(false);
            }
        }
        false
    }

    /// Stop all app processes (e.g. on app exit).
    pub fn stop_all(&self) {
        if let Ok(guard) = self.processes.lock() {
            let ids: Vec<String> = guard.keys().cloned().collect();
            drop(guard);
            for id in ids {
                self.stop(&id);
            }
        }
    }
}

impl Drop for AppProcessManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Kill a process and all its children.
fn kill_process_tree(handle: &mut AppProcessHandle) {
    #[cfg(unix)]
    {
        match i32::try_from(handle.pid) {
            Ok(pgid) => {
                // Send SIGTERM to the whole process group first (graceful)
                unsafe { libc::killpg(pgid, libc::SIGTERM); }
                // Give it a moment to exit
                thread::sleep(Duration::from_millis(300));
                // If still alive, force kill the group
                if handle.child.try_wait().ok().flatten().is_none() {
                    unsafe { libc::killpg(pgid, libc::SIGKILL); }
                }
            }
            Err(_) => {
                tracing::warn!(
                    "PID {} exceeds i32::MAX, cannot use killpg; falling back to child.kill()",
                    handle.pid,
                );
                let _ = handle.child.kill();
            }
        }
        let _ = handle.child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = handle.child.kill();
        let _ = handle.child.wait();
    }
}

/// Clean up a git worktree associated with a process handle (best-effort).
fn cleanup_worktree(worktree_path: Option<PathBuf>, repo_path: Option<PathBuf>) {
    if let (Some(wt), Some(repo)) = (worktree_path, repo_path) {
        if let Err(e) = crate::agents::worktree::remove_worktree(&wt, &repo) {
            tracing::warn!("Failed to remove validation worktree: {}", e);
        }
    }
}

/// Append a line to the log file (best-effort, never fails the caller).
fn append_to_log(path: &Path, stream: &str, line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] {}", stream, line);
    }
}

/// Split a shell-like command into program and args.
fn parse_shell_command(command: &str) -> Result<(String, Vec<String>), String> {
    let parts: Vec<String> = command.split_whitespace().map(String::from).collect();
    if parts.is_empty() {
        return Err("Empty app command".to_string());
    }
    let program = parts[0].clone();
    let args = parts[1..].to_vec();
    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shell_command_npm_run_dev() {
        let (prog, args) = parse_shell_command("npm run dev").unwrap();
        assert_eq!(prog, "npm");
        assert_eq!(args, ["run", "dev"]);
    }

    #[test]
    fn parse_shell_command_single() {
        let (prog, args) = parse_shell_command("yarn").unwrap();
        assert_eq!(prog, "yarn");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_shell_command_empty_err() {
        assert!(parse_shell_command("").is_err());
        assert!(parse_shell_command("   ").is_err());
    }
}
