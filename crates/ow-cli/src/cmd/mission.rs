//! Owner: lane W6-MISSION. Commands: `--mission-run`, the port of its branch in `src/cli.cpp`. AI-assisted (Claude
//! Code).
//!
//! `<package> --mission-run <mission-path> --cooked <dir> <step>...` runs one installed mission on the tracker
//! (`ow_mission::tracker`) and prints `{"mission":..,"name":..,"steps":[{"step","ok","set","status"[,<screen key>],
//! "kickoff_pending","effects":[{"t","kind","a","b","c","detail"}]}],"status":..,"errors":[..][,"script":{..}],
//! "tracker":[{"objective","message","complete","optional"}]}`. Exit 0 when no error line was recorded, else 1.
//!
//! Steps, as the C++: `accept`, `kickoff`, `obj:<name>[:<bit>]`, `custom:<name>`, `turnin`, `tick:<seconds>`, and the
//! script bridge's (`ow_mission::bridge`, built only when a step starts `script:`, `stage:` or `player:`):
//! `script:accept`, `script:turnin`, `script:screen`, `script:use`, `stage:<n>`, `player:<level>:<experience>`,
//! `pool`. Every dependency of the mission counts as complete (the C++ probe).
//!
//! The dialog steps drive the mission's dialog (`MissionSystem::dialog`, lane W5-DIALOG): `lines:<s>` (a test line
//! player: every line lasts that long), `timedlines:<s>` (a timed line player; a line stopped while it plays is the
//! stderr line `dialog line stopped while playing: <id>`), `talker:<name tag path>` (a pawn that can talk),
//! `kismet:<variable>|<name tag path>` (a new talker bound to the mission's named Kismet variable; no tag clears the
//! variable), `component:<speaker name tag>|<event tag>|<group>[,<group>...]` (a pawn's component `TriggerEvent`) and
//! `pawn:<body class>|<event tag>` (the component `TriggerEvent` of a pawn with that body class; its dialog groups and
//! the groups searched are printed with the step). An object a step names is an export of the source package (`no
//! such object: <path>` otherwise). A behavior class neither binary handles gives the same error line in both, and the
//! C++ exit code.
//!
//! **Cooked copies.** The C++ reads the mission and its data from the file it is given. The definition store gives each
//! object path one holder (the first file in scan order), so when another file holds the same paths (several mission
//! files in one folder, or a map package's cooked copy) the store refuses to load the given file's copy ("another file
//! holds this path under the identity policy", [`COPY_REFUSAL`]). On that refusal only, wherever it shows (the
//! command's error, or an error line a script run recorded), the whole command runs again on a store whose load set is
//! the given file alone (`IdentityPolicy::Loaded`), as `cmd/behavior.rs` does for a provider. In that store a reference
//! into another package is a stand-in named by its import path (the C++ resolves it).
//!
//! **Gameplay stub policy.** The VM refuses an unbound native (`StubPolicy::Refuse`, architecture 3.8), and the
//! bridge's own C++ stubs answer `NotYet` outside `ParityMode::Reference`. On the installed game the controller script
//! of accept and turn-in reaches `Actor.LocalPlayerControllers` and `MissionTracker.GetObjectivesProgress`, which the C++
//! logs as stubs (decision 2026-11-15-34 has the installed counts). Outside Reference the first is bound since decision
//! 2026-11-16-81 (the world's local players), the second since decision 2026-11-19-05 (the bridge's tracker, from the
//! note NATIVE_SLICE_OFF_STOPS.md); Reference keeps both C++ stubs.
//!
//! Order, as the C++: the source package is loaded first, then the arguments are checked, the store opens the cooked
//! root, the mission is read from the source package by name. Output is written as it is produced, so a step that
//! fails (`usage: ...`, a `std::stoi` text, a load error) leaves the earlier steps' text on stdout, then
//! `ow-package: <message>` on stderr with exit code 1 (a `NotYet`: exit code 3). The VM runs on its own thread
//! (`ow_vm::interp::on_vm_stack`). Lines end in `\n`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_behavior::handlers::Pack as BehaviorPack;
use ow_behavior::provider::BehaviorError;
use ow_core::num::{StrtodPrefix, strtod_prefix};
use ow_core::{DetSet, NotYet, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::{IdentityPolicy, LoadSet};
use ow_mission::bridge::{self, MissionScript};
use ow_mission::dialog::{DialogSystem, Line};
use ow_mission::tracker::{MissionSystem, find_object};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_object::heap::ObjectSpace;
use ow_package::package::Package;
use ow_vm::api::{NativePack, Vm};
use ow_vm::interp::on_vm_stack;

use crate::args::{Command, USAGE};
use crate::cmd::NOT_PORTED;
use crate::cmd::properties::error_text;
use crate::json::quote;

/// The end of the definition store's refusal to load a file's copy of a path another file holds (module
/// documentation, "Cooked copies").
const COPY_REFUSAL: &str = "another file holds this path under the identity policy";


/// Why the command failed: its message, and whether it is a `NotYet` (exit code 3).
#[derive(Debug)]
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

impl From<NotYet> for Failure {
    fn from(n: NotYet) -> Self {
        Failure {
            message: n.to_string(),
            not_yet: true,
        }
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = command;
    let args = args.to_vec();
    let ran = on_vm_stack(move || {
        let mut out = String::new();
        let mut outcome = execute(&args, mode, IdentityPolicy::FirstInScanOrder, &mut out);
        let refused = match &outcome {
            Err(f) => f.message.contains(COPY_REFUSAL),
            Ok(_) => out.contains(COPY_REFUSAL),
        };
        if refused && let Some(name) = source_name(&args) {
            out.clear();
            outcome = execute(&args, mode, IdentityPolicy::Loaded(LoadSet::new(vec![name])), &mut out);
        }
        (outcome, out)
    });
    let (outcome, out) = match ran {
        Ok(pair) => pair,
        Err(e) => (Err(Failure::from(format!("the VM thread failed: {e}"))), String::new()),
    };
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    match outcome {
        Ok(code) => ExitCode::from(code),
        Err(f) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {}", f.message);
            ExitCode::from(if f.not_yet { NOT_PORTED } else { 1 })
        }
    }
}

/// The VM of the command: the store over the cooked root, the standard attribute system, the Core natives and the
/// world-only behavior handlers (the C++ `BehaviorProvider` registers `Behavior_CompareObject` itself). Unbound
/// natives are refused (architecture 3.8: `--mission-run` is a gameplay command, `StubPolicy::Refuse`).
fn open(cooked: &str, mode: ParityMode, policy: IdentityPolicy) -> Result<Vm, Failure> {
    let defs = DefStore::open(PathBuf::from(cooked), policy, mode)
        .map_err(BehaviorError::from)?;
    let attrs = AttributeSystem::standard().map_err(|e| error_text(&e))?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm).map_err(|e| error_text(&e))?;
    BehaviorPack.register(&mut vm).map_err(BehaviorError::from)?;
    // The AI component's event broadcast, Off only (lane FIX-ACTIVATE-EVENT, decision 2026-11-19-42).
    ow_ai::awareness::actions::register_activate_event(&mut vm).map_err(|e| error_text(&e))?;
    Ok(vm)
}

