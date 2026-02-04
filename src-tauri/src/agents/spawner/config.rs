//! Configuration constants for agent spawning.

/// Maximum number of retries for transient errors
pub const MAX_TRANSIENT_RETRIES: u32 = 3;

/// Initial backoff delay in milliseconds
pub const INITIAL_BACKOFF_MS: u64 = 2000;

/// Known transient error patterns that should trigger a retry
pub const TRANSIENT_ERROR_PATTERNS: &[&str] = &[
    "Connection stalled",
    "connection reset",
    "connection timed out",
    "rate limit",
    "rate_limit",
    "too many requests",
    "503",
    "502",
    "504",
    "service unavailable",
    "temporarily unavailable",
    "ECONNRESET",
    "ETIMEDOUT",
    "ENOTFOUND",
    "socket hang up",
];
