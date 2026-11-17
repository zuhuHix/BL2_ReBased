//! Owner: lane W6-FIRE. Commands: `--slice-run`, the port of its branch in `src/cli.cpp`. AI-assisted (Claude Code).
//!
//! `<package> --slice-run <mission-path> --cooked <dir> <step>...` runs the stock Fire mission with the target dummy's
//! own provider (`ow_mission::slice::FireMissionSlice`): the mission is read from `Startup`, the dummy's provider,
//! the waypoint and the director from `<package>` (the map package, `Sanctuary_Dynamic`). It prints
//! `{"steps":[{"step","ok","status"[,<key>:<json>],"events":[{"kind","a","b","c","detail"}]}],"status":..,
//! "errors":[..],"script":{..},"dummy_boundary":[..],"dummy_boundary_calls":[..],"dummy_enabled_sequences":[..],
//! "dummy_trace":[..]}`, each step's record carrying the mission status after it. Every dependency of the mission
//! counts as complete (the C++ probe). Exit 0 when no error line was recorded, else 1; 3 when a `NotYet` was met.
//!
//! Steps, as the C++: `accept`, `screen` (the director's lists, printed with the step), `use` (the use key on the
//! director; the behaviors it ran and the interface it opened are printed with the step), `range`,
//! `touch:player|marcus`, `untouch:player|marcus`, `spawn`, `hit:fire`, `hit:other`, `damage:<damage type path>`
//! (`damage:` = None), `pawnhit:<damage>:<type>[:<source>]`, `turnin`, `confirmreward`, `close`, `cheatlevel`,
//! `lines:<s>`, `timedlines:<s>`, `talker:<name tag>`, `stage:<n>`, `player:<level>:<experience>`,
//! `health:<current>`, `tick:<s>`.
//!
//! **What is not here** (decision 2026-11-16-38): the player's hit through the stock pawn chain (`pawnhit:` once the
//! dummy has a pawn) is not wired into the slice yet (follow-up W6-COMBAT-RUN2), so that step stops the command with
//! `NotYet` (exit 3) after the earlier
//! steps' output; before the spawn it raises the event directly and prints the empty hit report, as the C++. The
//! level-up's health pool (`health:`, and the `max_health` / `health` events) is a C++ stand-in reproduced only under
//! `OW_PARITY=reference`; in Off mode `health:` and a level-up of a class that names a health pool are `NotYet`.
//!
//! **Cooked copies.** The C++ reads the mission from `Startup` and the dummy from the given file. The definition store
//! gives each object path one holder (the first file in scan order), so when another file holds the same paths (a
//! copy of the mission package, or a map package's cooked copies) the store refuses to load the copy the command
//! needs ("another file holds this path under the identity policy", [`COPY_REFUSAL`]). On that refusal only, wherever
//! it shows (the command's error, or an error line a run recorded), the whole command runs again on a store whose
//! load set is `Startup`, the given file, then the VM's code packages (`IdentityPolicy::Loaded`). The code
//! packages make class templates visible while the explicit sources keep precedence over cooked copies.
//!
//! **Gameplay stub policy.** `--slice-run` is a gameplay command: its VM refuses an unbound native
//! (`StubPolicy::Refuse`, architecture 3.8), so a native the C++ logs as a stub is a script error line here. Under
//! `OW_PARITY=reference` the natives in [`REFERENCE_STUBS`] are allowed as logged zero-result stubs, as the C++ logs
//! them on the Fire route (the C++ stub list of the installed run, decision 2026-11-16-38); each stays refused in Off
//! mode until its rule is read. `--mission-run` keeps refusing them in both modes (decision 2026-11-15-34).
//!
//! Order, as the C++: the source package is loaded first, then the arguments are checked, the store opens the cooked
//! root and the slice is built. Output is written as it is produced, so a step that fails (`usage: ...`, a
//! `std::stoi` text, a `NotYet`) leaves the earlier steps' text on stdout, then `ow-package: <message>` on stderr with
//! exit code 1 (a `NotYet`: 3). The VM runs on its own thread (`ow_vm::interp::on_vm_stack`). Lines end in `\n`.

