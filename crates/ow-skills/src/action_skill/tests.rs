//! Tests of the start decision, the cooldown flags, the constraint check, the natives (on an in-memory VM whose
//! functions are declared by path, so the pack binds them as it binds the installed ones) and the cooldown drain.
//! Invented classes, objects and numbers (AGENTS.md rule 9).

use std::cell::Cell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_object::ids::FuncId;
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};

use super::oracle::*;
use super::*;
use crate::skills::fixture::Fx;

// ---------------------------------------------------------------------------------------------------------------
// The start decision
// ---------------------------------------------------------------------------------------------------------------

fn ready() -> StartInputs {
    StartInputs {
        has_action_skill: true,
        ..StartInputs::default()
    }
}

#[test]
fn the_start_decision_in_the_records_order() {
    assert_eq!(decide_start(ready()), StartDecision::Activate);
    assert_eq!(
        decide_start(StartInputs {
            on_ladder: true,
            ..ready()
        }),
        StartDecision::RefusedOnLadder,
        "the ladder first"
    );
    assert_eq!(
        decide_start(StartInputs {
            on_ladder: true,
            has_action_skill: false,
            ..ready()
        }),
        StartDecision::RefusedOnLadder
    );
    assert_eq!(
        decide_start(StartInputs {
            has_action_skill: false,
            ..ready()
        }),
        StartDecision::NoActionSkill
    );
    assert_eq!(
        decide_start(StartInputs {
            vehicle_seat_refuses: true,
            ..ready()
        }),
        StartDecision::RefusedVehicleSeat
    );
    // Phaselock while the lock runs: active, the pool full (not cooling down), no toggle, no active ability.
    let lock = StartInputs {
        active: true,
        on_cooldown: true,
        ..ready()
    };
    assert_eq!(decide_start(lock), StartDecision::Nothing);
    assert_eq!(
        decide_start(StartInputs {
            active_ability_allowed: true,
            ..lock
        }),
        StartDecision::ActiveAbility
    );
    assert_eq!(
        decide_start(StartInputs {
            can_be_toggled_off: true,
            ..lock
        }),
        StartDecision::ToggleOff
    );
    // During the cooldown: not active, cooling down; no cooldown ability for Phaselock.
    let cooldown = StartInputs {
        on_cooldown: true,
        cooling_down: true,
        ..ready()
    };
    assert_eq!(
        decide_start(cooldown),
        StartDecision::Nothing,
        "the cooldown refuses a start"
    );
    assert_eq!(
        decide_start(StartInputs {
            cooldown_ability_allowed: true,
            ..cooldown
        }),
        StartDecision::CooldownAbility
    );
}

#[test]
fn gunzerking_changes_only_the_cooling_down_flag() {
    // Active and cooling down: the active-ability branch needs "not cooling down"; Gunzerking's flag is
    // "cooling down and not active", false here, so it takes the active-ability branch.
    let both = StartInputs {
        active: true,
        on_cooldown: true,
        cooling_down: true,
        active_ability_allowed: true,
        cooldown_ability_allowed: true,
        ..ready()
    };
    assert_eq!(decide_start(both), StartDecision::CooldownAbility);
    assert_eq!(
        decide_start(StartInputs {
            gunzerking: true,
            ..both
        }),
        StartDecision::ActiveAbility
    );
    // The cooldown refusal itself is not waived.
    let cooldown = StartInputs {
        on_cooldown: true,
        cooling_down: true,
        gunzerking: true,
        ..ready()
    };
    assert_eq!(decide_start(cooldown), StartDecision::Nothing);
}

#[test]
fn cooldown_flags_read_the_pool() {
    assert_eq!(cooldown_flags(0.0, 13.0), (false, false), "ready at 0");
    assert_eq!(
        cooldown_flags(13.0, 13.0),
        (true, false),
        "refilled, not yet draining"
    );
    assert_eq!(cooldown_flags(4.5, 13.0), (true, true));
    assert_eq!(cooldown_flags(-0.0, 13.0), (false, false));
}

// ---------------------------------------------------------------------------------------------------------------
// The constraint check
// ---------------------------------------------------------------------------------------------------------------

fn constraint(
    evaluators: Vec<ObjRef>,
    on_activation: bool,
    while_active: bool,
    while_paused: bool,
    on_failure: SkillState,
) -> Constraint {
    Constraint {
        evaluators,
        on_activation,
        while_active,
        while_paused,
        on_failure,
    }
}

