//! Owner: lane W2-REFLECT. Commands: `--class-check --cooked <directory> [--failures]` and
//! `--interface-compare --cooked <directory>`: the port of their branch in `src/cli.cpp` and of `checkClassBodies` /
//! `compareInterfaces` (`src/vm_sweep.cpp`), with the same argument checks in the same order, the same stdout JSON
//! (the `jsonString` escapes of `vm_sweep.cpp`), the same exit codes (`--class-check` exits 1 when any check failed)
//! and the same `ow-package: <message>` stderr line and exit code 1 on an error.
//!
//! Order, as the C++: the source package is loaded first (before the options are read, as every command after
//! `--resolve` in `main` does), then the options (`--cooked <dir>`, the last one wins; `--failures` for
//! `--class-check` only), then the store scans the cooked root, then the check runs on the source package loaded
//! again through the store.
//!
//! Rust-only modifier: `--interface-compare --cooked <directory> --layouts` loads the nine code packages into a
//! `ClassTable` (`ow_object::reflect::load_code_packages`) and prints how many classes got a layout and why each other
//! did not. The C++ binary has no such command (its VM keeps no layouts). `args.rs` lists it among `--interface-compare`'s
//! options; the C++ usage text, which `args.rs` keeps byte for byte, does not.
//!
//! Lines end in `\n` on every platform (the parity harness normalises line ends; see `cmd/resolve.rs`). The command
//! does not depend on the parity mode.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ClassId;
use ow_object::reflect::{self, ClassCheck, CodeIndex, InterfaceComparison};
use ow_package::package::Package;
use ow_package::store::PackageStore;

use crate::args::{Command, USAGE};

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = (command, mode);
    match execute(args) {
        Ok((out, code)) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = std::io::stdout().lock().write_all(out.as_bytes());
            ExitCode::from(code)
        }
        Err(message) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// The error and each of its causes (`source()`), joined with `: `, as the other command families print them.
fn message(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

/// `args` is the command line without the program name: `args[0]` is the source package, `args[1]` the mode.
/// Returns stdout's text and the exit code.
fn execute(args: &[String]) -> Result<(String, u8), String> {
    let mode = args.get(1).map(String::as_str).unwrap_or_default();
    let check = mode == "--class-check";
    Package::load(Path::new(&args[0])).map_err(|e| message(&e))?;
    let mut cooked = String::new();
    let mut list_failures = false;
    let mut layouts = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--cooked" => {
                i += 1;
                cooked = args.get(i).cloned().ok_or("missing value for --cooked")?;
            }
            "--failures" if check => list_failures = true,
            "--layouts" if !check => layouts = true,
            _ => return Err(USAGE.to_string()),
        }
        i += 1;
    }
    if cooked.is_empty() {
        return Err(USAGE.to_string());
    }
    let mut store = PackageStore::open(PathBuf::from(cooked)).map_err(|e| message(&e))?;
    if layouts {
        return layout_summary(&mut store).map(|out| (out, 0));
    }
    let mut index = CodeIndex::new();
    if !check {
        let comparison = index
            .compare_interfaces(&mut store)
            .map_err(|e| message(&e))?;
        return Ok((comparison_json(&comparison), 0));
    }
    let source = store
        .load_path(Path::new(&args[0]))
        .map_err(|e| message(&e))?;
    let summary = index
        .check_class_bodies(&mut store, source)
        .map_err(|e| message(&e))?;
    let code = u8::from(!summary.failures.is_empty());
    Ok((check_json(&summary, list_failures), code))
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

/// The C++ `checkClassBodies` JSON, with its line end.
fn check_json(c: &ClassCheck, list_failures: bool) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"package\":{},\"classes\":{},\"exact\":{},\"default_objects\":{},\"interface_classes\":{},\
         \"classes_with_interfaces\":{},\"interface_entries\":{},\"entries_to_interface_classes\":{},\
         \"table_struct_properties\":{},\"table_null\":{},\"failed\":{}",
        json_string(&c.package),
        c.classes,
        c.exact,
        c.default_objects,
        c.interface_classes,
        c.classes_with_interfaces,
        c.interface_entries,
        c.entries_to_interface_classes,
        c.table_struct_properties,
        c.table_null,
        c.failures.len()
    );
    if list_failures {
        out.push_str(",\"failures\":[");
        for (i, failure) in c.failures.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&json_string(failure));
        }
        out.push(']');
    }
    out.push_str("}\n");
    out
}

