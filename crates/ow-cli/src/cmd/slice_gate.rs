//! Owner: lane SLICE-RUN-OFF. Command: `--slice-gate --cooked <dir>`, the headless proof of the 14 slice-gate links
//! (docs/verification/SLICE_GATE.md) in `ParityMode::Off`. AI-assisted (Claude Code). Decision `2026-11-19-08`.
//!
//! It adds no gameplay rule and binds no native: each link is driven by a command that already exists, with a fixed
//! step list, and is graded from what that driver reports. It prints one JSON row per link, in gate order:
//!
//! `{"n":<1..14>,"link":<name>,"grading":"strict"|"past-errors","result":"ok"|"notyet"|"error",
//! "driver":"combat-run"|"slice-run"|"none","steps":[..],"stop":null|{"step","native","record","text"},
//! "blocked_by":null|<link>,"after_errors_in":null|<link>,"error_lines":[{"step","text"}..],"scope":<text>}`
//!
//! `tools/check_slice_run_off.py` grades the rows; the gate is 14 ok, 0 notyet, 0 error.
//!
//! **Drivers.**
//! - `combat-run` (`ow_combat::run`, over the mission bridge's player as `--combat-run` builds it): links 1, 3, 4, 5,
//!   13 and 14, in that order on one world. A step `spawn:<kind>:...` binds `{<kind>}` in later steps to the new pawn's
//!   index.
//! - `slice-run` (`ow_mission::slice::FireMissionSlice`, as `--slice-run` builds it, with the step list of
//!   `tools/verify_fire_event_order.py`): links 10 to 12, on a second world. On the definition store's copy refusal
//!   the group runs again on the explicit load set, as `--slice-run` does.
//! - `none`: links 2, 6, 7, 8 and 9 have no headless driver yet; each row is `notyet` with the reason.
//!
//! The two driver worlds are separate: the gate's one session is not modelled yet (the slice bridge refuses a world
//! that already has actors, and the map world is not brought up for play, decision 2026-11-19-04).
//!
//! **Grading.** A link runs its steps in order and stops at the first step that meets a `NotYet` (`notyet`), records
//! an error line, fails a step, or returns an error (`error`). With no stop, the link is `ok` only when its evidence is
//! there: a row of the combat report (`kill.`, `ratio.`, `loot.roll.`, `down.total.`, `down.respawn_health.`), or the
//! Fire mission's status after the link (1 Active, 2 ReadyToTurnIn, 3 Complete); else `error`, naming the missing
//! evidence. A link whose prerequisite link is not `ok` does not run: it takes that link's result and stop, with
//! `blocked_by`. A driver that cannot be set up stops each of its links at step `setup`.
//!
//! **`--past-errors`** (informational, never the gate). An error line (a script error the driver records and goes on
//! past, as `--slice-run` does) does not stop the link: it goes to `error_lines` and makes the link `error`, and the
//! steps go on. Only a hard stop (a `NotYet`, a step that returns an error or answers not ok) stops a link and blocks
//! the links that need it; a link that runs after a prerequisite with error lines names it in `after_errors_in`. So
//! each row shows how far its own steps get. The default grading is `strict`.
//!
//! The command runs in Off only (the slice gate counts only there, SLICE_RUN_PLAN.md section 1): under
//! `OW_PARITY=reference` it refuses. Exit 0 when all 14 links are ok, 3 when none is `error` but some is `notyet`, 1
//! otherwise.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use ow_combat::run::{PlayerBridge, Run};
use ow_core::{DetSet, Missing, NotYet, PackageName, ParityMode};
use ow_defs::identity::IdentityPolicy;
use ow_mission::bridge;
use ow_mission::slice::{DUMMY_PROVIDER, FireMissionSlice};
use ow_vm::interp::on_vm_stack;

use crate::args::Command;
use crate::cmd::NOT_PORTED;
use crate::cmd::{combat, slice};
use crate::json::quote;