#[test]
fn constraints_apply_by_phase_and_fail_to_their_state() {
    let mut fx = Fx::new();
    let [a, b, c] = [0, 1, 2].map(|i| fx.obj("Core.Object", &format!("E{i}")));
    let failing = Cell::new(Vec::<ObjRef>::new());
    let holds = |e: ObjRef| -> Result<bool, SkillError> {
        let f = failing.take();
        let r = !f.contains(&e);
        failing.set(f);
        Ok(r)
    };
    let cs = vec![
        constraint(vec![a], true, false, false, SkillState::Deactivated),
        constraint(vec![b], false, true, false, SkillState::Paused),
        constraint(vec![c], false, true, true, SkillState::Deactivated),
    ];
    assert_eq!(
        state_from_constraints(&cs, true, SkillState::Deactivated, holds).unwrap(),
        SkillState::Active
    );
    failing.set(vec![a]);
    assert_eq!(
        state_from_constraints(&cs, true, SkillState::Deactivated, holds).unwrap(),
        SkillState::Deactivated
    );
    assert_eq!(
        state_from_constraints(&cs, false, SkillState::Active, holds).unwrap(),
        SkillState::Active,
        "activation only"
    );
    failing.set(vec![b]);
    assert_eq!(
        state_from_constraints(&cs, false, SkillState::Active, holds).unwrap(),
        SkillState::Paused
    );
    assert_eq!(
        state_from_constraints(&cs, false, SkillState::Paused, holds).unwrap(),
        SkillState::Active,
        "while active only"
    );
    failing.set(vec![b, c]);
    assert_eq!(
        state_from_constraints(&cs, false, SkillState::Active, holds).unwrap(),
        SkillState::Deactivated,
        "a later Deactivated failure overrides a Paused one"
    );
}

