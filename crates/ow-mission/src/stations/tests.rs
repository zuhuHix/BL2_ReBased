//! Tests of the station and interactive-object census on an invented map (AGENTS.md rule 9).

use ow_core::ParityMode;

use super::*;
use crate::interactive::{Origin, census};
use crate::vendors::tests::{Map, Syn, V, map_with, open_map};

const LEVEL: &str = "TheWorld.PersistentLevel.";

/// The invented map of the vendors tests with stations, a placed switch and a treasure population added.
fn map(mode: ParityMode) -> Map {
    let mut s = Syn::default();
    s.add("TheWorld.PersistentLevel.FastTravelStation_0", "WillowGame.FastTravelStation", vec![
        ("TravelDefinition", V::Ref("Defs.Station_Toy")),
        ("TeleportDest", V::Ref("TheWorld.PersistentLevel.StationTeleporterDestination_0")),
        ("InteractiveObjectDefinition", V::Ref("Defs.IO_Station")),
    ]);
    s.add(
        "TheWorld.PersistentLevel.StationTeleporterDestination_0",
        "WillowGame.StationTeleporterDestination",
        vec![(
            "ExitPoints",
            V::Array(vec![
                V::Ref("TheWorld.PersistentLevel.StationTeleporterExitPoint_0"),
                V::Ref("TheWorld.PersistentLevel.StationTeleporterExitPoint_1"),
            ]),
        )],
    );
    // A destination that tags no ExitPoints.
    s.add(
        "TheWorld.PersistentLevel.StationTeleporterDestination_1",
        "WillowGame.StationTeleporterDestination",
        vec![],
    );
    for e in ["StationTeleporterExitPoint_0", "StationTeleporterExitPoint_1"] {
        let path: &'static str = Box::leak(format!("{LEVEL}{e}").into_boxed_str());
        s.add(path, "WillowGame.StationTeleporterExitPoint", vec![]);
    }
    s.add("TheWorld.PersistentLevel.ResurrectTravelStation_0", "WillowGame.ResurrectTravelStation", vec![
        ("TeleportDest", V::Ref("TheWorld.PersistentLevel.StationTeleporterDestination_1")),
        ("StationTouchRadius", V::Float(500.0)),
        ("StationTouchHeight", V::Float(24.0)),
    ]);
    s.add("Defs.Station_Toy", "WillowGame.FastTravelStationDefinition", vec![("bInitiallyActive", V::Bool(true))]);
    s.add("TheWorld.PersistentLevel.WillowInteractiveObject_0", "WillowGame.WillowInteractiveObject", vec![(
        "InteractiveObjectDefinition",
        V::Ref("Defs.IO_Switch"),
    )]);
    s.add("Defs.IO_Switch", "WillowGame.InteractiveObjectDefinition", vec![]);
    s.add("Defs.IO_Switch.BehaviorProviderDefinition_0", "GearboxFramework.BehaviorProviderDefinition", vec![]);
    s.add("Defs.IO_Switch.Behavior_ChangeUsabilityCost_0", "WillowGame.Behavior_ChangeUsabilityCost", vec![]);
    s.add("GD_Population_Treasure.Pop_Box", "WillowGame.WillowPopulationDefinition", vec![]);
    s.add("GD_Population_Treasure.Pop_Box.Factory", "WillowGame.PopulationFactoryInteractiveObject", vec![(
        "ObjectBalanceDefinition",
        V::Ref("Bal.Box"),
    )]);
    s.add("Bal.Box", "WillowGame.InteractiveObjectBalanceDefinition", vec![(
        "DefaultInteractiveObject",
        V::Ref("Defs.IO_Box"),
    )]);
    open_map(mode, map_with(s))
}

#[test]
fn the_placed_stations_are_read_with_their_definition_and_exit_points() {
    let m = map(ParityMode::Off);
    let (found, skipped) = stations(&m.view);
    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!(found.len(), 2);
    let fast = &found[0];
    assert_eq!(fast.kind, StationKind::FastTravel);
    assert_eq!(fast.level, "Toy_P");
    assert_eq!(fast.definition.as_deref(), Some("Defs.Station_Toy"));
    assert_eq!(fast.initially_active, Some(true));
    assert_eq!(fast.teleport_dest.as_deref(), Some("TheWorld.PersistentLevel.StationTeleporterDestination_0"));
    assert_eq!(fast.exit_points, Some(2));
    assert_eq!((fast.touch_radius, fast.touch_height), (None, None));
    let newu = &found[1];
    assert_eq!(newu.kind, StationKind::Resurrect);
    assert_eq!(newu.initially_active, None);
    assert_eq!(newu.teleport_dest.as_deref(), Some("TheWorld.PersistentLevel.StationTeleporterDestination_1"));
    assert_eq!(newu.exit_points, None, "ExitPoints untagged: no count, no default");
    assert_eq!((newu.touch_radius, newu.touch_height), (Some(500.0), Some(24.0)));
}

#[test]
fn only_the_three_station_classes_are_stations() {
    assert_eq!(StationKind::of_class("WillowGame.LevelTravelStation"), Some(StationKind::LevelTravel));
    assert_eq!(StationKind::of_class("TravelStation"), None);
    assert_eq!(StationKind::of_class("WillowGame.StationTeleporterDestination"), None);
}

#[test]
fn the_census_lists_placed_objects_then_shop_and_treasure_points() {
    let m = map(ParityMode::Off);
    let (objects, skipped) = census(&m.view);
    assert!(skipped.is_empty(), "{skipped:?}");
    let rows: Vec<(&str, Origin, &str, Option<&str>, bool, bool)> = objects
        .iter()
        .map(|o| {
            let name = o.path.rsplit('.').next().unwrap();
            (name, o.origin, o.class.as_str(), o.definition.as_deref(), o.has_provider, o.has_cost_behavior)
        })
        .collect();
    assert_eq!(rows, [
        ("FastTravelStation_0", Origin::Placed, "FastTravelStation", Some("Defs.IO_Station"), false, false),
        ("ResurrectTravelStation_0", Origin::Placed, "ResurrectTravelStation", None, false, false),
        ("WillowInteractiveObject_0", Origin::Placed, "WillowInteractiveObject", Some("Defs.IO_Switch"), true, true),
        ("WillowPopulationOpportunityPoint_0", Origin::Spawned, "PopulationFactoryVendingMachine", None, false, false),
        (
            "WillowPopulationOpportunityPoint_1",
            Origin::Spawned,
            "PopulationFactoryInteractiveObject",
            Some("Defs.IO_Box"),
            false,
            false
        ),
        ("WillowPopulationOpportunityPoint_2", Origin::Spawned, "", None, false, false),
        ("WillowPopulationOpportunityPoint_0", Origin::Spawned, "PopulationFactoryVendingMachine", None, false, false),
    ]);
    assert_eq!(objects[6].level, "Toy_Side");
}
