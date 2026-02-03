//! Stream reading utilities for agent output.

use std::io::{BufRead, BufReader};
use std::sync::Arc;

use super::super::{LogCallback, LogLine, LogStream};

#[allow(dead_code)]
pub fn read_stream<R: std::io::Read>(reader: R, stream: LogStream, on_log: Option<Arc<LogCallback>>) {
    let _ = read_stream_with_capture(reader, stream, on_log, false);
}

pub fn read_stream_with_capture<R: std::io::Read>(
    reader: R,
    stream: LogStream,
    on_log: Option<Arc<LogCallback>>,
    capture: bool,
) -> Option<String> {
    let reader = BufReader::new(reader);
    let mut captured = if capture { Some(Vec::new()) } else { None };

    for line in reader.lines() {
        match line {
            Ok(content) => {
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
