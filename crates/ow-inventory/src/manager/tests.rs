//! Tests of the inventory manager natives and the inventory queries on the invented world of `crate::equip::tests`
//! (AGENTS.md rule 9).

use ow_attr::api::{AttrError, EngineQuery};
use ow_core::notyet::Missing;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::FunctionDecl;

use super::*;
use crate::equip::tests::{World, missing};

/// Declares the native `WillowInventoryManager.<name>` with `params` so the VM calls the registered native.
fn native_fn(w: &mut World, name: &str, key: &str, params: Vec<ow_object::layout::PropertyDecl>) -> ow_object::ids::FuncId {
    let path = ow_core::ObjectPath::parse(&format!("WillowGame.WillowInventoryManager.{name}")).unwrap();
    let owner = w.class(MANAGER);
    w.vm
        .declare_function(FunctionDecl::native(path, key).with_owner(owner).with_params(params))
        .unwrap()
}

fn param(w: &mut World, name: &str, kind: ow_object::layout::PropKind, optional: bool) -> ow_object::layout::PropertyDecl {
    let flags = ow_object::layout::PropertyDecl::PARAM
        | if optional { ow_object::layout::PropertyDecl::OPTIONAL } else { 0 };
    ow_object::layout::PropertyDecl::new(w.vm.space.names.intern(name), kind).with_flags(flags)
}

