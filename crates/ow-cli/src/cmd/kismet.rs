//! Owner: lane W5-KISMET. Commands: `--kismet-census`, `--kismet-run` (the port of their branches in `src/cli.cpp`).
//!
//! - `<package> --kismet-census --cooked <directory>`: every export of the package whose class derives from
//!   `Engine.Sequence` is loaded through the executor (`ow_mission::kismet::Kismet`) and its link census printed:
//!   `{"sequences":[{"path":..,"ops":..,"outputs":..,"links":..,"unresolved":..,"variable_links":..}|
//!   {"path":..,"error":..},..],"totals":{..},"log_entries":N}`, `N` the decode log's length.
//! - `<package> --kismet-run <sequence-path> --cooked <directory> (--remote <name> | --mission <path> <name> |
//!   --op <name> | --originator <object-path>) [--tick <s>]...`: runs one sequence from an entry point, then once
//!   per `--tick`, and prints `{"sequence":..,"ops":..,"entry_matches":..,"executed":..,"trace":[..],
//!   "host_boundary":[..],"errors":[..],"log":[..]}`. World-acting ops (every `Engine.SequenceAction` without a
//!   logic handler) are recorded in `host_boundary` as `<Class>:<op> <- <input>`, not run, and fire nothing. The exit
//!   code is 0 when no error line was written and the entry matched something, else 1 (nothing on stderr).
//!
//! Order, as the C++: the source package is loaded first, then the options are checked (for `--kismet-run`, the
//! entry's words and the `--tick` pairs only after the sequence was loaded), the cooked root is scanned and the source
//! package is loaded again through the store by its name. A failure prints `ow-package: <text>` on stderr, exit 1,
//! nothing on stdout. The ticks' seconds are read as `std::stod` reads them, one at a time before each tick.
//!
//! No VM is involved: the executor's ops are native rules, as in the C++, so the stub policy of architecture 3.8
//! does not arise; an op without a handler is an error line ("unsupported op class"). The parity mode is passed to
//! the decoder (`ow_object::decode`).

use std::cell::RefCell;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use ow_core::ParityMode;
use ow_core::num::{StrtodPrefix, strtod_prefix};
use ow_mission::kismet::{Kismet, KismetError};
use ow_object::decode::{DecodeState, ValueCx};
use ow_object::heap::ObjectSpace;
use ow_object::reflect::CodeIndex;
use ow_package::package::Package;
use ow_package::store::PackageStore;

use crate::args::{Command, USAGE};
use crate::cmd::properties::error_text;
use crate::json::quote;

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let mut out = String::new();
    let outcome = if command.flag == "--kismet-census" {
        census(args, mode, &mut out).map(|()| true)
    } else {
        kismet_run(args, mode, &mut out)
    };
    match outcome {
        Ok(success) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = std::io::stdout().lock().write_all(out.as_bytes());
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(message) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

fn text(e: &dyn std::error::Error) -> String {
    error_text(e)
}

/// The decode context's parts for one cooked root: the store, the reflection index, the object space and the
/// decode state.
struct Parts {
    store: PackageStore,
    code: CodeIndex,
    space: ObjectSpace,
    state: DecodeState,
}

impl Parts {
    fn open(cooked: &str) -> Result<Parts, String> {
        Ok(Parts {
            store: PackageStore::open(PathBuf::from(cooked)).map_err(|e| text(&e))?,
            code: CodeIndex::new(),
            space: ObjectSpace::new(),
            state: DecodeState::new(),
        })
    }

    fn cx(&mut self, mode: ParityMode) -> ValueCx<'_> {
        ValueCx::new(
            &mut self.store,
            &mut self.code,
            &mut self.space,
            &mut self.state,
            mode,
        )
    }
}

/// `args[0]` the source package, `args[1]` `--kismet-census`.
fn census(args: &[String], mode: ParityMode, out: &mut String) -> Result<(), String> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| text(&e))?;
    if args.len() != 4 || args[2] != "--cooked" {
        return Err(USAGE.to_string());
    }
    let mut parts = Parts::open(&args[3])?;
    let mut cx = parts.cx(mode);
    let package = cx
        .code
        .package(cx.store, &source.name)
        .map_err(|e| text(&e))?;
    let sequence_class = cx
        .code
        .find_class(cx.store, "Engine.Sequence")
        .map_err(|e| text(&e))?;
    let pkg = cx.store.shared(package);
    let (mut sequences, mut failed, mut ops, mut links, mut unresolved, mut outputs) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    out.push_str("{\"sequences\":[");
    let mut first = true;
    for (i, exported) in pkg.exports.iter().enumerate() {
        let class = match cx.code.class_at(cx.store, package, exported.class) {
            Ok(Some(class)) => class,
            Ok(None) | Err(_) => continue,
        };
        if !cx.code.is_child_of(class, sequence_class) {
            continue;
        }
        let index = i32::try_from(i + 1).map_err(|_| "too many exports".to_string())?;
        let path = pkg.path_of(index).map_err(|e| text(&e))?;
        sequences += 1;
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("{{\"path\":{}", quote(&path)));
        let loaded = Kismet::new(&mut cx, package, &path)
            .and_then(|k| Ok((k.ops().len(), k.link_stats(&cx)?)));
        match loaded {
            Ok((count, stats)) => {
                ops += count;
                links += stats.links;
                unresolved += stats.unresolved;
                outputs += stats.outputs;
                out.push_str(&format!(
                    ",\"ops\":{count},\"outputs\":{},\"links\":{},\"unresolved\":{},\"variable_links\":{}}}",
                    stats.outputs, stats.links, stats.unresolved, stats.variable_links
                ));
            }
            Err(e) => {
                failed += 1;
                out.push_str(&format!(",\"error\":{}}}", quote(&text(&e))));
            }
        }
    }
    out.push_str(&format!(
        "],\"totals\":{{\"sequences\":{sequences},\"failed\":{failed},\"ops\":{ops},\"outputs\":{outputs},\"links\":{links},\"unresolved\":{unresolved}}},\"log_entries\":{}}}\n",
        cx.state.log.len()
    ));
    Ok(())
}

