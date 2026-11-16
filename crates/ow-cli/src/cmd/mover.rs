//! Owner: lane W5-DIALOG. Commands: `--mover-event`, `--mover-probe`: the ports of their branches in `src/cli.cpp`.
//! AI-assisted (Claude Code).
//!
//! - `<package> --mover-probe <actor> <action> --cooked <dir>` binds the mover ([`ow_mission::mover::Mover`]) and
//!   runs, for forward then reverse playback, the start notification, the finish notification and a 10 s timer
//!   step, printing `{"loading_diagnostics":[...],"events":[{"reverse","event","steps","checkpoint","error"}...]}`.
//!   Exit 1 when any event failed.
//! - `<package> --mover-event <actor> <action> --cooked <dir> <event>` binds the mover the same way and sends the
//!   remote event through the action's Kismet sequence, printing `{"event","matched","motion","trace",
//!   "host_boundary","errors"}` and, after requested motion, `"start_steps","finish_steps","start_error",
//!   "finish_error","finished_trace","finished_errors"` (the notifications and the motion's end). Exit 1 when nothing
//!   matched, no motion was requested or anything failed. The sequence runs on lane W5-KISMET's executor
//!   (decision 2026-11-15-32).
//!
//! Both load the source package first, as the C++ does before it reads the options, and run on a thread of
//! `ow_vm::interp::STACK_SIZE` bytes. A binding failure prints `ow-package: <the C++ text>` and exits 1. `mode` is the
//! run's parity mode, read once by `main`.
//!
//! **Stub policy.** `--mover-event` is a gameplay command and refuses an unbound native (`StubPolicy::Refuse` with an
//! empty allow list; architecture section 3.8). `--mover-probe` is not in that table: it is an exploration probe whose
//! broken-mover case `tests/vm_test.py` pins by the logged `UNIMPLEMENTED` text, so it keeps `LogAndZero`, as the C++
//! `Runtime` does for both.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_mission::mover::{Mover, MoverError};
use ow_package::package::Package;
use ow_vm::api::StubPolicy;
use ow_vm::interp::on_vm_stack;

use crate::args::{Command, USAGE};
use crate::cmd::properties::error_text;
use crate::json::quote;

/// How a command ended.
enum Failure {
    /// Printed after `ow-package: `, exit 1 (the C++ error path).
    Error(String),
    /// A `NotYet` of the port, exit 3.
    NotYet(String),
}

impl From<MoverError> for Failure {
    fn from(e: MoverError) -> Self {
        if e.is_not_yet() {
            Failure::NotYet(e.cpp_text())
        } else {
            Failure::Error(e.cpp_text())
        }
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let flag = command.flag;
    let outcome = on_vm_stack(|| {
        let mut out = String::new();
        let result = match flag {
            "--mover-probe" => probe(args, mode, &mut out),
            _ => event(args, mode, &mut out),
        };
        (out, result)
    })
    // A panic on the VM thread is reported, not re-raised: "the VM thread panicked".
    .unwrap_or_else(|e| (String::new(), Err(Failure::Error(e.to_string()))));
    let (stdout, result) = outcome;
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = std::io::stdout().lock().write_all(stdout.as_bytes());
    match result {
        Ok(code) => ExitCode::from(code),
        Err(Failure::Error(message)) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
            ExitCode::from(1)
        }
        Err(Failure::NotYet(message)) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "ow-package (Rust): {flag}: {message}"
            );
            ExitCode::from(super::NOT_PORTED)
        }
    }
}

/// The source package (loaded first, as the C++ does) and the mover bound from `args`; `count` is the C++ `argc - 1`.
fn bind(
    args: &[String],
    count: usize,
    mode: ParityMode,
    policy: StubPolicy,
) -> Result<Mover, Failure> {
    let package = Package::load(Path::new(&args[0])).map_err(|e| Failure::Error(error_text(&e)))?;
    if args.len() != count || args[4] != "--cooked" {
        return Err(Failure::Error(USAGE.to_string()));
    }
    Ok(Mover::new(
        PathBuf::from(&args[5]),
        &package.name,
        &args[2],
        &args[3],
        mode,
        policy,
    )?)
}

