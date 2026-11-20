//! Tests of the spawn natives' parts that need no definition store and no level, on the invented world of
//! `crate::equip::tests` (AGENTS.md rule 9). A full spawn needs a store, the registry built from it and a level with
//! its `WorldInfo`; the installed-game run is a local check (decision 2026-11-15-33).

use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::DetMap;
use ow_core::notyet::Missing;
use ow_object::ids::ObjRef;
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
    // A constant or a scale that is present but not a number is refused, not read as its default.
    for field in ["BaseValueConstant", "BaseValueScaleConstant"] {
        let mut init = DetMap::new();
        init.insert(field.to_string(), Pv::Str("seven".to_string()));
        let mut wrapped = DetMap::new();
        wrapped.insert("init".to_string(), Pv::Map(init));
        match registry_value(&mut w.vm, &Pv::Map(wrapped), None).unwrap_err() {
            ItemError::BadData(why) => assert!(why.contains(field), "{why}"),
            other => panic!("{other:?}"),
        }
    }
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

#[test]
fn the_pool_spawn_is_left_to_ow_combat_and_a_none_balance_spawns_nothing() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    // ow-combat::loot owns the pool native's key (review r1 of PR #340): nothing is bound for it here.
    assert!(!w.vm.has_native(FROM_POOL));
    assert!(w.vm.has_native(FROM_BALANCE));
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

// ---- one spawn on an invented package (review r1) --------------------------------------------------------------