#[test]
fn a_constraint_stops_at_its_first_failing_evaluator() {
    let mut fx = Fx::new();
    let [a, b] = [0, 1].map(|i| fx.obj("Core.Object", &format!("E{i}")));
    let asked = Cell::new(0);
    let cs = vec![constraint(
        vec![a, b],
        true,
        false,
        false,
        SkillState::Deactivated,
    )];
    let r = state_from_constraints(&cs, true, SkillState::Deactivated, |_| {
        asked.set(asked.get() + 1);
        Ok(false)
    });
    assert_eq!(r.unwrap(), SkillState::Deactivated);
    assert_eq!(asked.get(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// The natives, on an in-memory VM
// ---------------------------------------------------------------------------------------------------------------

struct V {
    vm: Vm,
    controller: ObjRef,
    pawn: ObjRef,
    skill_def: ObjRef,
    manager: ObjRef,
}

fn declare(vm: &mut Vm, path: &str, params: Vec<PropKind>, result: Option<PropKind>) -> FuncId {
    let (class, name) = path.rsplit_once('.').unwrap();
    let owner = vm.find_class(class).unwrap();
    let leaf = class.rsplit('.').next().unwrap();
    let kinds: Vec<String> = params.iter().map(|_| "x".to_string()).collect();
    let key = format!("{leaf}.{name}({})", kinds.join(","));
    let params = params
        .into_iter()
        .enumerate()
        .map(|(i, k)| PropertyDecl::new(vm.space.names.intern(&format!("P{i}")), k))
        .collect();
    let mut decl = FunctionDecl::native(ow_core::ObjectPath::parse(path).unwrap(), key)
        .with_owner(owner)
        .with_params(params);
    if let Some(r) = result {
        decl = decl.with_result(PropertyDecl::new(vm.space.names.intern("ReturnValue"), r));
    }
    vm.declare_function(decl).unwrap()
}

fn vm_world() -> V {
    let mut fx = Fx::new();
    let object = fx.object;
    let obj = PropKind::Object { class: object };
    let objs = PropKind::Array(Box::new(obj.clone()));
    let state_enum =
        fx.w.space
            .classes
            .enum_by_path("WillowGame.Skill.ESkillState")
            .unwrap();
    let controller_c = fx.define("Engine.Controller", None, vec![("Pawn", obj.clone())]);
    fx.define(
        "WillowGame.WillowPlayerController",
        Some(controller_c),
        vec![],
    );
    let pawn_c = fx.define("Engine.Pawn", None, vec![]);
    fx.define("Engine.Vehicle", Some(pawn_c), vec![]);
    fx.define(
        "WillowGame.SkillEffectManager",
        None,
        vec![("ActiveSkills", objs.clone())],
    );
    fx.define(
        "WillowGame.Skill",
        None,
        vec![
            ("Definition", obj.clone()),
            ("SkillInstigator", obj.clone()),
            (
                "SkillState",
                PropKind::Byte {
                    enum_: Some(state_enum),
                },
            ),
        ],
    );
    fx.define(
        "WillowGame.WeaponActionAvailableExpressionEvaluator",
        None,
        vec![],
    );
    let skill_def = fx.skill("Pkg.Skill", "S", "SKILL_TYPE_Action", 1);
    fx.set(skill_def, "bSubjectToGradeRules", Value::Bool(true));
    let e = fx.effect(
        None,
        "TARGET_Self",
        "MT_PostAdd",
        fx.init(0.5, None, None, None),
        fx.init(0.5, None, None, None),
        1,
        1,
        &[],
    );
    let effects = fx.array(vec![e]);
    fx.set(skill_def, "SkillEffectDefinitions", effects);
    let controller = fx.obj("WillowGame.WillowPlayerController", "Lvl.Controller");
    let pawn = fx.obj("Engine.Pawn", "Lvl.Pawn");
    fx.set(controller, "Pawn", Value::Object(Some(pawn)));
    let manager = fx.obj("WillowGame.SkillEffectManager", "Lvl.Manager");
    let space = std::mem::take(&mut fx.w.space);
    let mut vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::standard().unwrap()),
        ParityMode::Off,
    );
    let byte = PropKind::Byte { enum_: None };
    let [
        active,
        min_grade,
        value,
        value_from,
        state,
        attr_eval,
        vehicle,
        health,
        weapon,
    ] = NATIVE_PATHS;
    declare(
        &mut vm,
        active,
        vec![obj.clone(), obj.clone()],
        Some(PropKind::Bool),
    );
    declare(
        &mut vm,
        min_grade,
        vec![PropKind::Int],
        Some(PropKind::Bool),
    );
    let effect_struct = vm
        .space
        .classes
        .struct_by_path("WillowGame.SkillDefinition.SkillEffectData")
        .unwrap();
    declare(
        &mut vm,
        value,
        vec![PropKind::Struct(effect_struct), PropKind::Int, obj.clone()],
        Some(PropKind::Float),
    );
    declare(
        &mut vm,
        value_from,
        vec![obj.clone(), PropKind::Int, PropKind::Int, obj.clone()],
        Some(PropKind::Float),
    );
    declare(&mut vm, state, vec![PropKind::Bool], Some(byte.clone()));
    for e in [attr_eval, vehicle, health, weapon] {
        declare(&mut vm, e, vec![obj.clone()], Some(PropKind::Bool));
    }
    V {
        vm,
        controller,
        pawn,
        skill_def,
        manager,
    }
}

fn f(vm: &mut Vm, path: &str) -> FuncId {
    vm.find_function(path).unwrap()
}

fn new_skill(v: &mut V, state: SkillState) -> ObjRef {
    let c = v.vm.find_class("WillowGame.Skill").unwrap();
    let s = v.vm.instantiate(c, "Skill").unwrap();
    let set = |vm: &mut Vm, n: &str, x: Value| {
        let p = vm.space.prop(c, n).unwrap();
        vm.space
            .write(s, ow_object::ids::PropAddr::scalar(p), x)
            .unwrap();
    };
    set(&mut v.vm, "Definition", Value::Object(Some(v.skill_def)));
    set(
        &mut v.vm,
        "SkillInstigator",
        Value::Object(Some(v.controller)),
    );
    set(&mut v.vm, "SkillState", Value::Byte(state.byte()));
    s
}

fn set_skills(v: &mut V, skills: &[ObjRef]) {
    let c = v.vm.space.heap.get(v.manager).unwrap().class;
    let p = v.vm.space.prop(c, "ActiveSkills").unwrap();
    let list = Value::Array(Rc::new(
        skills.iter().map(|s| Value::Object(Some(*s))).collect(),
    ));
    v.vm.space
        .write(v.manager, ow_object::ids::PropAddr::scalar(p), list)
        .unwrap();
}

#[test]
fn is_skill_active_counts_active_and_paused_skills_of_this_instigator() {
    let mut v = vm_world();
    Pack.register(&mut v.vm).unwrap();
    let (c0, p0) = (v.controller, v.pawn);
    let call = |v: &mut V, controller: ObjRef| -> Value {
        let id = f(&mut v.vm, NATIVE_PATHS[0]);
        v.vm.call(
            id,
            Some(v.manager),
            vec![
                Value::Object(Some(controller)),
                Value::Object(Some(v.skill_def)),
            ],
        )
        .unwrap()
        .ret
    };
    assert_eq!(call(&mut v, c0), Value::Bool(false), "no skills");
    let off = new_skill(&mut v, SkillState::Deactivated);
    set_skills(&mut v, &[off]);
    assert_eq!(
        call(&mut v, c0),
        Value::Bool(false),
        "a deactivated skill does not count"
    );
    let paused = new_skill(&mut v, SkillState::Paused);
    set_skills(&mut v, &[off, paused]);
    assert_eq!(
        call(&mut v, c0),
        Value::Bool(true),
        "paused counts as active"
    );
    assert_eq!(call(&mut v, p0), Value::Bool(false), "another instigator");
}

#[test]
fn the_grade_natives() {
    let mut v = vm_world();
    Pack.register(&mut v.vm).unwrap();
    let id = f(&mut v.vm, NATIVE_PATHS[1]);
    assert_eq!(
        v.vm.call(id, Some(v.skill_def), vec![Value::Int(0)])
            .unwrap()
            .ret,
        Value::Bool(false)
    );
    assert_eq!(
        v.vm.call(id, Some(v.skill_def), vec![Value::Int(1)])
            .unwrap()
            .ret,
        Value::Bool(true)
    );
    let from = f(&mut v.vm, NATIVE_PATHS[3]);
    let d0 = v.skill_def;
    let call = |v: &mut V, def: Option<ObjRef>, index: i32, grade: i32| {
        v.vm.call(
            from,
            None,
            vec![
                Value::Object(def),
                Value::Int(index),
                Value::Int(grade),
                Value::Object(Some(v.controller)),
            ],
        )
        .unwrap()
        .ret
    };
    assert_eq!(
        call(&mut v, Some(d0), 0, 3),
        Value::Float(1.5),
        "0.5 + 0.5 x (3 - 1)"
    );
    assert_eq!(
        call(&mut v, Some(d0), 0, 0),
        Value::Float(0.0),
        "below the start grade"
    );
    assert_eq!(
        call(&mut v, Some(d0), 1, 3),
        Value::Float(0.0),
        "an index outside the list"
    );
    assert_eq!(call(&mut v, None, 0, 3), Value::Float(0.0), "no definition");
    let direct = f(&mut v.vm, NATIVE_PATHS[2]);
    let effect = need(&mut v.vm, v.skill_def, "SkillEffectDefinitions")
        .unwrap()
        .elements()
        .unwrap()[0]
        .clone();
    let r =
        v.vm.call(
            direct,
            None,
            vec![effect, Value::Int(5), Value::Object(Some(v.controller))],
        )
        .unwrap()
        .ret;
    assert_eq!(r, Value::Float(2.5));
}

/// A native `Evaluate` for the test: answers what `answer` holds and counts the calls.
fn bind_answer(vm: &mut Vm, path: &str, answer: Rc<Cell<bool>>, calls: Rc<Cell<u32>>) {
    ow_combat::hit::bind_fn(vm, path, move |_c| {
        calls.set(calls.get() + 1);
        Ok(Value::Bool(answer.get()))
    })
    .unwrap();
}

#[test]
fn the_constraint_native_calls_each_evaluators_evaluate_with_the_instigator() {
    let mut v = vm_world();
    // An invented evaluator class whose Evaluate the test binds.
    let object = v.vm.find_class("Core.Object").unwrap();
    let ec =
        v.vm.space
            .classes
            .define("Invented.TestEvaluator", Some(object), vec![])
            .unwrap();
    let obj = PropKind::Object { class: object };
    declare(
        &mut v.vm,
        "Invented.TestEvaluator.Evaluate",
        vec![obj],
        Some(PropKind::Bool),
    );
    let answer = Rc::new(Cell::new(true));
    let calls = Rc::new(Cell::new(0));
    bind_answer(
        &mut v.vm,
        "Invented.TestEvaluator.Evaluate",
        answer.clone(),
        calls.clone(),
    );
    Pack.register(&mut v.vm).unwrap();
    let evaluator = v.vm.instantiate(ec, "Eval").unwrap();
    // The definition's one constraint: on activation and while active, failing to Deactivated.
    let cstruct =
        v.vm.space
            .classes
            .struct_by_path("WillowGame.SkillDefinition.SkillConstraintData")
            .unwrap();
    let layout = v.vm.space.classes.struct_layout(cstruct).unwrap();
    let mut cv = match layout.zero() {
        Value::Struct(s) => ow_object::value::StructValue::clone(s),
        _ => panic!(),
    };
    let entry = |vm: &Vm, n: &str| usize::try_from(layout_entry(vm, cstruct, n)).unwrap();
    cv.fields[entry(&v.vm, "Evaluator")] = Value::Object(Some(evaluator));
    cv.fields[entry(&v.vm, "bApplyConstraintOnActivatation")] = Value::Bool(true);
    cv.fields[entry(&v.vm, "bApplyConstraintWhileActive")] = Value::Bool(true);
    let dc = v.vm.space.heap.get(v.skill_def).unwrap().class;
    let p = v.vm.space.prop(dc, "SkillConstraints").unwrap();
    v.vm.space
        .write(
            v.skill_def,
            ow_object::ids::PropAddr::scalar(p),
            Value::Array(Rc::new(vec![Value::Struct(Rc::new(cv))])),
        )
        .unwrap();
    let skill = new_skill(&mut v, SkillState::Deactivated);
    let id = f(&mut v.vm, NATIVE_PATHS[4]);
    assert_eq!(
        v.vm.call(id, Some(skill), vec![Value::Bool(true)])
            .unwrap()
            .ret,
        Value::Byte(1),
        "passes: Active"
    );
    answer.set(false);
    assert_eq!(
        v.vm.call(id, Some(skill), vec![Value::Bool(true)])
            .unwrap()
            .ret,
        Value::Byte(0),
        "fails: Deactivated"
    );
    assert_eq!(
        v.vm.call(id, Some(skill), vec![Value::Bool(false)])
            .unwrap()
            .ret,
        Value::Byte(1),
        "deactivated and not the activation check: nothing applies"
    );
    assert_eq!(calls.get(), 2);
}

fn layout_entry(vm: &Vm, s: ow_object::ids::StructId, name: &str) -> u32 {
    let layout = vm.space.classes.struct_layout(s).unwrap();
    layout.field(vm.space.names.find(name).unwrap()).unwrap().0
}

#[test]
fn the_skill_evaluators_read_the_pawn_or_refuse_what_is_not_read() {
    let mut v = vm_world();
    let health_c =
        v.vm.space
            .classes
            .by_path("WillowGame.HealthStateExpressionEvaluator")
            .unwrap();
    let vehicle_c =
        v.vm.space
            .classes
            .by_path("WillowGame.VehiclePassengerExpressionEvaluator")
            .unwrap();
    declare(
        &mut v.vm,
        "Engine.Pawn.IsAliveAndWell",
        vec![],
        Some(PropKind::Bool),
    );
    let alive = Rc::new(Cell::new(true));
    let calls = Rc::new(Cell::new(0));
    bind_answer(
        &mut v.vm,
        "Engine.Pawn.IsAliveAndWell",
        alive.clone(),
        calls.clone(),
    );
    Pack.register(&mut v.vm).unwrap();
    let health = v.vm.instantiate(health_c, "Health").unwrap();
    let set = |vm: &mut Vm, o: ObjRef, n: &str, x: Value| {
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, n).unwrap();
        vm.space
            .write(o, ow_object::ids::PropAddr::scalar(p), x)
            .unwrap();
    };
    set(&mut v.vm, health, "bHealthy", Value::Bool(true));
    let id = f(&mut v.vm, NATIVE_PATHS[7]);
    assert_eq!(
        v.vm.call(id, Some(health), vec![Value::Object(Some(v.controller))])
            .unwrap()
            .ret,
        Value::Bool(true)
    );
    alive.set(false);
    assert_eq!(
        v.vm.call(id, Some(health), vec![Value::Object(Some(v.controller))])
            .unwrap()
            .ret,
        Value::Bool(false)
    );
    assert_eq!(
        v.vm.call(id, Some(health), vec![Value::Object(None)])
            .unwrap()
            .ret,
        Value::Bool(false),
        "no pawn"
    );
    set(&mut v.vm, health, "bDead", Value::Bool(true));
    let e =
        v.vm.call(id, Some(health), vec![Value::Object(Some(v.controller))])
            .unwrap_err();
    assert!(
        matches!(e, VmError::NotYet(ref n) if n.missing == Missing::NotRead),
        "two flags: {e:?}"
    );
    // The vehicle test: on foot gives bNotInVehicle; a vehicle pawn is NotYet.
    let vehicle = v.vm.instantiate(vehicle_c, "Vehicle").unwrap();
    set(&mut v.vm, vehicle, "bNotInVehicle", Value::Bool(true));
    let id = f(&mut v.vm, NATIVE_PATHS[6]);
    assert_eq!(
        v.vm.call(id, Some(vehicle), vec![Value::Object(Some(v.controller))])
            .unwrap()
            .ret,
        Value::Bool(true)
    );
    let car_c = v.vm.find_class("Engine.Vehicle").unwrap();
    let car = v.vm.instantiate(car_c, "Car").unwrap();
    set(&mut v.vm, v.controller, "Pawn", Value::Object(Some(car)));
    assert!(matches!(
        v.vm.call(id, Some(vehicle), vec![Value::Object(Some(v.controller))]),
        Err(VmError::NotYet(_))
    ));
}

