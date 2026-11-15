//! Owner: lane W4-VMTOOLS. Commands: `--native-census`, `--vm-sweep`, `--native-selftest`: the ports of their branches
//! in `src/cli.cpp`, with the same argument checks in the same order, the same stdout JSON and the same
//! `ow-package: <message>` stderr line and exit code 1 on an error. AI-assisted (Claude Code).
//!
//! - `--native-selftest` loads no package: a self-test of the native registry of the Rust VM (registration, lookup,
//!   a registered call, a logged stub, a refused duplicate), printing the C++ summary line
//!   (`natives_test.py:9-14`).
//! - `<package> --vm-sweep --cooked <dir> [--class <name>] [--limit <n>] [--steps <n>] [--top <n>]`: every script
//!   function of the package run once (`ow_vm::sweep`), printed as the C++ `sweepPackage` JSON (`jsonString` escapes:
//!   a byte below 32 is a space). `seconds` is the wall-clock time, printed as `std::ostream` does (6 significant
//!   digits); it is the one field that never matches between two runs.
//! - `<package> --native-census <entry-file> --cooked <dir> [--steps <n>] [--no-static]`: the entry file run on a VM
//!   with the call census on, and the static closure of the bytecode it reaches (`ow_vm::census`), printed as the C++
//!   `nativeCensus` JSON (`quote` escapes).
//!
//! As in the C++ binary, the source package is loaded before the command's options are read; the sweep then runs
//! the source package loaded again through the store, the census does not use it. Both use `StubPolicy::LogAndZero`
//! and the Core natives and run the VM on a thread of `ow_vm::interp::STACK_SIZE` bytes. `mode` is the run's parity
//! mode, read once by `main`; it goes to the definition store and the VM. Lines end in `\n`; the parity harness reads
//! the C++ `\r\n` as `\n` (decision 2026-11-15-09).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::fmt::fmt_g;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::package::Package;
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::census::{CensusOptions, CensusReport};
use ow_vm::interp::on_vm_stack;
use ow_vm::sweep::{SweepOptions, SweepReport};

use crate::args::{Command, USAGE};
use crate::cmd::properties::{error_text, unsigned_number};
use crate::json::quote;

/// Why a command stopped.
#[derive(Debug)]
enum Failure {
    /// The C++ error text, printed after `ow-package: ` (exit code 1).
    Error(String),
    /// A `NotYet` of the Rust port (exit code 3, architecture section 4).
    NotYet(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Error(message)
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let result = match command.flag {
        "--native-selftest" => self_test(args),
        flag => on_vm_stack(|| match flag {
            "--vm-sweep" => sweep(args, mode),
            _ => census(args, mode),
        })
        // A panic on the VM thread is reported, not re-raised: "the VM thread panicked".
        .unwrap_or_else(|e| Err(Failure::Error(e.to_string()))),
    };
    match result {
        Ok(stdout) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = std::io::stdout().lock().write_all(stdout.as_bytes());
            ExitCode::SUCCESS
        }
        Err(Failure::Error(message)) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
            ExitCode::from(1)
        }
        Err(Failure::NotYet(message)) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "ow-package (Rust): {}: {message}",
                command.flag
            );
            ExitCode::from(super::NOT_PORTED)
        }
    }
}

fn usage() -> String {
    USAGE.to_string()
}

/// The C++ `nextValue`: the argument after `args[*at]`, moving the cursor onto it.
fn next_value(args: &[String], at: &mut usize, option: &str) -> Result<String, String> {
    *at += 1;
    args.get(*at)
        .cloned()
        .ok_or_else(|| format!("missing value for {option}"))
}

/// A VM over the cooked folder `cooked`, with the Core natives and the exploration stub policy (the C++
/// `PackageStore store(cooked); vm::Runtime runtime(store); runtime.registerCoreNatives();`).
fn open_vm(cooked: &Path, mode: ParityMode) -> Result<Vm, String> {
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
        .map_err(|e| error_text(&e))?;
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::LogAndZero);
    ow_vm::natives_core::register(&mut vm).map_err(|e| error_text(&e))?;
    Ok(vm)
}

