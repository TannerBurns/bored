//! Stream reading utilities for agent output.

use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::super::{LogCallback, LogLine, LogStream};

pub fn read_stream_with_capture<R: std::io::Read>(
    reader: R,
    stream: LogStream,
    on_log: Option<Arc<LogCallback>>,
    capture: bool,
    last_activity: Option<Arc<Mutex<Instant>>>,
) -> Option<String> {
    let reader = BufReader::new(reader);
    let mut captured = if capture { Some(Vec::new()) } else { None };

    for line in reader.lines() {
        match line {
            Ok(content) => {
                if let Some(ref activity) = last_activity {
                    if let Ok(mut ts) = activity.lock() {
                        *ts = Instant::now();
                    }
                }
                if let Some(ref mut lines) = captured {
                    lines.push(content.clone());
                }
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

    captured.map(|lines| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_stream_with_capture_returns_none_when_not_capturing() {
        let input = Cursor::new("line1\nline2\n");
        let result = read_stream_with_capture(input, LogStream::Stdout, None, false, None);
        assert!(result.is_none());
    }

    #[test]
    fn read_stream_with_capture_returns_content_when_capturing() {
        let input = Cursor::new("line1\nline2\n");
        let result = read_stream_with_capture(input, LogStream::Stdout, None, true, None);
        assert_eq!(result, Some("line1\nline2".to_string()));
    }

    #[test]
    fn read_stream_with_capture_empty_input() {
        let input = Cursor::new("");
        let result = read_stream_with_capture(input, LogStream::Stdout, None, true, None);
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn read_stream_with_capture_single_line() {
        let input = Cursor::new("single line");
        let result = read_stream_with_capture(input, LogStream::Stdout, None, true, None);
        assert_eq!(result, Some("single line".to_string()));
    }

    #[test]
    fn read_stream_with_capture_invokes_callback() {
        let input = Cursor::new("test line\n");
        let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let lines_clone = lines.clone();

        let callback: Arc<super::super::super::LogCallback> = Arc::new(Box::new(move |log_line| {
            lines_clone.lock().unwrap().push(log_line.content);
        }));

        let _ = read_stream_with_capture(input, LogStream::Stdout, Some(callback), false, None);

        let captured = lines.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0], "test line");
    }

    #[test]
    fn read_stream_with_capture_stderr_stream() {
        let input = Cursor::new("error output\n");
        let stream_type: Arc<Mutex<Option<LogStream>>> = Arc::new(Mutex::new(None));
        let stream_type_clone = stream_type.clone();

        let callback: Arc<super::super::super::LogCallback> = Arc::new(Box::new(move |log_line| {
            *stream_type_clone.lock().unwrap() = Some(log_line.stream);
        }));

        let _ = read_stream_with_capture(input, LogStream::Stderr, Some(callback), false, None);

        let captured_stream = stream_type.lock().unwrap();
        assert!(matches!(*captured_stream, Some(LogStream::Stderr)));
    }

    #[test]
    fn read_stream_updates_last_activity() {
        let input = Cursor::new("line1\nline2\n");
        let before = Instant::now();
        let activity = Arc::new(Mutex::new(before));
        let _ = read_stream_with_capture(
            input,
            LogStream::Stdout,
            None,
            true,
            Some(activity.clone()),
        );
        let after = *activity.lock().unwrap();
        assert!(after >= before);
    }
}