#[test]
fn the_pack_skips_functions_the_vm_does_not_declare() {
    let mut fx = Fx::new();
    let space = std::mem::take(&mut fx.w.space);
    let mut vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::standard().unwrap()),
        ParityMode::Off,
    );
    Pack.register(&mut vm).unwrap();
    crate::tree::Pack.register(&mut vm).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// The cooldown drain
// ---------------------------------------------------------------------------------------------------------------

struct P {
    vm: Vm,
    pool: ObjRef,
    def: ObjRef,
}

fn pool_world(class: &str) -> P {
    let mut fx = Fx::new();
    let object = fx.object;
    let obj = PropKind::Object { class: object };
    let objs = PropKind::Array(Box::new(obj.clone()));
    fx.define(
        "Engine.ResourceDefinition",
        None,
        vec![("bIntegerOnlyUpdates", PropKind::Bool)],
    );
    fx.define(
        "Engine.ResourcePoolDefinition",
        None,
        vec![
            ("Resource", obj.clone()),
            ("OnResourceDepleted", objs.clone()),
            ("OnResourceNotDepleted", objs.clone()),
            ("OnResourceRegenerated", objs.clone()),
            ("OnResourceNotRegenerated", objs.clone()),
        ],
    );
    let mut fields: Vec<(&str, PropKind)> = [
        "CurrentValue",
        "MaxValue",
        "MinValue",
        "ConsumptionRate",
        "ActiveRegenerationRate",
        "PassiveRegenerationRate",
        "OnIdleRegenerationRate",
        "OnIdleRegenerationDelay",
        "RateRemainder",
        "PoolIdleDelayStartTime",
    ]
    .into_iter()
    .map(|n| (n, PropKind::Float))
    .collect();
    fields.extend([
        ("RegenerationDisabled", PropKind::Int),
        ("bIsAuthoritative", PropKind::Bool),
        ("RegenerationPool", obj.clone()),
        ("Definition", obj.clone()),
    ]);
    let base = fx.define("Engine.ResourcePool", None, fields);
    if class != "Engine.ResourcePool" {
        fx.define(class, Some(base), vec![]);
    }
    let resource = fx.obj("Engine.ResourceDefinition", "D.Resource");
    let def = fx.obj("Engine.ResourcePoolDefinition", "D.CooldownPool");
    fx.set(def, "Resource", Value::Object(Some(resource)));
    let pool = fx.obj(class, "Lvl.Pool");
    fx.set(pool, "Definition", Value::Object(Some(def)));
    fx.set(pool, "bIsAuthoritative", Value::Bool(true));
    fx.set(pool, "MaxValue", Value::Float(13.0));
    fx.set(pool, "ConsumptionRate", Value::Float(1.0));
    let space = std::mem::take(&mut fx.w.space);
    let vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::standard().unwrap()),
        ParityMode::Off,
    );
    P { vm, pool, def }
}