/// The Fire mission (`tools/verify_fire_event_order.py`'s `MISSION`).
pub const FIRE_MISSION: &str = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire";
/// The Fire slice's source package, in the cooked folder.
pub const FIRE_SOURCE: &str = "Sanctuary_Dynamic.upk";

const USAGE: &str = "usage: ow-package --slice-gate --cooked <directory> [--past-errors]";

/// What drives a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Combat,
    Fire,
    /// No headless driver yet; the text says why.
    None(&'static str),
}

impl Driver {
    fn name(self) -> &'static str {
        match self {
            Driver::Combat => "combat-run",
            Driver::Fire => "slice-run",
            Driver::None(_) => "none",
        }
    }
}

/// What makes a link `ok` once its steps ran without a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// A row whose key starts with this text in the combat report.
    Row(&'static str),
    /// The Fire mission's status index after the link.
    Status(u8),
    /// The steps alone.
    Steps,
}

/// One link of the gate.
#[derive(Debug)]
pub struct Link {
    pub name: &'static str,
    pub driver: Driver,
    pub steps: &'static [&'static str],
    /// The links this one needs `ok` first.
    pub needs: &'static [&'static str],
    pub evidence: Evidence,
    /// What the driver covers of the link, in a few words.
    pub scope: &'static str,
}

const fn link(
    name: &'static str,
    driver: Driver,
    steps: &'static [&'static str],
    needs: &'static [&'static str],
    evidence: Evidence,
    scope: &'static str,
) -> Link {
    Link {
        name,
        driver,
        steps,
        needs,
        evidence,
        scope,
    }
}

/// The 14 links, in SLICE_GATE.md's order (the names of `tools/check_slice_gate.py`'s `LINKS`).
pub const LINKS: [Link; 14] = [
    link(
        "spawn_maya",
        Driver::Combat,
        &["maya:8", "players:1"],
        &[],
        Evidence::Steps,
        "the mission bridge's player from class defaults; no map spawn (decision 2026-11-16-54)",
    ),
    link(
        "reach_fight_area",
        Driver::None(
            "no headless driver: movement and the fight area are host-side (decision 2026-11-16-57), and the map \
             world is not brought up for play (decision 2026-11-19-04)",
        ),
        &[],
        &[],
        Evidence::Steps,
        "",
    ),
    link(
        "kill_enemy",
        Driver::Combat,
        &["spawn:baby:6", "hit:{baby}:9999::Bullet"],
        &["spawn_maya"],
        Evidence::Row("kill."),
        "a scripted lethal hit, not a shot of the held gun (SLICE_RUN_PLAN.md section 3.3)",
    ),
    link(
        "enemy_attacks_maya",
        Driver::Combat,
        &["spawn:adult:7", "swing:{adult}:melee"],
        &["spawn_maya"],
        Evidence::Row("ratio."),
        "a scripted melee swing at the bridge's player, not the enemy's AI",
    ),
    link(
        "loot_drop_on_kill",
        Driver::Combat,
        &[],
        &["kill_enemy"],
        Evidence::Row("loot.roll."),
        "the death roll of link 3's kill",
    ),
    link(
        "pick_up_gun",
        Driver::None("no headless driver: pickup is host-side (decision 2026-11-16-41)"),
        &[],
        &[],
        Evidence::Steps,
        "",
    ),
    link(
        "pick_up_aimed_not_nearest",
        Driver::None("no headless driver: pickup is host-side (decision 2026-11-16-41)"),
        &[],
        &[],
        Evidence::Steps,
        "",
    ),
    link(
        "equip_gun",
        Driver::None(
            "no headless driver: equip is library code with unit tests, and no run step joins a looted gun to a \
             fire (decisions 2026-11-15-33 and 2026-11-16-61)",
        ),
        &[],
        &[],
        Evidence::Steps,
        "",
    ),
    link(
        "phaselock_enemy",
        Driver::None(
            "no headless driver: the Phaselock cast has no run step (SLICE_RUN_PLAN.md section 3.5; decision \
             2026-11-16-42)",
        ),
        &[],
        &[],
        Evidence::Steps,
        "",
    ),
    link(
        "mission_accept",
        Driver::Fire,
        &[
            "stage:8",
            "player:8:21000",
            "use",
            "accept",
            "tick:0.02",
            "tick:0.1",
            "tick:0.5",
            "tick:1",
        ],
        &[],
        Evidence::Status(1),
        "the Fire mission on the slice bridge's world (tools/verify_fire_event_order.py steps)",
    ),
    link(
        "mission_objective",
        Driver::Fire,
        &[
            "range",
            "tick:1",
            "spawn",
            "tick:1",
            "hit:fire",
            "tick:0.02",
            "tick:2.5",
            "tick:1",
        ],
        &["mission_accept"],
        Evidence::Status(2),
        "the range, the dummy's spawn and the fire hit",
    ),
    link(
        "mission_turn_in",
        Driver::Fire,
        &["use", "turnin", "tick:0.1", "tick:1", "confirmreward", "close"],
        &["mission_objective"],
        Evidence::Status(3),
        "the turn-in, the reward page and the screen's close",
    ),
    link(
        "go_down",
        Driver::Combat,
        &[
            "cash:3000",
            "swing:{adult}:melee",
            "swing:{adult}:melee",
            "swing:{adult}:melee",
        ],
        &["enemy_attacks_maya"],
        Evidence::Row("down.total."),
        "three more scripted swings of link 4's adult (S-f, decision 2026-11-16-54)",
    ),
    link(
        "bleed_out_respawn",
        Driver::Combat,
        &["bleed:50"],
        &["go_down"],
        Evidence::Row("down.respawn_health."),
        "the bleed-out and the respawn (S-f)",
    ),
];

