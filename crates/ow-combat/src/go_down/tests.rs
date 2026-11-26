//! The go-down natives on an invented world (AGENTS.md rule 9: invented classes and values; the class, field and
//! function names are the packages' names the natives read). Each test names the NATIVE_GO_DOWN_PATH.md section it
//! checks. Graded by the `VmError` variant and the `NotYet` kind, never by text.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use crate::hit::CombatState;
use crate::hit::access::set;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// `FUNC_Native | FUNC_Public`.
const NATIVE: u32 = 0x400 | 0x20000;

struct Env {
    vm: Vm,
    dir: PathBuf,
    object: ClassId,
    pawn: ClassId,
    willow_pawn: ClassId,
    player_pawn: ClassId,
    controller: ClassId,
    world_info: ClassId,
    game_info: ClassId,
    globals: ClassId,
    damage_type: ClassId,
    dialog: ClassId,
    event_data: ClassId,
    data_set: StructId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn class(vm: &mut Vm, path: &str, base: ClassId, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, Some(base), props).unwrap()
}

/// The invented world: a toy Core, and the classes and fields the natives read, rooted at its `Object`.
fn env(mode: ParityMode) -> Env {
    let dir = std::env::temp_dir().join(format!("ow-combat-godown-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    // A toy WillowGame holding only the game info and its native GetSkillManager, so the virtual lookup finds it.
    let willow = {
        let mut t = ScriptToy::new(&["Object"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let game = t.class("WillowGameInfo", object);
        t.function(game, "GetSkillManager", &[], Some(("Object", object)), &[], &FunctionSpec::new(NATIVE, "GetSkillManager"), |_| None);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", core.as_slice()), ("WillowGame.upk", willow.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    vm.add_service(CombatState::default()).unwrap();
    let object = vm.find_class("Core.Object").unwrap();
    let o = PropKind::Object { class: object };
    // An invented NetMode enum, standalone not first, so the rule reads it by name.
    let names = ["NM_Client", "NM_Standalone", "NM_ListenServer"].map(|n| vm.space.names.intern(n)).to_vec();
    let net_mode = vm.space.classes.define_enum("Engine.WorldInfo.ENetMode", names).unwrap();
    let actor = class(&mut vm, "Engine.Actor", object, &[("WorldInfo", o.clone())]);
    let world_info = class(&mut vm, "Engine.WorldInfo", actor, &[
        ("NetMode", PropKind::Byte { enum_: Some(net_mode) }),
        ("Game", o.clone()),
    ]);
    let game_info = vm.find_class("WillowGame.WillowGameInfo").unwrap();
    let pawn = class(&mut vm, "Engine.Pawn", actor, &[("Controller", o.clone()), ("DrivenVehicle", o.clone())]);
    let data = decl(&mut vm, "Data", PropKind::Array(Box::new(PropKind::Int)));
    let data_set = vm.space.classes.define_struct("Engine.IInstanceData.InstanceDataSet", None, vec![data]).unwrap();
    let willow_pawn = class(&mut vm, "WillowGame.WillowPawn", pawn, &[
        ("bGibbed", PropKind::Bool),
        ("MyDeathDef", o.clone()),
        ("StatusEffectComp", o.clone()),
        ("InstanceDataState", PropKind::Struct(data_set)),
    ]);
    let player_pawn = class(&mut vm, "WillowGame.WillowPlayerPawn", willow_pawn, &[]);
    let controller = class(&mut vm, "Engine.Controller", actor, &[("Pawn", o.clone())]);
    let globals = class(&mut vm, "WillowGame.WillowGlobals", object, &[
        ("TheCustomizationManager", o.clone()),
        ("TheEffectsCoordinator", o.clone()),
        ("TheVehicleLifetimeManager", o.clone()),
    ]);
    let damage_type = class(&mut vm, "WillowGame.WillowDamageTypeDefinition", object, &[
        ("DamageType", PropKind::Byte { enum_: None }),
    ]);
    let event_data = class(&mut vm, "GearboxFramework.GearboxDialogEventData", object, &[("LiveTalkAction", o.clone())]);
    let dialog = class(&mut vm, "GearboxFramework.GearboxDialogComponent", object, &[("EventData", o.clone())]);
    Env {
        vm,
        dir,
        object,
        pawn,
        willow_pawn,
        player_pawn,
        controller,
        world_info,
        game_info,
        globals,
        damage_type,
        dialog,
        event_data,
        data_set,
    }
}

fn make(vm: &mut Vm, c: ClassId, name: &str) -> ObjRef {
    vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
}

/// Declares the native `path` on `owner` (its key is the path), with `params` (each a parameter) and a result.
fn native(vm: &mut Vm, path: &str, owner: ClassId, params: &[(&str, PropKind, u64)], result: Option<PropKind>) -> FuncId {
    let params = params
        .iter()
        .map(|(n, k, f)| decl(vm, n, k.clone()).with_flags(PropertyDecl::PARAM | f))
        .collect();
    let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_owner(owner).with_params(params);
    if let Some(k) = result {
        d = d.with_result(decl(vm, "ReturnValue", k));
    }
    vm.declare_function(d).unwrap()
}

fn kind(e: &VmError) -> Option<&Missing> {
    match e {
        VmError::NotYet(n) => Some(&n.missing),
        _ => None,
    }
}

fn not_read(e: &VmError) -> bool {
    matches!(kind(e), Some(Missing::NotRead))
}

/// A world whose combat state holds `WorldInfo` with `NetMode` at `mode`'s index.
fn world(e: &mut Env, net_mode: u8) -> ObjRef {
    let w = make(&mut e.vm, e.world_info, "TheWorld");
    set(&mut e.vm, w, "NetMode", Value::Byte(net_mode)).unwrap();
    crate::hit::with_state(&e.vm, |s| s.world = Some(w)).unwrap();
    w
}

const STANDALONE: u8 = 1;
const CLIENT: u8 = 0;
const LISTEN_SERVER: u8 = 2;

#[test]
fn reference_binds_none_of_them() {
    let mut e = env(ParityMode::Reference);
    let f = native(&mut e.vm, "Engine.Pawn.IsLocallyControlled", e.pawn, &[], Some(PropKind::Bool));
    super::register(&mut e.vm).unwrap();
    let key = e.vm.function(f).unwrap().native_key.clone().unwrap();
    assert!(!e.vm.has_native(&key), "Reference is unchanged");
}

/// Section 3.1.
#[test]
fn is_locally_controlled_follows_the_controller_and_the_world() {
    let mut e = env(ParityMode::Off);
    let f = native(&mut e.vm, "Engine.Pawn.IsLocallyControlled", e.pawn, &[], Some(PropKind::Bool));
    super::register(&mut e.vm).unwrap();
    let me = make(&mut e.vm, e.willow_pawn, "Pawn_7");
    world(&mut e, STANDALONE);
    assert_eq!(e.vm.call(f, Some(me), vec![]).unwrap().ret, Value::Bool(false), "no controller");
    let vehicle = make(&mut e.vm, e.pawn, "Vehicle_3");
    let controller = make(&mut e.vm, e.controller, "Controller_4");
    set(&mut e.vm, vehicle, "Controller", Value::Object(Some(controller))).unwrap();
    set(&mut e.vm, me, "DrivenVehicle", Value::Object(Some(vehicle))).unwrap();
    assert_eq!(e.vm.call(f, Some(me), vec![]).unwrap().ret, Value::Bool(true), "the driven vehicle's controller");
    world(&mut e, CLIENT);
    let err = e.vm.call(f, Some(me), vec![]).unwrap_err();
    assert!(not_read(&err), "a networked world: {err:?}");
    if let VmError::NotYet(n) = &err {
        assert!(!ow_core::notyet::is_partial_writes(&n.what), "no prefix at entry");
    }
}

/// Section 3.4.
#[test]
fn tech_death_index_is_minus_one_until_the_unread_test() {
    let mut e = env(ParityMode::Off);
    let f = native(
        &mut e.vm,
        "WillowGame.WillowPawn.GetTechDeathIndexForDamageType",
        e.willow_pawn,
        &[("DamageType", PropKind::Object { class: e.object }, 0)],
        Some(PropKind::Int),
    );
    super::register(&mut e.vm).unwrap();
    let me = make(&mut e.vm, e.willow_pawn, "Enemy_2");
    let dt = make(&mut e.vm, e.damage_type, "DT_Toy");
    let call = |e: &mut Env, dt: Option<ObjRef>| e.vm.call(f, Some(me), vec![Value::Object(dt)]);
    assert_eq!(call(&mut e, Some(dt)).unwrap().ret, Value::Int(-1), "no death definition");
    let def = make(&mut e.vm, e.object, "Death_1");
    let comp = make(&mut e.vm, e.object, "Effects_1");
    set(&mut e.vm, me, "MyDeathDef", Value::Object(Some(def))).unwrap();
    set(&mut e.vm, me, "StatusEffectComp", Value::Object(Some(comp))).unwrap();
    assert_eq!(call(&mut e, None).unwrap().ret, Value::Int(-1), "a None damage type");
    let other = make(&mut e.vm, e.object, "NotADamageType");
    assert_eq!(call(&mut e, Some(other)).unwrap().ret, Value::Int(-1), "another class");
    set(&mut e.vm, dt, "DamageType", Value::Byte(3)).unwrap();
    assert_eq!(call(&mut e, Some(dt)).unwrap().ret, Value::Int(-1), "a kind with no test");
    set(&mut e.vm, dt, "DamageType", Value::Byte(4)).unwrap();
    assert!(not_read(&call(&mut e, Some(dt)).unwrap_err()), "the unread status-effect test");
    set(&mut e.vm, me, "bGibbed", Value::Bool(true)).unwrap();
    assert_eq!(call(&mut e, Some(dt)).unwrap().ret, Value::Int(-1), "gibbed comes first");
}

/// Sections 3.2 and 4.5: the unresolved implementations, the trace and the player pawn's ragdoll key.
#[test]
fn unresolved_natives_refuse_by_kind_and_the_player_ragdoll_is_presented() {
    let mut e = env(ParityMode::Off);
    let local = native(&mut e.vm, "Engine.Controller.IsLocalController", e.controller, &[], Some(PropKind::Bool));
    let trace = native(&mut e.vm, "Engine.Actor.Trace", e.pawn, &[], Some(PropKind::Object { class: e.object }));
    let ragdoll = native(&mut e.vm, "WillowGame.WillowPlayerPawn.InitRagdoll", e.player_pawn, &[], Some(PropKind::Bool));
    let clear = native(
        &mut e.vm,
        "WillowGame.StatusEffectsComponent.ClearAllStatusEffects",
        e.object,
        &[("bForce", PropKind::Bool, PropertyDecl::OPTIONAL)],
        None,
    );
    super::register(&mut e.vm).unwrap();
    let c = make(&mut e.vm, e.controller, "Controller_1");
    let err = e.vm.call(local, Some(c), vec![]).unwrap_err();
    assert!(not_read(&err), "{err:?}");
    let p = make(&mut e.vm, e.player_pawn, "Maya_1");
    // The shared Off implementation (lane IMPL-ACTOR-TRACE), not the bare layer-absent refusal it replaced: this toy
    // declaration has no parameters, so it refuses on the missing TraceEnd before any layer is asked. Its own tests
    // are in `ow_world::world::actor_trace`.
    let err = e.vm.call(trace, Some(p), vec![]).unwrap_err();
    assert!(kind(&err).is_none(), "not a NotYet: {err:?}");
    assert_eq!(e.vm.call(ragdoll, Some(p), vec![]).unwrap().ret, Value::Bool(true));
    let comp = make(&mut e.vm, e.object, "Effects_2");
    assert!(not_read(&e.vm.call(clear, Some(comp), vec![Value::Bool(false)]).unwrap_err()));
}

/// Section 3.5.
#[test]
fn the_controller_skill_manager_is_the_willow_game_infos() {
    let mut e = env(ParityMode::Off);
    let f = native(
        &mut e.vm,
        "WillowGame.WillowPlayerController.GetSkillManager",
        e.controller,
        &[],
        Some(PropKind::Object { class: e.object }),
    );
    let g = e.vm.find_function("WillowGame.WillowGameInfo.GetSkillManager").unwrap();
    let manager = make(&mut e.vm, e.object, "SkillManager_0");
    let key = e.vm.function(g).unwrap().native_key.clone().unwrap();
    e.vm.register_native(&key, Rc::new(move |_| Ok(Value::Object(Some(manager))))).unwrap();
    super::register(&mut e.vm).unwrap();
    let c = make(&mut e.vm, e.controller, "WPC_0");
    let err = e.vm.call(f, Some(c), vec![]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::LayerAbsent(_))), "no world is a missing layer, not None: {err:?}");
    let w = world(&mut e, STANDALONE);
    let other = make(&mut e.vm, e.object, "NotAGameInfo");
    set(&mut e.vm, w, "Game", Value::Object(Some(other))).unwrap();
    assert_eq!(e.vm.call(f, Some(c), vec![]).unwrap().ret, Value::Object(None), "not a WillowGameInfo");
    let game = make(&mut e.vm, e.game_info, "Game_0");
    set(&mut e.vm, w, "Game", Value::Object(Some(game))).unwrap();
    assert_eq!(e.vm.call(f, Some(c), vec![]).unwrap().ret, Value::Object(Some(manager)));
}

/// Section 4.2.
#[test]
fn the_small_getters() {
    let mut e = env(ParityMode::Off);
    let engine = class(&mut e.vm, "Engine.Engine", e.object, &[]);
    let wi = native(&mut e.vm, "Engine.Engine.GetCurrentWorldInfo", engine, &[], Some(PropKind::Object { class: e.object }));
    let split = native(&mut e.vm, "Engine.Engine.IsSplitScreen", engine, &[], Some(PropKind::Bool));
    let custom = native(&mut e.vm, "WillowGame.WillowGlobals.GetCustomizationManager", e.globals, &[], Some(PropKind::Object { class: e.object }));
    let vehicles = native(&mut e.vm, "WillowGame.WillowGlobals.GetVehicleLifetimeManager", e.globals, &[], Some(PropKind::Object { class: e.object }));
    super::register(&mut e.vm).unwrap();
    let en = make(&mut e.vm, engine, "GameEngine_0");
    let err = e.vm.call(wi, Some(en), vec![]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::LayerAbsent(_))), "no WorldInfo: {err:?}");
    let w = world(&mut e, STANDALONE);
    assert_eq!(e.vm.call(wi, Some(en), vec![]).unwrap().ret, Value::Object(Some(w)));
    assert_eq!(e.vm.call(split, Some(en), vec![]).unwrap().ret, Value::Bool(false));
    let g = make(&mut e.vm, e.globals, "Globals_0");
    let m = make(&mut e.vm, e.object, "Customization_0");
    assert_eq!(e.vm.call(custom, Some(g), vec![]).unwrap().ret, Value::Object(None));
    set(&mut e.vm, g, "TheCustomizationManager", Value::Object(Some(m))).unwrap();
    assert_eq!(e.vm.call(custom, Some(g), vec![]).unwrap().ret, Value::Object(Some(m)));
    assert!(not_read(&e.vm.call(vehicles, Some(g), vec![]).unwrap_err()), "the lazy creation");
    set(&mut e.vm, g, "TheVehicleLifetimeManager", Value::Object(Some(m))).unwrap();
    assert_eq!(e.vm.call(vehicles, Some(g), vec![]).unwrap().ret, Value::Object(Some(m)));
}

/// Section 4.4.
#[test]
fn shadow_casting_does_nothing_without_instance_data() {
    let mut e = env(ParityMode::Off);
    let f = native(&mut e.vm, "WillowGame.WillowPlayerPawn.SetShadowCasting", e.player_pawn, &[("bCast", PropKind::Bool, 0)], None);
    super::register(&mut e.vm).unwrap();
    let p = make(&mut e.vm, e.player_pawn, "Maya_2");
    assert_eq!(e.vm.call(f, Some(p), vec![Value::Bool(true)]).unwrap().ret, Value::None);
    let filled = Value::structure(e.data_set, vec![Value::array(vec![Value::Int(5)])]);
    set(&mut e.vm, p, "InstanceDataState", filled).unwrap();
    assert!(not_read(&e.vm.call(f, Some(p), vec![Value::Bool(true)]).unwrap_err()));
}

/// NATIVE_RESPAWN_STATIONS.md 3.1 and NATIVE_DIALOG.md, as section 4 points to them.
#[test]
fn the_vehicle_test_and_the_dialog_stop() {
    let mut e = env(ParityMode::Off);
    let o = PropKind::Object { class: e.object };
    let ex = native(
        &mut e.vm,
        "Engine.Controller.IsUsingVehicleEx",
        e.controller,
        &[("bCheckDrivenVehicle", PropKind::Bool, PropertyDecl::OPTIONAL), ("Vehicle", o, PropertyDecl::OUT | PropertyDecl::OPTIONAL)],
        Some(PropKind::Bool),
    );
    let stop = native(
        &mut e.vm,
        "GearboxFramework.GearboxDialogComponent.StopTalking",
        e.dialog,
        &[("Tag", PropKind::Name, PropertyDecl::OPTIONAL), ("bForce", PropKind::Bool, PropertyDecl::OPTIONAL)],
        None,
    );
    super::register(&mut e.vm).unwrap();
    let c = make(&mut e.vm, e.controller, "Controller_5");
    assert_eq!(e.vm.call(ex, Some(c), vec![Value::Bool(true), Value::Object(None)]).unwrap().ret, Value::Bool(false));
    let p = make(&mut e.vm, e.pawn, "Pawn_5");
    set(&mut e.vm, c, "Pawn", Value::Object(Some(p))).unwrap();
    assert!(not_read(&e.vm.call(ex, Some(c), vec![Value::Bool(true), Value::Object(None)]).unwrap_err()));
    let d = make(&mut e.vm, e.dialog, "Dialog_0");
    let none = e.vm.space.names.intern("None");
    assert_eq!(e.vm.call(stop, Some(d), vec![Value::Name(none), Value::Bool(false)]).unwrap().ret, Value::None);
    let err = e.vm.call(stop, Some(d), vec![Value::Name(none), Value::Bool(true)]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::NotPorted)), "the flag set: {err:?}");
    let data = make(&mut e.vm, e.event_data, "Event_0");
    let act = make(&mut e.vm, e.object, "Act_0");
    set(&mut e.vm, data, "LiveTalkAction", Value::Object(Some(act))).unwrap();
    set(&mut e.vm, d, "EventData", Value::Object(Some(data))).unwrap();
    let err = e.vm.call(stop, Some(d), vec![Value::Name(none), Value::Bool(false)]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::NotPorted)), "a live line: {err:?}");
}

