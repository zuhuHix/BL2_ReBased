//! Owner: lane W6-SKILLS. The action skill and the skill manager's natives: the start decision and its refusals
//! (`ServerStartActionSkill`, restated), `SkillEffectManager.IsSkillActive`, the constraint check
//! (`Skill.CalculateStateBasedOnConstraints`) with the skill constraint evaluators, the grade natives, the cooldown
//! pool's per-frame drain, and the skill behavior handlers. AI-assisted (Claude Code).
//!
//! **Evidence.** NATIVE_ACTION_SKILL.md (lane G17), NATIVE_SKILLS.md sections 4-5, NATIVE_PHASELOCK_TARGETING.md
//! section 4, read from the executable and the installed script. Every rule is **UNVERIFIED in the running game**
//! except these, seen live: a cast during a reload starts and aborts the reload, a cast during a weapon swap's
//! put-down stays idle and uses no cooldown (2 of 2, 2026-10-02); with a target held, Maya going down ended the skill
//! within about 0.14 s (one cast, 2026-11-12); a base first lock released at 5.7 s and ended at 6.7 s, and a miss
//! fizzled and ended after 1 s, within about two frames (two casts each, 2026-11-12).
//!
//! **What runs as installed script** (architecture R2): `StartActiveSkillCooldown` (the refill, through the pool's
//! own `RefillPercentage`), `IsActionSkillOnCooldown`, `ActionSkillCallback`, `ServerStartActionSkill`,
//! `SkillEffectManager.ActivateSkill`, `Skill.Activate` and `Deactivate`, and the behaviors' `ApplyBehaviorToContext`.
//! This module binds the natives they call. [`oracle::decide_start`] restates `ServerStartActionSkill`'s decision list
//! as an oracle for a differential test against the script (Open), not for gameplay callers.
//!
//! **Bound by function path** ([`ow_combat::hit::bind`]): a native whose function the VM does not declare is skipped,
//! and the first binding of a key wins, so this pack and `ow-combat`'s evaluator natives never collide.
//!
//! **Not here, and why.** `Skill.AddSkillEffect` and `Skill.AdjustModifiers` (the record struct of an applied
//! effect is not named in the record), the manager's per-frame skill refresh, `NotifySkillEvent`, the `ActionSkill`
//! event natives other than the two below, `GetActionSkillDuration` and `SetActionSkillTime` (how a controller reaches its skill manager is not read). Each
//! stays unbound, so a gameplay run refuses at it (`StubPolicy::Refuse`).
//!
//! **The `ActionSkill` event natives** `OnActionSkillActivated` and `OnActionSkillDeactivated` (section 9; lane
//! FIX-PHASELOCK-CAST part B, decision 2026-11-19-12): Off only, through the behavior kernel's consumer table
//! ([`EVENT_PATHS`]; the rule and what stays `NotYet` are on `register_events`). This pack registers the kernel pack
//! for that table, as `ow-combat`'s does. Reference: unbound (no C++ stand-in).

use std::rc::Rc;

use ow_attr::api::{Comparison, EvalCx, ExprData, ExprMode, Operand2Usage};
use ow_attr::eval::{elements, field, number, object};
use ow_behavior::kernel::{BehaviorId, Provider};
use ow_behavior::provider::BehaviorError;
use ow_behavior::registry::Handler;
use ow_combat::hit::bind;
use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use crate::skills::{
    Constraint, SkillError, SkillState, modifier_value, passes_min_grade_test, read_effect,
    read_skill,
};

/// The record of the action skill.
pub const RECORD: &str = "NATIVE_ACTION_SKILL.md";
/// The record of the cooldown drain.
pub const RECORD_DRAIN: &str = "NATIVE_SKILLS.md section 5.2";

/// The physics value of a ladder (`EPhysics` 9, stock UE3 order; NATIVE_ACTION_SKILL.md "Enums used").
pub const PHYS_LADDER: u8 = 9;

// ---------------------------------------------------------------------------------------------------------------
// The start decision (`ServerStartActionSkill`, restated)
// ---------------------------------------------------------------------------------------------------------------

/// **Oracles, not for gameplay callers.** `ServerStartActionSkill` and `IsActionSkillOnCooldown` are world-only installed
/// script, which the game runs (architecture R2). These restate their decision for a differential test against that
/// script, which is Open until the behavior kernel's script natives are bound; until then nothing compares them with
/// the script, so no gameplay code may call them in its place.
pub mod oracle {

    /// What `ServerStartActionSkill` reads (section 2), gathered by the caller.
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct StartInputs {
        /// The pawn (a vehicle's driver when the pawn is a vehicle) is on a ladder (`Physics` 9).
        pub on_ladder: bool,
        /// The tree knows an action skill.
        pub has_action_skill: bool,
        /// The cooldown pool is valid and its current value is above 0.
        pub on_cooldown: bool,
        /// On cooldown and the current value is below the pool's maximum.
        pub cooling_down: bool,
        /// `IsSkillActive` for the action skill and this controller.
        pub active: bool,
        /// The definition is the one named `Skill_Gunzerking` (its one special case).
        pub gunzerking: bool,
        /// In a vehicle whose seat is not an attached-rider seat.
        pub vehicle_seat_refuses: bool,
        /// `bCanBeToggledOff`.
        pub can_be_toggled_off: bool,
        /// The action skill object allows another automatic active-ability activation now.
        pub active_ability_allowed: bool,
        /// The action skill object allows another automatic cooldown-ability activation now.
        pub cooldown_ability_allowed: bool,
    }