/// How error lines are graded (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grading {
    Strict,
    PastErrors,
}

impl Grading {
    fn name(self) -> &'static str {
        match self {
            Grading::Strict => "strict",
            Grading::PastErrors => "past-errors",
        }
    }
}

/// A link's grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    NotYet,
    Error,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::NotYet => "notyet",
            Outcome::Error => "error",
        }
    }
}

/// Where a link stopped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stop {
    pub step: String,
    /// The `NotYet`'s subject, or the unbound native's key; empty when the text names neither.
    pub native: String,
    /// The `NotYet`'s record; empty for an error.
    pub record: String,
    pub text: String,
}

/// One row of the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub n: usize,
    pub link: &'static str,
    pub result: Outcome,
    pub driver: &'static str,
    pub steps: Vec<String>,
    pub stop: Option<Stop>,
    pub blocked_by: Option<&'static str>,
    pub grading: Grading,
    pub after_errors_in: Option<&'static str>,
    /// The error lines of the link's steps, `(step, text)` (past-errors grading; the first is the stop).
    pub error_lines: Vec<(String, String)>,
    /// The link stopped (a hard stop, or under strict grading any stop) or was blocked: it blocks what needs it.
    pub stopped: bool,
    pub scope: &'static str,
}

impl Row {
    pub fn json(&self) -> String {
        let steps = self.steps.iter().map(|s| quote(s)).collect::<Vec<_>>().join(",");
        let stop = match &self.stop {
            None => "null".to_string(),
            Some(s) => format!(
                "{{\"step\":{},\"native\":{},\"record\":{},\"text\":{}}}",
                quote(&s.step),
                quote(&s.native),
                quote(&s.record),
                quote(&s.text)
            ),
        };
        let blocked = self.blocked_by.map_or_else(|| "null".to_string(), quote);
        let after = self.after_errors_in.map_or_else(|| "null".to_string(), quote);
        let lines = self
            .error_lines
            .iter()
            .map(|(step, text)| format!("{{\"step\":{},\"text\":{}}}", quote(step), quote(text)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"n\":{},\"link\":{},\"grading\":{},\"result\":{},\"driver\":{},\"steps\":[{}],\"stop\":{},\
             \"blocked_by\":{},\"after_errors_in\":{},\"error_lines\":[{}],\"scope\":{}}}",
            self.n,
            quote(self.link),
            quote(self.grading.name()),
            quote(self.result.name()),
            quote(self.driver),
            steps,
            stop,
            blocked,
            after,
            lines,
            quote(self.scope)
        )
    }
}

/// The phrases `NotYet`'s display gives each kind of `Missing` (`ow_core::notyet`), without their variable parts.
fn missing_phrases() -> [String; 4] {
    [
        Missing::NotRead.to_string(),
        Missing::NotPorted.to_string(),
        "which is not built yet".to_string(),
        "blocked on real-game question".to_string(),
    ]
}

/// Grades a stop text a driver recorded at `step`: a `NotYet` when the text carries a `NotYet`'s display (its subject
/// before the phrase, its record after "(see "), else an error (the key of an unbound native when it names one).
pub fn classify(step: &str, text: &str) -> (Outcome, Stop) {
    let mut stop = Stop {
        step: step.to_string(),
        text: text.to_string(),
        ..Stop::default()
    };
    for phrase in missing_phrases() {
        let Some(at) = text.find(&phrase) else {
            continue;
        };
        let head = &text[..at];
        // "<what>: needs <layer>, which is not built yet": the subject ends before ": needs ".
        let head = match head.rfind(": needs ") {
            Some(i) if phrase.starts_with("which") => &head[..i],
            _ => head,
        };
        stop.native = head.trim_end().trim_end_matches(':').to_string();
        if let Some(see) = text[at..].find("(see ") {
            let rest = &text[at + see + 5..];
            stop.record = rest[..rest.rfind(')').unwrap_or(rest.len())].to_string();
        }
        return (Outcome::NotYet, stop);
    }
    if let Some(at) = text.find("unbound native ") {
        stop.native = text[at + 15..]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
    }
    (Outcome::Error, stop)
}

/// A typed `NotYet` as a stop.
fn stop_of_not_yet(step: &str, n: &NotYet) -> Stop {
    Stop {
        step: step.to_string(),
        native: n.what.to_string(),
        record: n.record.to_string(),
        text: n.to_string(),
    }
}

/// What a driver did with one link: the steps it ran and where it stopped, if it did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ran {
    pub steps: Vec<String>,
    /// The hard stop (under strict grading, also the first error line).
    pub stop: Option<(Outcome, Stop)>,
    /// Error lines the steps went on past (past-errors grading only), `(step, text)`.
    pub error_lines: Vec<(String, String)>,
}