/// `args[0]` the source package, `args[1]` `--kismet-run`. `Ok(false)`: printed, exit 1.
fn kismet_run(args: &[String], mode: ParityMode, out: &mut String) -> Result<bool, String> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| text(&e))?;
    if args.len() < 7 || args[3] != "--cooked" {
        return Err(USAGE.to_string());
    }
    let mut parts = Parts::open(&args[4])?;
    let mut cx = parts.cx(mode);
    let package = cx
        .code
        .package(cx.store, &source.name)
        .map_err(|e| text(&e))?;
    let sequence = &args[2];
    let mut kismet = Kismet::new(&mut cx, package, sequence).map_err(|e| text(&e))?;
    let host_calls: Rc<RefCell<Vec<String>>> = Rc::default();
    let calls = Rc::clone(&host_calls);
    kismet.handle(
        "Engine.SequenceAction",
        Rc::new(
            move |k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32| {
                let desc = k.input_desc(cx, op, input)?;
                let line = format!("{}:{} <- {desc}", k.ops()[op].cls, k.ops()[op].name);
                calls.borrow_mut().push(line);
                Ok(())
            },
        ),
    );
    let entry = args[5].as_str();
    // Entry arguments, then optional `--tick <seconds>` pairs (each advances time and runs what is due).
    let ticks_at = if entry == "--mission" { 8 } else { 7 };
    if args.len() < ticks_at || !(args.len() - ticks_at).is_multiple_of(2) {
        return Err(USAGE.to_string());
    }
    if args[ticks_at..]
        .iter()
        .step_by(2)
        .any(|word| word != "--tick")
    {
        return Err(USAGE.to_string());
    }
    let failure = |e: KismetError| text(&e);
    let matched = match entry {
        "--remote" => kismet.remote_event(&mut cx, &args[6]).map_err(failure)?,
        "--mission" => kismet
            .mission_remote_event(&mut cx, &args[6], &args[7])
            .map_err(failure)?,
        "--op" => match kismet.find(&args[6]) {
            Some(op) => {
                kismet.activate_event(&mut cx, op).map_err(failure)?;
                1
            }
            None => 0,
        },
        "--originator" => {
            let events = kismet
                .events_for_originator(&mut cx, &args[6])
                .map_err(failure)?;
            for &op in &events {
                kismet.activate_event(&mut cx, op).map_err(failure)?;
            }
            events.len()
        }
        _ => return Err(USAGE.to_string()),
    };
    kismet.run(&mut cx).map_err(failure)?;
    for seconds in args[ticks_at..].iter().skip(1).step_by(2) {
        let seconds = stod(seconds)?;
        kismet.tick(&mut cx, seconds).map_err(failure)?;
    }
    let list = |lines: &[String]| lines.iter().map(|l| quote(l)).collect::<Vec<_>>().join(",");
    out.push_str(&format!(
        "{{\"sequence\":{},\"ops\":{},\"entry_matches\":{matched},\"executed\":{},\"trace\":[{}],\"host_boundary\":[{}],\"errors\":[{}],\"log\":[{}]}}\n",
        quote(sequence),
        kismet.ops().len(),
        kismet.executed,
        list(&kismet.trace),
        list(&host_calls.borrow()),
        list(&kismet.errors),
        list(&cx.state.log),
    ));
    Ok(kismet.errors.is_empty() && matched > 0)
}

/// `std::stod(text)`: decimal text, `inf` and `nan` (`ow_core::num::strtod_prefix`); an overflow to an infinity is the
/// MSVC `out_of_range`, no number the MSVC `invalid_argument` (their `what()` texts). A hexadecimal float is refused
/// (no parser for it is written). The same reading as `cmd/vm.rs`, whose helper is private to that module.
fn stod(text: &str) -> Result<f64, String> {
    let Some(StrtodPrefix {
        start,
        end,
        hexadecimal,
        ..
    }) = strtod_prefix(text)
    else {
        return Err("invalid stod argument".into());
    };
    if hexadecimal {
        return Err(format!(
            "hexadecimal float text is not read by the Rust binary: {text}"
        ));
    }
    let lower = text[start..end].to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    let value = if unsigned.starts_with("inf") {
        f64::INFINITY
    } else if unsigned.starts_with("nan") {
        f64::NAN
    } else {
        let v = unsigned
            .parse::<f64>()
            .map_err(|_| "invalid stod argument".to_string())?;
        if v.is_infinite() {
            return Err("stod argument out of range".into());
        }
        v
    };
    Ok(if lower.starts_with('-') {
        -value
    } else {
        value
    })
}