/// Section 3.1, review r1 of #500: the own controller, a driverless vehicle, a listen server, and the actor's own
/// `WorldInfo` read before the combat state's.
#[test]
fn is_locally_controlled_reads_the_actor_s_own_world() {
    let mut e = env(ParityMode::Off);
    let f = native(&mut e.vm, "Engine.Pawn.IsLocallyControlled", e.pawn, &[], Some(PropKind::Bool));
    super::register(&mut e.vm).unwrap();
    let me = make(&mut e.vm, e.pawn, "Pawn_8");
    let controller = make(&mut e.vm, e.controller, "Controller_8");
    set(&mut e.vm, me, "Controller", Value::Object(Some(controller))).unwrap();
    let err = e.vm.call(f, Some(me), vec![]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::LayerAbsent(_))), "a controller and no world: {err:?}");
    let own = make(&mut e.vm, e.world_info, "OwnWorld");
    set(&mut e.vm, own, "NetMode", Value::Byte(STANDALONE)).unwrap();
    set(&mut e.vm, me, "WorldInfo", Value::Object(Some(own))).unwrap();
    assert_eq!(e.vm.call(f, Some(me), vec![]).unwrap().ret, Value::Bool(true), "the own controller, the own world");
    world(&mut e, CLIENT);
    assert_eq!(e.vm.call(f, Some(me), vec![]).unwrap().ret, Value::Bool(true), "the actor's world wins");
    set(&mut e.vm, own, "NetMode", Value::Byte(LISTEN_SERVER)).unwrap();
    assert!(not_read(&e.vm.call(f, Some(me), vec![]).unwrap_err()), "a listen server is networked");
    let other = make(&mut e.vm, e.pawn, "Pawn_9");
    let vehicle = make(&mut e.vm, e.pawn, "Vehicle_9");
    set(&mut e.vm, other, "DrivenVehicle", Value::Object(Some(vehicle))).unwrap();
    assert_eq!(e.vm.call(f, Some(other), vec![]).unwrap().ret, Value::Bool(false), "a vehicle with no controller");
}

