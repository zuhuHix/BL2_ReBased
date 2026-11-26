//! Owner: lane SLICE-RUN-OFF. Command: `--slice-gate --cooked <dir>`, the headless proof of the 14 slice-gate links
//! (docs/verification/SLICE_GATE.md) in `ParityMode::Off`. AI-assisted (Claude Code). Decision `2026-11-19-08`.
//!
//! It adds no gameplay rule and binds no native: each link is driven by a command that already exists, with a fixed
//! step list, and is graded from what that driver reports. It prints one JSON row per link, in gate order:
//!
//! `{"n":<1..14>,"link":<name>,"grading":"strict"|"past-errors","result":"ok"|"notyet"|"error",
//! "driver":"combat-run"|"slice-run"|"none","steps":[..],"stop":null|{"step","native","record","text"},
//! "blocked_by":null|<link>,"after_errors_in":null|<link>,"error_lines":[{"step","text"}..],"scope":<text>,
//! "harness":[<text>..]}`, where `harness` lists the harness inputs the link's own steps logged (`harness input: ...`
//! lines of the VM log, with their outcomes), so a row shows what the run supplied (rule 7; review of #431). A combat
//! row first carries its world's set-up inputs, the `harness input:` lines logged while the world was built, each
//! prefixed `world set-up: ` ([`SETUP_MARK`], lane GATE-SETUP-HARNESS-LINES); on the `--map` route the route's own
//! notes come before those, in full on the first combat row and as one `world set-up: route stand-ins as above` line
//! on each later one ([`row_harness`]).
//!
//! `tools/check_slice_run_off.py` grades the rows; the gate is 14 ok, 0 notyet, 0 error.
//!
//! After the rows it prints a report to stderr ([`step_report`], lane SLICE-GATE-STEP-SHARE): each link's steps
//! `<passed>/<total>`, `links ok <P>/14` and the step share X. Stdout holds only the rows.
//!
//! **Drivers.**
//! - `combat-run` (`ow_combat::run`, over the mission bridge's player as `--combat-run` builds it): links 1, 3, 4, 5,
//!   13 and 14, in that order on one world. A step `spawn:<kind>:...` binds `{<kind>}` in later steps to the new pawn's
//!   index.
//! - `slice-run` (`ow_mission::slice::FireMissionSlice`, as `--slice-run` builds it, with the step list of
//!   `tools/verify_fire_event_order.py`): links 10 to 12, on a second world. On the definition store's copy refusal
//!   the group runs again on the explicit load set, as `--slice-run` does.
//! - `equip-run` (lane SLICE-GATE-DRIVERS, decision 2026-11-19-39): link 8, the phases of `--inventory-move
//!   --equip-run` (the spawn's `init` included) on one fixed weapon ([`EQUIP_WEAPON`]) on that command's own world;
//!   ok on its own evidence only ([`equip_evident`]).
//! - `phaselock-cast` (same lane): link 9, the installed Phaselock cast on its own world; ok on its own evidence only
//!   ([`phaselock_evident`], lane LINK9-EVIDENCE-CHECK): after the cast, the Phaselock skill holds the cast's target
//!   as its `LiftedPawn` and its `CurrentState` is `EPLS_Intro`, the two writes `LiftActionSkill.PhaseLockTarget`
//!   makes before `OnSelectedTarget` fires its event (the harness's evidence choice, UNVERIFIED in game).
//! - `sanctuary-walk` (lane LINK-2-HEADLESS-MOVER, decision 2026-11-19-66): link 2, H4's scripted walk on the
//!   installed Sanctuary (`ow_assets::movement`, the mover and collision `ow-host --walk` and `--play` use), on its own
//!   load. The walk's own checks (no fall through the world, no stuck segment) come first; then `fight_area`
//!   (lane LINK-2-FIGHT-AREA, decision 2026-11-19-76) places the target dummy at a labelled PORT FIXTURE spot
//!   ([`fight_area::FIGHT_AREA`]) through `ow_world`'s spawn path and passes when Maya's walked end and the dummy are
//!   both inside that area. That is the link's evidence, as a port fixture and not parity: Sanctuary's own
//!   population is a later seam.
//! - `none`: links 6 and 7 have no headless driver yet; each row is `notyet` with the reason.
//!
//! **`--map <name>`** (lane IMPL-SLICE-MAP-WORLD, SLICE_MAPLOAD.md section 4, lane 4): the `combat-run` links run
//! on the map route's world instead (`combat::map_route::build`: the map loaded and brought up for play, the local
//! player's play actor spawned, the bridge adopted from that world). The other drivers are unchanged. The route's
//! harness lines (the globals and the local player it supplies, the bring-up's failure count and the game info's own
//! stops) go to the first combat link's `harness`; a refusal while building the route is that link's `setup` stop.
//! The links' scope texts are the old route's until the maintainer changes link 1's scope (SLICE_MAPLOAD.md section
//! 7). Without `--map` nothing changes.
//!
//! The two driver worlds are separate: the gate's one session is not modelled yet (the slice bridge refuses a world
//! that already has actors, and the map world is not brought up for play, decision 2026-11-19-04).
//!
//! **Grading.** A link runs its steps in order and stops at the first step that meets a `NotYet` (`notyet`), records
//! an error line, fails a step, or returns an error (`error`). With no stop, the link is `ok` only when its evidence is
//! there: a row of the combat report that the link's own steps added (`kill.`, `ratio.`, `loot.`, `down.total.`,
//! `down.respawn_health.`; link 5, which has no steps, is graded on the rows link 3's kill added), or the Fire mission's
//! status after the link (1 Active, 2 ReadyToTurnIn, 3 Complete); else `error`, naming the missing
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
    /// `--inventory-move --equip-run`'s phases on one weapon (link 8).
    Equip,
    /// The installed Phaselock cast (link 9).
    Phaselock,
    /// H4's scripted Sanctuary walk through the shared mover (link 2).
    Walk,
    /// The headless pickup of `slice_pickup` (link 6).
    Pickup,
    /// The same with a decoy and the look (link 7).
    AimedPickup,
    /// No headless driver yet; the text says why. Since links 2, 6 and 7 got theirs, no link uses it; it stays for a
    /// link added before its driver.
    #[allow(dead_code)]
    None(&'static str),
}