fn set_pool(p: &mut P, n: &str, v: Value) {
    let c = p.vm.space.heap.get(p.pool).unwrap().class;
    let id = p.vm.space.prop(c, n).unwrap();
    p.vm.space
        .write(p.pool, ow_object::ids::PropAddr::scalar(id), v)
        .unwrap();
}

fn current(p: &mut P) -> f32 {
    number(
        "CurrentValue",
        &need(&mut p.vm, p.pool, "CurrentValue").unwrap(),
    )
    .unwrap()
}

#[test]
fn the_cooldown_drains_one_per_second_to_the_minimum() {
    let mut p = pool_world("Engine.ResourcePool");
    set_pool(&mut p, "CurrentValue", Value::Float(13.0));
    for second in 1..=13 {
        let d = drain(&mut p.vm, p.pool, 1.0).unwrap();
        assert_eq!(d.rate, -1.0);
        assert_eq!(d.after, 13.0 - second as f32);
    }
    let d = drain(&mut p.vm, p.pool, 1.0).unwrap();
    assert_eq!((d.before, d.after), (0.0, 0.0), "clamped at the minimum");
    // Ten frames of 0.1 s from 1.0 reach the f32 sum of ten steps, as the per-frame rule adds them.
    set_pool(&mut p, "CurrentValue", Value::Float(1.0));
    let mut expect = 1.0f32;
    for _ in 0..10 {
        expect = (expect + -0.1).max(0.0);
        drain(&mut p.vm, p.pool, 0.1).unwrap();
    }
    assert_eq!(current(&mut p).to_bits(), expect.to_bits());
}