/// `--mover-probe` (C++ `argc != 7`).
fn probe(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, Failure> {
    // An exploration probe: unbound natives are logged, as the C++ (pinned by tests/vm_test.py's broken mover).
    let mut mover = bind(args, 6, mode, StubPolicy::LogAndZero)?;
    out.push_str("{\"loading_diagnostics\":[");
    for (i, warning) in mover.loading_diagnostics().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(warning));
    }
    out.push_str("],\"events\":[");
    let mut failed = false;
    let mut first = true;
    for reverse in [false, true] {
        for event in 0..3 {
            let result = match event {
                2 => mover.advance(10.0),
                _ => mover.notify(event == 1, reverse),
            };
            let name = ["start", "finish", "timers"][event];
            let error = result
                .error
                .as_ref()
                .map(MoverError::cpp_text)
                .unwrap_or_default();
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "{{\"reverse\":{reverse},\"event\":{},\"steps\":{},\"checkpoint\":{},\"error\":{}}}",
                quote(name),
                result.steps,
                result.checkpoint,
                quote(&error)
            ));
            failed |= result.error.is_some();
        }
    }
    out.push_str("]}\n");
    Ok(u8::from(failed))
}

fn list(out: &mut String, name: &str, lines: &[String]) {
    out.push_str(&format!(",\"{name}\":["));
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(line));
    }
    out.push(']');
}

/// `--mover-event` (C++ `argc != 8`): the remote event through the action's sequence, then, when it requested
/// motion, the start and finish notifications and the motion's end. Exit 1 when nothing matched, no motion was
/// requested or anything failed.
fn event(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, Failure> {
    // A gameplay command: an unbound native is refused (architecture section 3.8); nothing is allowed yet.
    let mut mover = bind(args, 7, mode, StubPolicy::refuse_all())?;
    let name = &args[6];
    let dispatch = mover.remote_event(name);
    let mut text = format!(
        "{{\"event\":{},\"matched\":{},\"motion\":{}",
        quote(name),
        dispatch.matched,
        dispatch.motion
    );
    list(&mut text, "trace", &dispatch.trace);
    list(&mut text, "host_boundary", &dispatch.host_boundary);
    list(&mut text, "errors", &dispatch.errors);
    let mut failed = !dispatch.errors.is_empty() || dispatch.matched == 0 || dispatch.motion == 0;
    if dispatch.motion != 0 {
        let reverse = dispatch.motion < 0;
        let start = mover.notify(false, reverse);
        let finish = mover.notify(true, reverse);
        let done = mover.motion_finished(reverse);
        let error = |r: &ow_mission::mover::MoverResult| {
            r.error
                .as_ref()
                .map(MoverError::cpp_text)
                .unwrap_or_default()
        };
        text.push_str(&format!(
            ",\"start_steps\":{},\"finish_steps\":{},\"start_error\":{},\"finish_error\":{}",
            start.steps,
            finish.steps,
            quote(&error(&start)),
            quote(&error(&finish))
        ));
        list(&mut text, "finished_trace", &done.trace);
        list(&mut text, "finished_errors", &done.errors);
        failed |= start.error.is_some() || finish.error.is_some() || !done.errors.is_empty();
    }
    text.push_str(
        "}
",
    );
    out.push_str(&text);
    Ok(u8::from(failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_bad_command_line_is_the_usage_after_the_package_loads() {
        // The package is loaded first: a missing file is its error, not the usage.
        let r = bind(
            &strings(&["no-such.upk", "--mover-probe", "A", "B", "--cooked", "."]),
            6,
            ParityMode::Off,
            StubPolicy::LogAndZero,
        );
        assert!(matches!(r, Err(Failure::Error(m)) if m != USAGE));
    }
}