// ------------------------------------------------------------------------------------------- --native-selftest

/// `--native-selftest` (`args[0]` is the flag): no other argument.
fn self_test(args: &[String]) -> Result<String, Failure> {
    if args.len() != 1 {
        return Err(usage().into());
    }
    let summary = ow_vm::census::native_self_test()?;
    Ok(format!(
        "{{\"registered\":{},\"unimplemented\":{},\"duplicate_rejected\":{},\"passed\":true}}\n",
        summary.registered, summary.unimplemented, summary.duplicate_rejected
    ))
}

// ------------------------------------------------------------------------------------------------ --vm-sweep

fn sweep(args: &[String], mode: ParityMode) -> Result<String, Failure> {
    Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    let mut options = SweepOptions::default();
    let mut cooked = PathBuf::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--cooked" => cooked = PathBuf::from(next_value(args, &mut i, "--cooked")?),
            "--class" => options.class_filter = next_value(args, &mut i, "--class")?,
            "--limit" => options.limit = unsigned_number(&next_value(args, &mut i, "--limit")?)?,
            "--steps" => {
                options.step_limit = unsigned_number(&next_value(args, &mut i, "--steps")?)? as u64
            }
            "--top" => options.top = unsigned_number(&next_value(args, &mut i, "--top")?)?,
            _ => return Err(usage().into()),
        }
        i += 1;
    }
    if cooked.as_os_str().is_empty() {
        return Err(usage().into());
    }
    let mut vm = open_vm(&cooked, mode)?;
    let source = vm
        .defs
        .code_mut()
        .1
        .load_path(Path::new(&args[0]))
        .map_err(|e| error_text(&e))?;
    let report = ow_vm::sweep::sweep_package(&mut vm, source, &options);
    Ok(sweep_json(&report))
}

/// `jsonString` of `src/vm_sweep.cpp`: `"` and `\` escaped, every byte below 32 a space, everything else as is.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 32 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The C++ `sweepPackage` JSON, with its line end.
fn sweep_json(r: &SweepReport) -> String {
    let mut out = format!(
        "{{\"package\":{},\"attempted\":{},\"succeeded\":{},\"failed\":{},\"undecodable_skipped\":{},\
         \"no_owner_skipped\":{},\"total_steps\":{},\"slowest\":[{},{}],\"seconds\":{},\"failures\":[",
        json_string(&r.package),
        r.attempted,
        r.succeeded,
        r.failed(),
        r.undecodable_skipped,
        r.no_owner_skipped,
        r.total_steps,
        json_string(&r.slowest.0),
        r.slowest.1,
        fmt_g(r.seconds, 6)
    );
    let failures: Vec<String> = r
        .failures
        .iter()
        .map(|f| {
            format!(
                "{{\"count\":{},\"example\":{}}}",
                f.count,
                json_string(&f.example)
            )
        })
        .collect();
    out.push_str(&failures.join(","));
    out.push_str("],\"unimplemented_natives\":[");
    let natives: Vec<String> = r
        .unimplemented_natives
        .iter()
        .map(|n| {
            format!(
                "{{\"functions_reaching\":{},\"native\":{}}}",
                n.functions_reaching,
                json_string(&n.native)
            )
        })
        .collect();
    out.push_str(&natives.join(","));
    out.push_str("]}\n");
    out
}

// --------------------------------------------------------------------------------------------- --native-census

