//! Owner: lane W3-DEFS. Command: `--def-dump <object path> --cooked <dir> [--overlay <file>]...` (Rust-only).
//!
//! Finds the definition at `<object path>` through the definition store (`ow_defs::DefStore`, identity policy
//! `FirstInScanOrder`, the run's parity mode) over the cooked root, after pushing each `--overlay` file in order
//! (`ow_defs::overlay::Overlay::read_file`: a live capture or the test format), and prints it as `--object-dump ...
//! --all` prints an export: `{"path":..,"class":..,"properties":{..},"log":[..]}`, every property, through the same
//! printer (`cmd/objects.rs`), so the two can be compared for the same object. The path printed is the object's path in
//! the file that holds it; `log` holds the decode log (the C++ `Runtime::log` texts). The store's own lines (`defs:
//! ...`) and each overlay's outcome go to stderr, so stdout stays comparable with `--object-dump`.
//!
//! The definition is loaded the store's way: its class's default object, then its own tags from the prefix the store
//! derives for it; references inside it are the store's unloaded handles, printed by their paths. Errors print as
//! `ow-package: <message>` with exit code 1, a `NotYet` with exit code 3 (the not-ported code); a path no file holds is
//! `object not found: <path>`. Lines end in `\n`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_defs::overlay::Overlay;
use ow_defs::store::DefError;
use ow_object::heap::{ObjectSpace, Origin};

use crate::args::Command;
use crate::cmd::NOT_PORTED;
use crate::cmd::objects::Printer;
use crate::cmd::properties::error_text;
use crate::json::quote;

const USAGE: &str =
    "usage: ow-package --def-dump <object path> --cooked <dir> [--overlay <file>]...";

/// Why the command failed: its message, and whether it is a `NotYet` (exit code 3).
struct Failure {
    message: String,
    not_yet: bool,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure {
            message,
            not_yet: false,
        }
    }
}

impl From<DefError> for Failure {
    fn from(e: DefError) -> Self {
        Failure {
            not_yet: matches!(e, DefError::NotYet(_)),
            message: error_text(&e),
        }
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = command;
    let mut out = String::new();
    let mut err = String::new();
    let outcome = execute(args, mode, &mut out, &mut err);
    // Ignore failed writes (a closed pipe), as the other commands do.
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    let _ = std::io::stderr().lock().write_all(err.as_bytes());
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {}", f.message);
            ExitCode::from(if f.not_yet { NOT_PORTED } else { 1 })
        }
    }
}

/// The command line after `--def-dump`: the object path, the cooked root, the overlay files in order.
fn parse(args: &[String]) -> Result<(String, PathBuf, Vec<PathBuf>), String> {
    let rest = match args.first().map(String::as_str) {
        Some("--def-dump") => &args[1..],
        _ => return Err(USAGE.to_string()),
    };
    let Some((path, mut options)) = rest.split_first() else {
        return Err(USAGE.to_string());
    };
    if path.starts_with("--") {
        return Err(USAGE.to_string());
    }
    let mut cooked = None;
    let mut overlays = Vec::new();
    while let Some((flag, tail)) = options.split_first() {
        let Some((value, tail)) = tail.split_first() else {
            return Err(USAGE.to_string());
        };
        match flag.as_str() {
            "--cooked" if cooked.is_none() => cooked = Some(PathBuf::from(value)),
            "--overlay" => overlays.push(PathBuf::from(value)),
            _ => return Err(USAGE.to_string()),
        }
        options = tail;
    }
    let cooked = cooked.ok_or_else(|| USAGE.to_string())?;
    Ok((path.clone(), cooked, overlays))
}

fn execute(
    args: &[String],
    mode: ParityMode,
    out: &mut String,
    err: &mut String,
) -> Result<(), Failure> {
    let (path, cooked, overlays) = parse(args)?;
    let path = ObjectPath::parse(&path).map_err(|e| format!("{path}: {e}"))?;
    let mut defs = DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, mode)?;
    let mut w = ObjectSpace::new();
    for file in &overlays {
        push(&mut defs, &mut w, file, err)?;
    }
    let object = defs
        .find(&mut w, &path)?
        .ok_or_else(|| format!("object not found: {}", path.as_str()))?;
    for file in &overlays {
        report(&defs, file, err);
    }
    let class = w.heap.get(object).map_err(|e| e.to_string())?.class;
    let class_path = match defs.code().key_of(class) {
        Some(key) => defs.code().class(key).path.clone(),
        None => w.classes.layout(class).path().as_str().to_string(),
    };
    let held_at = match w.heap.get(object).map_err(|e| e.to_string())?.origin {
        Origin::Export { package, index } => defs
            .packages()
            .get(package)
            .path_of(index)
            .map_err(|e| error_text(&e))?,
        _ => w
            .names
            .text(w.heap.get(object).map_err(|e| e.to_string())?.name)
            .into_owned(),
    };
    let log = defs.take_log();
    let printer = Printer {
        store: defs.packages(),
        code: defs.code(),
        space: &w,
    };
    *out = format!(
        "{{\"path\":{},\"class\":{},\"properties\":{{",
        quote(&held_at),
        quote(&class_path)
    );
    let slots = &w.heap.get(object).map_err(|e| e.to_string())?.slots;
    for (i, (key, (slot, decl))) in printer.properties(class)?.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{}:", quote(&key));
        printer.value(out, &printer.property(slots, slot, decl), 0)?;
    }
    out.push_str("},\"log\":[");
    let mut first = true;
    for line in &log {
        if line.starts_with("defs: ") {
            let _ = writeln!(err, "{line}");
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&quote(line));
    }
    out.push_str("]}\n");
    Ok(())
}