mod level {
    //! An invented `Engine.upk` (AGENTS.md rule 9: invented classes and values only) with the actor fields a spawn
    //! and a destroy read, a `WorldInfo`, and an inventory class whose `SetGameStage`, `SetAwesomeLevel` and
    //! `InitializeInventory` are test natives that record what they see, so the order of one spawn is visible.

    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
    };
    use ow_core::ParityMode;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::ids::{ClassId, ObjRef};
    use ow_object::layout::{PropKind, PropertyDecl};
    use ow_object::value::Value;
    use ow_package::testkit::reflect::{toy_array_of, toy_struct};
    use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
    use ow_vm::api::{NativeCall, Vm};
    use ow_vm::error::VmError;
    use ow_world::actors::is_destroyed;
    use ow_world::world::{BalanceState, World};

    use crate::spawn::{Resolved, resolve_objects, spawn_picks};

    const NATIVE: u32 = 0x400 | 0x20000;

    fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param]) {
        let spec = FunctionSpec::new(NATIVE, name);
        t.function(owner, name, params, None, &[], &spec, |_| None);
    }

    fn package() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "Function",
            "ScriptStruct",
            "IntProperty",
            "FloatProperty",
            "BoolProperty",
            "NameProperty",
            "ByteProperty",
            "ObjectProperty",
            "ArrayProperty",
            "StructProperty",
        ]);
        let object = t.class("Object", 0);
        let vector = toy_struct(&mut t, object, "Vector", None);
        for f in ["X", "Y", "Z"] {
            t.prop("Float", vector, f, 0, 0);
        }
        let rotator = toy_struct(&mut t, object, "Rotator", None);
        for f in ["Pitch", "Yaw", "Roll"] {
            t.prop("Int", rotator, f, 0, 0);
        }
        let actor = t.class("Actor", object);
        for b in [
            "bStatic",
            "bNoDelete",
            "bDeleteMe",
            "bPendingDelete",
            "bCollideWorld",
            "bCollideWhenPlacing",
            "bCollideActors",
            "bNetTemporary",
            "bShouldBaseAtStartup",
        ] {
            t.prop("Bool", actor, b, 0, 0);
        }
        for b in ["Role", "RemoteRole", "TickGroup", "Physics"] {
            t.prop("Byte", actor, b, 0, 0);
        }
        for f in ["LifeSpan", "CreationTime"] {
            t.prop("Float", actor, f, 0, 0);
        }
        for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
            t.prop("Object", actor, o, 0, actor);
        }
        t.prop("Struct", actor, "Location", 0, vector);
        t.prop("Struct", actor, "Rotation", 0, rotator);
        for a in ["Attached", "Touching", "GeneratedEvents", "Components", "AllComponents"] {
            toy_array_of(&mut t, actor, a, "Object", object);
        }
        let world_info = t.class("WorldInfo", actor);
        for f in ["TimeSeconds", "RealTimeSeconds", "TimeDilation"] {
            t.prop("Float", world_info, f, 0, 0);
        }
        t.prop("Byte", world_info, "NetMode", 0, 0);
        let inventory = t.class("Inventory", actor);
        let willow = t.class("WillowInventory", inventory);
        t.prop("Name", willow, "SourceDefinitionName", 0, 0);
        t.prop("Bool", willow, "bDropOnDeath", 0, 0);
        native(&mut t, willow, "SetGameStage", &[Param::new("Int", "NewGameStage")]);
        native(&mut t, willow, "SetAwesomeLevel", &[Param::new("Int", "NewAwesomeLevel")]);
        let obj = |name: &str| Param {
            type_ref: object,
            ..Param::new("Object", name)
        };
        native(
            &mut t,
            willow,
            "InitializeInventory",
            &[obj("Balance"), obj("Manufacturer"), Param::new("Int", "GradeOrLevel"), obj("ContextSource")],
        );
        t.class("TestItem", willow);
        t.class("NotAnItem", actor);
        t.p.build()
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// What a recording native saw: the item, the call, its int argument (if any), the item's `SourceDefinitionName`
    /// and `bDropOnDeath` at that moment.
    pub(super) type Seen = (ObjRef, &'static str, Option<i32>, String, bool);

    pub(super) struct Level {
        pub w: World,
        pub seen: Rc<RefCell<Vec<Seen>>>,
        pub balance: ObjRef,
        pub manufacturer: ObjRef,
        pub definition: ObjRef,
        dir: PathBuf,
    }

    impl Drop for Level {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn observe(vm: &Vm, item: ObjRef) -> (String, bool) {
        let name = match crate::equip::read_prop(vm, item, "SourceDefinitionName") {
            Some(Value::Name(n)) => vm.space.names.text(n).to_string(),
            _ => String::new(),
        };
        let drop = matches!(crate::equip::read_prop(vm, item, "bDropOnDeath"), Some(Value::Bool(true)));
        (name, drop)
    }

    fn recorder(vm: &mut Vm, key: &str, call: &'static str, seen: Rc<RefCell<Vec<Seen>>>, int_at: usize) {
        vm.register_native(
            key,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let item = c.this.expect("a spawn call on the item");
                let arg = match c.arg(int_at) {
                    Ok(Value::Int(i)) => Some(*i),
                    _ => None,
                };
                // grade-or-level 99 makes InitializeInventory fail (the cleanup test).
                if call == "InitializeInventory" && arg == Some(99) {
                    return Err(VmError::Script {
                        function: call.into(),
                        message: "invented failure".into(),
                    });
                }
                let (name, drop) = observe(c.vm, item);
                seen.borrow_mut().push((item, call, arg, name, drop));
                Ok(Value::None)
            }),
        )
        .unwrap();
    }

    pub(super) fn level() -> Level {
        let dir = std::env::temp_dir().join(format!(
            "ow-inventory-spawn-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = package();
        let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        let seen = Rc::new(RefCell::new(Vec::new()));
        recorder(&mut vm, "WillowInventory.SetGameStage(int)", "SetGameStage", seen.clone(), 0);
        recorder(&mut vm, "WillowInventory.SetAwesomeLevel(int)", "SetAwesomeLevel", seen.clone(), 0);
        recorder(
            &mut vm,
            "WillowInventory.InitializeInventory(object,object,int,object)",
            "InitializeInventory",
            seen.clone(),
            2,
        );
        let snapshot = BalanceSnapshot::new(
            Playthrough::NORMAL,
            OpLevel::new(0).unwrap(),
            PlayerCount::SINGLE,
            ExpLevel::new(8),
            GameStage(8),
            AwesomeLevel::new(0),
            LevelCapUpgrades::NONE,
        )
        .unwrap();
        let mut w = World::new(vm, BalanceState::new(snapshot), &[]).unwrap();
        // Role = authority in the item class's defaults, as a server-side spawn has it (a destroy refuses below it).
        let cdo = w.vm.defs.class_default(&mut w.vm.space, "Engine.TestItem").unwrap();
        let item_class = w.vm.space.heap.get(cdo).unwrap().class;
        let role = w.vm.space.prop(item_class, "Role").unwrap();
        w.vm.space.write(cdo, ow_object::ids::PropAddr::scalar(role), Value::Byte(3)).unwrap();
        let c = w.vm.find_class("Engine.WorldInfo").unwrap();
        let wi = ow_vm::interp::instantiate_from_defaults(&mut w.vm, c).unwrap();
        w.actors().borrow_mut().place_world_info(wi).unwrap();
        // The balance, the manufacturer and a definition: runtime objects of hand-made classes.
        let object = w.vm.find_class("Engine.Object").unwrap();
        let meta = PropKind::Class { meta: object };
        let decl = PropertyDecl::new(w.vm.space.names.intern("InventoryClass"), meta);
        let def_class = w.vm.space.classes.define("Test.ItemDefinition", Some(object), vec![decl]).unwrap();
        let alloc = |w: &mut World, c: ClassId, name: &str| w.vm.space.alloc(c, name, None, Origin::Runtime).unwrap();
        let balance = alloc(&mut w, object, "Bal_Invented");
        let manufacturer = alloc(&mut w, object, "Maker_Invented");
        let definition = alloc(&mut w, def_class, "Def_Invented");
        Level {
            w,
            seen,
            balance,
            manufacturer,
            definition,
            dir,
        }
    }

    impl Level {
        pub fn set_class(&mut self, class: Option<&str>) {
            let c = class.map(|p| self.w.vm.find_class(p).unwrap());
            let def_class = self.w.vm.space.heap.get(self.definition).unwrap().class;
            let p = self.w.vm.space.prop(def_class, "InventoryClass").unwrap();
            self.w
                .vm
                .space
                .write(self.definition, ow_object::ids::PropAddr::scalar(p), Value::Class(c))
                .unwrap();
        }

        pub fn resolved(&mut self, grade: i32) -> Option<Resolved> {
            let (d, m) = (self.definition, self.manufacturer);
            resolve_objects(&mut self.w.vm, d, m, 7, grade).unwrap()
        }

        pub fn actor_count(&self) -> usize {
            self.w.actors().borrow().list().len()
        }

        /// Sets `Role` in the item class's defaults (a destroy refuses below authority).
        pub fn set_item_role(&mut self, role: u8) {
            let cdo = self.w.vm.defs.class_default(&mut self.w.vm.space, "Engine.TestItem").unwrap();
            let class = self.w.vm.space.heap.get(cdo).unwrap().class;
            let p = self.w.vm.space.prop(class, "Role").unwrap();
            self.w.vm.space.write(cdo, ow_object::ids::PropAddr::scalar(p), Value::Byte(role)).unwrap();
        }

        pub fn destroyed(&self, o: ObjRef) -> bool {
            is_destroyed(&self.w.vm.space, o).unwrap()
        }

        pub fn spawn(&mut self, picks: Vec<Option<Resolved>>) -> Result<(Vec<ObjRef>, bool), VmError> {
            let n = i32::try_from(picks.len()).unwrap();
            let mut picks = picks.into_iter();
            let b = self.balance;
            spawn_picks(&mut self.w.vm, b, n, 3, None, move |_| Ok(picks.next().flatten()))
        }
    }
}

#[test]
fn one_spawn_writes_and_calls_in_the_record_s_order() {
    let mut l = level::level();
    l.set_class(Some("Engine.TestItem"));
    let r = l.resolved(5).expect("a WillowInventory class resolves");
    let (made, all) = l.spawn(vec![Some(r)]).unwrap();
    assert!(all);
    assert_eq!(made.len(), 1);
    let item = made[0];
    let seen = l.seen.borrow().clone();
    let calls: Vec<(&str, Option<i32>, &str, bool)> =
        seen.iter().map(|(o, c, a, n, d)| { assert_eq!(*o, item); (*c, *a, n.as_str(), *d) }).collect();
    // SourceDefinitionName before the three calls, bDropOnDeath only after them.
    assert_eq!(
        calls,
        vec![
            ("SetGameStage", Some(7), "Bal_Invented", false),
            ("SetAwesomeLevel", Some(3), "Bal_Invented", false),
            ("InitializeInventory", Some(5), "Bal_Invented", false),
        ]
    );
    assert_eq!(crate::equip::read_prop(&l.w.vm, item, "bDropOnDeath"), Some(Value::Bool(true)));
}

#[test]
fn a_failed_pick_makes_the_answer_false_and_the_others_still_spawn() {
    let mut l = level::level();
    l.set_class(Some("Engine.TestItem"));
    let r = l.resolved(1);
    let (made, all) = l.spawn(vec![r, None, r]).unwrap();
    assert!(!all, "one pick of three failed");
    assert_eq!(made.len(), 2);
}

#[test]
fn a_class_that_is_no_inventory_or_no_class_fails_the_pick_before_anything_is_made() {
    let mut l = level::level();
    let before = l.actor_count();
    l.set_class(Some("Engine.NotAnItem"));
    assert_eq!(l.resolved(1), None);
    l.set_class(None);
    assert_eq!(l.resolved(1), None);
    assert_eq!(l.actor_count(), before, "no actor was spawned");
}

#[test]
fn an_error_after_a_spawn_destroys_every_actor_the_call_made() {
    let mut l = level::level();
    l.set_class(Some("Engine.TestItem"));
    let good = l.resolved(1);
    let failing = l.resolved(99);
    let e = l.spawn(vec![good, failing]).unwrap_err();
    assert!(e.to_string().contains("invented failure"), "{e}");
    // The actors the calls saw: the first spawn's and the failing one's.
    let mut made: Vec<ObjRef> = Vec::new();
    for (o, ..) in l.seen.borrow().iter() {
        if !made.contains(o) {
            made.push(*o);
        }
    }
    assert_eq!(made.len(), 2, "both actors were made");
    assert!(made.iter().all(|o| l.destroyed(*o)), "and both destroyed again");
}


#[test]
fn a_cleanup_the_world_refuses_is_reported_naming_both_errors() {
    let mut l = level::level();
    l.set_class(Some("Engine.TestItem"));
    // Role below authority: the world refuses to destroy the spawned actors.
    l.set_item_role(0);
    let good = l.resolved(1);
    let failing = l.resolved(99);
    let e = l.spawn(vec![good, failing]).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("the cleanup after a failure failed"), "{text}");
    assert!(text.contains("refused to destroy"), "the cleanup's failure: {text}");
    assert!(text.contains("invented failure"), "the step's: {text}");
    let mut made: Vec<ObjRef> = Vec::new();
    for (o, ..) in l.seen.borrow().iter() {
        if !made.contains(o) {
            made.push(*o);
        }
    }
    assert_eq!(made.len(), 2);
    assert!(made.iter().all(|o| !l.destroyed(*o)), "both actors are still there, as the error says");
}