fn census(args: &[String], mode: ParityMode) -> Result<String, Failure> {
    Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    if args.len() < 5 {
        return Err(usage().into());
    }
    let mut options = CensusOptions::default();
    let mut cooked = PathBuf::new();
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--cooked" => cooked = PathBuf::from(next_value(args, &mut i, "--cooked")?),
            "--steps" => {
                options.step_limit = unsigned_number(&next_value(args, &mut i, "--steps")?)? as u64
            }
            "--no-static" => options.static_closure = false,
            _ => return Err(usage().into()),
        }
        i += 1;
    }
    if cooked.as_os_str().is_empty() {
        return Err(usage().into());
    }
    let bytes = std::fs::read(&args[2]).map_err(|_| "cannot open entry file".to_string())?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut vm = open_vm(&cooked, mode)?;
    let report =
        ow_vm::census::native_census(&mut vm, &text, &options).map_err(|e| match e.not_yet() {
            Some(n) => Failure::NotYet(n.to_string()),
            None => Failure::Error(error_text(&e)),
        })?;
    Ok(census_json(&report))
}

/// `{"key":count,...}` in key order (C++ `jsonCountMap`).
fn count_map<'a>(counts: impl IntoIterator<Item = (&'a String, &'a u64)>) -> String {
    let items: Vec<String> = counts
        .into_iter()
        .map(|(k, n)| format!("{}:{n}", quote(k)))
        .collect();
    format!("{{{}}}", items.join(","))
}

fn list<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
    let items: Vec<String> = items.into_iter().map(|t| t.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn quoted_list<'a>(items: impl IntoIterator<Item = &'a String>) -> String {
    let items: Vec<String> = items.into_iter().map(|t| quote(t)).collect();
    format!("[{}]", items.join(","))
}

fn null_or(text: &Option<String>) -> String {
    text.as_deref().map_or_else(|| "null".to_string(), quote)
}

