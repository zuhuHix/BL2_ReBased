//! The inventory resolvers' Off rules over a synthetic `WillowGame.upk` (AGENTS.md rule 9: invented classes; only the
//! names `IBodyPawn` and `WillowEquipAbleItem` are the game's, as the resolver looks them up). Lane
//! IMPL-INVENTORY-CONTEXT, decision 2026-11-19-67, from NATIVE_INVENTORY_CONTEXT.md sections 2 and 4. A child of the
//! context-resolver tests, so it walks chains through their scene.

use ow_core::ParityMode;
use ow_defs::identity::IdentityPolicy;
use ow_package::testkit::reflect::{CLASS_INTERFACE, ClassBodySpec, Declarations, class_body};

use super::*;

const EQUIPPED: &str = "WillowGame.EquippedInventoryAttributeContextResolver";
const INVENTORY: &str = "WillowGame.InventoryAttributeContextResolver";

/// An invented `WillowGame.upk`: a base class; the interface `IBodyPawn` when `with_interface`; `WillowEquipAbleItem`
/// and `TestEquipChild` below it; `TestBodyPawn` naming `IBodyPawn` in its table and `TestBodyPawnChild` inheriting
/// it; `TestPlain` naming nothing.
fn willow_package(with_interface: bool) -> Vec<u8> {
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let base_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let base = d.p.add_export(0, "TestBase", base_body);
    d.default_object(base, "TestBase");
    let item = d.real_class("WillowEquipAbleItem", base, base, 2, &[], None);
    d.real_class("TestEquipChild", item, base, 2, &[], None);
    if with_interface {
        let iface = d.real_class("IBodyPawn", base, base, 2 | CLASS_INTERFACE, &[], None);
        let pawn = d.real_class("TestBodyPawn", base, base, 2, &[iface], None);
        d.real_class("TestBodyPawnChild", pawn, base, 2, &[], None);
    }
    d.real_class("TestPlain", base, base, 2, &[], None);
    d.build()
}