/// The rows of the 14 links, from what the drivers did (`ran[i]` for link `i`, `None` for a link not run) and the
/// evidence check (`evident(i)`). Links with no driver, or blocked, are graded here.
pub fn rows(ran: &[Option<Ran>], grading: Grading, evident: impl Fn(usize) -> bool) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::with_capacity(LINKS.len());
    for (i, l) in LINKS.iter().enumerate() {
        let mut row = Row {
            n: i + 1,
            link: l.name,
            result: Outcome::Ok,
            driver: l.driver.name(),
            steps: Vec::new(),
            stop: None,
            blocked_by: None,
            grading,
            after_errors_in: None,
            error_lines: Vec::new(),
            stopped: false,
            scope: l.scope,
        };
        if let Driver::None(why) = l.driver {
            row.result = Outcome::NotYet;
            row.stop = Some(Stop {
                step: String::new(),
                native: String::new(),
                record: "SLICE_RUN_PLAN.md".into(),
                text: why.into(),
            });
            row.stopped = true;
            out.push(row);
            continue;
        }
        let blocker = l.needs.iter().find_map(|need| {
            out.iter()
                .find(|r| r.link == *need && r.result != Outcome::Ok && r.stopped)
                .map(|r| (r.link, r.result, r.stop.clone()))
        });
        if let Some((name, result, stop)) = blocker {
            row.result = result;
            row.stop = stop;
            row.blocked_by = Some(name);
            row.stopped = true;
            out.push(row);
            continue;
        }
        row.after_errors_in = l.needs.iter().find_map(|need| {
            out.iter()
                .find(|r| r.link == *need && r.result != Outcome::Ok)
                .map(|r| r.link)
        });
        match ran.get(i).cloned().flatten() {
            None => {
                row.result = Outcome::Error;
                row.stopped = true;
                row.stop = Some(Stop {
                    text: "the driver did not run this link".into(),
                    ..Stop::default()
                });
            }
            Some(r) => {
                row.steps = r.steps;
                row.error_lines = r.error_lines;
                if let Some((result, stop)) = r.stop {
                    row.result = result;
                    row.stop = Some(stop);
                    row.stopped = true;
                } else if let Some((step, text)) = row.error_lines.first() {
                    let (result, stop) = classify(step, text);
                    row.result = result;
                    row.stop = Some(stop);
                    row.stopped = grading == Grading::Strict;
                } else if !evident(i) {
                    row.result = Outcome::Error;
                    row.stopped = true;
                    row.stop = Some(Stop {
                        step: row.steps.last().cloned().unwrap_or_default(),
                        text: format!("no stop, but no evidence: {}", evidence_text(l.evidence)),
                        ..Stop::default()
                    });
                }
            }
        }
        out.push(row);
    }
    out
}

