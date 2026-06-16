//! Output routing for human vs. TOON (agent) modes.
//!
//! In **human mode** (the default) `say!` prints colored lines exactly as
//! before. In **TOON mode** (`--toon`) all human decoration is suppressed and a
//! single structured document is printed at the end of the run:
//!
//! ```text
//! command: commit
//! ok: true
//! trace[2]{tool,args}:
//!   git,"commit -S -m ..."
//!   git,push
//! warnings[1]: deferred DoD checklist
//! result:
//!   type: fix
//!   subject: fix login bug
//!   signed: true
//! ```
//!
//! Commands describe their outcome with [`result`]; the git/gh command trace is
//! captured at a single chokepoint via [`trace`].

use crate::toon::{encode, Toon};
use colored::Colorize;
use std::cell::RefCell;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    toon: bool,
    command: String,
    trace: Vec<Toon>,
    warnings: Vec<String>,
    result: Option<Toon>,
}

/// Initialise the reporter for this run. In TOON mode colored output is disabled
/// globally so no ANSI escapes can leak into the document.
pub fn init(toon: bool, command: &str) {
    if toon {
        colored::control::set_override(false);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.toon = toon;
        s.command = command.to_string();
    });
}

pub fn is_toon() -> bool {
    STATE.with(|s| s.borrow().toon)
}

/// Print a human-facing decoration line (suppressed entirely in TOON mode).
/// Prefer the [`say!`](crate::say) macro at call sites.
pub fn human_line(line: String) {
    STATE.with(|s| {
        if !s.borrow().toon {
            println!("{}", line);
        }
    });
}

/// Record a warning. Printed (yellow) in human mode; collected into the TOON
/// document's `warnings[]` array in TOON mode.
pub fn warn(msg: impl Into<String>) {
    let msg = msg.into();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.toon {
            s.warnings.push(msg);
        } else {
            println!("{}", msg.yellow());
        }
    });
}

/// Record a structured command result for the TOON document. Ignored in human
/// mode (where the equivalent information was already printed via `say!`).
pub fn result(value: Toon) {
    STATE.with(|s| s.borrow_mut().result = Some(value));
}

/// Record a single git/gh invocation in the trace. Only called when the caller
/// is in verbose mode. In human mode it prints the familiar `[RUNNING]` line;
/// in TOON mode it appends to `trace[]`.
pub fn trace(tool: &str, argv: &[&str]) {
    push_trace(tool, argv, false);
}

/// Print a dry-run trace entry.
pub fn trace_dry_run(tool: &str, argv: &[&str]) {
    push_trace(tool, argv, true);
}

fn push_trace(tool: &str, argv: &[&str], dry_run: bool) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.toon {
            // Uniform shape so the trace renders as a compact tabular array.
            s.trace.push(Toon::obj(vec![
                ("tool", Toon::str(tool)),
                ("args", Toon::str(argv.join(" "))),
                ("dry_run", Toon::Bool(dry_run)),
            ]));
        } else if dry_run {
            println!(
                "{}",
                "[DRY RUN] Command would execute but no changes made".yellow()
            );
            println!("{} {}\n", tool, argv.join(" "));
        } else {
            println!("{} {} {}", "[RUNNING]".cyan(), tool, argv.join(" "));
        }
    });
}

/// Emit the final TOON document (no-op in human mode). Called once at the end of
/// `main` with the overall success state.
pub fn flush(ok: bool, error: Option<String>, code: Option<&str>) {
    STATE.with(|s| {
        let s = s.borrow();
        if !s.toon {
            return;
        }
        let mut fields: Vec<(String, Toon)> = vec![
            ("command".to_string(), Toon::str(s.command.clone())),
            ("ok".to_string(), Toon::Bool(ok)),
        ];
        if !s.trace.is_empty() {
            fields.push(("trace".to_string(), Toon::Arr(s.trace.clone())));
        }
        if !s.warnings.is_empty() {
            fields.push((
                "warnings".to_string(),
                Toon::Arr(s.warnings.iter().cloned().map(Toon::Str).collect()),
            ));
        }
        if let Some(r) = &s.result {
            fields.push(("result".to_string(), r.clone()));
        }
        if let Some(e) = error {
            fields.push(("error".to_string(), Toon::str(e)));
        }
        // Stable, machine-readable failure classifier for agents.
        if let Some(c) = code {
            fields.push(("code".to_string(), Toon::str(c)));
        }
        print!("{}", encode(&Toon::Obj(fields)));
    });
}

/// Human-facing decoration line. Prints in human mode, suppressed in TOON mode.
/// Drop-in replacement for `println!`.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::report::human_line(format!($($arg)*))
    };
}