/// The scene with a store over [`willow_package`] in `mode`, in a fresh temporary folder (removed by the caller).
fn scene_with_store(mode: ParityMode, with_interface: bool, tag: &str) -> (Scene, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ow-attr-inventory-{}-{tag}-{mode:?}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let package = willow_package(with_interface);
    let defs = DefStore::from_synthetic(
        &dir,
        &[("WillowGame.upk", package.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut s = scene();
    s.w.defs = Some(defs);
    (s, dir)
}

/// An object of the store's class `WillowGame.<name>`.
fn package_object(s: &mut Scene, name: &str) -> ObjRef {
    let class =
        s.w.defs.as_mut().unwrap().class_id(&mut s.w.space, &format!("WillowGame.{name}")).unwrap();
    s.w.obj(class, name)
}

/// The equipped resolver at `location`.
fn equipped(s: &mut Scene, location: u8) -> ObjRef {
    let props = vec![("EquipmentLocation", PropKind::Byte { enum_: None })];
    let r = registered(s, EQUIPPED, props);
    s.w.set(r, "EquipmentLocation", Value::Byte(location));
    r
}

/// The text of a `NotYet { NotRead }`, or a panic.
fn not_read_text(e: Result<Option<ObjRef>, AttrError>) -> String {
    match e {
        Err(AttrError::NotYet(n)) if n.missing == Missing::NotRead => n.what.into_owned(),
        other => panic!("expected NotYet {{ NotRead }}, got {other:?}"),
    }
}

/// Section 4: a None source is None; an equipable item (or below) is itself, at any location, even one past the
/// slots (the item path reads no slot); a class that implements no `IBodyPawn` by its package declaration is None.
#[test]
fn off_equipped_answers_an_item_itself_and_none_without_the_interface() {
    let (mut s, dir) = scene_with_store(ParityMode::Off, true, "equipped-item");
    let item = package_object(&mut s, "WillowEquipAbleItem");
    let child = package_object(&mut s, "TestEquipChild");
    let plain = package_object(&mut s, "TestPlain");
    let r = equipped(&mut s, 0);
    for location in [0, 3, 9] {
        s.w.set(r, "EquipmentLocation", Value::Byte(location));
        for o in [item, child] {
            assert_eq!(walk(&s.sys, &[Some(r)], Some(o), &mut s.w), Ok(Some(o)), "{location}");
        }
        assert_eq!(walk(&s.sys, &[Some(r)], Some(plain), &mut s.w), Ok(None));
        assert_eq!(walk(&s.sys, &[Some(r)], None, &mut s.w), Ok(None));
    }
    // No query is asked in Off: the queries table is empty and the answers above are the rule's.
    assert!(s.w.queries.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 4: a store whose packages hold no `IBodyPawn` class has no implementer, so a non-item source is None.
#[test]
fn off_equipped_without_the_interface_class_is_none() {
    let (mut s, dir) = scene_with_store(ParityMode::Off, false, "equipped-noiface");
    let plain = package_object(&mut s, "TestPlain");
    let r = equipped(&mut s, 1);
    assert_eq!(walk(&s.sys, &[Some(r)], Some(plain), &mut s.w), Ok(None));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 4: a source that implements `IBodyPawn` (its own table or a super class's) needs the accessor, which is not
/// settled: the named `NotYet`. A location of 4 or more is a data error before that.
#[test]
fn off_equipped_through_the_interface_waits_for_the_accessor() {
    let (mut s, dir) = scene_with_store(ParityMode::Off, true, "equipped-iface");
    let pawn = package_object(&mut s, "TestBodyPawn");
    let child = package_object(&mut s, "TestBodyPawnChild");
    let r = equipped(&mut s, 2);
    for o in [pawn, child] {
        let text = not_read_text(walk(&s.sys, &[Some(r)], Some(o), &mut s.w));
        assert!(text.contains("which IBodyPawn accessor the equipped resolver calls"), "{text}");
    }
    s.w.set(r, "EquipmentLocation", Value::Byte(4));
    match walk(&s.sys, &[Some(r)], Some(pawn), &mut s.w) {
        Err(AttrError::BadData { reason }) => assert!(reason.contains("EquipmentLocation 4"), "{reason}"),
        other => panic!("expected a data error, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 4 (review r2 of #445): when it cannot be established whether the source class implements `IBodyPawn`, the
/// answer is that named `NotYet`, never None: a class with no package export behind it, and a world without a
/// definition store.
#[test]
fn off_equipped_with_an_unreadable_interface_list_is_its_own_not_yet() {
    const NAMED: &str = "whether the source class implements IBodyPawn";
    // No store (the scene's own world, Off).
    let mut s = scene();
    let r = equipped(&mut s, 0);
    let player = s.player;
    let text = not_read_text(walk(&s.sys, &[Some(r)], Some(player), &mut s.w));
    assert!(text.contains(NAMED), "{text}");
    // A store, and a hand-made class with no package export.
    let (mut s, dir) = scene_with_store(ParityMode::Off, true, "equipped-unknown");
    let r = equipped(&mut s, 0);
    let player = s.player;
    let text = not_read_text(walk(&s.sys, &[Some(r)], Some(player), &mut s.w));
    assert!(text.contains(NAMED) && text.contains("no package export"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Reference keeps the engine query, as before the lane.
#[test]
fn reference_equipped_asks_the_engine_query() {
    let mut s = scene();
    s.w.defs = Some(reference_store());
    let r = equipped(&mut s, 3);
    let item = s.w.obj(s.c.actor, "ClassMod");
    let player = s.player;
    s.w.queries.push(((EngineQuery::EquippedInventory { location: 3 }, player), item));
    assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(Some(item)));
}

/// Section 2: the inventory resolver is script, so Off runs the class's script through the world, a None source
/// included (the script decides what None gives). Reference keeps the previous answers and runs no script.
#[test]
fn the_inventory_resolver_runs_its_script_in_off_only() {
    let mut s = scene();
    let r = registered(&mut s, INVENTORY, vec![]);
    let (pc, gun) = (s.pc, s.gun);
    let attr = s.w.attr;
    s.w.scripts.push(((r, pc), Some(gun)));
    assert_eq!(walk(&s.sys, &[Some(r)], Some(pc), &mut s.w), Ok(Some(gun)));
    assert_eq!(s.w.script_calls, vec![(r, attr, Some(pc))]);
    // A None source is the script's too (this world runs none for it).
    assert_eq!(missing(walk(&s.sys, &[Some(r)], None, &mut s.w)), Missing::LayerAbsent("ow-vm"));
    assert_eq!(s.w.script_calls.len(), 2);
    // Reference: NotYet for a source, None for a None source, no script.
    s.w.defs = Some(reference_store());
    s.w.script_calls.clear();
    let text = not_read_text(walk(&s.sys, &[Some(r)], Some(pc), &mut s.w));
    assert!(text.contains("equipped-only flag"), "{text}");
    assert_eq!(walk(&s.sys, &[Some(r)], None, &mut s.w), Ok(None));
    assert!(s.w.script_calls.is_empty());
}

#[test]
fn the_interface_and_item_paths_are_the_ones_read_from_the_packages() {
    assert_eq!(BODY_PAWN_INTERFACE, "WillowGame.IBodyPawn");
    assert_eq!(EQUIP_ABLE_ITEM, "WillowGame.WillowEquipAbleItem");
}