/// Section 3.4, review r1 of #500: every kind of the test refuses, any other answers -1, and a missing status-effect
/// component answers -1 with a death definition set.
#[test]
fn tech_death_kinds_and_the_missing_component() {
    let mut e = env(ParityMode::Off);
    let f = native(
        &mut e.vm,
        "WillowGame.WillowPawn.GetTechDeathIndexForDamageType",
        e.willow_pawn,
        &[("DamageType", PropKind::Object { class: e.object }, 0)],
        Some(PropKind::Int),
    );
    super::register(&mut e.vm).unwrap();
    let me = make(&mut e.vm, e.willow_pawn, "Enemy_3");
    let dt = make(&mut e.vm, e.damage_type, "DT_Toy2");
    let def = make(&mut e.vm, e.object, "Death_2");
    set(&mut e.vm, me, "MyDeathDef", Value::Object(Some(def))).unwrap();
    set(&mut e.vm, dt, "DamageType", Value::Byte(1)).unwrap();
    let call = |e: &mut Env| e.vm.call(f, Some(me), vec![Value::Object(Some(dt))]);
    assert_eq!(call(&mut e).unwrap().ret, Value::Int(-1), "no status-effect component");
    let comp = make(&mut e.vm, e.object, "Effects_3");
    set(&mut e.vm, me, "StatusEffectComp", Value::Object(Some(comp))).unwrap();
    for k in 0..=8u8 {
        set(&mut e.vm, dt, "DamageType", Value::Byte(k)).unwrap();
        // The expected kinds are written out here, not read from the constant the code uses (review r2 of #500).
        let tested = [1i64, 2, 4, 7].contains(&i64::from(k));
        match call(&mut e) {
            Ok(r) => assert!(!tested && r.ret == Value::Int(-1), "kind {k}: {:?}", r.ret),
            Err(err) => assert!(tested && not_read(&err), "kind {k}: {err:?}"),
        }
    }
}

