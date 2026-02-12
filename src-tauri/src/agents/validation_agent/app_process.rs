//! Manages the app subprocess (e.g. `npm run dev`) for a validation session.
//! Streams stdout/stderr to the frontend via ValidationAppLog SSE events.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;

use chrono::Utc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;

struct AppProcessHandle {
    child: Child,
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
    pub fn start(
        &self,
        session_id: String,
        command: String,
        working_dir: &Path,
        event_tx: broadcast::Sender<LiveEvent>,
    ) -> Result<(), String> {
        self.stop(&session_id);

        let (program, args) = parse_shell_command(&command)?;

        let mut cmd = Command::new(&program);
        cmd.args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn app: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to take stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to take stderr")?;

        let session_id_stdout = session_id.clone();
        let session_id_stderr = session_id.clone();
        let tx_stdout = event_tx.clone();
        let tx_stderr = event_tx.clone();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stdout.send(LiveEvent::ValidationAppLog {
                    session_id: session_id_stdout.clone(),
                    stream: "stdout".to_string(),
                    message: line,
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
        });

        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx_stderr.send(LiveEvent::ValidationAppLog {
                    session_id: session_id_stderr.clone(),
                    stream: "stderr".to_string(),
                    message: line,
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
        });

        self.processes.lock().map_err(|e| e.to_string())?.insert(
            session_id,
            AppProcessHandle { child },
        );

        Ok(())
    }

    /// Stop the app process for the given session, if any.
    pub fn stop(&self, session_id: &str) {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(mut handle) = guard.remove(session_id) {
                let _ = handle.child.kill();
                let _ = handle.child.wait();
            }
        }
    }

    /// Return true if an app process is running for this session.
    pub fn is_running(&self, session_id: &str) -> bool {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(handle) = guard.get_mut(session_id) {
                return handle.child.try_wait().map(|s| s.is_none()).unwrap_or(true);
            }
        }
        false
    }
}

/// Split a shell-like command into program and args. Simple whitespace split;
/// first token is the program, the rest are arguments.
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