use std::cell::RefCell;
use std::collections::BTreeSet;
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
use ow_mission::bridge;
use ow_mission::slice::{DUMMY_PROVIDER, FireMissionSlice, MISSION_PACKAGE};
use ow_object::heap::ObjectSpace;
use ow_package::package::Package;
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::interp::on_vm_stack;

use crate::args::{Command, USAGE};
use crate::cmd::NOT_PORTED;
use crate::cmd::properties::error_text;
use crate::json::quote;

/// The natives the C++ logs as zero-result stubs on the installed Fire route, allowed as such under
/// `OW_PARITY=reference` only (module documentation; approved by the hub, decision 2026-11-16-38). Each is an unread
/// rule, refused in Off mode. The key is the VM's (`Class.Name(types)`; an iterator's own key, whose logged text is the
/// C++ `native iterator ...`; `?.<Name>` for a function name not found), then why it is a stub. This deliberately
/// differs from `--mission-run`, whose VM refuses every one of them in both modes (decision 2026-11-15-34).
/// `Actor.SetTimer` and `Actor.ClearTimer`, which the C++ also stubs there, are not listed: the Rust world binds them.
pub const REFERENCE_STUBS: &[(&str, &str)] = &[
    ("AIComponent.ActivateEvent(name)", "the AI component's event dispatch; read as analyst, not implemented (decision 81)"),
    ("Actor.AttachComponent(object)", "component attachment; the VM has no scene or component model"),
    ("Actor.LocalPlayerControllers(class,object)", "the local-controller iterator; the C++ has none (Off binds it, decision 81)"),
    ("GFxMoviePlayer.GetVariableObject(string,class)", "a Flash movie query; the mission screen is not hosted"),
    ("MissionTracker.AllExpansionSideMissionsComplete(object,object)", "a tracker query whose rule is not read"),
    ("MissionTracker.GetActivePrimaryObjectiveSet(object)", "a tracker query whose rule is not read"),
    ("MissionTracker.GetAllMissions(array)", "the tracker's mission list; one mission is modelled, the rule is not read"),
    ("MissionTracker.GetObjectivesProgress(object,array,bool)", "a tracker query; read as analyst, not implemented (decision 81)"),
    ("MissionTracker.RegisterWaypoint(object,object)", "the HUD waypoint registry; presentation, not read"),
    ("Object.Localize(string,string,string)", "localized text lookup; no localization files are read"),
    ("PlayerController.IsLocalPlayerController()", "a controller query whose rule is not read"),
    ("QuestAcceptGFxMovie.UpdateMissionTextList()", "the mission screen's text list; the screen is not hosted"),
    ("Trigger.TriggerDetachSprites()", "editor sprites of a trigger; presentation"),
    ("WillowGFxMovie3D.FocusOn(object)", "the 3D movie's camera focus; presentation"),
    (
        "WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(object,byte,name)",
        "the telemetry service; no effect on play",
    ),
    (
        "WillowPlayerController.RefreshBalanceDataFromMissionCompletion(object)",
        "the balance refresh after a completion; its rule is not read",
    ),
    (
        "WillowPlayerController.UnlockAchievementIfConditionsMet(byte,string,int,bool,bool,bool)",
        "platform achievements; no effect on play",
    ),
    ("?.SpawnPlayerMovie", "a movie spawn the script reaches by name; the movie is not hosted"),
    (
        "GFxMovieDefinition.SpawnPlayerMovie(object,interface,object)",
        "the same movie spawn where the store resolves the movie definition the C++ holds as a classless stand-in \
         (the turn-in's reward page; decision 2026-11-16-48); the movie is not hosted",
    ),
];

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
        let (mut out, mut err) = (String::new(), String::new());
        let mut outcome = execute(&args, mode, IdentityPolicy::FirstInScanOrder, &mut out, &mut err);
        let refused = match &outcome {
            Err(f) => f.message.contains(COPY_REFUSAL),
            Ok(_) => out.contains(COPY_REFUSAL),
        };
        if refused && let Some(set) = load_set(&args) {
            // The rerun's output replaces the first run's, stderr lines included (they would print twice).
            out.clear();
            err.clear();
            outcome = execute(&args, mode, IdentityPolicy::Loaded(set), &mut out, &mut err);
        }
        (outcome, out, err)
    });
    let (outcome, out, err) = match ran {
        Ok(three) => three,
        Err(e) => (Err(Failure::from(format!("the VM thread failed: {e}"))), String::new(), String::new()),
    };
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    let _ = std::io::stderr().lock().write_all(err.as_bytes());
    match outcome {
        Ok(code) => ExitCode::from(code),
        Err(f) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {}", f.message);
            ExitCode::from(if f.not_yet { NOT_PORTED } else { 1 })
        }
    }
}