#[test]
fn a_zero_net_rate_holds_the_cooldown_and_regeneration_disabled_stops_it() {
    let mut p = pool_world("Engine.ResourcePool");
    set_pool(&mut p, "CurrentValue", Value::Float(13.0));
    // The cooldown manager's PreAdd -1 on the consumption rate: (1 + -1) = 0, no change (PHASELOCK_STOCK_DATA.md).
    set_pool(&mut p, "ConsumptionRate", Value::Float(0.0));
    assert_eq!(drain(&mut p.vm, p.pool, 1.0).unwrap().after, 13.0);
    set_pool(&mut p, "ConsumptionRate", Value::Float(1.0));
    set_pool(&mut p, "RegenerationDisabled", Value::Int(1));
    assert_eq!(drain(&mut p.vm, p.pool, 1.0).unwrap().after, 13.0);
}

#[test]
fn what_the_drain_has_not_read_is_not_yet_before_any_write() {
    let mut p = pool_world("Invented.HealthLikePool");
    set_pool(&mut p, "CurrentValue", Value::Float(5.0));
    assert!(
        matches!(drain(&mut p.vm, p.pool, 1.0), Err(VmError::NotYet(_))),
        "a subclass"
    );
    assert_eq!(current(&mut p), 5.0);
    let mut p = pool_world("Engine.ResourcePool");
    set_pool(&mut p, "CurrentValue", Value::Float(5.0));
    let other =
        p.vm.instantiate(p.vm.space.heap.get(p.pool).unwrap().class, "Other")
            .unwrap();
    set_pool(&mut p, "RegenerationPool", Value::Object(Some(other)));
    assert!(
        matches!(drain(&mut p.vm, p.pool, 1.0), Err(VmError::NotYet(_))),
        "a regeneration pool"
    );
    set_pool(&mut p, "RegenerationPool", Value::Object(None));
    // A definition with depleted behaviors: NotYet only on the step that reaches the minimum.
    let c = p.vm.space.heap.get(p.def).unwrap().class;
    let prop = p.vm.space.prop(c, "OnResourceDepleted").unwrap();
    let b = p.vm.instantiate(c, "Behavior").unwrap();
    p.vm.space
        .write(
            p.def,
            ow_object::ids::PropAddr::scalar(prop),
            Value::Array(Rc::new(vec![Value::Object(Some(b))])),
        )
        .unwrap();
    assert_eq!(drain(&mut p.vm, p.pool, 1.0).unwrap().after, 4.0);
    assert!(matches!(
        drain(&mut p.vm, p.pool, 10.0),
        Err(VmError::NotYet(_))
    ));
    assert_eq!(current(&mut p), 4.0, "nothing written");
    // Idle regeneration needs the world time: no world is NotYet.
    set_pool(&mut p, "OnIdleRegenerationRate", Value::Float(2.0));
    assert!(matches!(
        drain(&mut p.vm, p.pool, 1.0),
        Err(VmError::NotYet(_))
    ));
}

