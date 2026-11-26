//! NATIVE_COMBAT_MUSIC_AND_AFTER.md section 8, tests 1 to 5, on invented classes (AGENTS.md rule 9: the class and
//! field names are the packages'; every value is invented). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    play_through: StructId,
    balance_state: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx_in(mode: ParityMode) -> Fx {
    fx_with(mode, true)
}

/// The fixture, with or without the `CombatMusicManager` class.
fn fx_with(mode: ParityMode, manager: bool) -> Fx {
    fx_kind(mode, if manager { Kind::Manager } else { Kind::NoManager })
}

/// The fixture's variants (review of #808).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Manager,
    NoManager,
    /// The manager's `CombatMusicParams` declared as a reference to `Engine.DamageType` (item 2).
    MistypedParams,
    /// `WillowGame.GlobalsDefinition` held by a package but not in the class table (item 4).
    ColdDefinition,
}

/// A store whose `WillowGame` package holds a `GlobalsDefinition` class the table does not (item 4).
fn cold_store(mode: ParityMode) -> DefStore {
    use ow_package::testkit::script::ScriptToy;
    let mut t = ScriptToy::new(&[]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    t.class("GlobalsDefinition", object);
    let bytes = t.build();
    let dir = std::env::temp_dir().join(format!("ow-combat-music-cold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    DefStore::from_synthetic(&dir, &[("WillowGame.upk", bytes.as_slice())], ow_defs::identity::IdentityPolicy::FirstInScanOrder, mode).unwrap()
}

fn fx_kind(mode: ParityMode, kind: Kind) -> Fx {
    let manager = kind != Kind::NoManager;
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let store = if kind == Kind::ColdDefinition { cold_store(mode) } else { DefStore::new_empty() };
    let mut vm = Vm::new(space, store, Rc::new(AttributeSystem::new_empty()), mode);
    let props = vec![decl(&mut vm, "bCausedByWorld", PropKind::Bool)];
    let damage = vm.space.classes.define("Engine.DamageType", Some(object), props).unwrap();
    let o = PropKind::Object { class: object };
    let objects = PropKind::Array(Box::new(o.clone()));
    let props = vec![decl(&mut vm, "Instigator", o.clone()), decl(&mut vm, "GRI", o.clone())];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    vm.space.classes.define("Engine.WorldInfo", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "CurrentPlaythrough", PropKind::Int)];
    vm.space.classes.define("WillowGame.WillowGameReplicationInfo", Some(actor), props).unwrap();
    vm.space.classes.define("Engine.GameReplicationInfo", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "OnSpawnCustomizations", objects), decl(&mut vm, "MeshMaterial", o.clone())];
    let play_through = vm.space.classes.define_struct("WillowGame.AIPawnBalanceDefinition.PlayThroughData", None, props).unwrap();
    let base = vm.space.classes.define("Engine.BaseBalanceDefinition", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "PlayThroughs", PropKind::Array(Box::new(PropKind::Struct(play_through))))];
    vm.space.classes.define("WillowGame.AIPawnBalanceDefinition", Some(base), props).unwrap();
    let props = vec![decl(&mut vm, "BalanceDefinition", o.clone())];
    let balance_state = vm.space.classes.define_struct("Engine.Pawn.BalanceDefinitionStateData", None, props).unwrap();
    let props = vec![decl(&mut vm, "Mesh", o.clone()), decl(&mut vm, "BalanceDefinitionState", PropKind::Struct(balance_state))];
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), props).unwrap();
    vm.space.classes.define("WillowGame.WillowAIPawn", Some(pawn), vec![]).unwrap();
    // The music manager, its parameters, the game info, the globals and their definition.
    let props = vec![decl(&mut vm, "EnemyDamagesPlayerWeight", PropKind::Float)];
    vm.space.classes.define("WillowGame.CombatMusicParameters", Some(object), props).unwrap();
    let params_kind = if kind == Kind::MistypedParams { PropKind::Object { class: damage } } else { o.clone() };
    let props = vec![
        decl(&mut vm, "bCombatMusicManagerEnabled", PropKind::Bool),
        decl(&mut vm, "CombatMusicParams", params_kind),
    ];
    if manager {
        vm.space.classes.define("WillowGame.CombatMusicManager", Some(object), props).unwrap();
    }
    let props = vec![decl(&mut vm, "GlobalCombatMusicManager", o.clone())];
    vm.space.classes.define("WillowGame.WillowGameInfo", Some(actor), props).unwrap();
    let gearbox = vm.space.classes.define("GearboxFramework.GearboxGlobalsDefinition", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "CombatMusicProperties", o.clone())];
    if kind != Kind::ColdDefinition {
        vm.space.classes.define("WillowGame.GlobalsDefinition", Some(gearbox), props).unwrap();
    }
    let props = vec![decl(&mut vm, "TheGlobalsDefinition", PropKind::Object { class: gearbox })];
    vm.space.classes.define("WillowGame.WillowGlobals", Some(object), props).unwrap();
    Fx { vm, play_through, balance_state }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }
    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }
    fn lazy(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }
    /// A world with a world info whose `GRI` is a Willow replication info at `playthrough`.
    fn world(&mut self, playthrough: i32) -> ObjRef {
        let actor = self.class("Engine.Actor");
        let info = self.make("Engine.WorldInfo");
        let gri = self.make("WillowGame.WillowGameReplicationInfo");
        self.set(gri, "CurrentPlaythrough", Value::Int(playthrough));
        self.set(info, "GRI", Value::Object(Some(gri)));
        let mut actors = ow_world::actors::Actors::new(actor);
        actors.place_world_info(info).unwrap();
        self.vm.add_service(actors).unwrap();
        gri
    }
    /// A `PlayThroughs` entry with `behaviors` and `material`.
    fn entry(&self, behaviors: Vec<Value>, material: Option<ObjRef>) -> Value {
        Value::structure(self.play_through, vec![Value::array(behaviors), Value::Object(material)])
    }
    fn balance(&mut self, entries: Vec<Value>) -> ObjRef {
        let b = self.make("WillowGame.AIPawnBalanceDefinition");
        self.set(b, "PlayThroughs", Value::array(entries));
        b
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("{other:?}"),
    }
}