/// The C++ `compareInterfaces` JSON, with its line end.
fn comparison_json(c: &InterfaceComparison) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"classes\":{},\"interfaces\":{},\"both_yes\":{},\"table_only_yes\":{},\"coded_only_yes\":{},\
         \"functions_only_rule\":{{\"both_yes\":{},\"table_only_yes\":{},\"stand_in_only_yes\":{}}},\"slice\":{{",
        c.classes,
        c.interfaces,
        c.both_yes,
        c.table_only_yes,
        c.coded_only_yes,
        c.functions_only[0],
        c.functions_only[1],
        c.functions_only[2]
    );
    for (i, (name, counts)) in c.slice.iter().enumerate() {
        let _ = write!(
            out,
            "{}{}:{{\"table_yes\":{},\"table_no\":{},\"disagree_coded\":{},\"disagree_functions_only\":{}}}",
            if i > 0 { "," } else { "" },
            json_string(name),
            counts[0],
            counts[1],
            counts[2],
            counts[3]
        );
    }
    out.push_str("}}\n");
    out
}

/// The Rust-only `--layouts` summary: classes of the nine code packages, how many got a layout, the table's
/// struct and enum counts, and each failure with its cause chain.
fn layout_summary(store: &mut PackageStore) -> Result<String, String> {
    let mut space = ObjectSpace::new();
    let index = reflect::load_code_packages(store, &mut space.names, &mut space.classes)
        .map_err(|e| message(&e))?;
    let classes = index
        .classes()
        .filter(|(key, _)| index.layout_of(*key).is_some())
        .count();
    let failures = index.layout_failures();
    // Attribute value properties whose companions were linked, over the defined classes.
    let attributes: usize = (0..space.classes.len() as u32)
        .filter_map(|i| space.classes.get_layout(ClassId::new(i)))
        .filter(|layout| layout.is_defined())
        .map(|layout| {
            layout
                .own_decls()
                .filter(|(_, d)| d.attr.is_some() && d.is_stack_attribute())
                .count()
        })
        .sum();
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"classes\":{classes},\"defined\":{},\"failed\":{},\"table_classes\":{},\"linked_attributes\":{attributes},\"failures\":[",
        classes - failures.len(),
        failures.len(),
        space.classes.len()
    );
    for (i, (path, error)) in failures.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"class\":{},\"error\":{}}}",
            json_string(path),
            json_string(&message(*error))
        );
    }
    out.push_str("]}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The package is read before the options, then the options in the C++ order.
    #[test]
    fn bad_command_lines_fail_in_the_cxx_order() {
        assert_eq!(
            execute(&args(&["no-such-file.upk", "--class-check", "--bogus"])),
            Err("cannot open input package".into())
        );
    }

    #[test]
    fn json_strings_escape_as_vm_sweep_does() {
        assert_eq!(json_string("a\"b\\c\nd\u{1}é"), "\"a\\\"b\\\\c d é\"");
    }

    #[test]
    fn check_json_is_the_cxx_layout() {
        let mut c = ClassCheck::default();
        c.package = "Core".into();
        c.classes = 2;
        c.exact = 1;
        c.failures = vec!["X: class body does not end at the serial size".into()];
        assert_eq!(
            check_json(&c, false),
            "{\"package\":\"Core\",\"classes\":2,\"exact\":1,\"default_objects\":0,\"interface_classes\":0,\
             \"classes_with_interfaces\":0,\"interface_entries\":0,\"entries_to_interface_classes\":0,\
             \"table_struct_properties\":0,\"table_null\":0,\"failed\":1}\n"
        );
        assert!(check_json(&c, true).ends_with(
            ",\"failed\":1,\"failures\":[\"X: class body does not end at the serial size\"]}\n"
        ));
    }
}
