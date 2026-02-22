//! Agent process management.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::super::{LogCallback, LogStream, RunOutcome};
use super::cancel::CancelHandle;
use super::error::SpawnError;
use super::stream::read_stream_with_capture;

pub struct AgentProcess {
    child: Child,
    cancelled: Arc<AtomicBool>,
}

impl AgentProcess {
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

    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle::new(self.cancelled.clone())
    }

    #[allow(clippy::type_complexity)]
    pub fn wait_with_capture(
        mut self,
        timeout: Option<Duration>,
        on_log: Option<Arc<LogCallback>>,
        capture_stdout: bool,
    ) -> Result<(Option<i32>, RunOutcome, Option<String>, Option<String>), SpawnError> {
        let stdout = self.child.stdout.take();
        let stderr = self.child.stderr.take();
        let cancelled = self.cancelled.clone();

        let on_log_stdout = on_log.clone();
        let stdout_handle = stdout.map(|out| {
            thread::spawn(move || {
                read_stream_with_capture(out, LogStream::Stdout, on_log_stdout, capture_stdout)
            })
        });

        // Always capture stderr for transient error detection
        let on_log_stderr = on_log;
        let stderr_handle = stderr.map(|err| {
            thread::spawn(move || {
                read_stream_with_capture(err, LogStream::Stderr, on_log_stderr, true)
            })
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
                    let captured_stdout = if let Some(h) = stdout_handle {
                        h.join().ok().flatten()
                    } else {
                        None
                    };
                    let captured_stderr = if let Some(h) = stderr_handle {
                        h.join().ok().flatten()
                    } else {
                        None
                    };

                    let exit_code = status.code();
                    let outcome = if exit_code == Some(0) {
                        RunOutcome::Success
                    } else {
                        RunOutcome::Error
                    };

                    return Ok((exit_code, outcome, captured_stdout, captured_stderr));
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