/// The explicit slice sources, then the existing VM code-package dependency set, each once. This is caller
/// setup only; the real game's load order remains UNVERIFIED (decision 2026-11-17-03).
fn load_set(args: &[String]) -> Option<LoadSet> {
    let source = Package::load(Path::new(args.first()?)).ok()?;
    let source = PackageName::parse(&source.name).ok()?;
    let startup = PackageName::parse(MISSION_PACKAGE).ok()?;
    let mut packages = vec![startup];
    if !packages.contains(&source) {
        packages.push(source);
    }
    for name in ow_object::reflect::CODE_PACKAGES {
        let name = PackageName::parse(name).ok()?;
        if !packages.contains(&name) {
            packages.push(name);
        }
    }
    Some(LoadSet::new(packages))
}

/// The stub policy of the command's VM (module documentation, "Gameplay stub policy").
fn stub_policy(mode: ParityMode) -> StubPolicy {
    if mode == ParityMode::Reference {
        StubPolicy::Refuse {
            allow: REFERENCE_STUBS.iter().map(|(k, _)| k.to_string()).collect::<BTreeSet<_>>(),
        }
    } else {
        StubPolicy::refuse_all()
    }
}

/// The VM of the command: the store over the cooked root, the standard attribute system, the Core natives and the
/// world-only behavior handlers, as `--mission-run` builds it.
fn open(cooked: &str, mode: ParityMode, policy: IdentityPolicy) -> Result<Vm, Failure> {
    let defs = DefStore::open(PathBuf::from(cooked), policy, mode).map_err(BehaviorError::from)?;
    let attrs = AttributeSystem::standard().map_err(|e| error_text(&e))?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(stub_policy(mode));
    ow_vm::natives_core::register(&mut vm).map_err(|e| error_text(&e))?;
    BehaviorPack.register(&mut vm).map_err(BehaviorError::from)?;
    Ok(vm)
}

/// `args[0]` is the source package, `args[1]` the mode. Returns the exit code. The stderr lines the run writes before
/// its end (`timedlines:`'s stopped lines) go to `err`, which the caller prints after stdout, as `out` is buffered.
fn execute(
    args: &[String],
    mode: ParityMode,
    policy: IdentityPolicy,
    out: &mut String,
    err: &mut String,
) -> Result<u8, Failure> {
    let stopped = Rc::new(RefCell::new(String::new()));
    let result = execute_with(args, mode, policy, out, &stopped);
    err.push_str(&stopped.borrow());
    result
}