fn push(
    defs: &mut DefStore,
    w: &mut ObjectSpace,
    file: &Path,
    err: &mut String,
) -> Result<(), Failure> {
    let overlay = Overlay::read_file(file)?;
    let _ = writeln!(
        err,
        "defs: overlay {} ({:?}): {} patches",
        overlay.name,
        overlay.kind,
        overlay.patches.len()
    );
    defs.push_overlay(w, overlay)?;
    Ok(())
}

/// Each overlay's outcome after the load, on stderr.
fn report(defs: &DefStore, file: &Path, err: &mut String) {
    let _ = file;
    let mut id = 0;
    while let Some(status) = defs.overlay_status(ow_defs::overlay::OverlayId::new(id)) {
        let _ = writeln!(
            err,
            "defs: overlay {id}: {} applied, {} pending, {} failed",
            status.applied,
            status.pending,
            status.failed.len()
        );
        for (patch, error) in &status.failed {
            let _ = writeln!(
                err,
                "defs: overlay {id} patch {patch}: {}",
                error_text(error)
            );
        }
        id += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::testkit::ints;
    use ow_package::testkit::reflect::Declarations;

    /// `Core.upk`, every name and value invented: class `Thing` (`Count`, `Next`), `Default__Thing` (Count 1), and
    /// `Item` (Count 5, Next -> `Other`), `Other`.
    fn fixture() -> Vec<u8> {
        let mut d = Declarations::new(&["Int", "Object"]);
        let object = d.cls("Object", 0);
        d.default_object(object, "Object");
        let thing = d.cls("Thing", object);
        d.prop("Int", thing, "Count", 0);
        d.prop("Object", thing, "Next", thing);
        let mut payload = ints(&[0]);
        payload.extend(d.p.int_tag("Count", 1));
        payload.extend_from_slice(&d.none);
        d.p.add_export(thing, "Default__Thing", payload);
        let other = d.p.next_export() + 1;
        let mut payload = ints(&[0]);
        payload.extend(d.p.int_tag("Count", 5));
        payload.extend(d.p.object_tag("Next", other));
        payload.extend_from_slice(&d.none);
        d.p.add_export(thing, "Item", payload);
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&d.none);
        assert_eq!(d.p.add_export(thing, "Other", payload), other);
        d.build()
    }

    fn dump(tag: &str, extra: &[&str], overlay: Option<&str>) -> (Result<String, String>, String) {
        let dir = std::env::temp_dir().join(format!("ow-cli-w3defs-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Core.upk"), fixture()).unwrap();
        let mut args: Vec<String> = ["--def-dump"].map(String::from).to_vec();
        args.extend(extra.iter().map(|s| s.to_string()));
        args.push("--cooked".into());
        args.push(dir.to_str().unwrap().into());
        if let Some(text) = overlay {
            let file = dir.join("patch.txt");
            std::fs::write(&file, text).unwrap();
            args.push("--overlay".into());
            args.push(file.to_str().unwrap().into());
        }
        let (mut out, mut err) = (String::new(), String::new());
        let outcome = execute(&args, ParityMode::Off, &mut out, &mut err);
        let _ = std::fs::remove_dir_all(&dir);
        (outcome.map(|()| out).map_err(|f| f.message), err)
    }

    #[test]
    fn prints_every_property_as_the_object_dump_does() {
        let (out, _) = dump("plain", &["Core.Item"], None);
        assert_eq!(
            out.unwrap(),
            "{\"path\":\"Item\",\"class\":\"Core.Thing\",\"properties\":{\"count\":5,\"next\":\"Other\"},\"log\":[]}\n"
        );
    }

    #[test]
    fn an_overlay_is_applied_and_reported_on_stderr() {
        let (out, err) = dump(
            "overlay",
            &["Core.Item"],
            Some("Core.Item Count = 8\nCore.Item Nope = 1\n"),
        );
        assert_eq!(
            out.unwrap(),
            "{\"path\":\"Item\",\"class\":\"Core.Thing\",\"properties\":{\"count\":8,\"next\":\"Other\"},\"log\":[]}\n"
        );
        assert!(
            err.contains("defs: overlay 0: 1 applied, 0 pending, 1 failed"),
            "{err}"
        );
    }

    #[test]
    fn errors() {
        assert_eq!(
            dump("missing", &["Core.Nothing"], None).0.unwrap_err(),
            "object not found: Core.Nothing"
        );
        assert_eq!(dump("usage", &[], None).0.unwrap_err(), USAGE);
        assert_eq!(
            dump("code", &["Core.Thing"], None).0.unwrap_err(),
            "not a definition: Core.Thing: a code export"
        );
        assert!(parse(&["x".into(), "--def-dump".into()]).is_err());
        assert!(parse(&["--def-dump".into(), "A.B".into(), "--cooked".into()]).is_err());
        assert!(
            parse(&[
                "--def-dump".into(),
                "A.B".into(),
                "--bogus".into(),
                "x".into()
            ])
            .is_err()
        );
    }
}