/// Section 4.2, review r1 of #500: more than one local player is split screen.
#[test]
fn split_screen_with_two_players() {
    let mut e = env(ParityMode::Off);
    let engine = class(&mut e.vm, "Engine.Engine", e.object, &[]);
    let split = native(&mut e.vm, "Engine.Engine.IsSplitScreen", engine, &[], Some(PropKind::Bool));
    super::register(&mut e.vm).unwrap();
    let en = make(&mut e.vm, engine, "GameEngine_1");
    let one = make(&mut e.vm, e.controller, "WPC_1");
    crate::hit::with_state(&e.vm, |s| s.players.push(one)).unwrap();
    assert_eq!(e.vm.call(split, Some(en), vec![]).unwrap().ret, Value::Bool(false), "one player");
    let two = make(&mut e.vm, e.controller, "WPC_2");
    crate::hit::with_state(&e.vm, |s| s.players.push(two)).unwrap();
    assert_eq!(e.vm.call(split, Some(en), vec![]).unwrap().ret, Value::Bool(true), "two players");
}

/// Review r1 of #500: the out vehicle, a live line's other half, `SetAnchor`'s kind and more section 3.2 keys.
#[test]
fn the_out_vehicle_the_half_line_and_the_named_kinds() {
    let mut e = env(ParityMode::Off);
    let o = PropKind::Object { class: e.object };
    let ex = native(
        &mut e.vm,
        "Engine.Controller.IsUsingVehicleEx",
        e.controller,
        &[("bCheckDrivenVehicle", PropKind::Bool, PropertyDecl::OPTIONAL), ("Vehicle", o.clone(), PropertyDecl::OUT | PropertyDecl::OPTIONAL)],
        Some(PropKind::Bool),
    );
    let stop = native(
        &mut e.vm,
        "GearboxFramework.GearboxDialogComponent.StopTalking",
        e.dialog,
        &[("Tag", PropKind::Name, PropertyDecl::OPTIONAL), ("bForce", PropKind::Bool, PropertyDecl::OPTIONAL)],
        None,
    );
    let anchor = native(&mut e.vm, "Engine.Pawn.SetAnchor", e.pawn, &[("NewAnchor", o.clone(), 0)], None);
    let stance = native(&mut e.vm, "WillowGame.WillowPlayerPawn.SetDefaultStance", e.player_pawn, &[], None);
    let hidden = native(&mut e.vm, "Engine.Actor.SetHidden", e.pawn, &[("bNewHidden", PropKind::Bool, 0)], None);
    super::register(&mut e.vm).unwrap();
    let c = make(&mut e.vm, e.controller, "Controller_6");
    let stale = make(&mut e.vm, e.pawn, "StaleVehicle");
    let r = e.vm.call(ex, Some(c), vec![Value::Bool(true), Value::Object(Some(stale))]).unwrap();
    assert_eq!(r.ret, Value::Bool(false));
    assert_eq!(r.outs.get(1), Some(&Value::Object(None)), "the out vehicle is cleared: {:?}", r.outs);
    let d = make(&mut e.vm, e.dialog, "Dialog_1");
    let data = make(&mut e.vm, e.event_data, "Event_1");
    set(&mut e.vm, d, "EventData", Value::Object(Some(data))).unwrap();
    let none = e.vm.space.names.intern("None");
    assert_eq!(
        e.vm.call(stop, Some(d), vec![Value::Name(none), Value::Bool(false)]).unwrap().ret,
        Value::None,
        "event data with no live act is no live line"
    );
    let p = make(&mut e.vm, e.player_pawn, "Maya_3");
    let err = e.vm.call(anchor, Some(p), vec![Value::Object(None)]).unwrap_err();
    assert!(matches!(kind(&err), Some(Missing::NotPorted)), "SetAnchor: {err:?}");
    // SetDefaultStance has its rule now (lane IMPL-MAYA9-NATIVES, `crate::stance`): this toy player pawn declares none
    // of the fields the rule reads, so it is the rule's own script error, not the old placeholder's NotRead.
    assert!(
        matches!(e.vm.call(stance, Some(p), vec![]), Err(VmError::Script { .. })),
        "SetDefaultStance"
    );
    assert!(not_read(&e.vm.call(hidden, Some(p), vec![Value::Bool(true)]).unwrap_err()), "SetHidden");
}