fn execute_with(
    args: &[String],
    mode: ParityMode,
    policy: IdentityPolicy,
    out: &mut String,
    stopped: &Rc<RefCell<String>>,
) -> Result<u8, Failure> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    if args.len() < 5 || args[3] != "--cooked" {
        return Err(USAGE.to_string().into());
    }
    let name = PackageName::parse(&source.name).map_err(|e| format!("{}: {e}", source.name))?;
    let vm = open(&args[4], mode, policy)?;
    let mut world = bridge::world(vm)?;
    let slice = FireMissionSlice::new(&mut world, &args[2], &name, DUMMY_PROVIDER)?;
    let vm = &mut world.vm;
    let completed: DetSet<String> = slice.mission().dependencies().iter().cloned().collect();
    out.push_str("{\"steps\":[");
    for (i, step) in args[5..].iter().enumerate() {
        let (ok, extra) = run_step(vm, &slice, &completed, step, stopped)?;
        let _ = write!(
            out,
            "{}{{\"step\":{},\"ok\":{},\"status\":{}",
            if i == 0 { "" } else { "," },
            quote(step),
            ok,
            slice.mission().status().index()
        );
        if let Some((key, json)) = extra {
            let _ = write!(out, ",\"{key}\":{json}");
        }
        out.push_str(",\"events\":[");
        for (j, e) in slice.drain().iter().enumerate() {
            let _ = write!(
                out,
                "{}{{\"kind\":{},\"a\":{},\"b\":{},\"c\":{},\"detail\":{}}}",
                if j == 0 { "" } else { "," },
                quote(e.kind.json_name()),
                quote(&e.a),
                quote(&e.b),
                quote(&e.c),
                quote(&e.detail)
            );
        }
        out.push_str("]}");
    }
    let errors = slice.errors();
    let list = |lines: &[String]| lines.iter().map(|l| quote(l)).collect::<Vec<_>>().join(",");
    let _ = write!(
        out,
        "],\"status\":{},\"errors\":[{}],\"script\":{}",
        slice.mission().status().index(),
        list(&errors),
        slice.script().report_json(vm)?
    );
    let _ = write!(out, ",\"dummy_boundary\":[{}]", list(&slice.dummy_boundary()?));
    out.push_str(",\"dummy_boundary_calls\":[");
    for (i, call) in slice.dummy_boundary_calls()?.iter().enumerate() {
        let fields = call
            .fields
            .iter()
            .map(|(k, v)| format!("{}:{}", quote(k), quote(v)))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(
            out,
            "{}{{\"event\":{},\"class\":{},\"name\":{},\"sequence\":{},\"fields\":{{{}}}}}",
            if i == 0 { "" } else { "," },
            quote(&call.event),
            quote(&call.class),
            quote(&call.name),
            quote(&call.sequence),
            fields
        );
    }
    let _ = write!(
        out,
        "],\"dummy_enabled_sequences\":[{}],\"dummy_trace\":[{}]}}\n",
        list(&slice.dummy_enabled_sequences()?),
        list(&slice.dummy_trace()?)
    );
    // A NotYet anywhere (the slice's own, a script run's, an unhandled behavior class's): the output is not the C++
    // output.
    if !slice.not_yet().is_empty() {
        return Ok(NOT_PORTED);
    }
    Ok(if errors.is_empty() { 0 } else { 1 })
}

