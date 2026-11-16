//! Owner: lane W5-BEHAVIOR. Commands: `--behavior-dump`, `--behavior-run`, the port of their branches in
//! `src/cli.cpp`. AI-assisted (Claude Code).
//!
//! - `<package> --behavior-dump <provider-path> --cooked <dir>`: the provider's sequences, enable conditions, decoded
//!   variables (the untagged value block included) and every property-to-variable link, as the kernel sees them:
//!   `{"provider":..,"values_decoded":..,"diagnostics":[..],"sequences":[{"name":..,"enabled_on_spawn":..,
//!   "condition":"<Package.Class>:<name>","variables":[{"type","name","word","object"}],"behavior_inputs":[{"behavior",
//!   "property","link","variables":[..]}],"graph":[..]}]}`. Exit 0 when the value block decoded, else 1.
//! - `<package> --behavior-run <provider-path> --cooked <dir> <step>...`: one provider alone. Steps: `enable:<seq>`,
//!   `disable:<seq>`, `tick:<s>`, `event:<name>[:<Property>=<object path>[,...]]` (event output values) and
//!   `fire:<link id>:<name>` (the event with a link-id filter; -1 is every link). `Behavior_CompareObject` runs (the
//!   world handler of `ow_behavior::handlers`); every other class is reported at the boundary. Prints
//!   `{"provider":..,"values_decoded":..,"trace":[..],"boundary":[..],"errors":[..],"diagnostics":[..]}`; exit 0
//!   when no error was recorded, else 1.
//!
//! Order, as the C++: the source package is loaded first, then the arguments are checked, the store opens the cooked
//! root, the source package is loaded again through the store by its name and the provider is found in it by path
//! (a leading `<package>.` dropped, ASCII case folded). The provider is loaded from that file through the definition
//! store (`DefStore::load_export`); a file that is not the path's holder under the store's identity policy is refused
//! (the C++ reads the file it is given). Errors print `ow-package: <message>` with exit code 1 (a `NotYet` exit code
//! 3). Lines end in `\n`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::registry::AttributeSystem;
use ow_behavior::handlers::{COMPARE_OBJECT, Pack};
use ow_behavior::kernel::Provider;
use ow_behavior::provider::{BehaviorError, class_path_of, object_name};
use ow_core::num::{StrtodPrefix, strtod_prefix};
use ow_core::{DetMap, DetSet, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::{IdentityPolicy, LoadSet};
use ow_defs::store::DefError;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::prefix::PropertyPrefix;
use ow_package::package::Package;
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

use crate::args::{Command, USAGE};
use crate::cmd::NOT_PORTED;
use crate::cmd::properties::error_text;
use crate::json::quote;

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

impl From<BehaviorError> for Failure {
    fn from(e: BehaviorError) -> Self {
        Failure {
            not_yet: e.not_yet().is_some(),
            message: error_text(&e),
        }
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = command;
    let mut out = String::new();
    let outcome = execute(args, mode, &mut out);
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    match outcome {
        Ok(code) => ExitCode::from(code),
        Err(f) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {}", f.message);
            ExitCode::from(if f.not_yet { NOT_PORTED } else { 1 })
        }
    }
}

/// `args[0]` is the source package, `args[1]` the mode. Returns the exit code.
fn execute(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, Failure> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    let dump = args[1] == "--behavior-dump";
    if (dump && args.len() != 5) || (!dump && args.len() < 5) || args[3] != "--cooked" {
        return Err(USAGE.to_string().into());
    }
    let name = PackageName::parse(&source.name).map_err(|e| format!("{}: {e}", source.name))?;
    let mut vm = open(&args[4], mode, IdentityPolicy::FirstInScanOrder)?;
    let provider = match find_provider(&mut vm, &name, &args[2]) {
        // A cooked copy: read it from the given file (module documentation).
        Err(BehaviorError::Vm(VmError::Def(DefError::NotADefinition(_)))) => {
            vm = open(
                &args[4],
                mode,
                IdentityPolicy::Loaded(LoadSet::new(vec![name.clone()])),
            )?;
            find_provider(&mut vm, &name, &args[2])?
        }
        other => other?,
    };
    if dump {
        let p = Provider::new(&mut vm, provider)?;
        dump_json(&vm, &p, out)?;
        Ok(if p.values_decoded() { 0 } else { 1 })
    } else {
        let mut p = Provider::new(&mut vm, provider)?;
        // Every class but CompareObject (the world handler) is reported at the boundary: the classes of each sequence
        // name's behaviors, as the C++ collects them.
        let mut classes = DetSet::new();
        for name in p.sequence_names() {
            for behavior in p.behaviors(&name) {
                if behavior.class != COMPARE_OBJECT {
                    classes.insert(behavior.class.clone());
                }
            }
        }
        for class in &classes {
            p.report_at_boundary(class, None, Vec::new());
        }
        for step in &args[5..] {
            run_step(&mut vm, &mut p, step)?;
        }
        run_json(&p, out);
        Ok(if p.errors.is_empty() { 0 } else { 1 })
    }
}

/// A VM over a definition store of `cooked` (`policy`, the run's parity mode) with the world handlers registered.
fn open(cooked: &str, mode: ParityMode, policy: IdentityPolicy) -> Result<Vm, Failure> {
    let defs = DefStore::open(PathBuf::from(cooked), policy, mode).map_err(BehaviorError::from)?;
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    Pack.register(&mut vm).map_err(BehaviorError::from)?;
    Ok(vm)
}

/// The provider export `path` of the store's copy of package `name`, loaded (module documentation). A path the
/// package lacks is the C++ text, as a [`BehaviorError::Data`].
fn find_provider(vm: &mut Vm, name: &PackageName, path: &str) -> Result<ObjRef, BehaviorError> {
    let (_, store) = vm.defs.code_mut();
    let id = store.load(name).map_err(DefError::from)?;
    let index = store
        .get(id)
        .find_export(path)?
        .filter(|&i| i > 0)
        .ok_or_else(|| BehaviorError::Data(format!("provider not found: {path}")))?;
    Ok(vm
        .defs
        .load_export(&mut vm.space, id, index, PropertyPrefix::Ordinary)?)
}

fn run_step(vm: &mut Vm, p: &mut Provider, step: &str) -> Result<(), Failure> {
    let usage = || Failure::from(USAGE.to_string());
    if let Some(name) = step.strip_prefix("enable:") {
        p.set_sequence_enabled(vm, name, true)?;
    } else if let Some(name) = step.strip_prefix("disable:") {
        p.set_sequence_enabled(vm, name, false)?;
    } else if let Some(seconds) = step.strip_prefix("tick:") {
        p.tick(vm, stod(seconds)?)?;
    } else if let Some(rest) = step.strip_prefix("fire:") {
        let (id, name) = rest.split_once(':').ok_or_else(usage)?;
        p.fire_event(vm, name, &DetMap::new(), stoi(id)?, &[])?;
    } else if let Some(rest) = step.strip_prefix("event:") {
        let (name, list) = match rest.split_once(':') {
            Some((name, list)) => (name, Some(list)),
            None => (rest, None),
        };
        let mut outputs = DetMap::new();
        if let Some(list) = list {
            for pair in getline_pieces(list) {
                let (property, value) = pair.split_once('=').ok_or_else(usage)?;
                outputs.insert(property.to_string(), value.to_string());
            }
        }
        p.fire_event(vm, name, &outputs, -1, &[])?;
    } else {
        return Err(usage());
    }
    Ok(())
}

/// The pieces `std::getline(stream, piece, ',')` reads from `list`: split at each `,`, without the empty piece after a
/// final `,` (or of an empty text), which `getline` does not produce.
fn getline_pieces(list: &str) -> Vec<&str> {
    let mut pieces: Vec<&str> = list.split(',').collect();
    if pieces.last() == Some(&"") {
        pieces.pop();
    }
    pieces
}

/// `std::stoi(text)`: leading C white space, an optional sign and decimal digits; the MSVC exception texts.
fn stoi(text: &str) -> Result<i32, Failure> {
    let b = text.as_bytes();
    let mut at = 0;
    while at < b.len() && matches!(b[at], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        at += 1;
    }
    let start = at;
    if at < b.len() && (b[at] == b'+' || b[at] == b'-') {
        at += 1;
    }
    let digits = at;
    while at < b.len() && b[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return Err("invalid stoi argument".to_string().into());
    }
    text[start..at]
        .parse::<i32>()
        .map_err(|_| "stoi argument out of range".to_string().into())
}

/// `std::stod(text)`: decimal text, `inf` and `nan` (`ow_core::num::strtod_prefix`); an overflow to an infinity is the
/// MSVC `out_of_range`. A hexadecimal float is refused (no parser for it is written).
fn stod(text: &str) -> Result<f64, Failure> {
    let Some(StrtodPrefix {
        start,
        end,
        hexadecimal,
        ..
    }) = strtod_prefix(text)
    else {
        return Err("invalid stod argument".to_string().into());
    };
    if hexadecimal {
        return Err(
            format!("hexadecimal float text is not read by the Rust binary: {text}").into(),
        );
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
            return Err("stod argument out of range".to_string().into());
        }
        v
    };
    Ok(if lower.starts_with('-') {
        -value
    } else {
        value
    })
}