impl Driver {
    fn name(self) -> &'static str {
        match self {
            Driver::Combat => "combat-run",
            Driver::Fire => "slice-run",
            Driver::Equip => "equip-run",
            Driver::Phaselock => "phaselock-cast",
            Driver::Walk => "sanctuary-walk",
            Driver::Pickup => "pickup",
            Driver::AimedPickup => "pickup-aimed",
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
    /// What the link's own driver reports of its own run (`Ran::evident`); the text says what it checks.
    Driver(&'static str),
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
        Driver::Walk,
        &["load", "spawn", "walk", "fight_area"],
        &[],
        Evidence::Driver(
            "the walk's own checks (no fall through the world, no stuck segment), then Maya's walked end and the placed \
             dummy inside the fight area (a PORT FIXTURE, decision 2026-11-19-76)",
        ),
        "H4's scripted Sanctuary walk (SANCTUARY_SCRIPT, 1/60 s steps) through the host's mover and collision, on its \
         own load; the fight area is a port fixture spot with the target dummy placed by the driver through \
         ow_world's spawn path, not Sanctuary's own population",
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
        Evidence::Row("loot."),
        "the death roll of link 3's kill (the loot rows its hit added)",
    ),
    link(
        "pick_up_gun",
        Driver::Pickup,
        &["make", "touch", "key"],
        &[],
        Evidence::Driver("the weapon is the pawn's own afterwards (its Owner), taken from the pickup"),
        "the installed pickup script on EQUIP_WEAPON's pickup (spawned and initialised from the gun, a harness \
         stand-in for DropFrom) on the equip run's world, touched and taken by the key (NATIVE_PICKUP.md; decision \
         2026-11-19-68), not link 5's drop",
    ),
    link(
        "pick_up_aimed_not_nearest",
        Driver::AimedPickup,
        &["make", "touch", "look", "key"],
        &[],
        Evidence::Driver("the aimed gun is the pawn's own afterwards and the touched decoy is not"),
        "link 6's run with a second gun as the decoy the pawn touches, then the per-tick look and the key \
         (NATIVE_PICKUP.md section 4's fixture; decision 2026-11-19-68)",
    ),
    link(
        "equip_gun",
        Driver::Equip,
        &["make", "init", "bases", "equip", "unequip"],
        &[],
        Evidence::Driver(
            "the equip added external records to the weapon, the unequip took every one back, and no step left an \
             error line",
        ),
        "--equip-run's phases on one fixed weapon (EQUIP_WEAPON), not link 5's drop, on its own world: the spawn's \
         InitializeInternal first, then the bases, the equip and the unequip; no shot is fired (decision 2026-11-19-03)",
    ),
    link(
        "phaselock_enemy",
        Driver::Phaselock,
        &["spawn", "cast"],
        &[],
        Evidence::Driver(
            "after the cast, the Phaselock skill's LiftedPawn is the cast's target and its CurrentState is EPLS_Intro \
             (LiftActionSkill.PhaseLockTarget's writes)",
        ),
        "the installed LiftActionSkill.OnActionSkillStarted on Maya's Phaselock skill, with a player pawn, its \
         controller and an AI pawn each spawned by the world's own spawn (their components their own; decision \
         2026-11-19-70), on its own world (decision 2026-11-16-42)",
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
    /// The harness inputs the link's steps logged ([`Ran::harness`]).
    pub harness: Vec<String>,
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
             \"blocked_by\":{},\"after_errors_in\":{},\"error_lines\":[{}],\"scope\":{},\"harness\":[{}]}}",
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
            quote(self.scope),
            self.harness.iter().map(|h| quote(h)).collect::<Vec<_>>().join(",")
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

/// A driver's typed error as a graded stop (lane IMPL-PICKUP, review r3 of #452): a `NotYet` by its variant, with its
/// own text and record; an unbound native naming its key; anything else an error with its text.
pub fn classify_error(step: &str, e: &ow_vm::error::VmError) -> (Outcome, Stop) {
    use ow_vm::error::VmError;
    let text = ow_combat::hit::access::error_text(e);
    match e {
        VmError::NotYet(n) => (Outcome::NotYet, stop_of_not_yet(step, n)),
        VmError::UnboundNative(key) => (
            Outcome::Error,
            Stop { step: step.to_string(), native: key.clone(), text, ..Stop::default() },
        ),
        _ => (Outcome::Error, Stop { step: step.to_string(), text, ..Stop::default() }),
    }
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
    /// The ids of the combat report's rows the link's steps added (a link with no steps: its prerequisite's).
    pub new_rows: Vec<String>,
    /// What a driver with its own evidence check found of the link's own run (`Evidence::Driver`).
    pub evident: bool,
    /// The harness input lines the link's steps logged, in order, also when a step stopped.
    pub harness: Vec<String>,
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
            harness: Vec::new(),
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
                row.harness = r.harness;
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
        Evidence::Driver(what) => format!("the driver's own check: {what}"),
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

/// The marker a world set-up harness line carries in a row (lane GATE-SETUP-HARNESS-LINES).
pub const SETUP_MARK: &str = "world set-up: ";

/// The `harness input:` lines of a world's log before any link ran, each marked with [`SETUP_MARK`]. The map route's
/// own notes (`route_notes`), which it also logs, are left out: the first combat row carries them already, unmarked
/// (review of #777, item 1).
pub fn setup_harness(log: &[String], route_notes: &[String]) -> Vec<String> {
    log.iter()
        .filter(|l| l.starts_with("harness input:") && !route_notes.contains(l))
        .map(|l| format!("{SETUP_MARK}{l}"))
        .collect()
}

/// The line a later combat row carries for the map route's notes (the hub's ruling on review of #777): it runs on the
/// same world, built with those stand-ins.
pub const ROUTE_AS_ABOVE: &str = "world set-up: route stand-ins as above (the first combat row's route notes)";

/// One combat row's `harness` list, in order: the map route's notes in full on the first combat row (`first`), or
/// [`ROUTE_AS_ABOVE`] on a later one when the route left notes; then the world's set-up lines, then the lines the
/// link's own steps logged.
pub fn row_harness(route_notes: &[String], first: bool, setup: &[String], steps: &[String]) -> Vec<String> {
    let route: Vec<String> = match (first, route_notes.is_empty()) {
        (_, true) => Vec::new(),
        (true, false) => route_notes.to_vec(),
        (false, false) => vec![ROUTE_AS_ABOVE.to_string()],
    };
    route.into_iter().chain(setup.iter().cloned()).chain(steps.iter().cloned()).collect()
}

/// The combat group (module documentation): what each of its links did, with the report rows each one added.
fn run_combat(cooked: &Path, grading: Grading, ran: &mut [Option<Ran>], map: Option<&str>) {
    let mut route_notes: Vec<String> = Vec::new();
    let mut setup = || -> Result<Run, ow_vm::error::VmError> {
        let (world, bridge) = match map {
            None => {
                let mut world = combat::world(cooked, ParityMode::Off)?;
                let bridge = combat::MissionBridge::new(&mut world)?;
                (world, bridge)
            }
            Some(map) => combat::map_route::build(cooked, map, &mut route_notes)?,
        };
        let bridge: Rc<dyn PlayerBridge> = Rc::new(bridge);
        Run::with_bridge(world, Some(bridge))
    };
    let built = setup();
    let first_combat = LINKS.iter().position(|l| l.driver == Driver::Combat);
    let mut run = match built {
        Ok(r) => r,
        Err(e) => {
            for i in 0..LINKS.len() {
                if LINKS[i].driver == Driver::Combat && LINKS[i].needs.is_empty() {
                    ran[i] = Some(Ran {
                        steps: Vec::new(),
                        // The map route's refusals are graded by their variant; the old route keeps its text grading.
                        stop: Some(if map.is_some() {
                            typed_stop("setup", &e)
                        } else {
                            classify("setup", &ow_combat::hit::access::error_text(&e))
                        }),
                        harness: if Some(i) == first_combat { route_notes.clone() } else { Vec::new() },
                        ..Ran::default()
                    });
                }
            }
            return;
        }
    };
    // The world's own set-up harness lines (lane GATE-SETUP-HARNESS-LINES): every combat link runs on this one world,
    // so each combat row carries them, marked as set-up, before its own steps' lines (the PhysX level among them).
    let setup_lines = setup_harness(&run.world().vm.log, &route_notes);
    let mut bound: Vec<(String, usize)> = Vec::new();
    for i in 0..LINKS.len() {
        if !should_run(i, Driver::Combat, ran) {
            continue;
        }
        let mut r = Ran::default();
        let rows_before: Vec<String> = run.row_ids().map(str::to_string).collect();
        let log_before = run.world().vm.log.len();
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
            let sticky_before = run.sticky_stop().is_some();
            run.apply(&step);
            r.steps.push(step.clone());
            // A typed sticky stop this step set (a NotYet after partial writes, review r1 of #481, item 2) is graded by
            // its variant, before any text.
            if !sticky_before && let Some(e) = run.sticky_stop() {
                r.stop = Some(typed_stop(&step, e));
                break;
            }
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
        let log = &run.world().vm.log;
        let step_lines: Vec<String> = log[log_before.min(log.len())..]
            .iter()
            .filter(|l| l.starts_with("harness input:"))
            .cloned()
            .collect();
        r.harness = row_harness(&route_notes, Some(i) == first_combat, &setup_lines, &step_lines);
        r.new_rows = new_ids(&rows_before, run.row_ids());
        if LINKS[i].steps.is_empty()
            && let Some(need) = LINKS[i].needs.first().and_then(|n| LINKS.iter().position(|l| l.name == *n))
            && let Some(prior) = &ran[need]
        {
            r.new_rows = prior.new_rows.clone();
        }
        ran[i] = Some(r);
    }
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
    // The file in its own case (lane LINUX-PATHS, `ow_core::install`): Aspyr's Linux build names it in lower case.
    let source = ow_core::install::resolve_or_join(Path::new(cooked), FIRE_SOURCE);
    let setup = || -> Result<(ow_world::world::World, FireMissionSlice), slice::Failure> {
        let source = source.clone().map_err(|e| slice::Failure::from(e.to_string()))?;
        let package = ow_package::package::Package::load(&source)
            .map_err(|e| slice::Failure::from(e.to_string()))?;
        let name = PackageName::parse(&package.name).map_err(|e| slice::Failure::from(e.to_string()))?;
        let vm = slice::open(cooked, ParityMode::Off, policy)?;
        let mut world = bridge::world(vm)?;
        let fire = FireMissionSlice::new(&mut world, FIRE_MISSION, &name, DUMMY_PROVIDER)?;
        // Link 12's native and its globals preload (lane IMPL-LINK12-UNLOCK-ACHIEVEMENT), as `--slice-run` does.
        slice::unlock_setup(&mut world).map_err(|e| slice::Failure::from(ow_behavior::provider::BehaviorError::Vm(e)))?;
        // Link 12's balance preload (lane IMPL-REFRESH-BALANCE-DATA), as `--slice-run` does.
        slice::balance_setup(&mut world, &fire)?;
        Ok((world, fire))
    };
    let (mut world, fire) = match setup() {
        Ok(two) => two,
        Err(f) => {
            let refused = f.message.contains(slice::COPY_REFUSAL);
            // Graded by the failure's type, never by its text (review r1 of #464, item 5).
            let (_, stop) = classify("setup", &f.message);
            let outcome = if f.not_yet { Outcome::NotYet } else { Outcome::Error };
            for i in 0..LINKS.len() {
                if LINKS[i].driver == Driver::Fire && LINKS[i].needs.is_empty() {
                    ran[i] = Some(Ran {
                        steps: Vec::new(),
                        stop: Some((outcome, stop.clone())),
                        ..Ran::default()
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
                // A typed `NotYet` the bridge recorded is graded above; a recorded line here is an error, whatever
                // its text quotes (review r1 of #464, item 5).
                let (_, stop) = classify(step, text);
                if grading == Grading::Strict {
                    r.stop = Some((Outcome::Error, stop));
                    break;
                }
                r.error_lines.extend(lines.into_iter().map(|t| ((*step).to_string(), t)));
            }
            match result {
                Err(f) => {
                    refused |= f.message.contains(slice::COPY_REFUSAL);
                    let (_, stop) = classify(step, &f.message);
                    r.stop = Some((if f.not_yet { Outcome::NotYet } else { Outcome::Error }, stop));
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
    let Ok(source) = ow_core::install::resolve_or_join(Path::new(cooked), FIRE_SOURCE) else {
        return; // ambiguous: the first run above has already reported it
    };
    let args = [source.to_string_lossy().into_owned()];
    if let Some(set) = slice::load_set(&args) {
        ran.clone_from_slice(&first);
        status.fill(0);
        run_fire_once(cooked, IdentityPolicy::Loaded(set), grading, ran, status);
    }
}

/// Runs both groups and grades the 14 links.
fn gate(cooked: &str, grading: Grading, map: Option<&str>) -> Vec<Row> {
    let mut ran: Vec<Option<Ran>> = vec![None; LINKS.len()];
    run_combat(Path::new(cooked), grading, &mut ran, map);
    let mut status = vec![0u8; LINKS.len()];
    run_fire(cooked, grading, &mut ran, &mut status);
    run_equip(Path::new(cooked), grading, &mut ran);
    run_phaselock(Path::new(cooked), grading, &mut ran);
    run_walk(Path::new(cooked), &mut ran);
    run_pickup(Path::new(cooked), Driver::Pickup, grading, &mut ran);
    run_pickup(Path::new(cooked), Driver::AimedPickup, grading, &mut ran);
    rows(&ran, grading, |i| evident(i, &ran, &status))
}

/// Whether link `i` has its evidence (the one check `gate` and the tests share): the rows its own steps added, the
/// Fire mission's status after it, its driver's own check, or never for a link whose check is not chosen.
pub fn evident(i: usize, ran: &[Option<Ran>], status: &[u8]) -> bool {
    match LINKS[i].evidence {
        Evidence::Row(prefix) => evidence_in(ran[i].as_ref(), prefix),
        Evidence::Status(s) => status.get(i) == Some(&s),
        Evidence::Steps => true,
        Evidence::Driver(_) => ran[i].as_ref().is_some_and(|r| r.evident),
    }
}

/// Link 8's own evidence: no error line, the external records before the equip, after it and after the unequip
/// all seen, the equip adding at least one record, and the unequip leaving none.
pub fn equip_evident(
    error_lines: usize,
    before: Option<usize>,
    after_equip: Option<usize>,
    after_unequip: Option<usize>,
) -> bool {
    match (before, after_equip, after_unequip) {
        (Some(b), Some(e), Some(u)) => error_lines == 0 && e > b && u == 0,
        _ => false,
    }
}

/// The weapon link 8 equips: a weapons-file line of `--equip-run` (`<weapon type> <level> <balance>
/// [<Slot>=<part>]...`). A Bandit assault rifle at level 1, the parts of a seed-1 Reference roll of its balance
/// (`--item-card --weapons`). Fixed, not link 5's drop, because no headless run spawns that drop as an inventory yet.
pub const EQUIP_WEAPON: &str = "GD_Weap_AssaultRifle.A_Weapons.WT_Bandit_AssaultRifle 1 \
    GD_Weap_AssaultRifle.A_Weapons.AR_Bandit Barrel=GD_Weap_AssaultRifle.Barrel.AR_Barrel_Bandit \
    Body=GD_Weap_AssaultRifle.Body.AR_Body_Bandit Grip=GD_Weap_AssaultRifle.Grip.AR_Grip_Jakobs \
    Material=GD_Weap_AssaultRifle.ManufacturerMaterials.Mat_BanditMade_1 \
    Sight=GD_Weap_AssaultRifle.Sight.AR_Sight_None Stock=GD_Weap_AssaultRifle.Stock.AR_Stock_Jakobs \
    Prefix=GD_Weap_Shared_Names.Name.Prefix_Bandit.Prefix_Grip_Jakobs \
    Title=GD_Weap_AssaultRifle.Name.Title_Bandit.Title__MashineGun";

/// Records one step's typed error on `r` as the other drivers do: strict grading or a `NotYet` stops the link (true),
/// past-errors grading notes the line and goes on (false). It is graded by its variant ([`classify_error`]), so only a
/// `VmError::NotYet` is a not-yet stop (review r3 of #468: the equip driver uses it too).
fn step_error_typed(r: &mut Ran, step: &str, e: &ow_vm::error::VmError, grading: Grading) -> bool {
    let (outcome, stop) = classify_error(step, e);
    if grading == Grading::Strict || outcome == Outcome::NotYet {
        r.stop = Some((outcome, stop));
        return true;
    }
    r.error_lines.push((step.to_string(), ow_combat::hit::access::error_text(e)));
    false
}

/// The index of the one link a driver runs.
fn link_of(driver: Driver) -> Option<usize> {
    LINKS.iter().position(|l| l.driver == driver)
}

/// Link 8 (module documentation): `make` prepares the weapon and its owner as `--equip-run` does, then each of its
/// phases runs in turn, the spawn's `init` first. The evidence is the run's own ([`equip_evident`]).
///
/// The world has the globals object (`equip_run::equip_world`, NATIVE_WILLOW_GLOBALS.md section 6.1). Before the weapon
/// is made, `make` runs the weapon init's preflight (lane IMPL-CACHE-WEAPON-CARD-BODY, NATIVE_CACHE_WEAPON_CARD_BODY.md
/// section 4, [`ow_inventory::equip::card::preflight`]): NATIVE_WILLOW_GLOBALS.md section 6.2 steps 1 to 3, the name-part schema and
/// the card schema, each failure returned unprefixed and graded by its variant, with nothing written.
///
/// **The partial-writes mark is this layer's** (review r1 of #506; the hub's rule that the caller layer that wrote adds
/// it): `make` writes the weapon and its owner, so every phase's `NotYet` after it is terminal and gets "stopped after
/// partial writes: " through `ow_world::pools::after_partial_writes`, its kind and record kept. `make` itself is
/// marked only when its failure came after the weapon was allocated ([`make_marked`], review r2 of #506), so the
/// preflight's refusal and a failure before any write stay unprefixed. The init's natives
/// (`ChooseRandomNameParts`, `CacheWeaponCard`, `ApplyInternalSlotEffectModifiers`) return their pre-write `NotYet`s
/// unprefixed.
fn run_equip(cooked: &Path, grading: Grading, ran: &mut [Option<Ran>]) {
    let Some(i) = link_of(Driver::Equip) else { return };
    if !should_run(i, Driver::Equip, ran) {
        return;
    }
    let mut r = Ran::default();
    let mut world = match crate::cmd::inventory::equip_run::equip_world(cooked) {
        Ok(w) => w,
        Err(e) => {
            r.stop = Some(classify_error("setup", &e));
            ran[i] = Some(r);
            return;
        }
    };
    let vm = &mut world.vm;
    r.steps.push("make".into());
    if let Err(e) = ow_inventory::equip::card::preflight(vm) {
        r.stop = Some(classify_error("make", &e));
        ran[i] = Some(r);
        return;
    }
    let log_before = vm.log.len();
    let made = make_marked(vm, |vm, wrote| {
        // The equip run's own preparation (prepare, then the muzzle-flash preload of #573, the gestalt preload of
        // lane HARNESS-LINK8-GESTALT-PRELOAD, the new-mesh preload of lane LINK8-NEWMESH-PRELOAD and the material
        // preload of lane LINK-8-STARTUP-MATERIAL-PRELOAD), inside #506's mark.
        let weapon = crate::cmd::inventory::equip_run::prepare_link8_typed(vm, EQUIP_WEAPON, wrote)?;
        crate::cmd::inventory::equip_run::preload_muzzle_flashes(vm, weapon)?;
        crate::cmd::inventory::equip_run::preload_gestalt_mesh(vm, weapon)?;
        crate::cmd::inventory::equip_run::preload_gestalt_skeletal_mesh(vm, weapon)?;
        // Lane LINK-8-STARTUP-MATERIAL-PRELOAD: the Material part's material, a Startup export the game has resident.
        crate::cmd::inventory::equip_run::preload_material_part_material(vm, weapon)?;
        // Then the material classes' code and the MaterialClass default object (boot-loaded script classes).
        crate::cmd::inventory::equip_run::preload_material_classes(vm, weapon)?;
        // Lane SKELMESH-LOD-LAYER: the skeletal mesh data layer, whose LOD count the LOD row asks for.
        crate::cmd::inventory::equip_run::install_mesh_layer(vm, weapon)?;
        Ok(weapon)
    });
    // The set-up's harness inputs (the muzzle-flash preload), kept on success and on a make failure alike (review r1
    // of #573, item 5).
    r.harness = vm.log[log_before.min(vm.log.len())..]
        .iter()
        .filter(|l| l.starts_with("harness input:"))
        .cloned()
        .collect();
    let weapon = match made {
        Ok(w) => w,
        Err(e) => {
            r.stop = Some(classify_error("make", &e));
            ran[i] = Some(r);
            return;
        }
    };
    // The SetBaseValue harness preload (the hub's ruling on #572), logged as harness inputs.
    let log_before = vm.log.len();
    if let Err(e) = crate::cmd::inventory::equip_run::preload_threshold_attributes(vm, weapon) {
        r.stop = Some(classify("make", &ow_combat::hit::access::error_text(&e)));
        ran[i] = Some(r);
        return;
    }
    r.harness.extend(vm.log[log_before..].iter().filter(|l| l.starts_with("harness input:")).cloned()); // after #573's
    let (mut before, mut after_equip, mut after_unequip) = (None, None, None);
    for step in LINKS[i].steps.iter().skip(1) {
        r.steps.push((*step).to_string());
        if *step == "equip" {
            before = Some(crate::cmd::inventory::equip_run::external_records(vm, weapon));
        }
        // `make` wrote: a phase's NotYet is after partial writes (this layer's mark, review r1 of #506).
        let error = crate::cmd::inventory::equip_run::run_phase_typed(vm, step, weapon)
            .map(ow_world::pools::after_partial_writes);
        let records = crate::cmd::inventory::equip_run::external_records(vm, weapon);
        match *step {
            "equip" => after_equip = Some(records),
            "unequip" => after_unequip = Some(records),
            _ => {}
        }
        if let Some(e) = error
            && step_error_typed(&mut r, step, &e, grading)
        {
            break;
        }
    }
    r.evident = equip_evident(r.error_lines.len(), before, after_equip, after_unequip);
    ran[i] = Some(r);
}

/// Link 8's `make` (review r2 of #506): `prepare` with its typed error kept, marked "stopped after partial writes"
/// only when `prepare` set its `wrote` flag (the weapon was allocated before it failed); a failure before any write is
/// returned unprefixed.
pub(crate) fn make_marked<T>(
    vm: &mut ow_vm::api::Vm,
    prepare: impl FnOnce(&mut ow_vm::api::Vm, &mut bool) -> Result<T, ow_vm::error::VmError>,
) -> Result<T, ow_vm::error::VmError> {
    let mut wrote = false;
    prepare(vm, &mut wrote).map_err(|e| if wrote { ow_world::pools::after_partial_writes(e) } else { e })
}

/// Link 9 (module documentation): the installed cast on a world with the Core natives and `ow-skills`' tree and
/// action-skill packs and a `WorldInfo`, as `ow-skills`' installed run builds it. The evidence is the run's own
/// ([`phaselock_evident`], read after a cast that returned with no stop).
fn run_phaselock(cooked: &Path, grading: Grading, ran: &mut [Option<Ran>]) {
    let Some(i) = link_of(Driver::Phaselock) else { return };
    if !should_run(i, Driver::Phaselock, ran) {
        return;
    }
    let mut r = Ran::default();
    // The set-up's harness lines go into the row, also when the set-up fails (review r1 of #579).
    let world = phaselock_world(cooked, &mut r.harness);
    let mut world = match world {
        Ok(w) => w,
        Err(e) => {
            r.stop = Some(classify("setup", &ow_combat::hit::access::error_text(&e)));
            ran[i] = Some(r);
            return;
        }
    };
    drive_phaselock(&mut r, &mut world.vm, grading, &PHASELOCK_ACTORS, phaselock_cast, |vm, skill, (_, _, target)| {
        phaselock_evident(vm, skill, target)
    });
    ran[i] = Some(r);
}

/// The value name link 9's evidence expects in `CurrentState` (`LiftActionSkill.EPhaselockState`).
pub const PHASELOCK_LIFTED_STATE: &str = "EPLS_Intro";

/// Link 9's evidence (lane LINK9-EVIDENCE-CHECK; the harness's choice, UNVERIFIED in game): the skill's `LiftedPawn`
/// is `target` (a set target) and its `CurrentState` holds the value named [`PHASELOCK_LIFTED_STATE`] in the enum the
/// property declares. These are the writes `LiftActionSkill.PhaseLockTarget` makes, in script, before it calls
/// `OnSelectedTarget`; a cast that fizzles, is blocked or picks no target leaves them unset. Reads only. A missing
/// property, an undeclared enum or another value kind is no evidence.
pub fn phaselock_evident(vm: &ow_vm::api::Vm, skill: ow_object::ids::ObjRef, target: Option<ow_object::ids::ObjRef>) -> bool {
    use ow_object::ids::PropAddr;
    use ow_object::layout::PropKind;
    use ow_object::value::Value;
    let Some(target) = target else {
        return false;
    };
    let Ok(class) = vm.space.heap.get(skill).map(|o| o.class) else {
        return false;
    };
    let (Some(lifted), Some(state)) = (vm.space.prop(class, "LiftedPawn"), vm.space.prop(class, "CurrentState")) else {
        return false;
    };
    if !matches!(vm.space.read(skill, PropAddr::scalar(lifted)), Ok(Value::Object(Some(o))) if *o == target) {
        return false;
    }
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(state).map(|d| d.kind.clone()) else {
        return false;
    };
    // By name, as the decoder reads an enum value (`NameTable::find`, case-folded, then `EnumLayout::index_of`; review r1
    // of #568): the name table keeps the first spelling interned, so the text is not compared.
    let Some(want) = vm
        .space
        .names
        .find(PHASELOCK_LIFTED_STATE)
        .and_then(|n| vm.space.classes.enum_layout(e)?.index_of(n))
    else {
        return false;
    };
    matches!(vm.space.read(skill, PropAddr::scalar(state)), Ok(Value::Byte(b)) if usize::from(*b) == want)
}

/// The classes of the cast's actors, in spawn order: Maya's pawn, her controller, the target.
pub const PHASELOCK_ACTORS: [&str; 3] = ["WillowGame.WillowPlayerPawn", "WillowGame.WillowPlayerController", "WillowGame.WillowAIPawn"];

/// Link 9's two steps (decision 2026-11-19-70; review r2 of #454): the world spawns the cast's actors, then `cast`
/// runs on them. Each step's failure is graded from its typed error: a `NotYet` (the `VmError::NotYet` variant, not
/// its text) is the link's not-yet stop. Any other error is a hard stop, under both gradings, when it comes from the
/// spawn, because the cast cannot run without its actors; from the cast, an error follows the grading as any step's
/// does.
///
/// **The evidence** (review r1 of #568): `cast` returns the exact object it ran on, and only after a cast that
/// returned `Ok` is `evidence` asked, read-only through `&Vm`, about that receiver and the actors; its answer is
/// `Ran::evident`. A cast that errored (also one past-errors grading goes on past) is never evidence.
pub fn drive_phaselock(
    r: &mut Ran,
    vm: &mut ow_vm::api::Vm,
    grading: Grading,
    classes: &[&str],
    cast: impl FnOnce(&mut ow_vm::api::Vm, CastActors) -> Result<ow_object::ids::ObjRef, ow_vm::error::VmError>,
    evidence: impl FnOnce(&ow_vm::api::Vm, ow_object::ids::ObjRef, CastActors) -> bool,
) {
    r.steps.push("spawn".into());
    let logged = vm.log.len();
    let spawned = phaselock_actors(vm, classes);
    // The spawn's harness inputs (the target's mind), also when it stopped.
    r.harness.extend(vm.log[logged.min(vm.log.len())..].iter().filter(|l| l.starts_with("harness input:")).cloned());
    let actors = match spawned {
        Ok(a) => a,
        Err(e) => {
            r.stop = Some(typed_stop("spawn", &e));
            return;
        }
    };
    r.steps.push("cast".into());
    match cast(vm, actors) {
        Ok(receiver) => r.evident = evidence(vm, receiver, actors),
        Err(e) => {
            let (outcome, stop) = typed_stop("cast", &e);
            if grading == Grading::Strict || outcome == Outcome::NotYet {
                r.stop = Some((outcome, stop));
            } else {
                r.error_lines.push(("cast".to_string(), stop.text));
            }
        }
    }
}

/// A step's stop from its typed error, graded by the variant and never by the text (review r3 of #454): a
/// `VmError::NotYet` is not yet, its subject and record taken from the value itself (`stop_of_not_yet`), so a record
/// whose text holds another `NotYet` phrase cannot blank them; anything else is an error, whatever its text says, with
/// the fields `classify` reads from the text (the key of an unbound native, when it names one).
fn typed_stop(step: &str, e: &ow_vm::error::VmError) -> (Outcome, Stop) {
    if let ow_vm::error::VmError::NotYet(n) = e {
        return (Outcome::NotYet, stop_of_not_yet(step, n));
    }
    let text = ow_combat::hit::access::error_text(e);
    let (_, stop) = classify(step, &text);
    (Outcome::Error, stop)
}

/// Link 2 (module documentation): Sanctuary loaded and its collision built (`load`), Maya at its first player start
/// (`spawn`), H4's scripted walk (`walk`), then the fight area (`fight_area`, [`fight_area`]). The mover and the
/// collision are the host's own (`ow_assets::movement`); nothing here adds a movement rule. A walk that fails its own
/// checks is an `error`; one that passes goes on to `fight_area`.
fn run_walk(cooked: &Path, ran: &mut [Option<Ran>]) {
    let Some(i) = link_of(Driver::Walk) else {
        return;
    };
    if !should_run(i, Driver::Walk, ran) {
        return;
    }
    ran[i] = Some(walk_link(cooked));
}

fn walk_link(cooked: &Path) -> Ran {
    use ow_assets::movement::sanctuary;
    let mut r = Ran::default();
    r.steps.push("load".into());
    let loaded = match sanctuary::load(cooked, sanctuary::MAP) {
        Ok(l) => l,
        Err(text) => {
            r.stop = Some(classify("load", &text));
            return r;
        }
    };
    r.steps.push("spawn".into());
    // The player start the walk begins at (the one `spawn` takes): the fight area is placed from it.
    let start = loaded.scene.player_starts.values().next().map(|s| {
        let l = s.pose.location;
        [f64::from(l[0]), f64::from(l[1]), f64::from(l[2])]
    });
    let mut spawn_text = String::new();
    let world = &loaded.collision.world;
    let mut mover = match sanctuary::spawn(&loaded.scene, world, &mut spawn_text) {
        Ok(m) => m,
        Err(text) => {
            r.stop = Some(classify("spawn", &text));
            return r;
        }
    };
    // What the run was given: the map, the player start and the lift out of solid (the spawn's own lines).
    r.harness = spawn_text.lines().map(str::to_string).collect();
    r.steps.push("walk".into());
    // The same invocation as `ow-host --walk` (script and time step).
    let report = sanctuary::scripted_walk(world, &mut mover);
    walk_graded(&mut r, &report);
    if r.stop.is_some() {
        return r;
    }
    let Some(start) = start else {
        r.stop = Some(classify("fight_area", "the scene has no player start"));
        return r;
    };
    let area = fight_area::FIGHT_AREA;
    let centre = fight_area::centre(&area, start);
    let end = report.end;
    fight_area::graded(&mut r, &area, centre, end, || fight_area::place_dummy(cooked, world, &area, centre, end));
    r
}

/// Grades link 2's walk `report` (pure, so the tests share it with `walk_link`): a walk that failed its own checks
/// stops the link with an `error`; one that passed leaves no stop, and the link goes on to `fight_area`
/// ([`fight_area::graded`]), which alone can make it evident.
pub fn walk_graded(r: &mut Ran, report: &ow_assets::movement::walk::WalkReport) {
    let (fell_through, stuck, ticks) = (report.fell_through, report.stuck, report.ticks);
    if !report.passed() {
        r.stop = Some((
            Outcome::Error,
            Stop {
                step: "walk".into(),
                text: format!(
                    "the walk failed its own checks after {ticks} ticks: fell through {fell_through}, stuck {stuck}"
                ),
                ..Stop::default()
            },
        ));
    }
}

/// Links 6 and 7 (`crate::cmd::slice_pickup`): `make` on the equip run's world (link 7 with a decoy), then the link's
/// steps. The evidence is the run's own ([`crate::cmd::slice_pickup::taken`]).
fn run_pickup(cooked: &Path, driver: Driver, grading: Grading, ran: &mut [Option<Ran>]) {
    let Some(i) = link_of(driver) else { return };
    if !should_run(i, driver, ran) {
        return;
    }
    let mut r = Ran::default();
    // The set-up's errors stay typed and are graded by variant (review r1 of #476).
    let (mut world, setup) = match crate::cmd::inventory::equip_run::pickup_world(cooked) {
        Ok(w) => w,
        Err(e) => {
            r.stop = Some(classify_error("setup", &e));
            ran[i] = Some(r);
            return;
        }
    };
    // Harness input (the hub's ruling for lane IMPL-TOUCH-PICKUP-TRACE), an UNVERIFIED stand-in: this world loads no
    // level, so it gets link 9's no-level collision service. Every level part is clear and it holds no actor bounds,
    // so `TouchPickupTrace`'s actor walk decides the touch (NATIVE_TOUCH_PICKUP_TRACE.md section 4). Without it the
    // native refuses with LayerAbsent.
    if let Err(e) = ow_world::placement::install_collision(&mut world.vm, Rc::new(ow_world::placement::NoLevel)) {
        r.stop = Some(classify_error("setup", &e));
        ran[i] = Some(r);
        return;
    }
    r.harness.push(
        "harness input: the world's collision service is link 9's NoLevel (an UNVERIFIED stand-in: this world loads \
         no level, so every level part of the touch trace is clear)"
            .to_string(),
    );
    // The set-up's game info (lane LINK-6-PICKUP-GAMEINFO, `equip_run::pickup_game_info`), when it installed one.
    if world.vm.log.iter().any(|l| l == crate::cmd::inventory::equip_run::GAME_INFO_INPUT) {
        r.harness.push(crate::cmd::inventory::equip_run::GAME_INFO_INPUT.to_string());
    }
    // Link 6's take (lane IMPL-UPDATEAMMOCOUNTS, the hub's two rulings): the ammo-definition preload's lines and the
    // at-rest ruling's set-up line, when the set-up logged them.
    let at_rest = crate::cmd::inventory::equip_run::AT_REST_INPUT;
    let ammo = |l: &String| l.starts_with("harness input: set-up preload: ") && l.ends_with(crate::cmd::inventory::equip_run::AMMO_PRELOAD_TAIL);
    r.harness.extend(world.vm.log.iter().filter(|l| ammo(l) || *l == at_rest).cloned());
    let vm = &mut world.vm;
    r.steps.push("make".into());
    // The card's preflight before the weapon is made, as link 8's driver runs it (NATIVE_LINK67_PICKUP_GUN_INIT.md
    // section 3; lane IMPL-LINK67-PICKUP-GUN-INIT): nothing written yet, so its refusal is unprefixed.
    if let Err(e) = ow_inventory::equip::card::preflight(vm) {
        r.stop = Some(classify_error("make", &e));
        ran[i] = Some(r);
        return;
    }
    // The driver keeps `VmError`s; they become text only here, for the grading (review r2 of #452).
    let log_before = vm.log.len();
    let made = crate::cmd::slice_pickup::make(vm, &setup, EQUIP_WEAPON, driver == Driver::AimedPickup);
    // The guns' init preloads log their harness lines, as link 8's do; kept on success and on failure alike.
    r.harness.extend(vm.log[log_before.min(vm.log.len())..].iter().filter(|l| l.starts_with("harness input:")).cloned());
    let p = match made {
        Ok(p) => p,
        Err(e) => {
            r.stop = Some(classify_error("make", &e));
            ran[i] = Some(r);
            return;
        }
    };
    for step in LINKS[i].steps.iter().skip(1) {
        r.steps.push((*step).to_string());
        // A step's own harness inputs (GetSaveRotation's at-rest line, lane IMPL-UPDATEAMMOCOUNTS), kept on a stop too.
        let log_before = vm.log.len();
        let out = crate::cmd::slice_pickup::step(vm, step, &p);
        r.harness.extend(vm.log[log_before.min(vm.log.len())..].iter().filter(|l| l.starts_with("harness input:")).cloned());
        if let Some(e) = out
            && step_error_typed(&mut r, step, &e, grading)
        {
            break;
        }
    }
    r.evident = r.stop.is_none() && r.error_lines.is_empty() && crate::cmd::slice_pickup::taken(vm, &p);
    ran[i] = Some(r);
}

/// Maya's Phaselock skill, its script entry and the classes the cast is given.
pub const PHASELOCK_SKILL: &str = "GD_Siren_Skills.Phaselock.ActionSkill_Phaselock";
pub const PHASELOCK_CAST: &str = "WillowGame.LiftActionSkill.OnActionSkillStarted";

fn phaselock_world(
    cooked: &Path,
    harness: &mut Vec<String>,
) -> Result<ow_world::world::World, ow_vm::error::VmError> {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
    };
    let defs = ow_defs::DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .map_err(ow_vm::error::VmError::Def)?;
    let attrs = ow_attr::AttributeSystem::standard().map_err(ow_vm::error::VmError::Attr)?;
    let vm = ow_vm::api::Vm::new(ow_object::heap::ObjectSpace::new(), defs, Rc::new(attrs), ParityMode::Off);
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .map_err(|e| script_error(e.to_string()))?;
    let packs: [&dyn ow_vm::api::NativePack; 3] = [
        &ow_vm::natives_core::Pack,
        &ow_skills::tree::Pack,
        &ow_skills::action_skill::Pack,
    ];
    let mut world = ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs)
        .map_err(|e| script_error(format!("{e:?}")))?;
    // The classes the harness needs, resolved and reported, then the pawn rules (lane IMPL-LINK9-TARGET-MIND).
    phaselock_setup(&mut world.vm, harness)?;
    let class = world.vm.find_class("Engine.WorldInfo")?;
    let info = ow_vm::interp::instantiate_from_defaults(&mut world.vm, class)?;
    world
        .actors()
        .try_borrow_mut()
        .map_err(|_| ow_vm::error::VmError::ServiceBorrowed("Actors"))?
        .place_world_info(info)?;
    // This world loads no level, so its collision service holds no geometry (lane IMPL-SPAWN-TRACE-FILTERS, decision
    // 2026-11-19-90): a harness stand-in. The spot search still filters the live actors itself.
    ow_world::placement::install_collision(&mut world.vm, Rc::new(ow_world::placement::NoLevel))?;
    // The pickup worlds' empty scene layer (lane HARNESS-LINK9-SETUP, the hub's ruling on #623): a harness input, so
    // `Actor.AttachComponent` (the phaselock light's attach) makes only its script-visible writes.
    ow_inventory::attach_mesh::add_empty_scene(&mut world.vm)?;
    harness.push(TARGET_SCENE_INPUT.to_string());
    Ok(world)
}

/// Link 9's scene-layer harness line (lane HARNESS-LINK9-SETUP).
pub const TARGET_SCENE_INPUT: &str = concat!(
    "harness input: the scene layer: the pickup worlds' empty scene (ow_inventory::attach_mesh::SceneLayer; ",
    "UNVERIFIED stand-in for the game's render, collision and physics registration)"
);

fn script_error(message: String) -> ow_vm::error::VmError {
    ow_vm::error::VmError::Script {
        function: "slice-gate".into(),
        message,
    }
}

/// The mark of a failure after state was written (the behavior walk's, which the slice commands stop on).
#[cfg(test)]
pub use ow_behavior::kernel::run::PARTIAL_WRITES;

/// The cast's three actors: Maya's pawn, her controller, the target.
type CastActors = (Option<ow_object::ids::ObjRef>, Option<ow_object::ids::ObjRef>, Option<ow_object::ids::ObjRef>);

/// The installed cast on Maya's Phaselock skill; answers the skill it ran on (the receiver of `OnActionSkillStarted`).
fn phaselock_cast(vm: &mut ow_vm::api::Vm, actors: CastActors) -> Result<ow_object::ids::ObjRef, ow_vm::error::VmError> {
    let path = ow_core::ObjectPath::parse(PHASELOCK_SKILL).map_err(|e| script_error(e.to_string()))?;
    let found = vm.defs.find(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    let skill = found
        .map_err(ow_vm::error::VmError::Def)?
        .ok_or_else(|| script_error(format!("{PHASELOCK_SKILL}: no package holds it")))?;
    let (pawn, controller, target) = actors;
    // The QueryInterface set-up (lane IMPL-GEARBOXMIND-QUERYINTERFACE): the classes and interface tables the read-only
    // `Controller` rule can reach from the cast's actors are resolved before the cast asks anything, each logged as an
    // UNVERIFIED harness input (a load is a write, so the rule itself loads nothing).
    let roots: Vec<ow_object::ids::ObjRef> = [pawn, controller, target].into_iter().flatten().collect();
    let lines = ow_combat::pawn_interface::read_only::prepare(vm, &roots)?;
    vm.log.extend(lines);
    let f = vm.find_function(PHASELOCK_CAST)?;
    vm.call(
        f,
        Some(skill),
        vec![
            ow_object::value::Value::Object(pawn),
            ow_object::value::Value::Object(controller),
            ow_object::value::Value::Object(target),
        ],
    )?;
    Ok(skill)
}

/// The cast's actors (lane LINK-9-CAST-SPAWN, decision 2026-11-19-70): one actor of each of `classes` (three: Maya's
/// pawn, her controller and the target), each spawned through the world's own spawn (`ow_world::actors::spawn`, the
/// level's `WorldInfo` as the spawner, a plain request: no template, no owner, the spawner's place), so each actor gets
/// its own copy of its components (decision 2026-11-19-44) as a spawned actor does. Nothing is made by hand. A spawn
/// the world refuses (`None`) is a typed error; a rule the spawn has not read is its own `NotYet`.
///
/// **After a committed spawn** (review r2 of #454): an actor spawned before a later failure stays registered in the
/// world, so a later `NotYet` is marked "stopped after partial writes", its kind and record kept. Other errors pass
/// unchanged; nothing is rolled back.
pub fn phaselock_actors(vm: &mut ow_vm::api::Vm, classes: &[&str]) -> Result<CastActors, ow_vm::error::VmError> {
    let spawner = ow_world::actors::actors_of(vm)?
        .try_borrow()
        .map_err(|_| ow_vm::error::VmError::ServiceBorrowed("Actors"))?
        .world_info()
        .ok_or_else(|| script_error("the cast's world has no WorldInfo to spawn from".into()))?;
    let [pawn_class, controller_class, target_class] = classes else {
        return Err(script_error(format!("the cast needs three actor classes, not {}", classes.len())));
    };
    let mut committed = 0usize;
    let mut spawn = |class: &str| -> Result<Option<ow_object::ids::ObjRef>, ow_vm::error::VmError> {
        let made = vm
            .find_class(class)
            .and_then(|c| ow_world::actors::spawn(vm, spawner, &ow_world::actors::SpawnRequest::new(Some(c))))
            .and_then(|made| made.ok_or_else(|| script_error(format!("the world's spawn refused a {class}"))));
        match made {
            Ok(actor) => {
                committed += 1;
                Ok(Some(actor))
            }
            Err(ow_vm::error::VmError::NotYet(n)) if committed > 0 => Err(n.after_partial_writes().into()),
            Err(e) => Err(e),
        }
    };
    let pawn = spawn(pawn_class)?;
    let controller = spawn(controller_class)?;
    let target = spawn(target_class)?;
    if let Some(t) = target {
        // Three actors are committed by now: a NotYet of the mind is marked, as a spawn's after the first.
        target_mind(vm, spawner, t).map_err(|e| match e {
            ow_vm::error::VmError::NotYet(n) => n.after_partial_writes().into(),
            other => other,
        })?;
    }
    Ok((pawn, controller, target))
}

/// The mind class the target gets when its own `ControllerClass` names none (lane IMPL-LINK9-TARGET-MIND).
pub const TARGET_MIND_FALLBACK: &str = "WillowGame.WillowMind";

/// The text that starts link 9's harness line for the target's mind.
pub const TARGET_MIND_INPUT: &str = "harness input: the target's mind";

/// The interface a mind's flag store answers, resolved at link 9's set-up ([`phaselock_setup`]): the flag evaluator
/// checks a store by its class tables, which only hold a class once loaded.
pub const TARGET_FLAG_PROVIDER: &str = "GearboxFramework.IFlagProvider";

/// The interface the flag evaluator checks a rule-engine owner against, resolved at link 9's set-up for the same reason
/// (lane HARNESS-LINK9-SETUP): main's link 9 stopped at its class not being loaded.
pub const TARGET_RULE_ENGINE_OWNER: &str = "GearboxFramework.IRuleEngineOwner";

/// The text that starts each of link 9's set-up class lines ([`preload_class`]).
pub const TARGET_FLAG_PROVIDER_INPUT: &str = "harness input: set-up class";

/// Link 9's target mind (lane IMPL-LINK9-TARGET-MIND, a **harness input, UNVERIFIED**): in the game the population
/// system spawns an enemy with its AI controller; the harness stands in for that, so `CanLiftTargetIf` reads its flag
/// conditions through a mind as for a real enemy (NATIVE_FLAG_EXPRESSION_EVALUATE.md section 5).
/// - A target whose class declares no `Controller` gets none (not a pawn). A target that is already possessed keeps
///   its mind: a set `Controller`, else a set `MyWillowMind`, which `adopt` reads as possessed (review r1 of #579).
/// - The mind's class is the target's own `ControllerClass` when it names an `Engine.Controller` subclass (a
///   `WillowAIPawn`'s class default names `WillowGame.WillowMind`), else [`TARGET_MIND_FALLBACK`].
/// - **All four links are checked before anything is written** (review r1 of #579): the target's `Controller` and,
///   when declared, `MyWillowMind` must hold the mind's class; the mind's `Pawn` and, when declared, `MyWillowPawn`
///   must hold the target's class. A slot of another kind or a narrower class refuses the target with nothing spawned
///   and nothing linked, so a target never points at a mind whose `Pawn` was not set.
/// - Then the mind is spawned by the world's own spawn (so its components are its own) and the links are written
///   together, as the port already possesses an AI pawn (`ow_combat::hit::adopt`). The script's `Possess` is not run
///   (as in `adopt`).
///
/// The line is logged before the class is read. The fallback class and the classes the mind's flags need are resolved
/// by link 9's set-up ([`phaselock_setup`]); here the fallback is only read, and one set-up did not resolve is a
/// refusal with nothing loaded (review r2 of #579).
pub fn target_mind(
    vm: &mut ow_vm::api::Vm,
    spawner: ow_object::ids::ObjRef,
    target: ow_object::ids::ObjRef,
) -> Result<Option<ow_object::ids::ObjRef>, ow_vm::error::VmError> {
    use ow_object::ids::PropAddr;
    use ow_object::value::Value;
    let class = vm.space.heap.get(target)?.class;
    if vm.space.prop(class, "Controller").is_none() {
        return Ok(None);
    }
    for existing in ["Controller", "MyWillowMind"] {
        if let Some(p) = vm.space.prop(class, existing)
            && let Value::Object(Some(mind)) = vm.space.read(target, PropAddr::scalar(p))?
        {
            return Ok(Some(*mind));
        }
    }
    let named = match vm.space.prop(class, "ControllerClass").map(|p| vm.space.read(target, PropAddr::scalar(p))) {
        Some(Ok(Value::Class(Some(c)))) => Some(*c),
        _ => None,
    };
    let controller = vm.space.classes.by_path("Engine.Controller");
    let chosen = named.filter(|c| controller.is_some_and(|b| vm.space.classes.is_child_of(*c, b)));
    let path = match chosen {
        Some(c) => vm.space.classes.get_layout(c).map(|l| l.path().as_str().to_string()).unwrap_or_default(),
        None => TARGET_MIND_FALLBACK.to_string(),
    };
    vm.log.push(format!(
        "{TARGET_MIND_INPUT} {path} (spawned and linked as the port's adopt links an AI pawn; UNVERIFIED stand-in for \
         the population spawn's AI controller)"
    ));
    // The fallback was resolved at link 9's set-up; it is read here, never loaded (review r2 of #579): a lazy load
    // would write before the slot checks below can refuse.
    let mind_class = match chosen {
        Some(c) => c,
        None => match vm.space.classes.by_path(TARGET_MIND_FALLBACK) {
            Some(c) if class_ready(vm, c) => c,
            _ => {
                return Err(script_error(format!(
                    "the target's mind {TARGET_MIND_FALLBACK} was not resolved at link 9's set-up (nothing loaded, \
                     spawned or linked)"
                )));
            }
        },
    };
    // Every destination first: (holder class, field, required, the class the field must hold, on the target).
    let plan = [
        (class, "Controller", true, mind_class, true),
        (class, "MyWillowMind", false, mind_class, true),
        (mind_class, "Pawn", true, class, false),
        (mind_class, "MyWillowPawn", false, class, false),
    ];
    let mut slots = Vec::with_capacity(plan.len());
    for (holder, field, required, value_class, on_target) in plan {
        let Some(p) = vm.space.prop(holder, field) else {
            if required {
                return Err(script_error(format!("the target's mind {path}: {field} is not declared")));
            }
            continue;
        };
        let fits = match vm.space.classes.decl(p).map(|d| &d.kind) {
            Some(ow_object::layout::PropKind::Object { class: want }) => vm.space.classes.is_child_of(value_class, *want),
            _ => false,
        };
        if !fits {
            let held = vm.space.classes.get_layout(value_class).map(|l| l.path().to_string()).unwrap_or_default();
            return Err(script_error(format!(
                "the target's mind {path}: {field} cannot hold a {held} (nothing spawned or linked)"
            )));
        }
        slots.push((on_target, p));
    }
    let Some(mind) = ow_world::actors::spawn(vm, spawner, &ow_world::actors::SpawnRequest::new(Some(mind_class)))? else {
        return Err(script_error(format!("the world's spawn refused the target's mind {path}")));
    };
    for (on_target, p) in slots {
        let (o, v) = if on_target { (target, mind) } else { (mind, target) };
        vm.space.write(o, PropAddr::scalar(p), Value::Object(Some(v))).map_err(ow_vm::error::VmError::from)?;
    }
    Ok(Some(mind))
}

/// The classes link 9's set-up resolves ([`phaselock_setup`], reviews r1 and r2 of #579): the target mind's fallback
/// class, the mind's flag store interface and the rule-engine owner interface (lane HARNESS-LINK9-SETUP),
/// the two interfaces the pawn rules compare against, and every class whose answer `ow_combat::pawn_interface`
/// registers. Resolving them at set-up keeps the cast from loading any of them lazily (a load is a write).
pub fn phaselock_setup_classes() -> Vec<&'static str> {
    use ow_combat::pawn_interface as pi;
    let mut classes = vec![
        TARGET_MIND_FALLBACK,
        TARGET_FLAG_PROVIDER,
        TARGET_RULE_ENGINE_OWNER,
        pi::AI_INTERFACE,
        pi::PLAYER_MASTER_INTERFACE,
    ];
    classes.push(pi::PAWN);
    classes.extend([pi::WILLOW_AI_PAWN, pi::WILLOW_VEHICLE]);
    classes.extend(pi::OPEN_ANSWERS.iter().map(|(path, _, _)| *path));
    classes
}

/// Whether class `c` is ready to use with no load: its layout is defined and its function and state code is loaded.
pub fn class_ready(vm: &ow_vm::api::Vm, c: ow_object::ids::ClassId) -> bool {
    vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()) && vm.class_code_loaded(c)
}

/// One class resolved at link 9's set-up, with its harness line (review r2 of #579). The full class lookup always
/// runs, so a class whose layout another load defined still gets its code loaded here, not later. The line names
/// the layout and the code separately, each "ready" (it was before) or "loaded at set-up" (a write). A class absent
/// from the packages loads nothing. A failed lookup is logged with the state it found. The line is made whatever the
/// outcome.
pub fn preload_class(vm: &mut ow_vm::api::Vm, path: &str) -> (String, Result<(), ow_vm::error::VmError>) {
    let before = vm.space.classes.by_path(path);
    let layout_before = before.is_some_and(|c| vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()));
    let code_before = before.is_some_and(|c| vm.class_code_loaded(c));
    let state = |ready: bool| if ready { "ready" } else { "loaded at set-up" };
    match vm.find_class_if_present(path) {
        Ok(Some(c)) => {
            let code_after = vm.class_code_loaded(c);
            let code = match (code_before, code_after) {
                (true, _) => "ready",
                (false, true) => "loaded at set-up",
                // A class the store has no code key for (made at run time) has no code to load.
                (false, false) => "none to load",
            };
            let wrote = !layout_before || code == "loaded at set-up";
            let note = if wrote {
                " (a write; UNVERIFIED stand-in for the game having it loaded)"
            } else {
                ", nothing loaded"
            };
            (
                format!("{TARGET_FLAG_PROVIDER_INPUT} {path}: layout {}, code {code}{note}", state(layout_before)),
                Ok(()),
            )
        }
        Ok(None) => (format!("{TARGET_FLAG_PROVIDER_INPUT} {path}: not in the packages, nothing loaded"), Ok(())),
        Err(e) => (
            format!(
                "{TARGET_FLAG_PROVIDER_INPUT} {path}: the load failed (layout {}, code {} before): {}",
                if layout_before { "ready" } else { "not ready" },
                if code_before { "ready" } else { "not ready" },
                ow_combat::hit::access::error_text(&e)
            ),
            Err(e),
        ),
    }
}

/// The definition subobjects link 9's cast reads, loaded at its set-up (lane LINK9-RESOLVER-PRELOAD, a **harness
/// input, UNVERIFIED stand-in**): in the game the skill's packages are loaded whole, so a definition's subobjects are
/// resident with it; the port's store leaves them as handles, and the cast's preflight refuses an unloaded one (a
/// load is a write). The phaselock attribute's context resolver: `ResolveContext`'s chain entry (its R2).
pub const PHASELOCK_SETUP_OBJECTS: [&str; 1] = ["GD_Siren_Skills.Attributes.IsPhaselocked.PawnAttributeContextResolver_0"];

/// The text that starts each of link 9's set-up object lines ([`preload_object`]).
pub const SETUP_OBJECT_INPUT: &str = "harness input: set-up object";

/// One object loaded at link 9's set-up, with its harness line (lane LINK9-RESOLVER-PRELOAD). The store's lookup
/// loads what it finds; the line says whether that was a load ("loaded at set-up", a write, an UNVERIFIED stand-in for
/// the package's load with its definition) or the object was resident already ("ready, nothing loaded"), by the
/// store's load counter. A path no package holds loads nothing; a failed load keeps its error. The line is made
/// whatever the outcome.
pub fn preload_object(vm: &mut ow_vm::api::Vm, path: &str) -> (String, Result<(), ow_vm::error::VmError>) {
    let parsed = match ow_core::ObjectPath::parse(path) {
        Ok(p) => p,
        Err(e) => return (format!("{SETUP_OBJECT_INPUT} {path}: not a path ({e})"), Err(script_error(e.to_string()))),
    };
    let loads = vm.defs.load_transitions();
    let found = vm.defs.find(&mut vm.space, &parsed);
    vm.log.extend(vm.defs.take_log());
    let object = match found {
        Ok(Some(o)) => o,
        Ok(None) => return (format!("{SETUP_OBJECT_INPUT} {path}: not in the packages, nothing loaded"), Ok(())),
        Err(e) => {
            return (format!("{SETUP_OBJECT_INPUT} {path}: the load failed ({e})"), Err(ow_vm::error::VmError::Def(e)));
        }
    };
    let loaded = vm.defs.ensure_loaded(&mut vm.space, object);
    vm.log.extend(vm.defs.take_log());
    match loaded {
        Err(e) => (format!("{SETUP_OBJECT_INPUT} {path}: the load failed ({e})"), Err(ow_vm::error::VmError::Def(e))),
        Ok(()) if vm.defs.load_transitions() != loads => (
            format!("{SETUP_OBJECT_INPUT} {path}: loaded at set-up (a write; UNVERIFIED stand-in for the package's load with its definition)"),
            Ok(()),
        ),
        Ok(()) => (format!("{SETUP_OBJECT_INPUT} {path}: ready, nothing loaded"), Ok(())),
    }
}

/// The text that starts each set-up interface check's line ([`resolve_interface`]).
pub const SETUP_INTERFACE_INPUT: &str = "harness input: set-up interface check";

/// Resolves, at set-up, the interface entries `implements(class, iface)` reads on `class`'s chain, so a later
/// read-only check answers without a load (lane HARNESS-GEARLIKENESS-SETUP: the body composition's hook 1,
/// NATIVE_BODYCOMP_RENDER_HOOKS.md). Both classes must be in the packages; each is loaded first if it is not, and
/// the line then says the lookup loaded (a write). That is measured as [`preload_class`] measures it: each class's
/// layout and code state before and after the lookup.
///
/// The line says where the answer came from:
/// - **read:** the chain's bodies decoded, so the answer is the packages' own interface lists, each entry resolved to
///   its class. Resolving them at set-up, rather than when the game first asks, is the harness input (a write when an
///   entry was not resolved yet);
/// - **UNVERIFIED stand-in:** a chain whose bodies did not all decode answers by the structural fallback (as
///   `implements` does), which is not the game's interface list;
/// - **unresolved:** `implements` keeps an entry whose lookup failed as "resolved to nothing", which would read as a
///   settled answer. So a decoded answer is checked against the strict `implements_declared` (review r1 of #665,
///   MED 1): when the strict reading has no answer (an entry or a class chain did not resolve), fails, or disagrees,
///   the line says so and the set-up answers `NotYet` (`NotRead`), not the packages' answer.
///
/// A class or interface the packages do not hold is reported and nothing is resolved.
pub fn resolve_interface(
    vm: &mut ow_vm::api::Vm,
    class: &str,
    iface: &str,
) -> (String, Result<(), ow_vm::error::VmError>) {
    let found = |vm: &mut ow_vm::api::Vm, path: &str| vm.find_class_if_present(path);
    // Review r2 of #665, N1: a class lookup does not move the store's handle counter, so the layout and the code of
    // each class are compared before and after it, as `preload_class` does.
    let ready = |vm: &ow_vm::api::Vm, path: &str| {
        vm.space.classes.by_path(path).map(|c| {
            (vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()), vm.class_code_loaded(c))
        })
    };
    let before = [ready(vm, class), ready(vm, iface)];
    let lookup = (found(vm, class), found(vm, iface));
    let after = [ready(vm, class), ready(vm, iface)];
    let loaded = if before != after { " (the class lookup loaded code: a write)" } else { "" };
    let (c, i) = match lookup {
        (Ok(Some(c)), Ok(Some(i))) => (c, i),
        (Err(e), _) | (_, Err(e)) => {
            let text = ow_combat::hit::access::error_text(&e);
            return (format!("{SETUP_INTERFACE_INPUT} {class} implements {iface}{loaded}: the load failed: {text}"), Err(e));
        }
        _ => {
            let line =
                format!("{SETUP_INTERFACE_INPUT} {class} implements {iface}{loaded}: not in the packages, nothing resolved");
            return (line, Ok(()));
        }
    };
    let (ck, ik, decoded, before) = {
        let code = vm.defs.code();
        match (code.key_of(c), code.key_of(i)) {
            (Some(ck), Some(ik)) => (
                ck,
                ik,
                code.class(ik).body_decoded() && code.super_chain(ck).all(|k| code.class(k).body_decoded()),
                code.implements_resolved(ck, ik),
            ),
            _ => {
                return (
                    format!("{SETUP_INTERFACE_INPUT} {class} implements {iface}{loaded}: no code key, nothing resolved"),
                    Ok(()),
                );
            }
        }
    };
    let answer = vm.implements(c, i);
    if decoded {
        let strict = {
            let (code, store) = vm.defs.code_mut();
            code.implements_declared(store, ck, ik)
        };
        if !matches!(strict, Ok(Some(b)) if b == answer) {
            let why = match &strict {
                Err(e) => format!("an entry's lookup failed ({e})"),
                Ok(None) => "the strict reading has no answer (an entry or a class chain did not resolve)".to_string(),
                Ok(Some(_)) => "the strict reading disagrees".to_string(),
            };
            let line = format!(
                "{SETUP_INTERFACE_INPUT} {class} implements {iface}{loaded}: unresolved, {why}; no answer from the \
                 packages' interface lists"
            );
            let what = format!("the set-up interface check {class} implements {iface}: {why}");
            return (line, Err(NotYet::not_read(what, "NATIVE_BODYCOMP_RENDER_HOOKS.md").into()));
        }
    }
    let yes_no = if answer { "yes" } else { "no" };
    let source = if !decoded {
        "by the structural fallback (the class bodies did not all decode; UNVERIFIED stand-in for the packages' \
         interface lists)"
    } else if before.is_some() {
        "read from the packages' interface lists, already resolved, nothing resolved"
    } else {
        "read from the packages' interface lists; their entries resolved at set-up (a write; UNVERIFIED stand-in for \
         when the game resolves them)"
    };
    let line = format!("{SETUP_INTERFACE_INPUT} {class} implements {iface}{loaded}: {yes_no}, {source}");
    (line, Ok(()))
}

/// Link 9's set-up (review r1 of #579): each of [`phaselock_setup_classes`] is resolved and its line pushed onto
/// `harness`, a failure included (the first failure stops the set-up); then each of [`PHASELOCK_SETUP_OBJECTS`]
/// ([`preload_object`], lane LINK9-RESOLVER-PRELOAD), the same way. Then `Pawn`'s `QueryInterface` rules are
/// registered (decision 2026-11-19-62), which find those classes loaded already: the target's flags reach its mind
/// through them; without them a pawn takes the plain answer. They come through `ow_combat::holds::register`, which
/// registers them first and then the AI holds' natives, `GearboxMind.GetAIComponent` among them (lane
/// LINK9-GETAICOMPONENT-BINDING): the port's own Off registration, the one the combat worlds make, not a harness
/// stand-in. Off only (it binds nothing under Reference); a key bound first keeps its binding. Then the combat
/// pack's pawn natives (`ow_combat::hit::register_pawn_natives`, lane LINK9-PAWN-NATIVES: `IsStaggered` and
/// `DropCombatPerch` among them), the same Off registration the combat worlds make.
pub fn phaselock_setup(vm: &mut ow_vm::api::Vm, harness: &mut Vec<String>) -> Result<(), ow_vm::error::VmError> {
    for path in phaselock_setup_classes() {
        let (line, outcome) = preload_class(vm, path);
        harness.push(line);
        outcome?;
    }
    for path in PHASELOCK_SETUP_OBJECTS {
        let (line, outcome) = preload_object(vm, path);
        harness.push(line);
        outcome?;
    }
    ow_combat::holds::register(vm)?;
    ow_combat::hit::register_pawn_natives(vm)
}

/// The ids in `after` that `before` does not account for, as multisets (a repeated kill id counts each time).
pub fn new_ids<'a>(before: &[String], after: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut left: Vec<&str> = before.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    for id in after {
        match left.iter().position(|b| *b == id) {
            Some(i) => {
                left.swap_remove(i);
            }
            None => out.push(id.to_string()),
        }
    }
    out
}

/// Whether the rows link `ran` added hold one whose id starts with `prefix`.
pub fn evidence_in(ran: Option<&Ran>, prefix: &str) -> bool {
    ran.is_some_and(|r| r.new_rows.iter().any(|k| k.starts_with(prefix)))
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

/// A row's steps `(passed, total)` (lane SLICE-GATE-STEP-SHARE; a report only, not part of the grading). `total` is
/// the length of the link's own step list in [`LINKS`]. `passed` counts the row's steps that did not stop it:
/// - strict grading stops at the first stop, so every step but the last passed when the row's stop is that step;
/// - under past-errors grading, a step whose name is the stop's or an error line's did not pass. A repeated step name
///   (`tick:1`) that failed once counts as failed each time, so this can undercount there.
///
/// A blocked row ran no step, so 0 passed.
pub fn step_counts(row: &Row) -> (usize, usize) {
    let total = LINKS
        .iter()
        .find(|l| l.name == row.link)
        .map_or(row.steps.len(), |l| l.steps.len());
    let stop = row.stop.as_ref().map(|s| s.step.as_str());
    let passed = match row.grading {
        Grading::Strict => {
            let stopped_last = stop.is_some() && row.steps.last().map(String::as_str) == stop;
            row.steps.len() - usize::from(stopped_last)
        }
        Grading::PastErrors => row
            .steps
            .iter()
            .filter(|s| Some(s.as_str()) != stop && !row.error_lines.iter().any(|(e, _)| e == *s))
            .count(),
    };
    (passed.min(total), total)
}

/// A row's share of the gate's steps: passed over total, or, for a link with no steps of its own, 1 when `ok` and 0
/// otherwise (lane SLICE-GATE-STEP-SHARE).
pub fn step_share(row: &Row) -> f64 {
    match step_counts(row) {
        (_, 0) => f64::from(u8::from(row.result == Outcome::Ok)),
        (passed, total) => passed as f64 / total as f64,
    }
}

/// The report lines the gate prints to stderr after its rows (lane SLICE-GATE-STEP-SHARE): one per row,
/// `link <n> <name>: <result>, steps <passed>/<total>`, then `links ok <P>/<N>` and
/// `step share X = <sum>/<N> = <pct>%`, X being the mean of [`step_share`] over the rows. Stdout keeps only the JSON
/// rows, which `tools/check_slice_run_off.py` reads line by line; the grading and the exit code do not change.
pub fn step_report(rows: &[Row]) -> Vec<String> {
    let mut lines: Vec<String> = rows
        .iter()
        .map(|r| {
            let (passed, total) = step_counts(r);
            format!("link {} {}: {}, steps {passed}/{total}", r.n, r.link, r.result.name())
        })
        .collect();
    let n = rows.len();
    let ok = rows.iter().filter(|r| r.result == Outcome::Ok).count();
    let sum: f64 = rows.iter().map(step_share).sum();
    let pct = if n == 0 { 0.0 } else { 100.0 * sum / n as f64 };
    lines.push(format!("links ok {ok}/{n}"));
    let (sum, pct) = (ow_core::fmt::fmt_f(sum, 3), ow_core::fmt::fmt_f(pct, 1));
    lines.push(format!("step share X = {sum}/{n} = {pct}%"));
    lines
}

/// The command's arguments: the cooked directory, the grading and the `--map <name>` option, which comes last (module
/// documentation); `None` for any other shape.
pub(crate) fn parse_args(args: &[String]) -> Option<(String, Grading, Option<String>)> {
    let (args, map) = match args {
        [rest @ .., opt, name] if opt == "--map" => (rest, Some(name.clone())),
        _ => (args, None),
    };
    match args {
        [flag, opt, dir] if flag == "--slice-gate" && opt == "--cooked" => Some((dir.clone(), Grading::Strict, map)),
        [flag, opt, dir, past] if flag == "--slice-gate" && opt == "--cooked" && past == "--past-errors" => {
            Some((dir.clone(), Grading::PastErrors, map))
        }
        _ => None,
    }
}

pub fn run(_command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    if mode != ParityMode::Off {
        eprintln!("ow-package: --slice-gate runs in Off only (the slice gate counts only there)");
        return ExitCode::from(1);
    }
    let Some((cooked, grading, map)) = parse_args(args) else {
        eprintln!("ow-package: {USAGE}");
        return ExitCode::from(1);
    };
    let rows = match on_vm_stack(move || gate(&cooked, grading, map.as_deref())) {
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
    for line in step_report(&rows) {
        eprintln!("{line}");
    }
    ExitCode::from(exit_code(&rows))
}

pub mod fight_area;

#[cfg(test)]
#[path = "slice_gate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "slice_gate_driver_tests.rs"]
mod driver_tests;
#[cfg(test)]
#[path = "slice_gate_preload_tests.rs"]
mod preload_tests;
#[cfg(test)]
#[path = "slice_gate_shadow_tests.rs"]
mod shadow_tests;
