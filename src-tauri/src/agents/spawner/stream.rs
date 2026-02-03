//! Stream reading utilities for agent output.

use std::io::{BufRead, BufReader};
use std::sync::Arc;

use super::super::{LogCallback, LogLine, LogStream};

/// Read a stream line by line, calling the callback for each line
#[allow(dead_code)]
pub fn read_stream<R: std::io::Read>(reader: R, stream: LogStream, on_log: Option<Arc<LogCallback>>) {
    let _ = read_stream_with_capture(reader, stream, on_log, false);
}

/// Read a stream line by line, calling the callback for each line and optionally capturing output
pub fn read_stream_with_capture<R: std::io::Read>(
    reader: R,
    stream: LogStream,
    on_log: Option<Arc<LogCallback>>,
    capture: bool,
) -> Option<String> {
    let stream_name = match stream {
        LogStream::Stdout => "stdout",
        LogStream::Stderr => "stderr",
    };
    tracing::debug!(
        "Starting to read {} stream (capture={})",
        stream_name,
        capture
    );
    let reader = BufReader::new(reader);
    let mut line_count = 0;
    let mut captured = if capture { Some(Vec::new()) } else { None };

    for line in reader.lines() {
        match line {
            Ok(content) => {
                line_count += 1;
                tracing::debug!(
                    "[{}] Line {}: {} chars",
                    stream_name,
                    line_count,
                    content.len()
                );

                // Capture if requested
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
            Err(e) => {
                tracing::debug!("[{}] Stream ended with error: {}", stream_name, e);
                break;
            }
        }
    }
    tracing::debug!(
        "[{}] Stream finished, read {} lines",
        stream_name,
        line_count
    );

    captured.map(|lines| lines.join("\n"))
}