fn list(out: &mut String, key: &str, lines: &[String]) {
    let _ = write!(out, ",\"{key}\":[");
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(line));
    }
    out.push(']');
}

fn run_json(p: &Provider, out: &mut String) {
    let _ = write!(
        out,
        "{{\"provider\":{},\"values_decoded\":{}",
        quote(p.path()),
        p.values_decoded()
    );
    list(out, "trace", &p.trace);
    list(out, "boundary", &p.boundary);
    list(out, "errors", &p.errors);
    list(out, "diagnostics", p.diagnostics());
    out.push_str("}\n");
}

fn dump_json(vm: &Vm, p: &Provider, out: &mut String) -> Result<(), Failure> {
    let _ = write!(
        out,
        "{{\"provider\":{},\"values_decoded\":{},\"diagnostics\":[",
        quote(p.path()),
        p.values_decoded()
    );
    for (i, line) in p.diagnostics().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(line));
    }
    out.push_str("],\"sequences\":[");
    for (i, name) in p.sequence_names().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let condition = match p.enable_condition(name) {
            Some(c) => format!("{}:{}", class_path_of(vm, c)?, object_name(vm, c)?),
            None => String::new(),
        };
        let _ = write!(
            out,
            "{{\"name\":{},\"enabled_on_spawn\":{},\"condition\":{},\"variables\":[",
            quote(name),
            p.sequence_enabled(name),
            quote(&condition)
        );
        for (j, variable) in p.variables(name).iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"type\":{},\"name\":{},\"word\":{},\"object\":{}}}",
                quote(&variable.var_type),
                quote(&variable.name),
                variable.word,
                quote(&variable.object)
            );
        }
        out.push_str("],\"behavior_inputs\":[");
        let mut first = true;
        for behavior in p.behaviors(name) {
            for link in &behavior.variables {
                if !first {
                    out.push(',');
                }
                first = false;
                let _ = write!(
                    out,
                    "{{\"behavior\":{},\"property\":{},\"link\":{},\"variables\":[",
                    quote(&behavior.name),
                    quote(&link.property),
                    quote(&link.link_type)
                );
                for (v, index) in link.variables.iter().enumerate() {
                    if v > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{index}");
                }
                out.push_str("]}");
            }
        }
        out.push_str("],\"graph\":[");
        for (j, line) in p.graph(name).iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&quote(line));
        }
        out.push_str("]}");
    }
    out.push_str("]}\n");
    Ok(())
}