fn int(v: Value) -> i32 {
    match v {
        Value::Int(i) => i,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_backpack_count_and_limit_are_the_stored_fields() {
    let mut w = World::new();
    let m = w.manager;
    w.set(m, "BackpackInventoryCount", Value::Int(7));
    w.set(m, "InventorySlotMax_Misc", Value::Int(27));
    assert_eq!(count_unreadied_inventory(&w.vm.space, m).unwrap(), 7);
    assert_eq!(unreadied_inventory_max_size(&w.vm.space, m).unwrap(), 27);
    register_natives(&mut w.vm).unwrap();
    let f = native_fn(&mut w, "CountUnreadiedInventory", "WillowInventoryManager.CountUnreadiedInventory()", vec![]);
    assert_eq!(int(w.vm.call(f, Some(m), vec![]).unwrap().ret), 7);
    let f = native_fn(&mut w, "GetUnreadiedInventoryMaxSize", "WillowInventoryManager.GetUnreadiedInventoryMaxSize()", vec![]);
    assert_eq!(int(w.vm.call(f, Some(m), vec![]).unwrap().ret), 27);
}

#[test]
fn set_inventory_max_size_keeps_twelve_unless_told_otherwise() {
    assert_eq!(stored_inventory_size(3, false), 12);
    assert_eq!(stored_inventory_size(12, false), 12);
    assert_eq!(stored_inventory_size(30, false), 30);
    assert_eq!(stored_inventory_size(3, true), 3);
    assert_eq!(stored_inventory_size(-5, true), -5);
    let mut w = World::new();
    register_natives(&mut w.vm).unwrap();
    let size = param(&mut w, "NewSize", ow_object::layout::PropKind::Int, false);
    let keep = param(&mut w, "bOverrideDefaultMin", ow_object::layout::PropKind::Bool, true);
    let f = native_fn(&mut w, "SetInventoryMaxSize", "WillowInventoryManager.SetInventoryMaxSize(int,bool)", vec![size, keep]);
    let m = w.manager;
    w.vm.call(f, Some(m), vec![Value::Int(5)]).unwrap();
    assert_eq!(int(w.get(m, "InventorySlotMax_Misc")), 12, "the optional flag omitted is false");
    w.vm.call(f, Some(m), vec![Value::Int(5), Value::Bool(true)]).unwrap();
    assert_eq!(int(w.get(m, "InventorySlotMax_Misc")), 5);
}

#[test]
fn a_manager_native_on_another_class_is_refused() {
    let mut w = World::new();
    register_natives(&mut w.vm).unwrap();
    let f = native_fn(&mut w, "CountUnreadiedInventory", "WillowInventoryManager.CountUnreadiedInventory()", vec![]);
    let pawn = w.pawn;
    let e = w.vm.call(f, Some(pawn), vec![]).unwrap_err();
    assert!(e.to_string().contains("is not a"), "{e}");
    assert!(w.vm.call(f, None, vec![]).is_err());
}

fn chain(w: &mut World, entries: &[ObjRef]) {
    let m = w.manager;
    w.set(m, "InventoryChain", Value::Object(entries.first().copied()));
    for pair in entries.windows(2) {
        w.set(pair[0], "Inventory", Value::Object(Some(pair[1])));
    }
}

#[test]
fn readied_weapons_are_counted_along_the_chain_and_a_loop_is_minus_one() {
    let mut w = World::new();
    let weapon = w.class("WillowGame.WillowWeapon");
    let a = w.obj("WillowGame.WillowWeapon", "A");
    let b = w.obj("WillowGame.WillowWeapon", "B");
    let item = w.obj("WillowGame.WillowItem", "Shield");
    let c = w.obj("WillowGame.WillowWeapon", "C");
    for o in [a, item, c] {
        w.set(o, "bReadied", Value::Bool(true));
    }
    chain(&mut w, &[a, b, item, c]);
    let m = w.manager;
    assert_eq!(count_readied_weapons(&w.vm.space, m, weapon).unwrap(), 2, "B unreadied, the shield no weapon");
    w.set(c, "Inventory", Value::Object(Some(b)));
    assert_eq!(count_readied_weapons(&w.vm.space, m, weapon).unwrap(), -1);
    w.set(m, "InventoryChain", Value::Object(None));
    assert_eq!(count_readied_weapons(&w.vm.space, m, weapon).unwrap(), 0);
}

#[test]
fn the_weapon_ready_max_is_the_stored_attribute_value() {
    let mut w = World::new();
    let m = w.manager;
    w.set(m, "WeaponReadyMax", Value::Int(3));
    w.set(m, "WeaponReadyMaxBaseValue", Value::Int(2));
    assert_eq!(weapon_ready_max(&w.vm.space, m).unwrap(), 3, "the value, not the base");
    register_natives(&mut w.vm).unwrap();
    let flag = param(&mut w, "bWantBaseValue", ow_object::layout::PropKind::Bool, true);
    let f = native_fn(&mut w, "GetWeaponReadyMax", "WillowInventoryManager.GetWeaponReadyMax(bool)", vec![flag]);
    assert_eq!(int(w.vm.call(f, Some(m), vec![Value::Bool(true)]).unwrap().ret), 3, "the flag ignored");
}

#[test]
fn the_equipped_item_is_the_pawn_s_slot_for_a_controller_or_a_pawn() {
    let mut w = World::new();
    let shield = w.obj("WillowGame.WillowItem", "Shield");
    let pawn = w.pawn;
    let p = w.vm.space.prop(w.class("WillowGame.WillowPawn"), "EquippedItems").unwrap();
    w.vm.space.write(pawn, PropAddr::element(p, 0), Value::Object(Some(shield))).unwrap();
    let c = w.controller;
    assert_eq!(equipped_inventory(&w.vm.space, c, 0).unwrap(), Some(shield));
    assert_eq!(equipped_inventory(&w.vm.space, pawn, 0).unwrap(), Some(shield));
    assert_eq!(equipped_inventory(&w.vm.space, pawn, 1).unwrap(), None, "an empty slot");
    assert_eq!(equipped_inventory(&w.vm.space, pawn, 4).unwrap(), None, "location 4 is none");
    let other = w.obj("Engine.InventoryManager", "Plain");
    w.set(pawn, "InvManager", Value::Object(Some(other)));
    assert_eq!(equipped_inventory(&w.vm.space, c, 0).unwrap(), None, "not a WillowInventoryManager");
    w.set(c, "Pawn", Value::Object(None));
    assert_eq!(equipped_inventory(&w.vm.space, c, 0).unwrap(), None, "no pawn");
    match equipped_inventory(&w.vm.space, shield, 0).unwrap_err() {
        AttrError::NotYet(n) => assert_eq!(n.missing, Missing::NotRead),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_query_handlers_need_the_world_query_service() {
    let mut w = World::new();
    assert!(matches!(missing(register_queries(&mut w.vm).unwrap_err()), Missing::LayerAbsent(_)));
    ow_world::query::install(&mut w.vm).unwrap();
    register_queries(&mut w.vm).unwrap();
    let shield = w.obj("WillowGame.WillowItem", "Shield");
    let pawn = w.pawn;
    let p = w.vm.space.prop(w.class("WillowGame.WillowPawn"), "EquippedItems").unwrap();
    w.vm.space.write(pawn, PropAddr::element(p, 3), Value::Object(Some(shield))).unwrap();
    use ow_attr::api::AttrWorld;
    let c = w.controller;
    assert_eq!(w.vm.engine_query(EngineQuery::EquippedInventory { location: 3 }, c).unwrap(), Some(shield));
    let q = EngineQuery::Inventory {
        definition: shield,
        equipped_only: false,
    };
    match w.vm.engine_query(q, c).unwrap_err() {
        AttrError::NotYet(n) => assert_eq!(n.missing, Missing::NotRead),
        other => panic!("{other:?}"),
    }
    assert!(register_queries(&mut w.vm).is_err(), "registered twice");
}

// ------------------------------------------------------------------------------- ItemActors (IMPL-INVENTORY-CONTEXT)

/// `ItemActors(BaseClass, out Inv, optional bOnlyReadied)` declared on the manager, as the loader declares it.
fn item_actors_fn(w: &mut World) -> ow_object::ids::FuncId {
    use ow_object::layout::{PropKind, PropertyDecl};
    let object = w.class("Core.Object");
    let base = PropertyDecl::new(w.vm.space.names.intern("BaseClass"), PropKind::Class { meta: object })
        .with_flags(PropertyDecl::PARAM);
    let inv = PropertyDecl::new(w.vm.space.names.intern("Inv"), PropKind::Object { class: object })
        .with_flags(PropertyDecl::PARAM | PropertyDecl::OUT);
    let readied = param(w, "bOnlyReadied", PropKind::Bool, true);
    native_fn(w, "ItemActors", "WillowInventoryManager.ItemActors(class,object,bool)", vec![base, inv, readied])
}

/// The entries the bound `ItemActors` gives on the fixture's manager.
fn iterate(w: &mut World, f: ow_object::ids::FuncId, base: Value, readied: Option<bool>) -> Result<Vec<ObjRef>, VmError> {
    use ow_vm::api::{Arg, NativeCall};
    let it = w.vm.iterator_for(f).expect("ItemActors is bound");
    let mut args = vec![Arg::new(base, false, true), Arg::new(Value::None, true, true)];
    if let Some(b) = readied {
        args.push(Arg::new(Value::Bool(b), false, true));
    }
    let m = w.manager;
    let mut call = NativeCall::new(&mut w.vm, f, Some(m), args);
    let out = it(&mut call)?;
    assert_eq!(out.out_param, 1, "the out argument is Inv");
    Ok(out
        .items
        .into_iter()
        .map(|v| match v {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        })
        .collect())
}

fn item_chain(w: &mut World, entries: &[ObjRef]) {
    let m = w.manager;
    w.set(m, "ItemChain", Value::Object(entries.first().copied()));
    for pair in entries.windows(2) {
        w.set(pair[0], "Inventory", Value::Object(Some(pair[1])));
    }
}

fn item_class(w: &World) -> Value {
    Value::Class(Some(w.class("WillowGame.WillowItem")))
}

/// NATIVE_INVENTORY_CONTEXT.md section 3: the item chain in chain order (not the weapon chain), the entries of the
/// base class or below only, and with `bOnlyReadied` the readied ones only; the flag omitted is false.
#[test]
fn item_actors_walks_the_item_chain_in_order_with_its_filters() {
    let mut w = World::new();
    let f = item_actors_fn(&mut w);
    register_iterators(&mut w.vm).unwrap();
    let gun = w.obj("WillowGame.WillowWeapon", "Gun");
    let [a, b, c] = ["A", "B", "C"].map(|n| w.obj("WillowGame.WillowItem", n));
    for o in [a, c] {
        w.set(o, "bReadied", Value::Bool(true));
    }
    item_chain(&mut w, &[a, gun, b, c]);
    // A weapon on the weapon chain is not walked.
    let other = w.obj("WillowGame.WillowItem", "OnTheWeaponChain");
    chain(&mut w, &[other]);
    let base = item_class(&w);
    assert_eq!(iterate(&mut w, f, base.clone(), None).unwrap(), vec![a, b, c]);
    assert_eq!(iterate(&mut w, f, base.clone(), Some(false)).unwrap(), vec![a, b, c]);
    assert_eq!(iterate(&mut w, f, base, Some(true)).unwrap(), vec![a, c]);
    // A wider base class gives the weapon too.
    let inventory = Value::Class(Some(w.class("Engine.Inventory")));
    assert_eq!(iterate(&mut w, f, inventory, None).unwrap(), vec![a, gun, b, c]);
    // An empty chain gives nothing.
    let m = w.manager;
    w.set(m, "ItemChain", Value::Object(None));
    let base = item_class(&w);
    assert!(iterate(&mut w, f, base, None).unwrap().is_empty());
}

/// Section 3: the skipped entries are counted over the whole walk, and it ends after more than 100 of them.
#[test]
fn item_actors_ends_after_more_than_one_hundred_skipped_entries() {
    for (skips, reached) in [(100usize, true), (101, false)] {
        let mut w = World::new();
        let f = item_actors_fn(&mut w);
        register_iterators(&mut w.vm).unwrap();
        let first = w.obj("WillowGame.WillowItem", "First");
        let mut entries = vec![first];
        // The skips are split around a given entry, so the count runs over the whole walk, not per step.
        let middle = w.obj("WillowGame.WillowItem", "Middle");
        for i in 0..skips {
            if i == skips / 2 {
                entries.push(middle);
            }
            entries.push(w.obj("WillowGame.WillowWeapon", &format!("Skip{i}")));
        }
        let last = w.obj("WillowGame.WillowItem", "Last");
        entries.push(last);
        item_chain(&mut w, &entries);
        let base = item_class(&w);
        let got = iterate(&mut w, f, base, None).unwrap();
        let expected = if reached { vec![first, middle, last] } else { vec![first, middle] };
        assert_eq!(got, expected, "{skips} skipped");
    }
}

/// Section 3: a None `BaseClass` falls back to a class the note does not identify (`NotYet`); a chain that comes back
/// to an entry is refused (a Rust-only data error); Reference binds nothing.
#[test]
fn item_actors_refuses_a_none_base_class_and_a_looped_chain_and_is_off_only() {
    let mut w = World::new();
    let f = item_actors_fn(&mut w);
    register_iterators(&mut w.vm).unwrap();
    let [a, b] = ["A", "B"].map(|n| w.obj("WillowGame.WillowItem", n));
    item_chain(&mut w, &[a, b]);
    let e = iterate(&mut w, f, Value::Class(None), None).unwrap_err();
    assert_eq!(missing(e), Missing::NotRead);
    w.set(b, "Inventory", Value::Object(Some(a)));
    let base = item_class(&w);
    let e = iterate(&mut w, f, base, None).unwrap_err();
    assert!(e.to_string().contains("comes back to an entry"), "{e}");
    let mut w = World::new();
    let f = item_actors_fn(&mut w);
    w.vm.parity = ow_core::ParityMode::Reference;
    register_iterators(&mut w.vm).unwrap();
    assert!(w.vm.iterator_for(f).is_none(), "Reference leaves ItemActors unbound");
}