/// Hub ruling R3 (review of #504, NATIVE_MISSION_TEXT_LIST.md section 6): the revived and killed events share the
/// statistics submit routine, and the port has no telemetry service, so both answer nothing for every player, None or
/// set, with no write.
#[test]
fn the_shared_telemetry_answers_nothing_for_every_player() {
    let mut e = env(ParityMode::Off);
    let service = class(&mut e.vm, "WillowGame.WillowLeviathanService", e.object, &[]);
    let o = PropKind::Object { class: e.object };
    let params = [("Player", o, 0), ("Reason", PropKind::Byte { enum_: None }, 0)];
    // Both events are named here, so dropping one from `TELEMETRY` fails this test (review r2 of #500).
    let paths = [
        "WillowGame.WillowLeviathanService.RecordCharacterRevivedEventForPlayer",
        "WillowGame.WillowLeviathanService.RecordCharacterKilledEventForPlayer",
    ];
    let fs: Vec<FuncId> = paths.iter().map(|p| native(&mut e.vm, p, service, &params, None)).collect();
    super::register(&mut e.vm).unwrap();
    let s = make(&mut e.vm, service, "Leviathan_0");
    let player = make(&mut e.vm, e.controller, "WPC_7");
    for f in fs {
        for who in [None, Some(player)] {
            let r = e.vm.call(f, Some(s), vec![Value::Object(who), Value::Byte(1)]).unwrap();
            assert_eq!(r.ret, Value::None, "{who:?}");
        }
    }
}
