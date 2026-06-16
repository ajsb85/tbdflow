//! Agent-mode default resolution.
//!
//! tbdflow is frequently driven by AI agents (Claude Code) and CI, which can't
//! answer prompts and want machine-readable output. Rather than force every call
//! to repeat `--non-interactive --toon`, the global flags also pick up defaults
//! from the environment, with this precedence (most explicit wins):
//!
//! ```text
//! explicit CLI flag  >  TBDFLOW_* env var  >  agent/CI auto-detect  >  built-in default
//! ```
//!
//! Detection signals: `CLAUDECODE` (set by Claude Code) and `CI` enable
//! non-interactive mode automatically (they can't prompt). TOON and no-sign are
//! opt-in via their `TBDFLOW_*` env vars so humans in plain CI still get readable
//! output unless they ask otherwise.

/// True if an env var is set to a truthy value (`1`, `true`, `yes`, `on`).
pub fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_ascii_lowercase()),
        Some(ref v) if v == "1" || v == "true" || v == "yes" || v == "on"
    )
}

/// Resolve the effective `--non-interactive` value.
pub fn non_interactive(flag: bool) -> bool {
    flag || non_interactive_source().is_some()
}

/// Where non-interactive mode is coming from (None = off, or only the explicit flag).
pub fn non_interactive_source() -> Option<&'static str> {
    if env_truthy("TBDFLOW_NON_INTERACTIVE") {
        Some("env:TBDFLOW_NON_INTERACTIVE")
    } else if env_truthy("CLAUDECODE") {
        Some("detect:CLAUDECODE")
    } else if env_truthy("CI") {
        Some("detect:CI")
    } else {
        None
    }
}

/// Resolve the effective `--toon` value.
pub fn toon(flag: bool) -> bool {
    flag || env_truthy("TBDFLOW_TOON")
}

/// Resolve the effective `--no-sign` value.
pub fn no_sign(flag: bool) -> bool {
    flag || env_truthy("TBDFLOW_NO_SIGN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env is process-global; serialise these tests behind one (non-reentrant)
    // lock and set ALL vars for a case in a single call to avoid re-locking.
    static GUARD: Mutex<()> = Mutex::new(());

    /// Set each var (Some) or clear it (None) for the duration of `f`, restoring
    /// previous values afterwards. Holds the lock once — never nest this.
    fn with_vars<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _g = GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let prev: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(name, value)| {
                let before = std::env::var(name).ok();
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
                (name.to_string(), before)
            })
            .collect();
        f();
        for (name, before) in prev {
            match before {
                Some(v) => std::env::set_var(&name, v),
                None => std::env::remove_var(&name),
            }
        }
    }

    #[test]
    fn flag_always_wins() {
        with_vars(&[("TBDFLOW_TOON", None)], || assert!(toon(true)));
    }

    #[test]
    fn env_truthy_recognises_common_values() {
        for v in ["1", "true", "on"] {
            with_vars(&[("TBDFLOW_TOON", Some(v))], || assert!(toon(false)));
        }
        for v in ["0", "nope"] {
            with_vars(&[("TBDFLOW_TOON", Some(v))], || assert!(!toon(false)));
        }
    }

    #[test]
    fn claudecode_enables_non_interactive() {
        with_vars(
            &[
                ("TBDFLOW_NON_INTERACTIVE", None),
                ("CI", None),
                ("CLAUDECODE", Some("1")),
            ],
            || {
                assert!(non_interactive(false));
                assert_eq!(non_interactive_source(), Some("detect:CLAUDECODE"));
            },
        );
    }

    #[test]
    fn explicit_env_takes_precedence_in_source() {
        with_vars(
            &[
                ("TBDFLOW_NON_INTERACTIVE", Some("1")),
                ("CLAUDECODE", Some("1")),
            ],
            || assert_eq!(non_interactive_source(), Some("env:TBDFLOW_NON_INTERACTIVE")),
        );
    }
}