fn evidence_text(e: Evidence) -> String {
    match e {
        Evidence::Row(prefix) => format!("the combat report has no {prefix}* row"),
        Evidence::Status(s) => format!("the Fire mission's status is not {s}"),
        Evidence::Steps => "none needed".into(),
    }
}

/// Whether link `i` runs: its driver is `driver` and each prerequisite already ran without a stop.
fn should_run(i: usize, driver: Driver, ran: &[Option<Ran>]) -> bool {
    let l = &LINKS[i];
    l.driver == driver
        && l.needs.iter().all(|need| {
            LINKS
                .iter()
                .position(|x| x.name == *need)
                .and_then(|j| ran[j].as_ref())
                .is_some_and(|r| r.stop.is_none())
        })
}

/// Replaces each `{kind}` of `step` with its bound pawn index.
fn bind(step: &str, bound: &[(String, usize)]) -> Result<String, String> {
    let mut out = step.to_string();
    for (kind, index) in bound {
        out = out.replace(&format!("{{{kind}}}"), &index.to_string());
    }
    if out.contains('{') {
        return Err(format!("{step}: no pawn of that kind was spawned"));
    }
    Ok(out)
}

/// The combat group (module documentation): what each of its links did, and the report's row keys.
fn run_combat(cooked: &Path, grading: Grading, ran: &mut [Option<Ran>]) -> String {
    let setup = || -> Result<Run, ow_vm::error::VmError> {
        let mut world = combat::world(cooked, ParityMode::Off)?;
        let bridge: Rc<dyn PlayerBridge> = Rc::new(combat::MissionBridge::new(&mut world)?);
        Run::with_bridge(world, Some(bridge))
    };
    let mut run = match setup() {
        Ok(r) => r,
        Err(e) => {
            let text = ow_combat::hit::access::error_text(&e);
            for i in 0..LINKS.len() {
                if LINKS[i].driver == Driver::Combat && LINKS[i].needs.is_empty() {
                    ran[i] = Some(Ran {
                        steps: Vec::new(),
                        stop: Some(classify("setup", &text)),
                        error_lines: Vec::new(),
                    });
                }
            }
            return String::new();
        }
    };
    let mut bound: Vec<(String, usize)> = Vec::new();
    for i in 0..LINKS.len() {
        if !should_run(i, Driver::Combat, ran) {
            continue;
        }
        let mut r = Ran::default();
        for step in LINKS[i].steps {
            let step = match bind(step, &bound) {
                Ok(s) => s,
                Err(text) => {
                    r.stop = Some((Outcome::Error, Stop { step: (*step).into(), text, ..Stop::default() }));
                    break;
                }
            };
            let before_errors = run.errors().len();
            let before_pawns = (0..).take_while(|&n| run.enemy(n).is_some()).count();
            run.apply(&step);
            r.steps.push(step.clone());
            let lines: Vec<String> = run.errors()[before_errors..]
                .iter()
                .map(|t| t.strip_prefix(&format!("{step}: ")).unwrap_or(t).to_string())
                .collect();
            if let Some(text) = lines.first() {
                let (outcome, stop) = classify(&step, text);
                if grading == Grading::Strict || outcome == Outcome::NotYet {
                    r.stop = Some((outcome, stop));
                    break;
                }
                r.error_lines.extend(lines.into_iter().map(|t| (step.clone(), t)));
            }
            let after_pawns = (0..).take_while(|&n| run.enemy(n).is_some()).count();
            if after_pawns > before_pawns
                && let Some(kind) = step.strip_prefix("spawn:").and_then(|s| s.split(':').next())
            {
                bound.push((kind.to_string(), after_pawns - 1));
            }
        }
        ran[i] = Some(r);
    }
    run.report(0.0)
}