    /// What `ServerStartActionSkill` does (section 2). The target variable is cleared after every branch.
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StartDecision {
        /// On a ladder: nothing else changes (the target is not even stored).
        RefusedOnLadder,
        /// No action skill in the tree: nothing.
        NoActionSkill,
        /// In a vehicle seat that is not an attached rider's: nothing.
        RefusedVehicleSeat,
        /// Ask the manager to activate it with the tree's grade and `ActionSkillCallback` (the manager may still refuse:
        /// activation not allowed, the grade test, or a failed constraint inside `Skill.Activate`).
        Activate,
        /// A second press of a skill that can be toggled off: deactivate it.
        ToggleOff,
        /// The active-ability path (`ServerStartActionSkillActiveAbility`).
        ActiveAbility,
        /// The cooldown-ability path (`ServerStartActionSkillCooldownAbility`).
        CooldownAbility,
        /// None of the branches applies (a press during Phaselock's lock or its cooldown).
        Nothing,
    }

    /// `ServerStartActionSkill`'s decision list (NATIVE_ACTION_SKILL.md section 2, the consistent reading; UNVERIFIED in
    /// the game, except that a cast during a put-down uses no cooldown, which is the constraint inside `Activate`).
    pub fn decide_start(i: StartInputs) -> StartDecision {
        if i.on_ladder {
            return StartDecision::RefusedOnLadder;
        }
        if !i.has_action_skill {
            return StartDecision::NoActionSkill;
        }
        // Only for Skill_Gunzerking the cooling-down flag is "cooling down and not active"; the cooldown refusal is not
        // waived (Corrections, NATIVE_ACTION_SKILL.md).
        let cooling_down = if i.gunzerking {
            i.cooling_down && !i.active
        } else {
            i.cooling_down
        };
        if !i.on_cooldown && !i.active {
            if i.vehicle_seat_refuses {
                return StartDecision::RefusedVehicleSeat;
            }
            return StartDecision::Activate;
        }
        if i.can_be_toggled_off {
            return StartDecision::ToggleOff;
        }
        if i.active && !cooling_down {
            return if i.active_ability_allowed {
                StartDecision::ActiveAbility
            } else {
                StartDecision::Nothing
            };
        }
        if cooling_down {
            return if i.cooldown_ability_allowed {
                StartDecision::CooldownAbility
            } else {
                StartDecision::Nothing
            };
        }
        StartDecision::Nothing
    }

    /// The cooldown flags of section 5 from the pool's current and maximum values: `(on_cooldown, cooling_down)`, where
    /// on cooldown is `current > 0` and cooling down is on cooldown and `current < max`. Ready is `current <= 0`.
    pub fn cooldown_flags(current: f32, max: f32) -> (bool, bool) {
        let on = current > 0.0;
        (on, on && current < max)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The constraint check
// ---------------------------------------------------------------------------------------------------------------

/// `Skill.CalculateStateBasedOnConstraints(bActivation)` (NATIVE_PHASELOCK_TARGETING.md section 4): a constraint
/// applies when `bApplyConstraintOnActivatation` and this is the activation check, or `bApplyConstraintWhileActive`
/// and the skill is Active, or `bApplyConstraintWhilePaused` and it is Paused. It holds when every one of its
/// evaluators holds (`holds`, in order, stopping at the first false). A failing constraint sets the result to its
/// `OnFailure`, and a `Deactivated` failure stops the check; with no failure the result is Active.
///
/// The record says "the first failing constraint sets the result ... if that is `SKILL_Deactivated` the check stops
/// there"; read here as: each failure sets the result, and only a Deactivated one stops (a Paused failure followed by
/// a Deactivated one gives Deactivated). UNVERIFIED; all three of Phaselock's constraints fail to Deactivated, so
/// both readings agree for it.
pub fn state_from_constraints(
    constraints: &[Constraint],
    activation: bool,
    current: SkillState,
    mut holds: impl FnMut(ObjRef) -> Result<bool, SkillError>,
) -> Result<SkillState, SkillError> {
    let mut result = SkillState::Active;
    for c in constraints {
        let applies = (c.on_activation && activation)
            || (c.while_active && current == SkillState::Active)
            || (c.while_paused && current == SkillState::Paused);
        if !applies {
            continue;
        }
        let mut ok = true;
        for &e in &c.evaluators {
            if !holds(e)? {
                ok = false;
                break;
            }
        }
        if !ok {
            result = c.on_failure;
            if c.on_failure == SkillState::Deactivated {
                break;
            }
        }
    }
    Ok(result)
}

/// The skill state of the byte `b` (`ESkillState`).
pub fn skill_state_of_byte(b: u8) -> Option<SkillState> {
    match b {
        0 => Some(SkillState::Deactivated),
        1 => Some(SkillState::Active),
        2 => Some(SkillState::Paused),
        _ => None,
    }
}

fn script_error(function: &str, message: impl Into<String>) -> VmError {
    VmError::Script {
        function: function.to_string(),
        message: message.into(),
    }
}

/// Property `name` of `o` (loaded first), `None` when the class has none.
pub(crate) fn get(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<Value>, VmError> {
    let mut cx = EvalCx::new(vm);
    Ok(ow_attr::eval::property(&mut cx, o, name)?)
}

/// Property `name` of `o`, which the class must have.
pub(crate) fn need(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let mut cx = EvalCx::new(vm);
    Ok(ow_attr::eval::required(&mut cx, o, name)?)
}

/// An object (or interface) value; `Value::None` reads as no object; any other kind is bad data, never "no object"
/// (review r1 of PR #342, item 6).
fn obj_value(what: &str, v: &Value) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(script_error(
            what,
            format!("expected an object, found {}", other.kind_name()),
        )),
    }
}

/// The function `name` as `o`'s class sees it: its method (the loaded code, from the class up), else a function
/// declared by path on the class or a super class (a class defined without a package declares its functions that
/// way). `None` when neither has one.
pub(crate) fn method_of(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
) -> Result<Option<ow_object::ids::FuncId>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    if let Some(f) = vm.find_method(class, name, None)? {
        return Ok(Some(f));
    }
    let mut cursor = Some(class);
    let mut steps = 0usize;
    while let Some(c) = cursor {
        steps += 1;
        if steps > crate::skills::DEPTH_BUDGET {
            break;
        }
        let layout = vm.space.classes.layout(c);
        let path = format!("{}.{name}", layout.path().as_str());
        cursor = layout.super_class();
        if let Ok(f) = vm.find_function(&path) {
            return Ok(Some(f));
        }
    }
    Ok(None)
}

