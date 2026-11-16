//! `BalancedActorAttributeContextResolver` over a synthetic `Engine.upk` (AGENTS.md rule 9: invented classes; only
//! the interface's name, `IBalancedActor`, is the engine's, as the resolver looks it up). Lane FIX-ATTR-CONTEXTS,
//! decision 2026-11-16-49. A child of the context-resolver tests, so it walks chains through their scene.

use ow_core::ParityMode;
use ow_defs::identity::IdentityPolicy;
use ow_package::testkit::reflect::{CLASS_INTERFACE, ClassBodySpec, Declarations, class_body};

use super::*;

const RESOLVER: &str = "Engine.BalancedActorAttributeContextResolver";

/// An invented `Engine.upk`: a base class; `IBalancedActor` and `IBalancedActorChild` (derived from it), both
/// interfaces, when `with_interface`; `TestBalanced` naming `IBalancedActor` in its table, `TestBalancedChild`
/// inheriting it, `TestChildImplementer` naming the derived interface only, and `TestPlain` naming nothing.
fn engine_package(with_interface: bool) -> Vec<u8> {
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let base_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let base = d.p.add_export(0, "TestBase", base_body);
    d.default_object(base, "TestBase");
    if with_interface {
        let iface = d.real_class("IBalancedActor", base, base, 2 | CLASS_INTERFACE, &[], None);
        let child = d.real_class(
            "IBalancedActorChild",
            iface,
            base,
            2 | CLASS_INTERFACE,
            &[],
            None,
        );
        let balanced = d.real_class("TestBalanced", base, base, 2, &[iface], None);
        d.real_class("TestBalancedChild", balanced, base, 2, &[], None);
        d.real_class("TestChildImplementer", base, base, 2, &[child], None);
    }
    d.real_class("TestPlain", base, base, 2, &[], None);
    d.build()
}

/// The scene with a store over [`engine_package`] in `mode`, in a fresh temporary folder (removed by the caller).
fn scene_with_store(
    mode: ParityMode,
    with_interface: bool,
    tag: &str,
) -> (Scene, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "ow-attr-balanced-{}-{tag}-{mode:?}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let package = engine_package(with_interface);
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", package.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut s = scene();
    s.w.defs = Some(defs);
    (s, dir)
}

/// An object of the store's class `Engine.<name>`.
fn package_object(s: &mut Scene, name: &str) -> ObjRef {
    let class =
        s.w.defs
            .as_mut()
            .unwrap()
            .class_id(&mut s.w.space, &format!("Engine.{name}"))
            .unwrap();
    s.w.obj(class, name)
}

#[test]
fn balanced_actor_is_the_source_when_its_class_implements_the_interface_in_both_modes() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut s, dir) = scene_with_store(mode, true, "implements");
        let r = registered(&mut s, RESOLVER, vec![]);
        let balanced = package_object(&mut s, "TestBalanced");
        let inherited = package_object(&mut s, "TestBalancedChild");
        let derived = package_object(&mut s, "TestChildImplementer");
        let plain = package_object(&mut s, "TestPlain");
        three_rules(&mut s, r, balanced, balanced, Some(plain));
        // A super class's table counts, and so does an interface derived from IBalancedActor.
        for o in [inherited, derived] {
            assert_eq!(
                walk(&s.sys, &[Some(r)], Some(o), &mut s.w),
                Ok(Some(o)),
                "{mode:?}"
            );
        }
        // A None source is None, and the chain ends there.
        assert_eq!(walk(&s.sys, &[Some(r)], None, &mut s.w), Ok(None));
        let chain = [Some(r), Some(s.no_context)];
        assert_eq!(walk(&s.sys, &chain, None, &mut s.w), Ok(None));
        // A class with no package export behind it implements nothing (as `Vm::implements` answers).
        let player = s.player;
        assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(None));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn balanced_actor_without_the_interface_class_has_no_implementer() {
    let (mut s, dir) = scene_with_store(ParityMode::Off, false, "nointerface");
    let r = registered(&mut s, RESOLVER, vec![]);
    let plain = package_object(&mut s, "TestPlain");
    assert_eq!(walk(&s.sys, &[Some(r)], Some(plain), &mut s.w), Ok(None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn balanced_actor_needs_the_store_and_a_known_ancestry() {
    // No store: the interface tables cannot be read.
    let mut s = scene();
    let r = registered(&mut s, RESOLVER, vec![]);
    let player = s.player;
    assert_eq!(
        missing(walk(&s.sys, &[Some(r)], Some(player), &mut s.w)),
        Missing::LayerAbsent("ow-defs")
    );
    assert_eq!(walk(&s.sys, &[Some(r)], None, &mut s.w), Ok(None));
    // A "no" over an engine-native stand-in is not proven.
    let (mut s, dir) = scene_with_store(ParityMode::Off, true, "standin");
    let r = registered(&mut s, RESOLVER, vec![]);
    let native =
        s.w.space
            .classes
            .define("Engine.NativeOnlyBalancedThing", None, vec![])
            .unwrap();
    s.w.space.classes.mark_native(native).unwrap();
    let odd = s.w.obj(native, "Odd");
    assert_eq!(
        missing(walk(&s.sys, &[Some(r)], Some(odd), &mut s.w)),
        Missing::NotRead
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_interface_path_is_the_one_read_from_the_packages() {
    assert_eq!(BALANCED_ACTOR_INTERFACE, "Engine.IBalancedActor");
}
