//! Manages the app subprocess (e.g. `npm run dev`) for a validation session.
//! Streams stdout/stderr to the frontend via ValidationAppLog SSE events.
//! On Unix, spawns in a new process group so stop() kills the entire tree.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::api::state::LiveEvent;

struct AppProcessHandle {
    child: Child,
    /// PID used for process-group kill on Unix
    pid: u32,
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

        // Put the child in its own process group so we can kill the whole tree
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn app: {}", e))?;
        let pid = child.id();

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
            AppProcessHandle { child, pid },
        );

        Ok(())
    }

    /// Stop the app process for the given session, if any.
    /// On Unix, kills the entire process group (SIGTERM then SIGKILL) so child
    /// processes like node/vite spawned by npm are also terminated.
    pub fn stop(&self, session_id: &str) {
        if let Ok(mut guard) = self.processes.lock() {
            if let Some(mut handle) = guard.remove(session_id) {
                kill_process_tree(&mut handle);
            }
        }
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
        let pgid = handle.pid as i32;
        // Send SIGTERM to the whole process group first (graceful)
        unsafe { libc::killpg(pgid, libc::SIGTERM); }
        // Give it a moment to exit
        thread::sleep(Duration::from_millis(300));
        // If still alive, force kill the group
        if handle.child.try_wait().ok().flatten().is_none() {
            unsafe { libc::killpg(pgid, libc::SIGKILL); }
        }
        let _ = handle.child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = handle.child.kill();
        let _ = handle.child.wait();
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