/// The source package's name, when it loads.
fn source_name(args: &[String]) -> Option<PackageName> {
    let source = Package::load(Path::new(args.first()?)).ok()?;
    PackageName::parse(&source.name).ok()
}

/// `args[0]` is the source package, `args[1]` the mode. Returns the exit code.
fn execute(args: &[String], mode: ParityMode, policy: IdentityPolicy, out: &mut String) -> Result<u8, Failure> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    if args.len() < 5 || args[3] != "--cooked" {
        return Err(USAGE.to_string().into());
    }
    let name = PackageName::parse(&source.name).map_err(|e| format!("{}: {e}", source.name))?;
    let vm = open(&args[4], mode, policy)?;
    let steps = &args[5..];
    let wants_script = steps
        .iter()
        .any(|s| s.starts_with("script:") || s.starts_with("stage:") || s.starts_with("player:"));
    let mut world;
    let mut plain;
    let mission;
    let mut script = None;
    // The bridge needs the world's natives (pools, the level-up rebase, EvaluateInitializationData); without it the
    // tracker runs on the bare VM, as the C++ does.
    let vm: &mut Vm = if wants_script {
        world = bridge::world(vm)?;
        mission = MissionSystem::new(&mut world.vm, &name, &args[2])?;
        let s = MissionScript::new(&mut world, &mission)?;
        // The director, when the package has him.
        s.place_marcus(&mut world.vm, &name, bridge::MARCUS_PAWN)?;
        script = Some(s);
        &mut world.vm
    } else {
        plain = vm;
        mission = MissionSystem::new(&mut plain, &name, &args[2])?;
        &mut plain
    };
    let completed: DetSet<String> = mission.dependencies().iter().cloned().collect();
    let _ = write!(
        out,
        "{{\"mission\":{},\"name\":{},\"steps\":[",
        quote(mission.path()),
        quote(mission.name())
    );
    for (i, step) in steps.iter().enumerate() {
        let (ok, screen) = run_step(vm, &mission, script.as_ref(), &completed, &name, step)?;
        let _ = write!(
            out,
            "{}{{\"step\":{},\"ok\":{},\"set\":{},\"status\":{}",
            if i == 0 { "" } else { "," },
            quote(step),
            ok,
            quote(&mission.active_set()),
            mission.status().index()
        );
        if let Some((key, json)) = screen {
            let _ = write!(out, ",\"{key}\":{json}");
        }
        let _ = write!(out, ",\"kickoff_pending\":{},\"effects\":[", mission.kickoff_pending());
        for (j, e) in mission.drain().iter().enumerate() {
            let _ = write!(
                out,
                "{}{{\"t\":{},\"kind\":{},\"a\":{},\"b\":{},\"c\":{},\"detail\":{}}}",
                if j == 0 { "" } else { "," },
                ow_core::fmt::fmt_ostream(e.time),
                quote(e.kind.json_name()),
                quote(&e.a),
                quote(&e.b),
                quote(&e.c),
                quote(&e.detail)
            );
        }
        out.push(']');
        // A script run that failed after a behavior walk wrote state ends the run at this step, and the VM is not
        // used again (review r1 of #439, item 2b; Rust-only: the C++ never meets a NotYet).
        let partial = script.as_ref().and_then(MissionScript::partial_stop);
        if let Some(text) = &partial {
            let _ = write!(out, ",\"stopped\":{}", quote(&format!("stopped after partial writes: {text}")));
        }
        out.push('}');
        if partial.is_some() {
            break;
        }
    }
    let mut errors = mission.errors();
    if let Some(s) = &script {
        errors.extend(s.errors());
    }
    let _ = write!(
        out,
        "],\"status\":{},\"errors\":[{}]",
        mission.status().index(),
        errors.iter().map(|e| quote(e)).collect::<Vec<_>>().join(",")
    );
    if let Some(s) = &script {
        let _ = write!(out, ",\"script\":{}", s.report_json(vm)?);
    }
    out.push_str(",\"tracker\":[");
    for (i, line) in mission.tracker_lines().iter().enumerate() {
        let _ = write!(
            out,
            "{}{{\"objective\":{},\"message\":{},\"complete\":{},\"optional\":{}}}",
            if i == 0 { "" } else { "," },
            quote(&line.name),
            quote(&line.message),
            line.complete,
            line.optional
        );
    }
    out.push_str("]}\n");
    // A script run that stopped on a NotYet: the output is not the C++ output (module documentation). An unhandled
    // behavior class is unhandled in the C++ as well (the same error line, the C++ exit code).
    if script.as_ref().is_some_and(|s| !s.not_yet().is_empty()) {
        return Ok(NOT_PORTED);
    }
    Ok(if errors.is_empty() { 0 } else { 1 })
}