// The invented packages of the `ow-behavior` tests, the same file (it uses only `ow_package::testkit`).
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../ow-behavior/src/kernel/fixtures/packages.rs"]
mod packages;

#[cfg(test)]
mod tests {
    use super::*;
    use packages::{install, provider_package, rules, toy_provider};

    /// The invented install the CLI tests run on: the code packages, `Things`, ToyProvider exact (`TestBpd`), with a
    /// short value block (`TestShort`) and with an unknown link type (`TestLink`), and ToyRules (`TestRules`).
    fn files() -> Vec<(String, Vec<u8>)> {
        install(&[
            (
                "TestBpd",
                provider_package(&toy_provider(
                    "exact",
                    "BVAR_InstanceData",
                    "BVARLINK_Input",
                )),
            ),
            (
                "TestShort",
                provider_package(&toy_provider(
                    "short",
                    "BVAR_InstanceData",
                    "BVARLINK_Input",
                )),
            ),
            (
                "TestLink",
                provider_package(&toy_provider(
                    "exact",
                    "BVAR_InstanceData",
                    "BVARLINK_Unknown",
                )),
            ),
            ("TestRules", provider_package(&rules())),
        ])
    }

    /// Writes the invented install under ignored `local/` so the C++ binary can be run on the same bytes (the
    /// expected outputs below are what it printed). Run by hand: `cargo test -p ow-cli -- --ignored write_fixture`.
    #[test]
    #[ignore]
    fn write_fixture() {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/W5-BEHAVIOR/cli-fixture");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in files() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }

    /// What the C++ binary printed (stdout, CRLF read as LF, without the final newline) and its exit code for each
    /// case on the bytes of [`files`] (`local/p2/W5-BEHAVIOR/tools/cli_expectations.py`). Invented data only.
    const CASES: &[(&str, &[&str], u8, &str)] = &[
        (
            "TestBpd",
            &["--behavior-dump", "ToyProvider"],
            0,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"diagnostics\":[],\"sequences\":[{\"name\":\"Main\",\"enabled_on_spawn\":true,\"condition\":\"\",\"variables\":[{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":-5,\"object\":\"Things.DamageType.ToyFire\"},{\"type\":\"BVAR_Int\",\"name\":\"Count\",\"word\":7,\"object\":\"\"},{\"type\":\"BVAR_NamedVariable\",\"name\":\"Ref\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Float\",\"name\":\"None\",\"word\":1069547520,\"object\":\"\"}],\"behavior_inputs\":[{\"behavior\":\"Cmp\",\"property\":\"ObjectA\",\"link\":\"BVARLINK_Input\",\"variables\":[0]},{\"behavior\":\"Cmp\",\"property\":\"ObjectB\",\"link\":\"BVARLINK_Input\",\"variables\":[1]}],\"graph\":[\"event OnHit -> Cmp id=-1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:Cmp -> Same id=0 delay=0.000000\",\"WillowGame.Behavior_CompareObject:Cmp -> Diff id=1 delay=0.000000\"]},{\"name\":\"Extra\",\"enabled_on_spawn\":false,\"condition\":\"\",\"variables\":[{\"type\":\"BVAR_InstanceData\",\"name\":\"None\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":0,\"object\":\"\"}],\"behavior_inputs\":[],\"graph\":[]}]}",
        ),
        (
            "TestShort",
            &["--behavior-dump", "ToyProvider"],
            1,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":false,\"diagnostics\":[\"variable value block shorter than its variables in ToyProvider\"],\"sequences\":[{\"name\":\"Main\",\"enabled_on_spawn\":true,\"condition\":\"\",\"variables\":[{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Int\",\"name\":\"Count\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_NamedVariable\",\"name\":\"Ref\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Float\",\"name\":\"None\",\"word\":0,\"object\":\"\"}],\"behavior_inputs\":[{\"behavior\":\"Cmp\",\"property\":\"ObjectA\",\"link\":\"BVARLINK_Input\",\"variables\":[0]},{\"behavior\":\"Cmp\",\"property\":\"ObjectB\",\"link\":\"BVARLINK_Input\",\"variables\":[1]}],\"graph\":[\"event OnHit -> Cmp id=-1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:Cmp -> Same id=0 delay=0.000000\",\"WillowGame.Behavior_CompareObject:Cmp -> Diff id=1 delay=0.000000\"]},{\"name\":\"Extra\",\"enabled_on_spawn\":false,\"condition\":\"\",\"variables\":[{\"type\":\"BVAR_InstanceData\",\"name\":\"None\",\"word\":0,\"object\":\"\"},{\"type\":\"BVAR_Object\",\"name\":\"None\",\"word\":0,\"object\":\"\"}],\"behavior_inputs\":[],\"graph\":[]}]}",
        ),
        (
            "TestRules",
            &["--behavior-dump", "ToyRules"],
            0,
            "{\"provider\":\"ToyRules\",\"values_decoded\":true,\"diagnostics\":[],\"sequences\":[{\"name\":\"Rules\",\"enabled_on_spawn\":true,\"condition\":\"\",\"variables\":[],\"behavior_inputs\":[],\"graph\":[\"event Order -> A id=-1 delay=0.000000\",\"event Order -> B id=-1 delay=0.000000\",\"event Branch -> P id=-1 delay=0.000000\",\"event Filter -> C4 id=4 delay=0.000000\",\"event Filter -> C5 id=5 delay=0.000000\",\"event Twice -> Twice id=0 delay=0.000000\",\"event Twice -> Twice id=0 delay=0.000000\",\"event Once -> C4 id=0 delay=0.000000\",\"event Delay2 -> C4 id=0 delay=0.000000\",\"event Off -> C4 id=0 delay=0.000000\",\"event DefOff -> CmpOff id=-1 delay=0.000000\",\"event DefOn -> CmpOn id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:A -> A2 id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:B -> B2 id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:P -> X id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:P -> Y id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:X -> X2 id=-1 delay=0.000000\",\"WillowGame.Behavior_ToySame:Y -> Y2 id=-1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOff -> Dflt id=-1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOff -> Zero id=0 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOff -> One id=1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOn -> Dflt id=-1 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOn -> Zero id=0 delay=0.000000\",\"WillowGame.Behavior_CompareObject:CmpOn -> One id=1 delay=0.000000\"]}]}",
        ),
        (
            "TestBpd",
            &[
                "--behavior-run",
                "ToyProvider",
                "event:OnHit:DamageType=Things.DamageType.ToyFire",
            ],
            0,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"trace\":[\"OnHit -> Cmp\",\"OnHit -> Same\"],\"boundary\":[\"OnHit -> WillowGame.Behavior_ToySame:Same\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestBpd",
            &[
                "--behavior-run",
                "ToyProvider",
                "event:OnHit:DamageType=Things.DamageType.ToyIce",
                "event:OnHit:DamageType=Things.DamageType.ToyFire",
            ],
            0,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"trace\":[\"OnHit -> Cmp\",\"OnHit -> Diff\",\"OnHit -> Cmp\",\"OnHit -> Same\"],\"boundary\":[\"OnHit -> WillowGame.Behavior_ToyDifferent:Diff\",\"OnHit -> WillowGame.Behavior_ToySame:Same\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestBpd",
            &[
                "--behavior-run",
                "ToyProvider",
                "disable:Main",
                "event:OnHit:DamageType=Things.DamageType.ToyFire",
                "enable:Main",
                "event:OnHit",
            ],
            0,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"trace\":[\"OnHit -> Cmp\",\"OnHit -> Diff\"],\"boundary\":[\"OnHit -> WillowGame.Behavior_ToyDifferent:Diff\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestBpd",
            &[
                "--behavior-run",
                "ToyProvider",
                "enable:Extra",
                "tick:0.5",
                "fire:0:OnHit",
                "fire:-1:OnHit",
            ],
            0,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"trace\":[\"OnHit -> Cmp\",\"OnHit -> Diff\"],\"boundary\":[\"OnHit -> WillowGame.Behavior_ToyDifferent:Diff\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestShort",
            &[
                "--behavior-run",
                "ToyProvider",
                "event:OnHit:DamageType=Things.DamageType.ToyFire",
            ],
            1,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":false,\"trace\":[\"OnHit -> Cmp\"],\"boundary\":[],\"errors\":[\"variable values not decoded for Cmp.ObjectA in ToyProvider\",\"variable values not decoded for Cmp.ObjectB in ToyProvider\"],\"diagnostics\":[\"variable value block shorter than its variables in ToyProvider\"]}",
        ),
        (
            "TestLink",
            &[
                "--behavior-run",
                "ToyProvider",
                "event:OnHit:DamageType=Things.DamageType.ToyIce",
            ],
            1,
            "{\"provider\":\"ToyProvider\",\"values_decoded\":true,\"trace\":[\"OnHit -> Cmp\"],\"boundary\":[],\"errors\":[\"unexpected variable link type BVARLINK_Unknown for Cmp.ObjectA\"],\"diagnostics\":[]}",
        ),
        (
            "TestRules",
            &[
                "--behavior-run",
                "ToyRules",
                "fire:4:Filter",
                "event:Twice",
                "event:DefOff",
                "event:DefOn",
            ],
            0,
            "{\"provider\":\"ToyRules\",\"values_decoded\":true,\"trace\":[\"Filter -> C4\",\"Twice -> Twice\",\"Twice -> Twice\",\"DefOff -> CmpOff\",\"DefOff -> One\",\"DefOn -> CmpOn\",\"DefOn -> Dflt\",\"DefOn -> One\"],\"boundary\":[\"Filter -> WillowGame.Behavior_ToySame:C4\",\"Twice -> WillowGame.Behavior_ToySame:Twice\",\"Twice -> WillowGame.Behavior_ToySame:Twice\",\"DefOff -> WillowGame.Behavior_ToySame:One\",\"DefOn -> WillowGame.Behavior_ToySame:Dflt\",\"DefOn -> WillowGame.Behavior_ToySame:One\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestRules",
            &[
                "--behavior-run",
                "ToyRules",
                "event:Order",
                "event:Branch",
                "event:Once",
                "event:Once",
                "event:Off",
            ],
            0,
            "{\"provider\":\"ToyRules\",\"values_decoded\":true,\"trace\":[\"Order -> A\",\"Order -> A2\",\"Order -> B\",\"Order -> B2\",\"Branch -> P\",\"Branch -> Y\",\"Branch -> Y2\",\"Branch -> X\",\"Branch -> X2\",\"Once -> C4\"],\"boundary\":[\"Order -> WillowGame.Behavior_ToySame:A\",\"Order -> WillowGame.Behavior_ToySame:A2\",\"Order -> WillowGame.Behavior_ToySame:B\",\"Order -> WillowGame.Behavior_ToySame:B2\",\"Branch -> WillowGame.Behavior_ToySame:P\",\"Branch -> WillowGame.Behavior_ToySame:Y\",\"Branch -> WillowGame.Behavior_ToySame:Y2\",\"Branch -> WillowGame.Behavior_ToySame:X\",\"Branch -> WillowGame.Behavior_ToySame:X2\",\"Once -> WillowGame.Behavior_ToySame:C4\"],\"errors\":[],\"diagnostics\":[]}",
        ),
        (
            "TestRules",
            &[
                "--behavior-run",
                "ToyRules",
                "event:Delay2",
                "tick:1",
                "event:Delay2",
                "tick:2",
                "event:Delay2",
            ],
            0,
            "{\"provider\":\"ToyRules\",\"values_decoded\":true,\"trace\":[\"Delay2 -> C4\",\"Delay2 -> C4\"],\"boundary\":[\"Delay2 -> WillowGame.Behavior_ToySame:C4\",\"Delay2 -> WillowGame.Behavior_ToySame:C4\"],\"errors\":[],\"diagnostics\":[]}",
        ),
    ];

    struct Install(std::path::PathBuf);

    impl Drop for Install {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn install_dir(tag: &str) -> Install {
        let dir =
            std::env::temp_dir().join(format!("ow-cli-behavior-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in files() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        Install(dir)
    }

    fn run_case(dir: &Path, package: &str, args: &[&str]) -> (Result<u8, String>, String) {
        let mut argv = vec![
            dir.join(format!("{package}.upk")).display().to_string(),
            args[0].to_string(),
            args[1].to_string(),
            "--cooked".to_string(),
            dir.display().to_string(),
        ];
        argv.extend(args[2..].iter().map(|a| a.to_string()));
        let mut out = String::new();
        let result = execute(&argv, ParityMode::Off, &mut out).map_err(|f| f.message);
        (result, out)
    }

    #[test]
    fn the_output_is_the_cxx_output_on_the_invented_install() {
        let install = install_dir("cases");
        for (package, args, code, stdout) in CASES {
            let (result, out) = run_case(&install.0, package, args);
            assert_eq!(result, Ok(*code), "{package} {args:?}");
            assert_eq!(
                out,
                format!(
                    "{stdout}
"
                ),
                "{package} {args:?}"
            );
        }
    }

    #[test]
    fn refusals_carry_the_cxx_texts() {
        let install = install_dir("refusals");
        let (result, out) = run_case(&install.0, "TestBpd", &["--behavior-dump", "Missing"]);
        assert_eq!(result, Err("provider not found: Missing".to_string()));
        assert!(out.is_empty());
        let (result, _) = run_case(
            &install.0,
            "TestBpd",
            &["--behavior-dump", "ToyProvider.Cmp"],
        );
        assert_eq!(
            result,
            Err(
                "not a behavior provider: ToyProvider.Cmp (WillowGame.Behavior_CompareObject)"
                    .to_string()
            )
        );
        for bad in ["fire:1", "event:OnHit:x", "bogus"] {
            let (result, _) = run_case(
                &install.0,
                "TestBpd",
                &["--behavior-run", "ToyProvider", bad],
            );
            assert_eq!(result, Err(USAGE.to_string()), "{bad}");
        }
        let (result, _) = run_case(
            &install.0,
            "TestBpd",
            &["--behavior-run", "ToyProvider", "tick:x"],
        );
        assert_eq!(result, Err("invalid stod argument".to_string()));
        // A dump takes no step.
        let (result, _) = run_case(
            &install.0,
            "TestBpd",
            &["--behavior-dump", "ToyProvider", "extra"],
        );
        assert_eq!(result, Err(USAGE.to_string()));
    }

    #[test]
    fn getline_pieces_drop_only_a_final_empty_piece() {
        assert_eq!(getline_pieces(""), Vec::<&str>::new());
        assert_eq!(getline_pieces("a=b"), vec!["a=b"]);
        assert_eq!(getline_pieces("a=b,"), vec!["a=b"]);
        assert_eq!(getline_pieces(","), vec![""]);
        assert_eq!(getline_pieces("a=b,,c=d"), vec!["a=b", "", "c=d"]);
    }

    #[test]
    fn stoi_and_stod_follow_the_msvc_runtime() {
        assert_eq!(stoi(" -1x").ok(), Some(-1));
        assert_eq!(stoi("4").ok(), Some(4));
        assert_eq!(
            stoi("x").err().map(|f| f.message),
            Some("invalid stoi argument".into())
        );
        assert_eq!(
            stoi("99999999999").err().map(|f| f.message),
            Some("stoi argument out of range".into())
        );
        assert_eq!(stod("1.5s").ok(), Some(1.5));
        assert_eq!(
            stod("x").err().map(|f| f.message),
            Some("invalid stod argument".into())
        );
        assert_eq!(
            stod("1e999").err().map(|f| f.message),
            Some("stod argument out of range".into())
        );
    }
}