/// The Fire group (module documentation) on one store policy: what each of its links did, the mission's status after
/// each, and whether the store's copy refusal showed.
fn run_fire_once(
    cooked: &str,
    policy: IdentityPolicy,
    grading: Grading,
    ran: &mut [Option<Ran>],
    status: &mut [u8],
) -> bool {
    let source = Path::new(cooked).join(FIRE_SOURCE);
    let setup = || -> Result<(ow_world::world::World, FireMissionSlice), slice::Failure> {
        let package = ow_package::package::Package::load(&source)
            .map_err(|e| slice::Failure::from(e.to_string()))?;
        let name = PackageName::parse(&package.name).map_err(|e| slice::Failure::from(e.to_string()))?;
        let vm = slice::open(cooked, ParityMode::Off, policy)?;
        let mut world = bridge::world(vm)?;
        let fire = FireMissionSlice::new(&mut world, FIRE_MISSION, &name, DUMMY_PROVIDER)?;
        Ok((world, fire))
    };
    let (mut world, fire) = match setup() {
        Ok(two) => two,
        Err(f) => {
            let refused = f.message.contains(slice::COPY_REFUSAL);
            let (outcome, stop) = classify("setup", &f.message);
            let outcome = if f.not_yet { Outcome::NotYet } else { outcome };
            for i in 0..LINKS.len() {
                if LINKS[i].driver == Driver::Fire && LINKS[i].needs.is_empty() {
                    ran[i] = Some(Ran {
                        steps: Vec::new(),
                        stop: Some((outcome, stop.clone())),
                        error_lines: Vec::new(),
                    });
                }
            }
            return refused;
        }
    };
    let completed: DetSet<String> = fire.mission().dependencies().iter().cloned().collect();
    let sink = Rc::new(std::cell::RefCell::new(String::new()));
    let mut refused = false;
    for i in 0..LINKS.len() {
        if !should_run(i, Driver::Fire, ran) {
            continue;
        }
        let mut r = Ran::default();
        for step in LINKS[i].steps {
            let (errors, not_yet) = (fire.errors().len(), fire.not_yet().len());
            let result = slice::run_step(&mut world.vm, &fire, &completed, step, &sink);
            let _ = fire.drain();
            r.steps.push((*step).to_string());
            if let Some(n) = fire.not_yet().get(not_yet) {
                r.stop = Some((Outcome::NotYet, stop_of_not_yet(step, n)));
                break;
            }
            let lines: Vec<String> = fire.errors()[errors..].to_vec();
            refused |= lines.iter().any(|t| t.contains(slice::COPY_REFUSAL));
            if let Some(text) = lines.first() {
                let (outcome, stop) = classify(step, text);
                if grading == Grading::Strict || outcome == Outcome::NotYet {
                    r.stop = Some((outcome, stop));
                    break;
                }
                r.error_lines.extend(lines.into_iter().map(|t| ((*step).to_string(), t)));
            }
            match result {
                Err(f) => {
                    refused |= f.message.contains(slice::COPY_REFUSAL);
                    let (outcome, stop) = classify(step, &f.message);
                    r.stop = Some((if f.not_yet { Outcome::NotYet } else { outcome }, stop));
                    break;
                }
                Ok((false, _)) => {
                    r.stop = Some((
                        Outcome::Error,
                        Stop {
                            step: (*step).to_string(),
                            text: format!("the step {step} answered not ok"),
                            ..Stop::default()
                        },
                    ));
                    break;
                }
                Ok(_) => {}
            }
        }
        status[i] = fire.mission().status().index();
        ran[i] = Some(r);
    }
    refused
}