/// A step's success and, for the bridge's list and use steps, the extra key and its JSON.
type StepResult = (bool, Option<(&'static str, String)>);

/// One step: whether it succeeded and, for the bridge's list and use steps, the extra key and its JSON.
fn run_step(
    vm: &mut Vm,
    mission: &MissionSystem,
    script: Option<&MissionScript>,
    completed: &DetSet<String>,
    package: &PackageName,
    step: &str,
) -> Result<StepResult, Failure> {
    let usage = || Failure::from(USAGE.to_string());
    // An object of the source package, by path (the C++ `object`).
    let object_ref = |vm: &mut Vm, path: &str| -> Result<ObjRef, Failure> {
        find_object(vm, package, path)?.ok_or_else(|| format!("no such object: {path}").into())
    };
    let object = |vm: &mut Vm, path: &str| -> Result<Value, Failure> { Ok(Value::Object(Some(object_ref(vm, path)?))) };
    let dialog = mission.dialog();
    let busy = || Failure::from("dialog: the dialog system is already in use".to_string());
    let mut ok = true;
    let mut screen = None;
    if step == "accept" {
        ok = mission.accept(vm, completed)?;
    } else if let (true, Some(s)) = (step == "script:screen", script) {
        screen = Some(("screen", s.mission_lists_json(vm, completed)?));
    } else if let (true, Some(s)) = (step == "script:use", script) {
        let (opened, json) = s.use_marcus_json(vm, completed)?;
        ok = opened;
        screen = Some(("use", json));
    } else if let (true, Some(s)) = (step == "script:accept", script) {
        ok = s.accept(vm, completed)?;
    } else if let (true, Some(s)) = (step == "script:turnin", script) {
        ok = s.turn_in(vm)?;
    } else if let (Some(n), Some(s)) = (step.strip_prefix("stage:"), script) {
        s.set_region_game_stage(stoi(n)?);
    } else if let (Some(rest), Some(s)) = (step.strip_prefix("player:"), script) {
        let (level, xp) = rest.split_once(':').ok_or_else(usage)?;
        s.set_player_experience(vm, stoi(level)?, stoll(xp)?)?;
    } else if let (true, Some(s)) = (step == "pool", script) {
        s.update_experience_pool(vm)?;
    } else if let Some(seconds) = step.strip_prefix("lines:") {
        let seconds = stod(seconds)?;
        dialog.try_borrow_mut().map_err(|_| busy())?.set_test_line_length(seconds);
    } else if let Some(seconds) = step.strip_prefix("timedlines:") {
        let length = stod(seconds)?;
        let mut d = dialog.try_borrow_mut().map_err(|_| busy())?;
        d.set_timed_line_player(move |_: &Line| length);
        d.on_line_stopped = Some(Box::new(|id: i32| {
            let _ = writeln!(std::io::stderr().lock(), "dialog line stopped while playing: {id}");
        }));
    } else if let Some(tag) = step.strip_prefix("talker:") {
        dialog.try_borrow_mut().map_err(|_| busy())?.register_talker(tag, None);
    } else if let Some(rest) = step.strip_prefix("kismet:") {
        let (variable, tag) = rest.split_once('|').ok_or_else(usage)?;
        let mut actor = None;
        if !tag.is_empty() {
            // Any object stands for the pawn; its identity is what matters (the C++).
            actor = Some(object_ref(vm, tag)?);
            dialog.try_borrow_mut().map_err(|_| busy())?.register_talker(tag, actor);
        }
        mission.bind_kismet_variable(variable, actor)?;
    } else if let Some(rest) = step.strip_prefix("component:") {
        let mut parts = rest.splitn(3, '|');
        let (Some(speaker), Some(tag), Some(list)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(usage());
        };
        let mut groups = Vec::new();
        for group in list.split(',') {
            groups.push(object(vm, group)?);
        }
        let tag = object(vm, tag)?;
        let mut d = dialog.try_borrow_mut().map_err(|_| busy())?;
        ok = d.trigger_on_component(vm, speaker, &groups, &tag).map_err(BehaviorError::from)?;
    } else if let Some(rest) = step.strip_prefix("pawn:") {
        let (body, tag) = rest.split_once('|').ok_or_else(usage)?;
        let body = object(vm, body)?;
        let pawn = dialog.try_borrow_mut().map_err(|_| busy())?.pawn_dialog(vm, &body).map_err(BehaviorError::from)?;
        ok = false;
        if !pawn.name_tag.is_empty() {
            let tag = object(vm, tag)?;
            let mut d = dialog.try_borrow_mut().map_err(|_| busy())?;
            ok = d.trigger_on_component(vm, &pawn.name_tag, &pawn.groups, &tag).map_err(BehaviorError::from)?;
        }
        let mut groups = Vec::new();
        for g in &pawn.groups {
            groups.push(quote(&DialogSystem::object_path(vm, g).map_err(BehaviorError::from)?));
        }
        let searched: Vec<String> = dialog.try_borrow().map_err(|_| busy())?.last_search().iter().map(|g| quote(g)).collect();
        screen = Some((
            "pawn",
            format!(
                "{{\"name_tag\":{},\"groups\":[{}],\"searched\":[{}]}}",
                quote(&pawn.name_tag),
                groups.join(","),
                searched.join(",")
            ),
        ));
    } else if step == "turnin" {
        ok = mission.turn_in_mission(vm)?;
    } else if step == "kickoff" {
        ok = mission.kickoff(vm, false)?;
    } else if let Some(rest) = step.strip_prefix("obj:") {
        let (name, bit) = match rest.split_once(':') {
            Some((name, bit)) => (name, stoi(bit)?),
            None => (rest, 0),
        };
        ok = mission.update_objective(vm, name, bit)?;
    } else if let Some(name) = step.strip_prefix("custom:") {
        ok = mission.custom_event(vm, name)?;
    } else if let Some(seconds) = step.strip_prefix("tick:") {
        mission.tick(vm, stod(seconds)?)?;
    } else {
        return Err(usage());
    }
    Ok((ok, screen))
}

/// `std::stoi(text)`: leading C white space, an optional sign and decimal digits; the MSVC exception texts.
fn stoi(text: &str) -> Result<i32, Failure> {
    let digits = cxx_integer(text, "stoi")?;
    digits
        .parse::<i32>()
        .map_err(|_| "stoi argument out of range".to_string().into())
}

/// `std::stoll(text)`, the same rule over 64 bits.
fn stoll(text: &str) -> Result<i64, Failure> {
    let digits = cxx_integer(text, "stoll")?;
    digits
        .parse::<i64>()
        .map_err(|_| "stoll argument out of range".to_string().into())
}

/// The sign and digits `std::stoi` / `std::stoll` read from the start of `text` (trailing text ignored).
fn cxx_integer<'t>(text: &'t str, function: &str) -> Result<&'t str, Failure> {
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
        return Err(format!("invalid {function} argument").into());
    }
    Ok(&text[start..at])
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
        return Err(format!("hexadecimal float text is not read by the Rust binary: {text}").into());
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
    Ok(if lower.starts_with('-') { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stoi_stoll_and_stod_follow_the_msvc_runtime() {
        assert_eq!(stoi(" -1x").ok(), Some(-1));
        assert_eq!(stoi("x").unwrap_err().message, "invalid stoi argument");
        assert_eq!(stoi("99999999999").unwrap_err().message, "stoi argument out of range");
        assert_eq!(stoll("4990").ok(), Some(4990));
        assert_eq!(stoll("-").unwrap_err().message, "invalid stoll argument");
        assert_eq!(stod("0.5").ok(), Some(0.5));
        assert_eq!(stod("abc").unwrap_err().message, "invalid stod argument");
        assert_eq!(stod("1e999").unwrap_err().message, "stod argument out of range");
    }
}
