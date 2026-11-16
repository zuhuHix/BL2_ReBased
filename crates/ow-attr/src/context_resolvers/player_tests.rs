//! The player-controller rule (`player_controller_of`) over a synthetic `WillowGame.upk` (AGENTS.md rule 9: invented
//! classes; only the interfaces' names, `IPlayerBehavior` and `IPlayerMaster`, are the engine's, as the rule looks
//! them up). Lane FIX-PHASELOCK-CHAIN, decision 2026-11-16-69. A child of the context-resolver tests.

use ow_core::ParityMode;
use ow_defs::identity::IdentityPolicy;
use ow_package::testkit::reflect::{CLASS_INTERFACE, ClassBodySpec, Declarations, class_body};

use super::*;

/// An invented `WillowGame.upk`: a base class, the two interfaces, `TestBehavior` naming `IPlayerBehavior`,
/// `TestMaster` naming `IPlayerMaster` only, `TestBoth` naming both, `TestPlain` naming neither.
fn willow_package() -> Vec<u8> {
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let base_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let base = d.p.add_export(0, "TestBase", base_body);
    d.default_object(base, "TestBase");
    let behavior = d.real_class("IPlayerBehavior", base, base, 2 | CLASS_INTERFACE, &[], None);
    let master = d.real_class("IPlayerMaster", base, base, 2 | CLASS_INTERFACE, &[], None);
    d.real_class("TestBehavior", base, base, 2, &[behavior], None);
    d.real_class("TestMaster", base, base, 2, &[master], None);
    d.real_class("TestBoth", base, base, 2, &[behavior, master], None);
    d.real_class("TestPlain", base, base, 2, &[], None);
    d.build()
}

fn object_of(s: &mut Scene, name: &str) -> ObjRef {
    let class =
        s.w.defs
            .as_mut()
            .unwrap()
            .class_id(&mut s.w.space, &format!("WillowGame.{name}"))
            .unwrap();
    s.w.obj(class, name)
}

#[test]
fn the_player_controller_rule_in_both_modes() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let dir = std::env::temp_dir().join(format!(
            "ow-attr-player-{}-{mode:?}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let package = willow_package();
        let defs = DefStore::from_synthetic(
            &dir,
            &[("WillowGame.upk", package.as_slice())],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut s = scene();
        s.w.defs = Some(defs);
        let behavior = object_of(&mut s, "TestBehavior");
        let both = object_of(&mut s, "TestBoth");
        let master = object_of(&mut s, "TestMaster");
        let plain = object_of(&mut s, "TestPlain");
        let player = s.player;
        let ask = |s: &mut Scene, o: ObjRef| player_controller_of(s.w.parts(), o);
        // An IPlayerBehavior (with or without IPlayerMaster) is its own player controller.
        assert_eq!(ask(&mut s, behavior), Ok(Some(behavior)), "{mode:?}");
        assert_eq!(ask(&mut s, both), Ok(Some(both)), "{mode:?}");
        // Neither interface, or a class with no package export behind it: None.
        assert_eq!(ask(&mut s, plain), Ok(None), "{mode:?}");
        assert_eq!(ask(&mut s, player), Ok(None), "{mode:?}");
        // An IPlayerMaster only: its call is not run here.
        match ask(&mut s, master) {
            Err(AttrError::NotYet(n)) => {
                assert_eq!(n.missing, Missing::NotPorted, "{mode:?}");
                assert!(n.what.contains("GetPlayerMasterPlayerController"), "{}", n.what);
            }
            other => panic!("{mode:?}: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    // No store: the interface tables cannot be read.
    let mut s = scene();
    let player = s.player;
    match player_controller_of(s.w.parts(), player) {
        Err(AttrError::NotYet(n)) => assert_eq!(n.missing, Missing::LayerAbsent("ow-defs")),
        other => panic!("{other:?}"),
    }
}

/// Review r1 of PR #368: a store with packages but without `WillowGame.upk` cannot say whether the source implements
/// the interfaces (their package is missing), so the rule answers `NotYet` naming it, not None; only a store with no
/// package at all has no implementer.
#[test]
fn a_store_without_the_interfaces_package_is_not_yet() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let dir = std::env::temp_dir().join(format!(
            "ow-attr-player-nowillow-{}-{mode:?}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
        let base_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
        let base = d.p.add_export(0, "TestBase", base_body);
        d.default_object(base, "TestBase");
        d.real_class("TestPlain", base, base, 2, &[], None);
        let package = d.build();
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Engine.upk", package.as_slice())],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut s = scene();
        s.w.defs = Some(defs);
        let class =
            s.w.defs
                .as_mut()
                .unwrap()
                .class_id(&mut s.w.space, "Engine.TestPlain")
                .unwrap();
        let plain = s.w.obj(class, "TestPlain");
        match player_controller_of(s.w.parts(), plain) {
            Err(AttrError::NotYet(n)) => {
                assert_eq!(n.missing, Missing::NotRead, "{mode:?}");
                assert!(n.what.contains("WillowGame"), "{}", n.what);
            }
            other => panic!("{mode:?}: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