#[test]
fn the_skill_behavior_handlers_cover_activate_and_deactivate() {
    let mut fx = Fx::new();
    let consumer = fx.obj("Core.Object", "Consumer");
    let classes: Vec<&str> = skill_behavior_handlers(consumer)
        .iter()
        .map(|(c, _)| *c)
        .collect();
    assert_eq!(classes, BEHAVIOR_CLASSES.to_vec());
}

/// Review r1 of PR #342, item 1: a pool resting at its minimum with a negative rate stores nothing, so its idle stamp
/// stays; the edge test uses the value SetCurrentValue would store, so an integer truncation onto the minimum counts.
#[test]
fn the_drain_tests_the_stored_value_and_keeps_the_idle_stamp_at_rest() {
    let mut p = pool_world("Engine.ResourcePool");
    set_pool(&mut p, "CurrentValue", Value::Float(0.0));
    set_pool(&mut p, "PoolIdleDelayStartTime", Value::Float(7.0));
    let d = drain(&mut p.vm, p.pool, 1.0).unwrap();
    assert_eq!((d.before, d.after), (0.0, 0.0));
    let stamp = number(
        "PoolIdleDelayStartTime",
        &need(&mut p.vm, p.pool, "PoolIdleDelayStartTime").unwrap(),
    )
    .unwrap();
    assert_eq!(stamp, 7.0, "no SetCurrentValue, so the stamp did not move");

    // Integer-only: from 1 with a step of -0.6 the target is 0.4, which the pool truncates to 0 (the minimum).
    let mut p = pool_world("Engine.ResourcePool");
    let resource = obj_value("Resource", &need(&mut p.vm, p.def, "Resource").unwrap())
        .unwrap()
        .unwrap();
    let rc = p.vm.space.heap.get(resource).unwrap().class;
    let id = p.vm.space.prop(rc, "bIntegerOnlyUpdates").unwrap();
    p.vm.space
        .write(
            resource,
            ow_object::ids::PropAddr::scalar(id),
            Value::Bool(true),
        )
        .unwrap();
    set_pool(&mut p, "CurrentValue", Value::Float(1.0));
    set_pool(&mut p, "ConsumptionRate", Value::Float(0.6));
    let c = p.vm.space.heap.get(p.def).unwrap().class;
    let prop = p.vm.space.prop(c, "OnResourceDepleted").unwrap();
    let b = p.vm.instantiate(c, "Behavior").unwrap();
    p.vm.space
        .write(
            p.def,
            ow_object::ids::PropAddr::scalar(prop),
            Value::Array(Rc::new(vec![Value::Object(Some(b))])),
        )
        .unwrap();
    assert!(
        matches!(drain(&mut p.vm, p.pool, 1.0), Err(VmError::NotYet(_))),
        "the truncation reaches the minimum"
    );
    assert_eq!(current(&mut p), 1.0, "nothing written");
    p.vm.space
        .write(
            p.def,
            ow_object::ids::PropAddr::scalar(prop),
            Value::Array(Rc::new(vec![])),
        )
        .unwrap();
    let d = drain(&mut p.vm, p.pool, 1.0).unwrap();
    assert_eq!(d.after, 0.0);
    // At 0 with the same step the value stays 0 but the remainder would move: not read.
    assert!(matches!(
        drain(&mut p.vm, p.pool, 1.0),
        Err(VmError::NotYet(_))
    ));
}