/// A step's success and, for `screen`, `use` and `pawnhit:`, the extra key and its JSON.
type StepResult = (bool, Option<(&'static str, String)>);

/// The JSON of the director's lists (the C++ `missionListsJson`).
fn lists_json(lists: &bridge::MissionLists) -> String {
    let one = |paths: &[String]| paths.iter().map(|p| quote(p)).collect::<Vec<_>>().join(",");
    format!(
        "{{\"eligible\":[{}],\"in_progress\":[{}],\"redeemable\":[{}]}}",
        one(&lists.eligible),
        one(&lists.in_progress),
        one(&lists.redeemable)
    )
}

/// The JSON of the use key's result (the C++ `useJson`).
fn use_json(u: &bridge::MarcusUse) -> String {
    let cascade = u.cascade.iter().map(|l| quote(l)).collect::<Vec<_>>().join(",");
    format!(
        "{{\"interface_opened\":{},\"movie\":{},\"director\":{},\"on_use_tag\":{},\"on_use_speaker\":{},\
         \"on_use_target\":{},\"on_use_groups\":{{\"count\":{},\"first\":{},\"last\":{}}},\"cascade\":[{}]}}",
        u.interface_opened,
        quote(&u.movie),
        quote(&u.director),
        quote(&u.on_use_tag),
        quote(&u.on_use_speaker),
        quote(&u.on_use_target),
        u.on_use_group_count,
        quote(&u.on_use_first_group),
        quote(&u.on_use_last_group),
        cascade
    )
}

/// The hit report of a `pawnhit:` step whose hit raised the event directly (the C++ `lastHit_` as constructed: no
/// pawn hit ran; the pawn chain itself is `NotYet`, module documentation).
const EMPTY_HIT: &str = "{\"health_before\":0,\"health_after\":0,\"arriving\":0,\"applied\":0,\
                         \"demigod_before\":false,\"demigod_after\":false,\"events\":[]}";

/// One step (module documentation).
fn run_step(
    vm: &mut Vm,
    slice: &FireMissionSlice,
    completed: &DetSet<String>,
    step: &str,
    stopped: &Rc<RefCell<String>>,
) -> Result<StepResult, Failure> {
    let usage = || Failure::from(USAGE.to_string());
    let mut ok = true;
    let mut extra = None;
    if step == "accept" {
        ok = slice.accept(vm, completed)?;
    } else if step == "screen" {
        extra = Some(("screen", lists_json(&slice.screen(vm, completed)?)));
    } else if step == "use" {
        let u = slice.use_marcus(vm, completed)?;
        ok = u.interface_opened;
        extra = Some(("use", use_json(&u)));
    } else if step == "range" {
        ok = slice.enter_range(vm)?;
    } else if step == "touch:player" || step == "touch:marcus" {
        slice.touch_waypoint(vm, step == "touch:player", true)?;
    } else if step == "untouch:player" || step == "untouch:marcus" {
        slice.touch_waypoint(vm, step == "untouch:player", false)?;
    } else if step == "spawn" {
        ok = slice.spawn_dummy(vm)?;
    } else if step == "hit:fire" {
        ok = slice.hit_dummy(vm, true)?;
    } else if step == "hit:other" {
        ok = slice.hit_dummy(vm, false)?;
    } else if let Some(path) = step.strip_prefix("damage:") {
        ok = slice.damage_dummy(vm, path, "", -1.0)?;
    } else if let Some(rest) = step.strip_prefix("pawnhit:") {
        // pawnhit:<damage>:<damage type definition path>[:<WillowDmgSource_* class path>]
        let (damage, rest) = rest.split_once(':').ok_or_else(usage)?;
        let (path, source) = rest.split_once(':').unwrap_or((rest, ""));
        let damage = stod(damage)?;
        ok = slice.damage_dummy(vm, path, source, damage)?;
        extra = Some(("hit", EMPTY_HIT.to_string()));
    } else if step == "turnin" {
        ok = slice.turn_in(vm)?;
    } else if step == "confirmreward" {
        ok = slice.confirm_reward(vm)?;
    } else if step == "close" {
        ok = slice.close_screen(vm)?;
    } else if step == "cheatlevel" {
        slice.cheat_level_up(vm)?;
    } else if let Some(seconds) = step.strip_prefix("lines:") {
        let length = stod(seconds)?;
        with_dialog(slice, |d| d.set_test_line_length(length))?;
    } else if let Some(seconds) = step.strip_prefix("timedlines:") {
        let length = stod(seconds)?;
        let sink = stopped.clone();
        with_dialog(slice, |d| {
            d.set_timed_line_player(move |_| length);
            d.on_line_stopped = Some(Box::new(move |line| {
                let _ = writeln!(sink.borrow_mut(), "dialog line stopped while playing: {line}");
            }));
        })?;
    } else if let Some(tag) = step.strip_prefix("talker:") {
        with_dialog(slice, |d| d.register_talker(tag, None))?;
    } else if let Some(n) = step.strip_prefix("stage:") {
        slice.set_region_game_stage(stoi(n)?);
    } else if let Some(rest) = step.strip_prefix("player:") {
        let (level, xp) = rest.split_once(':').ok_or_else(usage)?;
        slice.set_player_experience(vm, stoi(level)?, stoll(xp)?)?;
    } else if let Some(current) = step.strip_prefix("health:") {
        slice.set_player_health(vm, stof(current)?)?;
    } else if let Some(seconds) = step.strip_prefix("tick:") {
        slice.tick(vm, stod(seconds)?)?;
    } else {
        return Err(usage());
    }
    Ok((ok, extra))
}

/// Runs `f` on the mission's dialog (`MissionSystem::with_dialog`).
fn with_dialog(slice: &FireMissionSlice, f: impl FnOnce(&mut ow_mission::dialog::DialogSystem)) -> Result<(), Failure> {
    Ok(slice.mission().with_dialog(f)?)
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
    strto(text, "stod")
}

/// `std::stof(text)`: the same text rules; a finite value outside the float range is the MSVC `out_of_range`.
fn stof(text: &str) -> Result<f32, Failure> {
    let v = strto(text, "stof")?;
    if v.is_finite() && v.abs() > f64::from(f32::MAX) {
        return Err("stof argument out of range".to_string().into());
    }
    // In range (checked above), so the narrowing rounds to the nearest float as strtof does.
    Ok(v as f32)
}

fn strto(text: &str, function: &str) -> Result<f64, Failure> {
    let Some(StrtodPrefix {
        start,
        end,
        hexadecimal,
        ..
    }) = strtod_prefix(text)
    else {
        return Err(format!("invalid {function} argument").into());
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
            .map_err(|_| format!("invalid {function} argument"))?;
        if v.is_infinite() {
            return Err(format!("{function} argument out of range").into());
        }
        v
    };
    Ok(if lower.starts_with('-') { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stoi_stoll_stod_and_stof_follow_the_msvc_runtime() {
        assert_eq!(stoi(" -1x").ok(), Some(-1));
        assert_eq!(stoi("x").unwrap_err().message, "invalid stoi argument");
        assert_eq!(stoll("21000").ok(), Some(21000));
        assert_eq!(stod("0.02").ok(), Some(0.02));
        assert_eq!(stod("abc").unwrap_err().message, "invalid stod argument");
        assert_eq!(stof("20").ok(), Some(20.0));
        assert_eq!(stof("1e39").unwrap_err().message, "stof argument out of range");
        assert_eq!(stof("q").unwrap_err().message, "invalid stof argument");
    }

    #[test]
    fn the_reference_allow_list_is_reference_only() {
        // Off mode refuses every stub site, these keys included.
        assert_eq!(stub_policy(ParityMode::Off), StubPolicy::refuse_all());
        for (key, reason) in REFERENCE_STUBS {
            assert!(!reason.is_empty(), "{key} carries its reason");
            assert!(!reason.contains("  "), "{key}: its reason has no run of spaces");
            assert!(stub_policy(ParityMode::Reference).allows_stub(key));
            assert!(!stub_policy(ParityMode::Off).allows_stub(key));
        }
        // Only the listed keys: any other unbound native stays refused in Reference mode too.
        assert!(!stub_policy(ParityMode::Reference).allows_stub("Actor.SetTimer(float,bool,name,object)"));
        // The turn-in's movie spawn on the resolved movie definition (decision 2026-11-16-48): Reference only.
        let spawn = "GFxMovieDefinition.SpawnPlayerMovie(object,interface,object)";
        assert!(stub_policy(ParityMode::Reference).allows_stub(spawn));
        assert!(!stub_policy(ParityMode::Off).allows_stub(spawn));
        let keys: BTreeSet<&str> = REFERENCE_STUBS.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys.len(), REFERENCE_STUBS.len(), "no key twice");
    }

    #[test]
    fn the_empty_hit_report_is_the_cxx_default() {
        assert!(EMPTY_HIT.starts_with("{\"health_before\":0,"));
        assert!(!EMPTY_HIT.contains(' '));
    }
}

#[cfg(test)]
#[path = "slice_tests.rs"]
mod loadset_tests;