/// [`grade_plan`], checked write-free and load-free.
fn plan(x: &Fx, balance: ObjRef, pawn: Option<ObjRef>) -> Result<GradePlan, VmError> {
    let before = x.marks();
    let r = grade_plan(&x.vm, balance, pawn);
    assert_eq!(x.marks(), before, "the plan reads only");
    r
}

fn answer(p: GradePlan) -> (bool, bool) {
    match p {
        GradePlan::Answer { answer, base_error } => (answer, base_error),
        GradePlan::Run { .. } => panic!("a run"),
    }
}

/// Test 1: `ApplyGradeCustomizations`.
#[test]
fn apply_grade_customizations_by_the_balance_s_own_index() {
    let mut x = fx();
    let gri = x.world(0);
    let pawn = x.make("WillowGame.WillowAIPawn");
    let empty = || vec![];
    let (a, b) = (x.entry(empty(), None), x.entry(empty(), None));
    let balance = x.balance(vec![a, b]);
    assert_eq!(answer(plan(&x, balance, Some(pawn)).unwrap()), (true, false), "playthrough 0");
    x.set(gri, "CurrentPlaythrough", Value::Int(5));
    assert_eq!(answer(plan(&x, balance, Some(pawn)).unwrap()), (true, false), "5, clamped to 1");
    x.set(gri, "CurrentPlaythrough", Value::Int(-2));
    assert_eq!(answer(plan(&x, balance, Some(pawn)).unwrap()), (false, false), "a negative playthrough");
    x.set(gri, "CurrentPlaythrough", Value::Int(0));
    let none = x.balance(vec![]);
    assert_eq!(answer(plan(&x, none, Some(pawn)).unwrap()), (false, false), "no PlayThroughs");
    assert_eq!(answer(plan(&x, balance, None).unwrap()), (false, false), "no pawn");
    let thing = x.make("Engine.Actor");
    assert_eq!(not_read(plan(&x, balance, Some(thing)).err().unwrap()), PAWN_QUERY, "not a pawn");
    let base = x.make("Engine.BaseBalanceDefinition");
    assert_eq!(answer(plan(&x, base, Some(pawn)).unwrap()), (false, true), "a base balance: false, logged");
    // A one-behaviour list: the behaviour is run on the pawn with its Instigator.
    let instigator = x.make("Engine.Actor");
    x.set(pawn, "Instigator", Value::Object(Some(instigator)));
    let behavior = x.make("Core.Object");
    let one = x.entry(vec![Value::Object(Some(behavior))], None);
    let runs = x.balance(vec![one]);
    match plan(&x, runs, Some(pawn)).unwrap() {
        GradePlan::Run { pawn: p, instigator: i, behaviors } => {
            assert_eq!((p, i), (pawn, Some(instigator)));
            assert_eq!(behaviors, Value::array(vec![Value::Object(Some(behavior))]));
        }
        GradePlan::Answer { .. } => panic!("expected a run"),
    }
    // A replication info that is not a Willow one: index 0; no world: -1.
    let plain = x.make("Engine.GameReplicationInfo");
    let info = x.vm.service::<ow_world::actors::Actors>().unwrap().borrow().world_info().unwrap();
    x.set(info, "GRI", Value::Object(Some(plain)));
    assert_eq!(playthrough_index(&x.vm).unwrap(), 0);
    assert_eq!(playthrough_index(&fx().vm).unwrap(), -1, "no world");
    // An unloaded balance or pawn: NotRead.
    let lazy = x.lazy("WillowGame.AIPawnBalanceDefinition");
    assert!(not_read(plan(&x, lazy, Some(pawn)).err().unwrap()).contains("has not loaded"));
}

