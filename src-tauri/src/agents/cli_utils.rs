//! Shared CLI availability checking utilities.
//!
//! Both Claude and Cursor agents need to check if their CLI tool is installed
//! and get its version. This module provides generic functions parameterized
//! by the command name.
//!
//! Results are cached with a 30-second TTL so that repeated calls from
//! multiple frontend components don't spawn redundant subprocesses.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(30);

struct CacheEntry<T: Clone> {
    value: T,
    fetched_at: Instant,
}

impl<T: Clone> CacheEntry<T> {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

static AVAILABILITY_CACHE: Mutex<Option<HashMap<String, CacheEntry<bool>>>> = Mutex::new(None);
static VERSION_CACHE: Mutex<Option<HashMap<String, CacheEntry<Option<String>>>>> = Mutex::new(None);

/// Check whether a CLI tool is available on the system.
pub fn is_cli_available(cmd: &str) -> bool {
    {
        let guard = AVAILABILITY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cache) = guard.as_ref() {
            if let Some(entry) = cache.get(cmd) {
                if entry.is_fresh() {
                    return entry.value;
                }
            }
        }
    }

    let result = Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    {
        let mut guard = AVAILABILITY_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = guard.get_or_insert_with(HashMap::new);
        cache.insert(cmd.to_string(), CacheEntry { value: result, fetched_at: Instant::now() });
    }

    result
}

/// Get the version string of a CLI tool, if available.
pub fn get_cli_version(cmd: &str) -> Option<String> {
    {
        let guard = VERSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cache) = guard.as_ref() {
            if let Some(entry) = cache.get(cmd) {
                if entry.is_fresh() {
                    return entry.value.clone();
                }
            }
        }
    }

    let result = Command::new(cmd)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    {
        let mut guard = VERSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = guard.get_or_insert_with(HashMap::new);
        cache.insert(cmd.to_string(), CacheEntry { value: result.clone(), fetched_at: Instant::now() });
    }

    result
}

/// Invalidate all cached CLI availability and version results.
#[cfg(test)]
pub fn clear_cli_cache() {
    if let Ok(mut guard) = AVAILABILITY_CACHE.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = VERSION_CACHE.lock() {
        *guard = None;
    }
}

/// Check if a shell command matches known dangerous patterns.
pub fn is_dangerous_command(command: &str) -> bool {
    use std::sync::OnceLock;

    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            r"rm\s+-rf\s+/",
            r"rm\s+-rf\s+~/",
            r"git\s+push\s+.*--force",
            r"sudo\s+rm",
            r"mkfs\.",
            r"dd\s+if=.*of=/dev",
            r":\(\)\{\s*:\|:&\s*\};:",
        ]
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect()
    });

    patterns.iter().any(|r| r.is_match(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_cli_available_returns_false_for_missing_command() {
        clear_cli_cache();
        assert!(!is_cli_available("nonexistent-command-12345"));
    }

    #[test]
    fn get_cli_version_returns_none_for_missing_command() {
        clear_cli_cache();
        let result = get_cli_version("nonexistent-command-12345");
        assert!(result.is_none());
    }

    #[test]
    fn availability_cache_returns_consistent_results() {
        clear_cli_cache();
        let first = is_cli_available("nonexistent-cache-test-99999");
        let second = is_cli_available("nonexistent-cache-test-99999");
        assert_eq!(first, second);
    }

    #[test]
    fn version_cache_returns_consistent_results() {
        clear_cli_cache();
        let first = get_cli_version("nonexistent-cache-test-99999");
        let second = get_cli_version("nonexistent-cache-test-99999");
        assert_eq!(first, second);
    }

    #[test]
    fn clear_cache_resets_entries() {
        is_cli_available("nonexistent-clear-test-99999");
        get_cli_version("nonexistent-clear-test-99999");
        clear_cli_cache();
        let guard = AVAILABILITY_CACHE.lock().unwrap();
        assert!(guard.is_none());
        drop(guard);
        let guard = VERSION_CACHE.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn different_commands_cached_independently() {
        let a1 = is_cli_available("nonexistent-iso-a-99999");
        let b1 = is_cli_available("nonexistent-iso-b-99999");
        let a2 = is_cli_available("nonexistent-iso-a-99999");
        let b2 = is_cli_available("nonexistent-iso-b-99999");
        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn version_cache_different_commands_cached_independently() {
        let a1 = get_cli_version("nonexistent-viso-a-99999");
        let b1 = get_cli_version("nonexistent-viso-b-99999");
        let a2 = get_cli_version("nonexistent-viso-a-99999");
        let b2 = get_cli_version("nonexistent-viso-b-99999");
        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn cache_entry_is_fresh_within_ttl() {
        let entry = CacheEntry {
            value: true,
            fetched_at: Instant::now(),
        };
        assert!(entry.is_fresh());
    }

    #[test]
    fn cache_entry_is_stale_after_ttl() {
        let entry = CacheEntry {
            value: true,
            fetched_at: Instant::now() - CACHE_TTL - Duration::from_secs(1),
        };
        assert!(!entry.is_fresh());
    }

    // ── is_dangerous_command ───────────────────────────────────────

    #[test]
    fn dangerous_rm_rf_root() {
        assert!(is_dangerous_command("rm -rf /"));
    }

    #[test]
    fn dangerous_rm_rf_home() {
        assert!(is_dangerous_command("rm -rf ~/"));
    }

    #[test]
    fn dangerous_force_push() {
        assert!(is_dangerous_command("git push origin main --force"));
    }

    #[test]
    fn dangerous_sudo_rm() {
        assert!(is_dangerous_command("sudo rm -rf something"));
    }

    #[test]
    fn safe_cargo_test() {
        assert!(!is_dangerous_command("cargo test"));
    }

    #[test]
    fn safe_git_push() {
        assert!(!is_dangerous_command("git push origin main"));
    }

    #[test]
    fn safe_rm_single_file() {
        assert!(!is_dangerous_command("rm temp.txt"));
    }

    #[test]
    fn dangerous_mkfs() {
        assert!(is_dangerous_command("mkfs.ext4 /dev/sda1"));
    }

    #[test]
    fn dangerous_dd_to_device() {
        assert!(is_dangerous_command("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn dangerous_fork_bomb() {
        assert!(is_dangerous_command(":(){:|:&};:"));
    }

    #[test]
    fn safe_dd_to_file() {
        assert!(!is_dangerous_command("dd if=/dev/zero of=output.img bs=1M count=100"));
    }

    #[test]
    fn safe_empty_command() {
        assert!(!is_dangerous_command(""));
    }
}
