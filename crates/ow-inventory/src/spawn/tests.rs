//! Tests of the spawn natives' parts that need no definition store and no level, on the invented world of
//! `crate::equip::tests` (AGENTS.md rule 9). A full spawn needs a store, the registry built from it and a level with
//! its `WorldInfo`; the installed-game run is a local check (decision 2026-11-15-33).

use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::DetMap;
use ow_core::notyet::Missing;
use ow_object::value::Value;
use ow_vm::api::FunctionDecl;

use super::*;
use crate::equip::tests::{World, missing};

fn snapshot(pt: Playthrough) -> BalanceSnapshot {
    BalanceSnapshot::new(
        pt,
        OpLevel::new(0).unwrap(),
        PlayerCount::SINGLE,
        ExpLevel::new(10),
        GameStage(10),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

#[test]
fn a_registry_value_is_a_number_an_init_through_the_evaluator_or_nothing() {
    let mut w = World::new();
    assert_eq!(registry_value(&mut w.vm, &Pv::Float(2.5), None).unwrap(), Some(2.5));
    assert_eq!(registry_value(&mut w.vm, &Pv::Int(3), None).unwrap(), Some(3.0));
    assert_eq!(registry_value(&mut w.vm, &Pv::Null, None).unwrap(), None);
    let mut init = DetMap::new();
    init.insert("BaseValueConstant".to_string(), Pv::Float(4.0));
    init.insert("BaseValueScaleConstant".to_string(), Pv::Float(0.5));
    let mut wrapped = DetMap::new();
    wrapped.insert("init".to_string(), Pv::Map(init));
    assert_eq!(registry_value(&mut w.vm, &Pv::Map(wrapped), None).unwrap(), Some(2.0));
    // An untagged scale is the struct default 1.
    let mut init = DetMap::new();
    init.insert("BaseValueConstant".to_string(), Pv::Float(4.0));
    let mut wrapped = DetMap::new();
    wrapped.insert("init".to_string(), Pv::Map(init));
    assert_eq!(registry_value(&mut w.vm, &Pv::Map(wrapped), None).unwrap(), Some(4.0));
    assert!(matches!(
        registry_value(&mut w.vm, &Pv::Map(DetMap::new()), None).unwrap_err(),
        ItemError::BadData(_)
    ));
}

#[test]
fn the_cap_comes_from_the_world_s_balance_state() {
    let mut w = World::new();
    assert!(matches!(missing(stage_cap(&w.vm).unwrap_err()), Missing::LayerAbsent(_)));
    w.vm.add_service(BalanceState::new(snapshot(Playthrough::NORMAL))).unwrap();
    assert_eq!(stage_cap(&w.vm).unwrap(), 50);
}

#[test]
fn the_balance_spawn_needs_the_item_source() {
    let mut w = World::new();
    w.vm.add_service(BalanceState::new(snapshot(Playthrough::NORMAL))).unwrap();
    assert!(matches!(
        missing(candidates(&mut w.vm, "Test.Balance", 5, None).unwrap_err()),
        Missing::LayerAbsent(_)
    ));
}

fn declare(w: &mut World, name: &str, key: &str) -> ow_object::ids::FuncId {
    let path = ow_core::ObjectPath::parse(&format!("WillowGame.ItemPool.{name}")).unwrap();
    let decl = FunctionDecl::native(path, key).with_static(true);
    w.vm.declare_function(decl).unwrap()
}

#[test]
fn the_pool_spawn_is_not_ported_and_a_none_balance_spawns_nothing() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let pool = declare(&mut w, "SpawnBalancedInventoryFromPool", FROM_POOL);
    assert!(matches!(missing(w.vm.call(pool, None, vec![]).unwrap_err()), Missing::NotPorted));
    let path = ow_core::ObjectPath::parse("WillowGame.ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition").unwrap();
    let object = w.class("Core.Object");
    let p = |w: &mut World, n: &str, k: ow_object::layout::PropKind, out: bool| {
        let flags = ow_object::layout::PropertyDecl::PARAM | if out { ow_object::layout::PropertyDecl::OUT } else { 0 };
        ow_object::layout::PropertyDecl::new(w.vm.space.names.intern(n), k).with_flags(flags)
    };
    let obj = ow_object::layout::PropKind::Object { class: object };
    let params = vec![
        p(&mut w, "InvBalanceDefinition", obj.clone(), false),
        p(&mut w, "Quantity", ow_object::layout::PropKind::Int, false),
        p(&mut w, "GameStage", ow_object::layout::PropKind::Int, false),
        p(&mut w, "AwesomeLevel", ow_object::layout::PropKind::Int, false),
        p(&mut w, "ContextSource", obj.clone(), false),
        p(&mut w, "SpawnedInventory", ow_object::layout::PropKind::Array(Box::new(obj)), true),
    ];
    let balance = w.vm.declare_function(FunctionDecl::native(path, FROM_BALANCE).with_static(true).with_params(params)).unwrap();
    let r = w
        .vm
        .call(
            balance,
            None,
            vec![
                Value::Object(None),
                Value::Int(1),
                Value::Int(5),
                Value::Int(0),
                Value::Object(None),
                Value::array(vec![]),
            ],
        )
        .unwrap();
    assert_eq!(r.ret, Value::Bool(false));
}

#[test]
fn a_runtime_object_s_path_is_its_outer_chain() {
    let mut w = World::new();
    let outer = w.obj("Core.Object", "Outer");
    let c = w.class("Core.Object");
    let inner = w.vm.space.alloc(c, "Inner", Some(outer), ow_object::heap::Origin::Runtime).unwrap();
    assert_eq!(object_path(&w.vm, inner).unwrap(), "Outer.Inner");
}