/// Review r1 of PR #342, item 1: a delta that is not finite or is negative is refused before anything is written.
#[test]
fn the_drain_refuses_a_bad_delta() {
    let mut p = pool_world("Engine.ResourcePool");
    set_pool(&mut p, "CurrentValue", Value::Float(5.0));
    for dt in [f32::NAN, f32::INFINITY, -0.5] {
        assert!(
            matches!(drain(&mut p.vm, p.pool, dt), Err(VmError::Script { .. })),
            "{dt}"
        );
    }
    assert_eq!(current(&mut p), 5.0);
}

/// Review r1 of PR #342, item 6: data of the wrong kind is an error, never read as "nothing" or false, and the weapon
/// evaluator with no source is `NotYet` (what the native answers is not read).
#[test]
fn wrong_kinds_and_a_missing_source_are_errors_not_answers() {
    let mut v = vm_world();
    let object = v.vm.find_class("Core.Object").unwrap();
    let int = PropKind::Int;
    let decls = vec![
        PropertyDecl::new(v.vm.space.names.intern("ActiveSkills"), int.clone()),
        PropertyDecl::new(v.vm.space.names.intern("bSubjectToGradeRules"), int),
    ];
    let bad =
        v.vm.space
            .classes
            .define("Invented.BadData", Some(object), decls)
            .unwrap();
    let o = v.vm.instantiate(bad, "Bad").unwrap();
    Pack.register(&mut v.vm).unwrap();
    assert!(matches!(
        is_skill_active(&mut v.vm, o, Some(v.controller), Some(v.skill_def)),
        Err(VmError::Script { .. })
    ));
    let min_grade = f(&mut v.vm, NATIVE_PATHS[1]);
    assert!(matches!(
        v.vm.call(min_grade, Some(o), vec![Value::Int(1)]),
        Err(VmError::Script { .. })
    ));
    let weapon_c =
        v.vm.find_class("WillowGame.WeaponActionAvailableExpressionEvaluator")
            .unwrap();
    let weapon = v.vm.instantiate(weapon_c, "Weapon").unwrap();
    let id = f(&mut v.vm, NATIVE_PATHS[8]);
    assert!(matches!(
        v.vm.call(id, Some(weapon), vec![Value::Object(None)]),
        Err(VmError::NotYet(_))
    ));
}

/// Review r1 of PR #342, item 7: the pack collects the output links a skill behavior's script selects when it holds
/// the `ActivateBehaviorOutputLink` binding; another pack's binding is recorded so the behavior run answers `NotYet`
/// instead of "no links"; an undeclared function leaves nothing to collect.
#[test]
fn the_output_link_collector_knows_who_holds_the_binding() {
    let declare_link = |vm: &mut Vm| {
        let object = vm.find_class("Core.Object").unwrap();
        vm.space
            .classes
            .define("GearboxFramework.BehaviorKernel", Some(object), vec![])
            .unwrap();
        let struct_kind = PropKind::Int;
        declare(vm, OUTPUT_LINK, vec![struct_kind, PropKind::Int], None)
    };
    // Bound here: a call appends to the innermost frame.
    let mut v = vm_world();
    let link = declare_link(&mut v.vm);
    Pack.register(&mut v.vm).unwrap();
    let state = selected(&v.vm).unwrap();
    assert_eq!(state.borrow().mine, Some(true));
    state.borrow_mut().frames.push(Vec::new());
    v.vm.call(link, None, vec![Value::Int(0), Value::Int(2)])
        .unwrap();
    v.vm.call(link, None, vec![Value::Int(0), Value::Int(2)])
        .unwrap();
    assert_eq!(
        state.borrow_mut().frames.pop(),
        Some(vec![2, 2]),
        "call order, duplicates kept"
    );
    // Bound by another pack first.
    let mut v = vm_world();
    let link = declare_link(&mut v.vm);
    let key = v.vm.function(link).unwrap().native_key.clone().unwrap();
    v.vm.register_native(
        &key,
        Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::None)),
    )
    .unwrap();
    Pack.register(&mut v.vm).unwrap();
    assert_eq!(selected(&v.vm).unwrap().borrow().mine, Some(false));
    // Undeclared.
    let mut v = vm_world();
    Pack.register(&mut v.vm).unwrap();
    assert_eq!(selected(&v.vm).unwrap().borrow().mine, None);
}
