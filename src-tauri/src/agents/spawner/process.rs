//! Agent process management.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

    /// Wait for the child process to exit, capturing stdout/stderr.
    ///
    /// `idle_timeout` is an inactivity timeout: the timer resets every time a
    /// line of output is received on stdout or stderr.  The process is only
    /// killed when no output has been produced for the full duration.
    #[allow(clippy::type_complexity)]
    pub fn wait_with_capture(
        mut self,
        idle_timeout: Option<Duration>,
        on_log: Option<Arc<LogCallback>>,
        capture_stdout: bool,
    ) -> Result<(Option<i32>, RunOutcome, Option<String>, Option<String>), SpawnError> {
        let stdout = self.child.stdout.take();
        let stderr = self.child.stderr.take();
        let cancelled = self.cancelled.clone();

        let last_activity: Option<Arc<Mutex<Instant>>> =
            idle_timeout.map(|_| Arc::new(Mutex::new(Instant::now())));

        let on_log_stdout = on_log.clone();
        let activity_stdout = last_activity.clone();
        let stdout_handle = stdout.map(|out| {
            thread::spawn(move || {
                read_stream_with_capture(
                    out,
                    LogStream::Stdout,
                    on_log_stdout,
                    capture_stdout,
                    activity_stdout,
                )
            })
        });

        let on_log_stderr = on_log;
        let activity_stderr = last_activity.clone();
        let stderr_handle = stderr.map(|err| {
            thread::spawn(move || {
                read_stream_with_capture(
                    err,
                    LogStream::Stderr,
                    on_log_stderr,
                    true,
                    activity_stderr,
                )
            })
        });

        loop {
            if cancelled.load(Ordering::Relaxed) {
                let _ = self.child.kill();
                if let Some(h) = stdout_handle {
                    let _ = h.join();
                }
                if let Some(h) = stderr_handle {
                    let _ = h.join();
                }
                return Err(SpawnError::Cancelled);
            }

            if let (Some(ref activity), Some(idle)) = (&last_activity, idle_timeout) {
                let elapsed = Instant::now().duration_since(*activity.lock().unwrap());
                if elapsed >= idle {
                    let _ = self.child.kill();
                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }
                    return Err(SpawnError::Timeout(idle.as_secs()));
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