/// The C++ `nativeCensus` JSON, with its line end.
fn census_json(r: &CensusReport) -> String {
    let mut out = String::from("{\"entries\":[");
    let entries: Vec<String> = r
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut s = format!(
                "{{\"index\":{i},\"function\":{},\"self\":{},\"args\":{},\"params\":{},\"status\":{},\
                 \"stop_reason\":{},\"error\":{},\"result\":{},\"steps\":{},\"script_functions_entered\":{},\
                 \"script_calls\":{},\"native_calls_implemented\":{},\"native_calls_stub\":{},\
                 \"distinct_natives_implemented\":{},\"distinct_natives_stub\":{},\"none_contexts\":{},\
                 \"log_categories\":{},\"log_head\":{}",
                quote(&e.function),
                quote(&e.this),
                quoted_list(&e.args),
                quoted_list(&e.params),
                quote(if e.completed() { "completed" } else { "stopped" }),
                null_or(&e.stop_reason),
                null_or(&e.error),
                if e.completed() {
                    quote(&e.result)
                } else {
                    "null".to_string()
                },
                e.steps,
                e.scripts.len(),
                e.scripts.values().sum::<u64>(),
                e.natives.values().sum::<u64>(),
                e.stubs.values().sum::<u64>(),
                e.natives.len(),
                e.stubs.len(),
                count_map(&e.none_contexts),
                count_map(&e.log_categories),
                quoted_list(&e.log_head),
            );
            if let Some(reach) = &e.reach {
                for (name, counts) in [("a", &reach.a), ("b", &reach.b)] {
                    s.push_str(&format!(
                        ",\"static_{name}_script_functions\":{},\"static_{name}_natives\":{},\
                         \"static_{name}_natives_implemented\":{}",
                        counts.scripts, counts.natives, counts.natives_implemented
                    ));
                }
            }
            s.push('}');
            s
        })
        .collect();
    out.push_str(&entries.join(","));
    out.push_str("],\"script_functions\":{");
    let scripts: Vec<String> = r
        .script_functions
        .iter()
        .map(|(path, s)| {
            format!(
                "{}:{{\"dynamic_calls\":{},\"static_closure\":{}}}",
                quote(path),
                s.dynamic_calls,
                quote(&s.static_closure.to_string())
            )
        })
        .collect();
    out.push_str(&scripts.join(","));
    out.push_str("},\"natives\":{");
    let natives: Vec<String> = r
        .natives
        .iter()
        .map(|(path, n)| {
            let by_entry: Vec<String> = n
                .dynamic_by_entry
                .iter()
                .map(|(entry, count)| format!("{}:{count}", quote(&entry.to_string())))
                .collect();
            format!(
                "{}:{{\"key\":{},\"package\":{},\"owner\":{},\"name\":{},\"native_index\":{},\"operator\":{},\
                 \"implemented\":{},\"dynamic_calls\":{},\"dynamic_by_entry\":{{{}}},\"static_sites_a\":{},\
                 \"static_sites_b\":{},\"static_entries_a\":{},\"static_entries_b\":{},\"top_callers\":{}}}",
                quote(path),
                quote(&n.key),
                quote(&n.package),
                quote(&n.owner),
                quote(&n.name),
                n.native_index,
                n.operator,
                n.implemented,
                n.dynamic_by_entry.values().sum::<u64>(),
                by_entry.join(","),
                n.static_sites_a,
                n.static_sites_b,
                list(&n.static_entries_a),
                list(&n.static_entries_b),
                quoted_list(&n.top_callers),
            )
        })
        .collect();
    out.push_str(&natives.join(","));
    out.push('}');
    if let Some(s) = &r.static_summary {
        let unknown: Vec<String> = s
            .unknown_native_index
            .iter()
            .map(|(index, count)| format!("{}:{count}", quote(&index.to_string())))
            .collect();
        out.push_str(&format!(
            ",\"static\":{{\"script_functions_walked\":{},\"called_script_functions\":{},\"called_natives\":{},\
             \"delegate_sites\":{},\"unresolved_virtual\":{},\"missing_method\":{},\"unknown_native_index\":{{{}}},\
             \"undecodable\":{},\"dynamic_outside_static\":{}}}",
            s.script_functions_walked,
            s.called_script_functions,
            s.called_natives,
            s.delegate_sites,
            count_map(&s.unresolved_virtual),
            count_map(&s.missing_method),
            unknown.join(","),
            quoted_list(&s.undecodable),
            quoted_list(&s.dynamic_outside_static),
        ));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn self_test_prints_the_c_summary_and_refuses_extra_arguments() {
        assert_eq!(
            self_test(&strings(&["--native-selftest"])).unwrap(),
            "{\"registered\":1,\"unimplemented\":1,\"duplicate_rejected\":true,\"passed\":true}\n"
        );
        assert!(matches!(
            self_test(&strings(&["--native-selftest", "extra"])),
            Err(Failure::Error(m)) if m == USAGE
        ));
    }

    #[test]
    fn a_not_yet_census_line_is_the_not_ported_failure() {
        // An invented package (AGENTS.md rule 9): `Core.Object` only.
        let mut t = ow_package::testkit::script::ScriptToy::new(&["Int"]);
        t.class("Object", 0);
        let dir = std::env::temp_dir().join(format!("ow-cli-census-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let package = dir.join("Core.upk");
        std::fs::write(&package, t.build()).unwrap();
        let (p, d) = (package.display().to_string(), dir.display().to_string());
        let run = |entries: &str| {
            let file = dir.join("entries.txt");
            std::fs::write(&file, entries).unwrap();
            let f = file.display().to_string();
            census(
                &strings(&[&p, "--native-census", &f, "--cooked", &d]),
                ParityMode::Off,
            )
        };
        let not_yet = run("export e Core Object\n");
        assert!(
            matches!(&not_yet, Err(Failure::NotYet(m)) if m.contains("census line export Core Object")),
            "{:?}",
            not_yet.as_ref().err().map(|_| ())
        );
        let missing = run("export e Core Nope\n");
        assert!(matches!(
            &missing,
            Err(Failure::Error(m)) if m == "entry file line 1: export not found: Core Nope"
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_string_turns_control_bytes_into_spaces() {
        assert_eq!(json_string("a\"b\\c\td"), "\"a\\\"b\\\\c d\"");
    }
}