/// Test 2: `UpdatePlayThroughMaterial`.
#[test]
fn update_play_through_material_s_gates() {
    let mut x = fx();
    x.world(0);
    let pawn = x.make("WillowGame.WillowAIPawn");
    let mesh = x.make("Core.Object");
    let material = x.make("Core.Object");
    let state = |x: &Fx, b: Option<ObjRef>| Value::structure(x.balance_state, vec![Value::Object(b)]);
    let plain = x.entry(vec![], None);
    let balance = x.balance(vec![plain]);
    let before = x.marks();
    update_material(&x.vm, pawn).unwrap();
    x.set(pawn, "BalanceDefinitionState", state(&x, Some(balance)));
    update_material(&x.vm, pawn).unwrap();
    assert_eq!(x.marks().1, before.1);
    x.set(pawn, "Mesh", Value::Object(Some(mesh)));
    update_material(&x.vm, pawn).unwrap();
    let base = x.make("Engine.BaseBalanceDefinition");
    x.set(pawn, "BalanceDefinitionState", state(&x, Some(base)));
    update_material(&x.vm, pawn).unwrap();
    let set = x.entry(vec![], Some(material));
    let painted = x.balance(vec![set]);
    x.set(pawn, "BalanceDefinitionState", state(&x, Some(painted)));
    let before = x.marks();
    assert_eq!(not_read(update_material(&x.vm, pawn).unwrap_err()), MATERIAL_SLOTS);
    assert_eq!(x.marks(), before, "nothing written");
}

