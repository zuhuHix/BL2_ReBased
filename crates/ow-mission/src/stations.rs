//! Owner: lane W7-WORLDLOGIC. Travel stations (fast travel, level travel and the New-U respawn stations): which ones a
//! map places and what the script needs of them. AI-assisted (Claude Code). Decision 2026-11-16-60.
//!
//! **Every rule is UNVERIFIED in the running game unless a line says how it was confirmed.** Records:
//! SLICE_WORLD_PLACEMENT.md section 1e (the stations of Sanctuary, the selection order, the 2026-11-12 live
//! observations), COMBAT_DOWN_RESPAWN.md (the respawn timeline and fee), and the installed script (`TravelStation`,
//! `FastTravelStation`, `ResurrectTravelStation`, `TeleporterDestination`, `WillowPlayerPawn.GetBestPlayerPlacementPoint`,
//! `WillowPlayerController.Behavior_RegisterStation`, read with `research/script_disasm.py`).
//!
//! **Registration and selection are script (architecture rule R2): the VM runs them.** Read from the script, in our
//! own words:
//!
//! | Step | What the script does |
//! |---|---|
//! | registering a station (`Behavior_RegisterStation`, server only) | activates it (`SetStationActivatedState(true)`), tells every player controller (`ClientNotifySomeoneTouchedStation`, then `RegisterStationForPlayer`), then `MissionTracker.FastTravelStationRegistered` |
//! | activating (`SetStationActivatedState`), stations with an active state only | on a change, or when told to change regardless: becoming active replaces the previous checkpoint (below), runs the custom event `OnOutpostActivate` and marks it active; becoming inactive runs `OnOutpostDeactivate`, except on the initial activation |
//! | replacing the checkpoint (`ReplacePreviouslyActivatedStation`) | the replication info's `ActiveRespawnCheckpointTeleportActor` becomes this station's `TeleportDest`; the station that owned the old checkpoint, if another, is deactivated |
//! | `CanResurrectHere(bLevelTravel)` | a plain travel station: never; a fast-travel station: when it should be active, is active, or its definition is `bInitiallyActive`; a resurrect (New-U) station: unless the respawn is a level travel; a level-travel station: never |
//! | the respawn point (`GetBestPlayerPlacementPoint`) | the checkpoint, when set; else the first station (actor order) that is active or should be; else the nearest station that can resurrect here; else the nearest other station; each gives its `TeleportDest` |
//! | the exit point (`TeleporterDestination.GetNextExitPoint`) | round robin from `ExitPointsCounter` over the exit points: a walking actor skips the vehicle exit points on the first pass and takes only those on a second pass, and an exit point with a pawn inside the actor's collision radius (32 uu without one) is skipped; the counter moves past the point taken; when none qualifies, the point at the counter is taken and the counter moves on by one |
//!
//! Observed live (SLICE_WORLD_PLACEMENT.md 1e, session 1 of 2026-11-12, two loads of one save): the checkpoint was
//! set from the load on, the respawn went to it, and the first respawn after the load took the second exit point. Only
//! one station was active and it was also the nearest, so the selection order above is not told apart in the game.
//!
//! **Not read, so the VM path stops at them:** the natives `TravelStation.GetTravelStationDefinition` (and the
//! fast-travel and level-travel overrides), `FastTravelStation.GetIsRegistered`, `TravelStation.TouchDenied`,
//! `IsFastTravelEnabled`, `FindTravelStation`, `StationTeleporterDestination.UpdateExitPointLocations` /
//! `UpdateExitPointHeights`, and what activates a station in play (touch radius against interaction; the caller of
//! `SetStationActivatedState` other than registration was not traced).
//!
//! **What this module owns:** reading the placed stations of a map ([`stations`]), which a world spawns before the
//! script can register or choose them.

use crate::vendors::MapView;
use ow_items::registry::Pv;

#[cfg(test)]
mod tests;

/// The record of the stations.
pub const RECORD: &str = "docs/verification/SLICE_WORLD_PLACEMENT.md section 1e";

/// The kind of a travel station, by its class.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StationKind {
    /// `FastTravelStation`.
    FastTravel,
    /// `LevelTravelStation`.
    LevelTravel,
    /// `ResurrectTravelStation` (a New-U station).
    Resurrect,
}

impl StationKind {
    /// The kind of a class (any package prefix), `None` for a class that is no travel station read here.
    pub fn of_class(class: &str) -> Option<StationKind> {
        match class.rsplit('.').next().unwrap_or(class) {
            "FastTravelStation" => Some(StationKind::FastTravel),
            "LevelTravelStation" => Some(StationKind::LevelTravel),
            "ResurrectTravelStation" => Some(StationKind::Resurrect),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            StationKind::FastTravel => "fast_travel",
            StationKind::LevelTravel => "level_travel",
            StationKind::Resurrect => "resurrect",
        }
    }
}

/// One placed station as the packages describe it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    /// The level package's file stem.
    pub level: String,
    pub path: String,
    pub kind: StationKind,
    /// The station's definition (`TravelDefinition`, else `StationDefinition`, else `InteractiveObjectDefinition`).
    pub definition: Option<String>,
    /// A fast-travel definition's `bInitiallyActive` (untagged reads false: the class default is not read here).
    pub initially_active: Option<bool>,
    /// `TeleportDest`.
    pub teleport_dest: Option<String>,
    /// The number of `ExitPoints` of the destination.
    pub exit_points: usize,
    /// `StationTouchRadius` and `StationTouchHeight` when tagged.
    pub touch: Option<(f64, f64)>,
}

fn text(v: &Pv) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty() && *s != "None").map(str::to_string)
}

/// The placed travel stations of every level of `view`, in level then export order.
pub fn stations(view: &MapView) -> Vec<Station> {
    let mut out = Vec::new();
    for level in &view.levels {
        for (path, class) in &level.exports {
            if path.matches('.').count() != 2 || !path.starts_with("TheWorld.PersistentLevel.") {
                continue;
            }
            let Some(kind) = StationKind::of_class(class) else {
                continue;
            };
            let props = level.props(path).unwrap_or(Pv::Null);
            let definition = ["TravelDefinition", "StationDefinition", "InteractiveObjectDefinition"]
                .iter()
                .find_map(|k| text(props.at(k)));
            let initially_active = match (kind, &definition) {
                (StationKind::FastTravel, Some(d)) => level.props(d).map(|p| p.at("bInitiallyActive").truthy()),
                _ => None,
            };
            let teleport_dest = text(props.at("TeleportDest"));
            let exit_points = teleport_dest
                .as_deref()
                .and_then(|d| level.props(d))
                .map_or(0, |p| p.at("ExitPoints").items().len());
            let touch = match (props.at("StationTouchRadius").number(), props.at("StationTouchHeight").number()) {
                (Some(r), Some(h)) => Some((r, h)),
                (Some(r), None) => Some((r, 0.0)),
                (None, Some(h)) => Some((0.0, h)),
                (None, None) => None,
            };
            out.push(Station {
                level: level.name.clone(),
                path: path.clone(),
                kind,
                definition,
                initially_active,
                teleport_dest,
                exit_points,
                touch,
            });
        }
    }
    out
}