/// The Fire group, run again on the explicit load set on the store's copy refusal (module documentation).
fn run_fire(cooked: &str, grading: Grading, ran: &mut [Option<Ran>], status: &mut [u8]) {
    let mut first: Vec<Option<Ran>> = vec![None; LINKS.len()];
    first.clone_from_slice(ran);
    if !run_fire_once(cooked, IdentityPolicy::FirstInScanOrder, grading, ran, status) {
        return;
    }
    let args = [Path::new(cooked).join(FIRE_SOURCE).to_string_lossy().into_owned()];
    if let Some(set) = slice::load_set(&args) {
        ran.clone_from_slice(&first);
        status.fill(0);
        run_fire_once(cooked, IdentityPolicy::Loaded(set), grading, ran, status);
    }
}

/// Runs both groups and grades the 14 links.
fn gate(cooked: &str, grading: Grading) -> Vec<Row> {
    let mut ran: Vec<Option<Ran>> = vec![None; LINKS.len()];
    let report = run_combat(Path::new(cooked), grading, &mut ran);
    let mut status = vec![0u8; LINKS.len()];
    run_fire(cooked, grading, &mut ran, &mut status);
    rows(&ran, grading, |i| match LINKS[i].evidence {
        Evidence::Row(prefix) => report.contains(&format!("\"{prefix}")),
        Evidence::Status(s) => status[i] == s,
        Evidence::Steps => true,
    })
}

/// The exit code of a set of rows (module documentation).
pub fn exit_code(rows: &[Row]) -> u8 {
    if rows.iter().any(|r| r.result == Outcome::Error) {
        1
    } else if rows.iter().any(|r| r.result == Outcome::NotYet) {
        NOT_PORTED
    } else {
        0
    }
}

pub fn run(_command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    if mode != ParityMode::Off {
        eprintln!("ow-package: --slice-gate runs in Off only (the slice gate counts only there)");
        return ExitCode::from(1);
    }
    let (cooked, grading) = match args {
        [flag, opt, dir] if flag == "--slice-gate" && opt == "--cooked" => (dir.clone(), Grading::Strict),
        [flag, opt, dir, past] if flag == "--slice-gate" && opt == "--cooked" && past == "--past-errors" => {
            (dir.clone(), Grading::PastErrors)
        }
        _ => {
            eprintln!("ow-package: {USAGE}");
            return ExitCode::from(1);
        }
    };
    let rows = match on_vm_stack(move || gate(&cooked, grading)) {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("ow-package: the VM thread failed: {e}");
            return ExitCode::from(1);
        }
    };
    let mut out = String::new();
    for r in &rows {
        let _ = writeln!(out, "{}", r.json());
    }
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    ExitCode::from(exit_code(&rows))
}

#[cfg(test)]
#[path = "slice_gate_tests.rs"]
mod tests;