/// Test 3: `GetCombatMusicManager`.
#[test]
fn get_combat_music_manager_creates_it_once() {
    let mut x = fx();
    let game = x.make("WillowGame.WillowGameInfo");
    let globals = x.make("WillowGame.WillowGlobals");
    let definition = x.make("WillowGame.GlobalsDefinition");
    let params = x.make("WillowGame.CombatMusicParameters");
    x.set(definition, "CombatMusicProperties", Value::Object(Some(params)));
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    let q = ow_world::query::install(&mut x.vm).unwrap();
    q.borrow_mut().set_globals(globals);
    // The class default, built (invented values: enabled).
    let class = x.class("WillowGame.CombatMusicManager");
    let default = x.vm.space.alloc(class, "Default__CombatMusicManager", None, Origin::Default { class }).unwrap();
    x.set(default, "bCombatMusicManagerEnabled", Value::Bool(true));
    // An unloaded definition: NotRead before the creation.
    let lazy = x.lazy("WillowGame.GlobalsDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(lazy)));
    let before = x.marks();
    assert!(matches!(combat_music_manager(&mut x.vm, game), Err(VmError::NotYet(_))));
    assert_eq!(x.marks(), before, "nothing made");
    assert_eq!(x.get(game, "GlobalCombatMusicManager"), Value::Object(None));
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    // None: made from the class defaults, stored, its params from the definition.
    let manager = combat_music_manager(&mut x.vm, game).unwrap();
    assert_eq!(x.get(game, "GlobalCombatMusicManager"), Value::Object(Some(manager)));
    assert_eq!(x.get(manager, "CombatMusicParams"), Value::Object(Some(params)));
    assert_eq!(x.vm.space.heap.get(manager).unwrap().outer, Some(game), "the game info is its outer");
    assert_eq!(x.get(manager, "bCombatMusicManagerEnabled"), Value::Bool(true), "the class default's values");
    assert_ne!(manager, default);
    // Set: the same object, nothing written.
    let before = x.marks();
    assert_eq!(combat_music_manager(&mut x.vm, game).unwrap(), manager);
    assert_eq!(x.marks(), before);
    // No manager class in the table: the typed NotRead, nothing written.
    let mut y = fx_with(ParityMode::Off, false);
    let game = y.make("WillowGame.WillowGameInfo");
    let before = y.marks();
    assert_eq!(not_read(combat_music_manager(&mut y.vm, game).unwrap_err()), MANAGER_CLASS);
    assert_eq!(y.marks(), before);
}

/// Test 4: `PlayerDamagedByEnemy`, with and without the harness setting.
#[test]
fn player_damaged_by_enemy_s_gates_and_the_no_op() {
    let mut x = fx();
    let manager = x.make("WillowGame.CombatMusicManager");
    let params = x.make("WillowGame.CombatMusicParameters");
    let enemy = x.make("Engine.Actor");
    let damage = x.class("Engine.DamageType");
    let default = x.vm.space.alloc(damage, "Default__DamageType", None, Origin::Default { class: damage }).unwrap();
    let before = x.marks();
    assert!(!damage_gates(&x.vm, manager, Some(enemy), None).unwrap(), "no params");
    x.set(manager, "CombatMusicParams", Value::Object(Some(params)));
    assert!(!damage_gates(&x.vm, manager, None, None).unwrap(), "no enemy");
    assert!(damage_gates(&x.vm, manager, Some(enemy), Some(damage)).unwrap(), "past the gates");
    assert_eq!(x.marks().0, before.0 + 1, "one fixture write only");
    x.set(default, "bCausedByWorld", Value::Bool(true));
    assert!(!damage_gates(&x.vm, manager, Some(enemy), Some(damage)).unwrap(), "caused by the world");
    // A damage type whose class default is not built: NotRead (building it loads).
    let other = x.vm.space.classes.define("Test.UnbuiltDamage", Some(damage), vec![]).unwrap();
    assert_eq!(not_read(damage_gates(&x.vm, manager, Some(enemy), Some(other)).unwrap_err()), DAMAGE_DEFAULT);
    // A params object that declares no weight: a script error.
    let wrong = x.make("Core.Object");
    x.set(manager, "CombatMusicParams", Value::Object(Some(wrong)));
    assert!(matches!(damage_gates(&x.vm, manager, Some(enemy), None), Err(VmError::Script { .. })));
    // Past the gates: the threat model's NotRead, nothing written; under the harness setting a no-op with one line.
    x.set(manager, "CombatMusicParams", Value::Object(Some(params)));
    x.set(default, "bCausedByWorld", Value::Bool(false));
    let before = x.marks();
    assert_eq!(not_read(player_damaged_answer(&mut x.vm, manager, Some(enemy), Some(damage)).unwrap_err()), THREAT_MODEL);
    assert_eq!(x.marks(), before);
    // The harness setting: Off only, once, logged.
    enable_threat_model_no_op(&mut x.vm).unwrap();
    let (before, logged) = (x.marks(), x.vm.log.len());
    player_damaged_answer(&mut x.vm, manager, Some(enemy), Some(damage)).unwrap();
    assert_eq!(x.marks(), before, "the no-op writes nothing");
    assert_eq!(&x.vm.log[logged..], [THREAT_LINE.to_string()], "one line per call");
    player_damaged_answer(&mut x.vm, manager, None, Some(damage)).unwrap();
    assert_eq!(x.vm.log.len(), logged + 1, "a gated call logs nothing");
    assert!(x.vm.log.iter().any(|l| l == THREAT_INPUT));
    assert!(matches!(enable_threat_model_no_op(&mut x.vm), Err(VmError::Registration { .. })), "twice");
    let mut r = fx_in(ParityMode::Reference);
    assert!(matches!(enable_threat_model_no_op(&mut r.vm), Err(VmError::Registration { .. })), "Off only");
}

/// The globals, their definition and the params for the lazy creation; answers (game, params).
fn music_world(x: &mut Fx, definition_class: &str) -> (ObjRef, ObjRef) {
    let game = x.make("WillowGame.WillowGameInfo");
    let globals = x.make("WillowGame.WillowGlobals");
    let definition = x.make(definition_class);
    let params = x.make("WillowGame.CombatMusicParameters");
    if x.vm.space.prop(x.class(definition_class), "CombatMusicProperties").is_some() {
        x.set(definition, "CombatMusicProperties", Value::Object(Some(params)));
    }
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    let q = ow_world::query::install(&mut x.vm).unwrap();
    q.borrow_mut().set_globals(globals);
    let class = x.class("WillowGame.CombatMusicManager");
    x.vm.space.alloc(class, "Default__CombatMusicManager", None, Origin::Default { class }).unwrap();
    (game, params)
}

/// Review of #808, item 2: a manager whose `CombatMusicParams` does not accept the definition's params is a data
/// error before anything is made, and a retry answers the same, never a half-made manager.
#[test]
fn a_mistyped_params_field_refuses_before_the_manager_is_made() {
    let mut x = fx_kind(ParityMode::Off, Kind::MistypedParams);
    let (game, _) = music_world(&mut x, "WillowGame.GlobalsDefinition");
    for attempt in 0..2 {
        let before = x.marks();
        assert!(matches!(combat_music_manager(&mut x.vm, game), Err(VmError::Script { .. })), "attempt {attempt}");
        assert_eq!(x.marks(), before, "attempt {attempt}: nothing made or written");
        assert_eq!(x.get(game, "GlobalCombatMusicManager"), Value::Object(None));
    }
}

/// Review of #808, item 4: the two definition classes come from the resident class table only. A class a package holds
/// but the table does not is a typed NotRead, and nothing enters the reflection cache.
#[test]
fn a_cold_definition_class_is_not_read_and_loads_nothing() {
    let mut x = fx_kind(ParityMode::Off, Kind::ColdDefinition);
    let (game, _) = music_world(&mut x, "GearboxFramework.GearboxGlobalsDefinition");
    let (before, classes) = (x.marks(), x.vm.defs.code().classes().count());
    let what = not_read(combat_music_manager(&mut x.vm, game).unwrap_err());
    assert!(what.starts_with(GLOBALS_CLASS_COLD) && what.contains("WillowGame.GlobalsDefinition"), "{what}");
    assert_eq!(x.marks(), before, "nothing written or loaded");
    assert_eq!(x.vm.defs.code().classes().count(), classes, "no class entered the reflection cache");
}

/// Review of #808, item 3: the no-params and no-enemy gates answer before the damage default and the weight are read:
/// an unbuilt damage default and a params object with no weight change nothing there.
#[test]
fn the_no_op_gates_answer_before_the_damage_default() {
    let mut x = fx();
    let manager = x.make("WillowGame.CombatMusicManager");
    let enemy = x.make("Engine.Actor");
    let damage = x.class("Engine.DamageType");
    let unbuilt = x.vm.space.classes.define("Test.UnbuiltDamage", Some(damage), vec![]).unwrap();
    assert!(!damage_gates(&x.vm, manager, Some(enemy), Some(unbuilt)).unwrap(), "no params");
    let wrong = x.make("Core.Object");
    x.set(manager, "CombatMusicParams", Value::Object(Some(wrong)));
    assert!(!damage_gates(&x.vm, manager, None, Some(unbuilt)).unwrap(), "no enemy");
    // Caused by the world: answered before the weight, so the params' missing weight does not matter.
    let default = x.vm.space.alloc(damage, "Default__DamageType", None, Origin::Default { class: damage }).unwrap();
    x.set(default, "bCausedByWorld", Value::Bool(true));
    assert!(!damage_gates(&x.vm, manager, Some(enemy), Some(damage)).unwrap(), "caused by the world");
    // Past every gate, the missing weight is a data error.
    x.set(default, "bCausedByWorld", Value::Bool(false));
    assert!(matches!(damage_gates(&x.vm, manager, Some(enemy), Some(damage)), Err(VmError::Script { .. })));
}