/// Calls `name` on `o` (its class's method, script or native) and answers its bool result.
pub(crate) fn call_bool(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    args: Vec<Value>,
) -> Result<bool, VmError> {
    let Some(f) = method_of(vm, o, name)? else {
        return Err(NotYet::not_read(
            format!(
                "{name} on {} (the class declares no such function)",
                ow_attr::eval::class_path(&vm.space, o)
            ),
            RECORD,
        )
        .into());
    };
    match vm.call(f, Some(o), args)?.ret {
        Value::Bool(b) => Ok(b),
        other => Err(script_error(
            name,
            format!("expected a bool result, found {}", other.kind_name()),
        )),
    }
}

/// The pawn an evaluator's context names: the object itself when it is a pawn, a controller's `Pawn`.
fn pawn_of(vm: &mut Vm, o: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let space = &vm.space;
    if crate::skills::is_a(space, o, "Engine.Pawn")? {
        return Ok(Some(o));
    }
    if crate::skills::is_a(space, o, "Engine.Controller")? {
        return obj_value("Pawn", &need(vm, o, "Pawn")?);
    }
    Ok(None)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this
        .ok_or_else(|| script_error(what, "called without an object"))
}

fn arg_obj(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    obj_value("an argument", c.arg(i)?)
}

fn arg_int(c: &NativeCall<'_>, i: usize, what: &str) -> Result<i32, VmError> {
    match c.arg(i)? {
        Value::Int(v) => Ok(*v),
        Value::Byte(b) => Ok(i32::from(*b)),
        other => Err(script_error(
            what,
            format!("argument {i}: expected an int, found {}", other.kind_name()),
        )),
    }
}

fn byte_of(v: &Value, what: &str) -> Result<u8, VmError> {
    match v {
        Value::Byte(b) => Ok(*b),
        other => Err(script_error(
            what,
            format!("expected a byte, found {}", other.kind_name()),
        )),
    }
}

/// The evaluator `Evaluate(Object)` of `evaluator` with `source`, through the VM (whichever binding it has).
pub fn evaluate(vm: &mut Vm, evaluator: ObjRef, source: Option<ObjRef>) -> Result<bool, VmError> {
    ow_attr::eval::ensure_loaded(&mut EvalCx::new(vm), evaluator)?;
    call_bool(vm, evaluator, "Evaluate", vec![Value::Object(source)])
}

/// The native of `Skill.CalculateStateBasedOnConstraints(bActivation)` on the skill `skill`.
pub fn calculate_state(
    vm: &mut Vm,
    skill: ObjRef,
    activation: bool,
) -> Result<SkillState, VmError> {
    let definition =
        obj_value("Definition", &need(vm, skill, "Definition")?)?.ok_or_else(|| {
            VmError::from(NotYet::not_read(
                "a constraint check on a skill with no definition",
                RECORD,
            ))
        })?;
    let current = skill_state_of_byte(byte_of(&need(vm, skill, "SkillState")?, "SkillState")?)
        .ok_or_else(|| {
            script_error(
                "CalculateStateBasedOnConstraints",
                "SkillState outside ESkillState",
            )
        })?;
    let instigator = obj_value("SkillInstigator", &need(vm, skill, "SkillInstigator")?)?;
    let def = read_skill(&mut EvalCx::new(vm), definition)?;
    let state = state_from_constraints(&def.constraints, activation, current, |e| {
        Ok(evaluate(vm, e, instigator)?)
    })?;
    Ok(state)
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

/// `SkillEffectManager.IsSkillActive(Controller, SkillDefinition)` (section 3): any skill of the manager's
/// `ActiveSkills` with this definition, this instigator and a state other than Deactivated (Paused counts).
pub fn is_skill_active(
    vm: &mut Vm,
    manager: ObjRef,
    controller: Option<ObjRef>,
    definition: Option<ObjRef>,
) -> Result<bool, VmError> {
    let list = need(vm, manager, "ActiveSkills")?;
    let entries = list.elements().ok_or_else(|| {
        script_error(
            "IsSkillActive",
            format!(
                "ActiveSkills: expected an array, found {}",
                list.kind_name()
            ),
        )
    })?;
    let mut skills = Vec::with_capacity(entries.len());
    for e in entries {
        // A None entry holds no skill; any other kind is bad data.
        if let Some(o) = obj_value("ActiveSkills", e)? {
            skills.push(o);
        }
    }
    for s in skills {
        let d = obj_value("Definition", &need(vm, s, "Definition")?)?;
        let i = obj_value("SkillInstigator", &need(vm, s, "SkillInstigator")?)?;
        let state = byte_of(&need(vm, s, "SkillState")?, "SkillState")?;
        if d == definition && i == controller && state != SkillState::Deactivated.byte() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Skill.CalculateModifierValue(EffectData, Grade, ContextSource)`: the grade rule with the two initialization
/// values evaluated by the one evaluator with `source` as the context source (NATIVE_SKILLS.md 4.3).
pub fn calculate_modifier_value(
    vm: &mut Vm,
    effect: &Value,
    grade: i32,
    source: Option<ObjRef>,
) -> Result<f32, VmError> {
    let e = read_effect(&vm.space, effect)?;
    let sys = vm.attrs.clone();
    let mut cx = EvalCx::new(vm);
    let base = sys.evaluate_init(&e.base, source, None, &mut cx)?;
    let per = sys.evaluate_init(&e.per_grade, source, None, &mut cx)?;
    Ok(modifier_value(&e, grade, base, per))
}

/// The function paths this module binds.
pub const NATIVE_PATHS: [&str; 9] = [
    "WillowGame.SkillEffectManager.IsSkillActive",
    "WillowGame.SkillDefinition.DoesSkillPassMinGradeTest",
    "WillowGame.Skill.CalculateModifierValue",
    "WillowGame.Skill.CalculateModifierValueFromDefinitionEffectArray",
    "WillowGame.Skill.CalculateStateBasedOnConstraints",
    "GearboxFramework.AttributeExpressionEvaluator.Evaluate",
    "WillowGame.VehiclePassengerExpressionEvaluator.Evaluate",
    "WillowGame.HealthStateExpressionEvaluator.Evaluate",
    "WillowGame.WeaponActionAvailableExpressionEvaluator.Evaluate",
];

/// Reads an `AttributeExpressionData` value (the enum bytes by the package values `ow-attr` names).
fn expr_data(vm: &Vm, v: &Value) -> Result<ExprData, VmError> {
    let space = &vm.space;
    let bad = |w: &str| {
        script_error(
            "AttributeExpressionEvaluator.Evaluate",
            format!("{w} outside its enum"),
        )
    };
    Ok(ExprData {
        attribute_operand1: object("AttributeOperand1", field(space, v, "AttributeOperand1")?)?,
        comparison_operator: Comparison::from_raw(i64::from(byte_of(
            field(space, v, "ComparisonOperator")?,
            "ComparisonOperator",
        )?))
        .ok_or_else(|| bad("ComparisonOperator"))?,
        operand2_usage: Operand2Usage::from_raw(i64::from(byte_of(
            field(space, v, "Operand2Usage")?,
            "Operand2Usage",
        )?))
        .ok_or_else(|| bad("Operand2Usage"))?,
        attribute_operand2: object("AttributeOperand2", field(space, v, "AttributeOperand2")?)?,
        constant_operand2: number("ConstantOperand2", field(space, v, "ConstantOperand2")?)?,
    })
}

/// The skill manager, constraint and grade natives as a [`NativePack`] (module documentation: bound by path).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
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
        bind(
            vm,
            active,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, active)?;
                let (controller, definition) = (arg_obj(c, 0)?, arg_obj(c, 1)?);
                Ok(Value::Bool(is_skill_active(
                    c.vm, me, controller, definition,
                )?))
            }),
        )?;
        bind(
            vm,
            min_grade,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, min_grade)?;
                let grade = arg_int(c, 0, min_grade)?;
                let subject = match need(c.vm, me, "bSubjectToGradeRules")? {
                    Value::Bool(b) => b,
                    other => {
                        return Err(script_error(
                            min_grade,
                            format!(
                                "bSubjectToGradeRules: expected a bool, found {}",
                                other.kind_name()
                            ),
                        ));
                    }
                };
                Ok(Value::Bool(passes_min_grade_test(subject, grade)))
            }),
        )?;
        bind(
            vm,
            value,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let effect = c.arg(0)?.clone();
                let grade = arg_int(c, 1, value)?;
                let source = arg_obj(c, 2)?;
                Ok(Value::Float(calculate_modifier_value(
                    c.vm, &effect, grade, source,
                )?))
            }),
        )?;
        bind(
            vm,
            value_from,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let definition = arg_obj(c, 0)?;
                let index = arg_int(c, 1, value_from)?;
                let grade = arg_int(c, 2, value_from)?;
                let source = arg_obj(c, 3)?;
                // 0.0 for a None definition or an index outside the effect list (section 4.3).
                let Some(d) = definition else {
                    return Ok(Value::Float(0.0));
                };
                let effects = need(c.vm, d, "SkillEffectDefinitions")?;
                let entry = usize::try_from(index)
                    .ok()
                    .and_then(|i| effects.elements().and_then(|e| e.get(i)).cloned());
                match entry {
                    Some(e) => Ok(Value::Float(calculate_modifier_value(
                        c.vm, &e, grade, source,
                    )?)),
                    None => Ok(Value::Float(0.0)),
                }
            }),
        )?;
        bind(
            vm,
            state,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, state)?;
                let activation = matches!(c.arg(0)?, Value::Bool(true));
                Ok(Value::Byte(calculate_state(c.vm, me, activation)?.byte()))
            }),
        )?;
        // AttributeExpressionEvaluator.Evaluate(ContextSource): its one expression, as EvaluateExpressions (And of one)
        // with no override source (NATIVE_ATTRIBUTES.md section 11).
        bind(
            vm,
            attr_eval,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, attr_eval)?;
                let source = arg_obj(c, 0)?;
                let e = need(c.vm, me, "Expression")?;
                let data = expr_data(c.vm, &e)?;
                let sys = c.vm.attrs.clone();
                let mut cx = EvalCx::new(&mut *c.vm);
                Ok(Value::Bool(sys.expressions_hold(
                    &[data],
                    ExprMode::And,
                    source,
                    None,
                    &mut cx,
                )?))
            }),
        )?;
        // VehiclePassengerExpressionEvaluator.Evaluate: not in a vehicle gives bNotInVehicle. The in-vehicle branch
        // (driver of a WillowVehicle, or the seat kind) is NotYet: how the native tells the seat is not read. "In a
        // vehicle" is read as the context's pawn being a Vehicle (UNVERIFIED).
        bind(
            vm,
            vehicle,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, vehicle)?;
                let pawn = match arg_obj(c, 0)? {
                    Some(o) => pawn_of(c.vm, o)?,
                    None => None,
                };
                if let Some(p) = pawn
                    && crate::skills::is_a(&c.vm.space, p, "Engine.Vehicle")?
                {
                    return Err(NotYet::not_read(
                        "VehiclePassengerExpressionEvaluator in a vehicle (the seat test)",
                        crate::skills::RECORD_CONSTRAINTS,
                    )
                    .into());
                }
                Ok(Value::Bool(matches!(
                    need(c.vm, me, "bNotInVehicle")?,
                    Value::Bool(true)
                )))
            }),
        )?;
        // HealthStateExpressionEvaluator.Evaluate: no pawn gives false; bHealthy asks the pawn's IsAliveAndWell
        // (whose body is not read: the pawn's own binding decides); bInjured / bDead ask IsInjured / IsDead. More than
        // one flag, or none, is NotYet (how the native combines them is not read).
        bind(
            vm,
            health,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, health)?;
                let Some(pawn) = (match arg_obj(c, 0)? {
                    Some(o) => pawn_of(c.vm, o)?,
                    None => None,
                }) else {
                    return Ok(Value::Bool(false));
                };
                let set = |vm: &mut Vm, n: &str| -> Result<bool, VmError> {
                    Ok(matches!(get(vm, me, n)?, Some(Value::Bool(true))))
                };
                let flags = [
                    ("bHealthy", "IsAliveAndWell"),
                    ("bInjured", "IsInjured"),
                    ("bDead", "IsDead"),
                ];
                let mut on = Vec::new();
                for (flag, function) in flags {
                    if set(c.vm, flag)? {
                        on.push(function);
                    }
                }
                match on.as_slice() {
                    [function] => Ok(Value::Bool(call_bool(c.vm, pawn, function, vec![])?)),
                    _ => Err(NotYet::not_read(
                        "HealthStateExpressionEvaluator with no or several state flags",
                        crate::skills::RECORD_CONSTRAINTS,
                    )
                    .into()),
                }
            }),
        )?;
        // WeaponActionAvailableExpressionEvaluator.Evaluate: the controller's installed CanPerformWeaponAction(0).
        bind(
            vm,
            weapon,
            Rc::new(move |c: &mut NativeCall<'_>| {
                // A None source: what the native answers is not read (it reaches the controller's script).
                let Some(o) = arg_obj(c, 0)? else {
                    return Err(NotYet::not_read(
                        "WeaponActionAvailableExpressionEvaluator with no source",
                        crate::skills::RECORD_CONSTRAINTS,
                    )
                    .into());
                };
                if !crate::skills::is_a(&c.vm.space, o, "WillowGame.WillowPlayerController")? {
                    return Err(NotYet::not_read("WeaponActionAvailableExpressionEvaluator on a source that is not a player controller", crate::skills::RECORD_CONSTRAINTS).into());
                }
                Ok(Value::Bool(call_bool(
                    c.vm,
                    o,
                    "CanPerformWeaponAction",
                    vec![Value::Byte(0)],
                )?))
            }),
        )?;
        register_events(vm)?;
        register_sm_component(vm)?;
        special_move::register(vm)?;
        register_cast_opinion(vm)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// `Pawn.IsFriendly` for the cast (review r2 of #437)
// ---------------------------------------------------------------------------------------------------------------

/// The record of `Pawn.IsFriendly` in Off.
pub const RECORD_IS_FRIENDLY: &str = ow_combat::hit::opinion::RECORD;

/// The one path by which the cast's worlds get `Pawn.IsFriendly`, which `LiftActionSkill.GetPlayerAnimation` asks on
/// its target path (this pack serves both the W6-SKILLS installed run and the slice gate's cast). It is the combat
/// pack's own allegiance registration (`ow_combat::hit::register_allegiance`), so every world gets the same binding
/// whatever order its packs register in:
/// - Off: NATIVE_IS_FRIENDLY.md's rules, `IsFriendly` with `WillowPawn`'s charm layer (lane IMPL-IS-FRIENDLY,
///   decision 2026-11-19-60); the groundwork boolean is never bound for it in Off;
/// - Reference: the combat pack's allegiance natives, the C++ stand-in.
fn register_cast_opinion(vm: &mut Vm) -> Result<(), VmError> {
    ow_combat::hit::register_allegiance(vm)
}

// ---------------------------------------------------------------------------------------------------------------
// `GearboxPawn.GetSMComponent` (NATIVE_SPECIAL_MOVE_COMPONENT.md), Off only
// ---------------------------------------------------------------------------------------------------------------

/// The native key of `GearboxPawn.GetSMComponent()` (lane FIX-GETSMCOMPONENT, decision 2026-11-19-33).
pub const GET_SM_COMPONENT: &str = "GearboxPawn.GetSMComponent()";

/// The record of the special-move component native.
pub const RECORD_SM_COMPONENT: &str = "NATIVE_SPECIAL_MOVE_COMPONENT.md";

/// `GearboxPawn.GetSMComponent()`, coded from NATIVE_SPECIAL_MOVE_COMPONENT.md alone (UNVERIFIED in the running game):
/// the pawn's `SMComponent` as it is, nothing created, checked or registered; None when the field is None.
///
/// The game's pawn is expected to hold its own instance of the component: a default sub-object, copied per actor at
/// spawn. The note's open question 1 is answered by stock UE3 component instancing, not by this game (UNVERIFIED
/// here; the note's bl-sdk check settles it). Since decision 2026-11-19-44 the world's spawn makes that copy in Off
/// (`ow_world::actors::instance_components`), outered to the new actor, so a spawned pawn's component is answered.
/// A pawn whose component's outer is not the pawn is still `NotYet { NotRead }`. That is the class default's or an
/// archetype's shared sub-object, held by a pawn the spawn did not instance (one made from class defaults by a
/// caller that does not instance them). Answering that shared object would let a `Play` on one pawn write the
/// template. A class that declares no `SMComponent`, or one
/// holding a non-object, is a typed error.
///
/// Reference: unbound. The C++ has no stand-in, so the VM's stub policy answers, as before (R3).
fn register_sm_component(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    vm.register_native(GET_SM_COMPONENT, Rc::new(get_sm_component))
}

fn get_sm_component(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = this(c, GET_SM_COMPONENT)?;
    let class = c.vm.space.heap.get(pawn)?.class;
    let prop = c
        .vm
        .space
        .prop(class, "SMComponent")
        .ok_or_else(|| script_error(GET_SM_COMPONENT, "the pawn's class declares no SMComponent"))?;
    let value = c.vm.space.read(pawn, ow_object::ids::PropAddr::scalar(prop))?.clone();
    match value {
        Value::Object(None) => Ok(Value::Object(None)),
        Value::Object(Some(component)) => {
            if c.vm.space.heap.get(component)?.outer != Some(pawn) {
                return Err(NotYet::not_read(
                    "GearboxPawn.GetSMComponent on a pawn whose SMComponent is not its own instance (a shared class \
                     default or archetype sub-object): the pawn was not made by a spawn that instances its \
                     components (NATIVE_SPECIAL_MOVE_COMPONENT.md open question 1, answered by stock UE3 component \
                     instancing, decision 2026-11-19-44)",
                    RECORD_SM_COMPONENT,
                )
                .into());
            }
            Ok(Value::Object(Some(component)))
        }
        other => Err(script_error(
            GET_SM_COMPONENT,
            format!("SMComponent holds {}, not an object", other.kind_name()),
        )),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The `ActionSkill` event natives (NATIVE_ACTION_SKILL.md section 9), Off only
// ---------------------------------------------------------------------------------------------------------------

/// The `ActionSkill` event natives this module binds in Off (lane FIX-PHASELOCK-CAST part B, decision 2026-11-19-12),
/// with how many object parameters follow the consumer handle.
pub const EVENT_PATHS: [(&str, usize); 2] = [
    ("WillowGame.ActionSkill.OnActionSkillActivated", 2),
    ("WillowGame.ActionSkill.OnActionSkillDeactivated", 1),
];

/// Section 9: each native builds a behavior event named after the function, attaches its object parameters in order,
/// and delivers it to the action skill's provider for the consumer handle through the kernel; it returns nothing.
/// The rule as bound here (all UNVERIFIED):
/// - the event fires on the one provider the kernel registered for the handle's process, every link followed (no
///   filter), the parameters as the event's positional payload (the kernel's payload rule, NATIVE_MARCUS_USE_CHAIN.md
///   "Event payload to named variable");
/// - a handle nobody owns (-1 among them): nothing fires (NATIVE_BEHAVIOR_CONTEXT.md, "kernel entry points given -1");
/// - `NotYet { NotRead }`: a kernel process with no provider or several (which ones the delivery reaches is not read),
///   and a handle another crate gave out (how such a consumer receives the event is not read).
///
/// Reference: unbound. The C++ has no stand-in, so the VM's stub policy answers, as before.
fn register_events(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    // The kernel's consumer table answers who owns the handle (registering it again is a no-op).
    ow_behavior::kernel::natives::Pack.register(vm)?;
    for (path, objects) in EVENT_PATHS {
        bind(
            vm,
            path,
            Rc::new(move |c: &mut NativeCall<'_>| fire_action_skill_event(c, path, objects)),
        )?;
    }
    Ok(())
}

fn fire_action_skill_event(
    c: &mut NativeCall<'_>,
    path: &'static str,
    objects: usize,
) -> Result<Value, VmError> {
    use ow_behavior::kernel::natives::{
        Owner, consumer_pid, owner_of, provider_paths, with_provider,
    };
    let event = path.rsplit('.').next().unwrap_or(path);
    let pid = consumer_pid(c.vm, c.arg(0)?)
        .ok_or_else(|| script_error(path, "the first argument is no BehaviorConsumerHandle"))?;
    let payload = (1..=objects)
        .map(|i| arg_obj(c, i))
        .collect::<Result<Vec<_>, _>>()?;
    match owner_of(c.vm, pid)? {
        None => Ok(Value::None),
        Some(Owner::Crate(name)) => Err(NotYet::not_read(
            format!("{event} on a consumer handle {name} gave out (how that consumer receives it)"),
            RECORD,
        )
        .into()),
        Some(Owner::Kernel) => {
            let paths = provider_paths(c.vm, pid)?.unwrap_or_default();
            let [provider] = paths.as_slice() else {
                return Err(NotYet::not_read(
                    format!(
                        "{event} on a consumer with {} providers (which of them the delivery reaches)",
                        paths.len()
                    ),
                    RECORD,
                )
                .into());
            };
            with_provider(c.vm, pid, provider, |vm, p| {
                p.fire_event(vm, event, &Default::default(), -1, &payload)
            })?
            .transpose()
            .map_err(|e: BehaviorError| match e {
                BehaviorError::Vm(v) => v,
                other => script_error(path, other.to_string()),
            })?;
            Ok(Value::None)
        }
        Some(_) => Err(NotYet::not_read(
            format!("{event} on a consumer of an owner kind this lane does not know"),
            RECORD,
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The cooldown pool's per-frame drain
// ---------------------------------------------------------------------------------------------------------------

/// What one drain step did.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drained {
    pub before: f32,
    pub after: f32,
    /// The net rate per second used.
    pub rate: f32,
}

fn pool_float(vm: &mut Vm, pool: ObjRef, name: &str) -> Result<f32, VmError> {
    let v = need(vm, pool, name)?;
    Ok(number(name, &v)?)
}

/// The base `ResourcePool` per-frame update for `dt` seconds (NATIVE_SKILLS.md 5.2): nothing while
/// `RegenerationDisabled` is non-zero; the net rate is `ActiveRegenerationRate + PassiveRegenerationRate -
/// ConsumptionRate`, plus `OnIdleRegenerationRate` once `OnIdleRegenerationDelay` has passed since
/// `PoolIdleDelayStartTime`; the new value `current + rate x dt + RateRemainder` goes through `SetCurrentValue` (the
/// truncation, the clamp, the remainder, the idle stamp) only when the value `SetCurrentValue` would store
/// ([`ow_world::pools::stored_value`]) differs from the current one, so a pool resting at an extreme keeps its idle
/// stamp. The edge test (reaching or leaving the minimum or the maximum) uses that stored value too, so an integer
/// truncation that lands on the minimum counts.
///
/// Refused before anything is written: a delta that is not finite or is negative (bad data, as `World::tick`);
/// `NotYet` for a pool class other than `Engine.ResourcePool` (the experience pool overrides the update), an attached
/// `RegenerationPool` (its charge rule is not read), an integer-only pool whose step moves only its remainder, and a
/// step that reaches or leaves an extreme when the definition lists behaviors for those events (which list fires on
/// which event is read from field order only, UNVERIFIED in the record). Status: UNVERIFIED in the running game.
pub fn drain(vm: &mut Vm, pool: ObjRef, dt: f32) -> Result<Drained, VmError> {
    // A delta that is not finite or is negative is refused, as `World::tick` refuses it (`WorldError::BadDelta`).
    if !dt.is_finite() || dt < 0.0 {
        return Err(script_error(
            "drain",
            format!(
                "a frame delta must be finite and not negative (got {})",
                ow_core::fmt::fmt_g(f64::from(dt), 9)
            ),
        ));
    }
    let class = ow_attr::eval::class_path(&vm.space, pool);
    if class != "Engine.ResourcePool" {
        return Err(NotYet::not_read(
            format!("the per-frame update of a {class} (only the base class is read)"),
            RECORD_DRAIN,
        )
        .into());
    }
    if obj_value("RegenerationPool", &need(vm, pool, "RegenerationPool")?)?.is_some() {
        return Err(NotYet::not_read(
            "the per-frame update with a regeneration pool attached",
            RECORD_DRAIN,
        )
        .into());
    }
    let before = pool_float(vm, pool, "CurrentValue")?;
    let disabled = match need(vm, pool, "RegenerationDisabled")? {
        Value::Int(i) => i,
        other => {
            return Err(script_error(
                "drain",
                format!("RegenerationDisabled: {}", other.kind_name()),
            ));
        }
    };
    if disabled != 0 {
        return Ok(Drained {
            before,
            after: before,
            rate: 0.0,
        });
    }
    let mut rate = pool_float(vm, pool, "ActiveRegenerationRate")?
        + pool_float(vm, pool, "PassiveRegenerationRate")?
        - pool_float(vm, pool, "ConsumptionRate")?;
    let idle = pool_float(vm, pool, "OnIdleRegenerationRate")?;
    if idle != 0.0 {
        let delay = pool_float(vm, pool, "OnIdleRegenerationDelay")?;
        let stamp = pool_float(vm, pool, "PoolIdleDelayStartTime")?;
        let now = world_time(vm)?;
        if now - stamp >= delay {
            rate += idle;
        }
    }
    let remainder = pool_float(vm, pool, "RateRemainder")?;
    let target = before + rate * dt + remainder;
    // The value SetCurrentValue would store (its truncation, then its clamp order), which both tests below use.
    let Some(stored) = ow_world::pools::stored_value(vm, pool, target)? else {
        // Not authoritative: SetCurrentValue stores nothing.
        return Ok(Drained {
            before,
            after: before,
            rate,
        });
    };
    let changed = stored.current.to_bits() != before.to_bits();
    if !changed {
        if stored
            .remainder
            .is_some_and(|r| r.to_bits() != remainder.to_bits())
        {
            // An integer-only pool whose step moves only the kept fraction: whether the native writes the fraction
            // (and moves the idle stamp) without a value change is not read.
            return Err(NotYet::not_read(
                "the per-frame update of an integer-only pool whose step changes only its remainder",
                RECORD_DRAIN,
            )
            .into());
        }
        // Nothing changes (a pool resting at an extreme): no SetCurrentValue, so the idle stamp stays.
        return Ok(Drained {
            before,
            after: before,
            rate,
        });
    }
    // The edges are the bounds the clamp used (an integer-only pool's maximum is truncated, so a pool with
    // MaxValue 10.5 fills to 10 and that counts as reaching its maximum). Which bound the native's edge test uses is
    // not in the record (UNVERIFIED); this reads the one the stored value was clamped to.
    let (min, max) = (stored.minimum, stored.maximum);
    let crosses = |edge: f32| (before == edge) != (stored.current == edge);
    if (crosses(min) || crosses(max))
        && let Some(def) = obj_value("Definition", &need(vm, pool, "Definition")?)?
    {
        for list in [
            "OnResourceDepleted",
            "OnResourceNotDepleted",
            "OnResourceRegenerated",
            "OnResourceNotRegenerated",
        ] {
            let v = need(vm, def, list)?;
            if !elements(list, &v)?.is_empty() {
                return Err(NotYet::not_read(
                    format!("the pool's {list} behaviors (their event mapping)"),
                    RECORD_DRAIN,
                )
                .into());
            }
        }
    }
    ow_world::pools::set_current(vm, pool, target)?;
    let after = pool_float(vm, pool, "CurrentValue")?;
    Ok(Drained {
        before,
        after,
        rate,
    })
}

fn world_time(vm: &mut Vm) -> Result<f32, VmError> {
    let actors = ow_world::actors::actors_of(vm)?;
    let wi = actors
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("ow_world::actors::Actors"))?
        .world_info();
    match wi {
        Some(w) => pool_float(vm, w, "TimeSeconds"),
        None => Err(NotYet::layer_absent(
            "the world time for the idle regeneration",
            "a level with a WorldInfo",
            RECORD_DRAIN,
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The skill behavior handlers
// ---------------------------------------------------------------------------------------------------------------

/// The behavior classes whose installed `ApplyBehaviorToContext` the skill handlers run.
pub const BEHAVIOR_CLASSES: [&str; 2] = [
    "WillowGame.Behavior_ActivateSkill",
    "WillowGame.Behavior_DeactivateSkill",
];

/// Runs the installed `ApplyBehaviorToContext` of behavior `id` once per context (the behavior's Context link, else
/// `consumer`), with `ContextObject` and `SelfObject` set and every other parameter at its zero value. The script then
/// asks the context's `ISkillBehavior` to activate or deactivate the skill.
///
/// **It selects no output link, so the answer is the empty list.** Checked in the installed bytecode with
/// `research/script_disasm.py` (local, review r2 of PR #342): neither `Behavior_ActivateSkill.ApplyBehaviorToContext`
/// nor `Behavior_DeactivateSkill.ApplyBehaviorToContext` calls `BehaviorKernel.ActivateBehaviorOutputLink`, and none
/// of the six `Behavior_ActivateSkill` / `Behavior_DeactivateSkill` functions they reach (`ISkillBehavior`,
/// `WillowPlayerController`, `WillowMind`) calls it or receives the kernel information it needs. So this crate binds
/// no output-link native, and the kernel follows only the behavior's default link, as for any behavior that selects
/// none.
///
/// Refused before any call: `bUseAdditionalSkillTarget` set (`NotYet`), the one path on which the script reads the
/// instigator, the other participant and the kernel information, which this handler zero-fills; and that flag of any
/// kind other than a bool (bad data). A class without the flag (`Behavior_DeactivateSkill`) reads as not set.
pub fn run_skill_behavior(
    vm: &mut Vm,
    p: &mut Provider,
    id: BehaviorId,
    consumer: ObjRef,
) -> Result<Vec<i32>, VmError> {
    let object = p.behavior(id).object;
    let contexts = p.contexts_for(vm, id, consumer)?;
    apply_to_contexts(vm, object, &contexts, consumer)
}

/// The work of [`run_skill_behavior`] once the provider has given the behavior object and its contexts.
pub fn apply_to_contexts(
    vm: &mut Vm,
    object: ObjRef,
    contexts: &[ObjRef],
    consumer: ObjRef,
) -> Result<Vec<i32>, VmError> {
    match get(vm, object, "bUseAdditionalSkillTarget")? {
        None | Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => {
            return Err(NotYet::not_read(
                "Behavior_ActivateSkill with bUseAdditionalSkillTarget: its script reads the instigator, \
                 the other participant and the kernel information, which this handler does not have",
                RECORD,
            )
            .into());
        }
        Some(other) => {
            return Err(script_error(
                "run_skill_behavior",
                format!(
                    "bUseAdditionalSkillTarget: expected a bool, found {}",
                    other.kind_name()
                ),
            ));
        }
    }
    let Some(apply) = method_of(vm, object, "ApplyBehaviorToContext")? else {
        return Err(
            NotYet::not_read("a skill behavior without ApplyBehaviorToContext", RECORD).into(),
        );
    };
    let params = vm
        .function(apply)
        .map(|d| d.params.clone())
        .unwrap_or_default();
    for &context in contexts {
        let args = params
            .iter()
            .map(|d| match vm.space.names.text(d.name).as_ref() {
                "ContextObject" => Value::Object(Some(context)),
                "SelfObject" => Value::Object(Some(consumer)),
                _ => vm.space.classes.zero_value(&d.kind),
            })
            .collect();
        vm.call(apply, Some(object), args)?;
    }
    Ok(Vec::new())
}

/// The skill behavior handlers for a provider registered on `consumer`, to be set with `Provider::handle` (a handler
/// needs the consumer, which the registry's `HandlerFn` does not receive: decision 2026-11-15-30).
pub fn skill_behavior_handlers(consumer: ObjRef) -> Vec<(&'static str, Handler)> {
    BEHAVIOR_CLASSES
        .iter()
        .map(|class| {
            (
                *class,
                Handler::run(move |vm, p, id, _event| {
                    run_skill_behavior(vm, p, id, consumer).map_err(BehaviorError::Vm)
                }),
            )
        })
        .collect()
}

pub mod manager;

/// `SpecialMoveComponent.Play` and the natives it reaches (lane FIX-SM-PLAY, decision 2026-11-19-52), Off only.
pub mod special_move;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod events_tests;

#[cfg(test)]
mod sm_component_tests;
