//! Owner: lane W5-POOLS. The level-up rebase: `RecalculateAttributeInitializedState` on a controller
//! (NATIVE_LEVEL_UP_ATTRIBUTES.md sections 1 and 2). AI-assisted (Claude Code).
//!
//! **Confirmed in game 2026-10-07 (lane L1, two local captures, REALGAME_GROUND_TRUTH.md):** the player controller's
//! call rewrites the health pool's base and effective maximum and leaves current health untouched; nothing else of the
//! controller, pawn, replication info or pools changed inside it. The rest is read from the executable and
//! **UNVERIFIED**.
//!
//! **What the call does here**, in the record's order (the controller-class part, then the player-controller part):
//! 1. *Class-level pawn attributes* (only with a pawn that is not a vehicle and a `CharacterClass`): the encumbrance
//!    resistance value and twelve damage-modifier attributes from the class's init data. In `ParityMode::Off` they are
//!    written (lane IMPL-LINKS-13-14, NATIVE_LINKS_13_14.md section 3, #796 r1; UNVERIFIED), the controller as the
//!    context source: `Pawn.EncumbranceResistance`'s value only, from `BaseEncumbranceResistance`; then for Normal,
//!    Explosive, Shock, Corrosive, Incendiary and Amp in that order, `<Kind>ImpactDamageModifier` from
//!    `Base<Kind>DamageModifiers.ResistanceToImpact` and `<Kind>StatusEffectDamageModifier` from its
//!    `ResistanceToStatusEffect`, each by its base with the value recomputed from its stack ([`plan_class_part`]). The
//!    plan (declarations, each init-data definition and its chains' resolvers resident, else `NotRead` naming it) is
//!    made before any write; the writes come before the pool rebase.
//! 2. *Pool rebase, always* (even with no pawn): `ResourcePoolManager.RecalculateBaseValues` on the controller's
//!    `ResourcePoolManager`, which runs the installed script `ResourcePool.CalculateBaseValues(true)` on every pool
//!    in slot order (`crate::pools::recalculate_base_values`). The script sets each pool's base maximum from its
//!    definition with the pool's `AssociatedProvider` as context, keeps the modifier stack, and moves current only
//!    when the pool was full at its last `UpdateLastValues` (the follow rule, script).
//! 3. *Status-effect resistances* (a `WillowPlayerController` with a `MyWillowPawn` that has a `StatusEffectComp`,
//!    and a `WillowCharacterClassDefinition`): ten attributes of the status component; `NotYet { NotRead }` in Off mode
//!    when it applies, as step 1.
//!
//! In `ParityMode::Reference` steps 1 and 3 are skipped and their conditions are not read (no `CharacterClass`,
//! `Pawn` or `MyWillowPawn`), as the C++ does (`src/mission_script.cpp`: "not modelled"; for Maya's class the data
//! are the constants 0 and 1). A stand-in pinned by `tests/mission_script_test.py` (scenarios S7 and S7b, `--slice-run`)
//! (W6 lanes). Every refusal is decided before the rebase, so a `NotYet` changes nothing.
//!
//! **The `WillowMind` override** (NATIVE_MIND_ROUTE.md section 3 and NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md
//! section 4.1, lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC; Off only; Reference has no stand-in and stays `NotYet`),
//! in the game's order: the skill refresh, then the shared controller part above, then the mind's own part
//! ([`plan_mind_part`]). Every refusal of the controller part's class-level step and of the mind's own part is decided
//! first, before the skill refresh writes; an evaluator refusal of the mind part after an earlier write is prefixed.
//! Before all of it the spawn's balance step runs when the spawn recorded one ([`crate::spawn_balance::run_step`]).
//! The `WillowMind` key is bound to this rule too ([`NATIVE_KEYS`]).
//!
//! **Not read:** the `WillowVehicle` thunk (its own native, not registered
//! here). No change notification is sent and no script event fires (record section 1).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use crate::actors::object;
use crate::pools::{self, is_a};

/// The record.
pub const RECORD: &str = "NATIVE_LEVEL_UP_ATTRIBUTES.md sections 1-2";

/// The script-callable keys the installed packages declare (one shared thunk calls the class's virtual).
pub const NATIVE_KEYS: [&str; 2] = [
    "Controller.RecalculateAttributeInitializedState()",
    "WillowPlayerController.RecalculateAttributeInitializedState()",
];
/// The `WillowMind` key, bound in Off only (lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC, the note's section 4.1;
/// review of #792, item 4: Reference keeps it unbound, as before).
pub const MIND_KEY: &str = "WillowMind.RecalculateAttributeInitializedState()";

/// What [`recalculate_attribute_initialized_state`] ran.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recalculated {
    /// Steps 1 and 3 were not run and their conditions not read (Reference mode, as the C++).
    pub class_parts_skipped: bool,
    /// The manager whose pools were rebased (None: no manager, nothing rebased).
    pub manager: Option<ObjRef>,
}

/// `RecalculateAttributeInitializedState()` on `controller` (module documentation).
pub fn recalculate_attribute_initialized_state(
    vm: &mut Vm,
    controller: ObjRef,
) -> Result<Recalculated, VmError> {
    let mind = is_a(&vm.space, controller, "WillowMind")?;
    if mind && vm.parity == ParityMode::Reference {
        return Err(NotYet::not_read(
            "the WillowMind override of RecalculateAttributeInitializedState (no C++ stand-in)",
            RECORD,
        )
        .into());
    }
    // The marks and prefix wrapper is Off's (review of #792, item 4): Reference's refusals keep their text.
    let entry = marks(vm);
    let off = vm.parity == ParityMode::Off;
    let result = (|| -> Result<Recalculated, VmError> {
        if mind {
            crate::spawn_balance::run_step(vm, controller)?;
        }
        let skipped = vm.parity == ParityMode::Reference;
        // The refusals up front: the controller part's class-level step (its plan), then the mind's own part.
        let class_part = if skipped { None } else { plan_class_part(vm, controller)? };
        if !skipped {
            refuse_class_parts(&vm.space, controller)?;
        }
        let part = if mind { plan_mind_part(vm, controller)? } else { None };
        if mind {
            skill_refresh(vm, controller)?;
        }
        if let Some(class_part) = class_part {
            apply_mind_part(vm, controller, &class_part)?;
        }
        let manager = object(&vm.space, controller, "ResourcePoolManager")?;
        pools::recalculate_base_values(vm, manager)?;
        if let Some(part) = part {
            apply_mind_part(vm, controller, &part)?;
        }
        Ok(Recalculated { class_parts_skipped: skipped, manager })
    })();
    match result {
        Err(e) if off && marks(vm) != entry => Err(pools::after_partial_writes(e)),
        other => other,
    }
}

/// Step 1 alone, the class-level part (module documentation): planned, then written, as
/// [`recalculate_attribute_initialized_state`] does it; `false` when the part does not apply (no `Pawn`, no
/// `CharacterClass`, or a vehicle pawn). Off only, else a registration refusal. For a harness or test that checks the
/// part's inputs without the rest of the recalculation (review of #803, item 1).
pub fn run_class_part(vm: &mut Vm, controller: ObjRef) -> Result<bool, VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: "the class-level part".to_string(),
            reason: "the class-level part alone is an Off entry; Reference skips step 1",
        });
    }
    let Some(part) = plan_class_part(vm, controller)? else { return Ok(false) };
    apply_mind_part(vm, controller, &part)?;
    Ok(true)
}

/// #518's counters.
fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// The ten resistances of the mind's own part, in the note's order (section 4.1).
pub const RESISTANCES: [&str; 10] = [
    "IgniteChance",
    "ShockChance",
    "CorrosiveChance",
    "SlowChance",
    "AmpChance",
    "IgniteDuration",
    "ShockDuration",
    "CorrosiveDuration",
    "SlowDuration",
    "AmpDuration",
];
/// A definition the mind part reads that the store has not loaded.
pub const MIND_NOT_LOADED: &str =
    "the WillowMind part of RecalculateAttributeInitializedState: an object the store has not loaded (a load is a write)";

/// Where one value of the mind part lands: a resistance, a stack attribute (written by its base, the value recomputed
/// from its stack), or `CommerceMarkup`, a plain float written as the value (review of #792, item 3).
#[derive(Debug, Clone, Copy)]
enum Target {
    Stack(ObjRef, ow_object::layout::AttrCompanions),
    Plain(ObjRef, ow_object::ids::PropId),
}

/// The mind part's plan: each value's init data and where it lands, in write order.
#[derive(Debug, Clone)]
pub struct MindPart {
    writes: Vec<(ow_attr::api::InitData, Target)>,
}

fn mind_bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: "WillowMind.RecalculateAttributeInitializedState".to_string(), message: message.into() }
}

fn mind_resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(MIND_NOT_LOADED, RECORD_MIND).into());
    }
    Ok(())
}

/// `o`'s `name`'s declaration.
fn decl_of(vm: &Vm, o: ObjRef, name: &str) -> Result<(ow_object::ids::PropId, ow_object::layout::PropertyDecl), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| mind_bad(format!("no {name}")))?;
    let d = vm.space.classes.decl(p).cloned().ok_or_else(|| mind_bad(format!("{name} undeclared")))?;
    Ok((p, d))
}

/// The init data `field` of the AI class, and its definitions checked resident (with their chains' resolvers).
fn init_data_of(vm: &mut Vm, class: ObjRef, field: &str) -> Result<ow_attr::api::InitData, VmError> {
    let (p, d) = decl_of(vm, class, field)?;
    if !matches!(d.kind, ow_object::layout::PropKind::Struct(_)) {
        return Err(mind_bad(format!("{field} is not declared as a struct")));
    }
    let v = vm.space.read(class, ow_object::ids::PropAddr::scalar(p))?.clone();
    init_data_resident(vm, &v, mind_resident)
}

/// The init data in the struct value `v`, its definitions checked resident by `resident` (with their chains'
/// resolvers, which the evaluator's read preflight checks).
fn init_data_resident(
    vm: &mut Vm,
    v: &Value,
    resident: fn(&Vm, ObjRef) -> Result<(), VmError>,
) -> Result<ow_attr::api::InitData, VmError> {
    let data = ow_attr::eval::init_data(&vm.space, v)?;
    if let Some(attr) = data.base_value_attribute {
        resident(vm, attr)?;
        let attrs = vm.attrs.clone();
        attrs.preflight_read_resident(attr, &mut ow_attr::api::EvalCx::new(vm))?;
    }
    if let Some(def) = data.initialization_definition {
        resident(vm, def)?;
    }
    Ok(data)
}

/// The damage kinds of the class-level part, in its write order (NATIVE_LINKS_13_14.md section 3).
pub const DAMAGE_KINDS: [&str; 6] = ["Normal", "Explosive", "Shock", "Corrosive", "Incendiary", "Amp"];
/// A class-level part object the store has not loaded (the text names it).
pub const CLASS_NOT_LOADED: &str =
    "the class-level part of RecalculateAttributeInitializedState: an object the store has not loaded (a load is a write)";
/// The record of the class-level part.
pub const RECORD_CLASS: &str = "NATIVE_LINKS_13_14.md section 3";

fn class_bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: "RecalculateAttributeInitializedState".to_string(), message: message.into() }
}

/// [`CLASS_NOT_LOADED`] naming `o` (its handle's path, else its class's).
fn class_resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        let named = vm.defs.handle_path(o).unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, o));
        return Err(NotYet::not_read(format!("{CLASS_NOT_LOADED}: {named}"), RECORD_CLASS).into());
    }
    Ok(())
}

/// The class-level part, planned (module documentation, step 1): `None` without a pawn, with a `Vehicle` pawn, or
/// without a `CharacterClass`. Reads only, loads nothing.
fn plan_class_part(vm: &mut Vm, controller: ObjRef) -> Result<Option<MindPart>, VmError> {
    let class = object(&vm.space, controller, "CharacterClass")?;
    let pawn = object(&vm.space, controller, "Pawn")?;
    let (Some(class), Some(pawn)) = (class, pawn) else { return Ok(None) };
    if is_a(&vm.space, pawn, "Vehicle")? {
        return Ok(None);
    }
    class_resident(vm, class)?;
    class_resident(vm, pawn)?;
    let struct_field = |vm: &Vm, field: &str| -> Result<Value, VmError> {
        let (p, d) = decl_of(vm, class, field).map_err(|_| class_bad(format!("the class declares no {field}")))?;
        if !matches!(d.kind, ow_object::layout::PropKind::Struct(_)) {
            return Err(class_bad(format!("{field} is not declared as a struct")));
        }
        Ok(vm.space.read(class, ow_object::ids::PropAddr::scalar(p))?.clone())
    };
    let member = |vm: &Vm, v: &Value, field: &str, name: &str| -> Result<Value, VmError> {
        let s = v.as_struct().ok_or_else(|| class_bad(format!("{field} is not a struct value")))?;
        let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| class_bad(format!("{field}'s struct has no layout")))?;
        let n = vm.space.names.find(name).ok_or_else(|| class_bad(format!("{field} declares no {name}")))?;
        let (i, d) = layout.field(n).ok_or_else(|| class_bad(format!("{field} declares no {name}")))?;
        if !matches!(d.kind, ow_object::layout::PropKind::Struct(_)) {
            return Err(class_bad(format!("{field}.{name} is not declared as a struct")));
        }
        Ok(s.fields[i as usize].clone())
    };
    let mut writes = Vec::new();
    // The encumbrance resistance: the value only (a float, or the value of an attribute).
    let v = struct_field(vm, "BaseEncumbranceResistance")?;
    let data = init_data_resident(vm, &v, class_resident)?;
    let target = match decl_of(vm, pawn, "EncumbranceResistance") {
        Ok((p, d))
            if matches!(d.kind, ow_object::layout::PropKind::Float | ow_object::layout::PropKind::FloatAttribute)
                && d.attr.is_none_or(|c| c.value == p) =>
        {
            Target::Plain(pawn, p)
        }
        _ => return Err(class_bad("EncumbranceResistance is not declared as a float value")),
    };
    writes.push((data, target));
    for kind in DAMAGE_KINDS {
        let field = format!("Base{kind}DamageModifiers");
        let v = struct_field(vm, &field)?;
        for (member_name, target_name) in
            [("ResistanceToImpact", format!("{kind}ImpactDamageModifier")), ("ResistanceToStatusEffect", format!("{kind}StatusEffectDamageModifier"))]
        {
            let m = member(vm, &v, &field, member_name)?;
            let data = init_data_resident(vm, &m, class_resident)?;
            let target = stack_target(vm, pawn, &target_name).map_err(|e| match e {
                VmError::Script { message, .. } => class_bad(message),
                other => other,
            })?;
            writes.push((data, target));
        }
    }
    Ok(Some(MindPart { writes }))
}

/// The stack attribute `name` of `o`, its companions named `<name>BaseValue` and `<name>ModifierStack`.
fn stack_target(vm: &Vm, o: ObjRef, name: &str) -> Result<Target, VmError> {
    let (p, d) = decl_of(vm, o, name)?;
    let Some(c) = d.attr.filter(|c| c.value == p) else {
        return Err(mind_bad(format!("{name} is not declared as a stack attribute")));
    };
    let (base, base_decl) = decl_of(vm, o, &format!("{name}BaseValue"))?;
    let (stack, stack_decl) = decl_of(vm, o, &format!("{name}ModifierStack"))?;
    let float = |k: &ow_object::layout::PropKind| {
        matches!(k, ow_object::layout::PropKind::Float | ow_object::layout::PropKind::FloatAttribute)
    };
    if base != c.base || stack != c.stack || !float(&d.kind) || !float(&base_decl.kind)
        || !matches!(stack_decl.kind, ow_object::layout::PropKind::Array(_))
    {
        return Err(mind_bad(format!("{name}'s base and stack are not declared as its float companions")));
    }
    Ok(Target::Stack(o, c))
}

/// The mind's own part, planned (module documentation): `None` without both `AIClass` and `MyWillowPawn`. Reads only,
/// loads nothing.
fn plan_mind_part(vm: &mut Vm, mind: ObjRef) -> Result<Option<MindPart>, VmError> {
    let class = object(&vm.space, mind, "AIClass")?;
    let pawn = object(&vm.space, mind, "MyWillowPawn")?;
    let (Some(class), Some(pawn)) = (class, pawn) else { return Ok(None) };
    mind_resident(vm, class)?;
    mind_resident(vm, pawn)?;
    let mut writes = Vec::new();
    let data = init_data_of(vm, class, "BaseCommerceMarkup")?;
    // `CommerceMarkup` is the value only, a plain float (NATIVE_MIND_ROUTE.md section 3; the installed package
    // declares a `FloatProperty`): any other declaration is a script error.
    let target = match decl_of(vm, pawn, "CommerceMarkup")? {
        (p, d) if matches!(d.kind, ow_object::layout::PropKind::Float) && d.attr.is_none() => Target::Plain(pawn, p),
        _ => return Err(mind_bad("CommerceMarkup is not declared as a plain float")),
    };
    writes.push((data, target));
    let (p, d) = decl_of(vm, pawn, "StatusEffectComp")?;
    if !matches!(d.kind, ow_object::layout::PropKind::Object { .. }) {
        return Err(mind_bad("StatusEffectComp is not declared as an object"));
    }
    if let Value::Object(Some(component)) = vm.space.read(pawn, ow_object::ids::PropAddr::scalar(p))?.clone() {
        mind_resident(vm, component)?;
        for name in RESISTANCES {
            let data = init_data_of(vm, class, &format!("Base{name}ResistanceModifier"))?;
            let target = stack_target(vm, component, &format!("{name}ResistanceModifier"))?;
            writes.push((data, target));
        }
    }
    Ok(Some(MindPart { writes }))
}

/// The mind part's writes, and the class-level part's (module documentation): each value evaluated with the mind
/// (the controller) as the context source and no override, then written; a resistance by its base, the value recomputed from its modifier stack (`ow-attr`'s stack
/// fold, the one-rounding form, UNVERIFIED).
fn apply_mind_part(vm: &mut Vm, mind: ObjRef, part: &MindPart) -> Result<(), VmError> {
    let attrs = vm.attrs.clone();
    for (data, target) in &part.writes {
        let v = attrs.evaluate_init(data, Some(mind), None, &mut ow_attr::api::EvalCx::new(vm))?;
        match *target {
            Target::Stack(owner, c) => ow_attr::registry::let_base(
                &mut vm.space,
                owner,
                c,
                Value::Float(v),
                ow_attr::stack::CombineForm::WeaponGen,
            )?,
            Target::Plain(owner, p) => vm.space.write(owner, ow_object::ids::PropAddr::scalar(p), Value::Float(v))?,
        }
    }
    Ok(())
}

/// The record of the mind's override (decision 2026-11-19-32).
pub const RECORD_MIND: &str = "NATIVE_MIND_ROUTE.md section 3";

/// The mind override's skill refresh (NATIVE_MIND_ROUTE.md section 3 step 1), Off only: the world's game info a
/// `WillowGameInfo`, its skill manager through the script native `GetSkillManager` (made lazily,
/// NATIVE_SKILL_MANAGER.md). Unless the manager's `bUpdatingSkills` is set, every `ActiveSkills` entry whose
/// `SkillInstigator` is this mind is flagged and gets one call on the skill that is not read: a matching entry is
/// `NotYet { NotRead }`; with none, the flag set and cleared again leaves nothing changed.
fn skill_refresh(vm: &mut Vm, mind: ObjRef) -> Result<(), VmError> {
    let world_info = object(&vm.space, mind, "WorldInfo")?;
    let game = match world_info {
        Some(w) => object(&vm.space, w, "Game")?,
        None => None,
    };
    let Some(game) = game.filter(|g| is_a(&vm.space, *g, "WillowGameInfo").unwrap_or(false)) else {
        return Ok(());
    };
    let f = vm.find_function("WillowGame.WillowGameInfo.GetSkillManager")?;
    let manager = match vm.call(f, Some(game), Vec::new())?.ret {
        Value::Object(o) => o,
        _ => None,
    };
    let Some(manager) = manager else { return Ok(()) };
    if matches!(crate::actors::field(&vm.space, manager, "bUpdatingSkills")?, Value::Bool(true)) {
        return Ok(());
    }
    let skills = crate::actors::objects(&vm.space, manager, "ActiveSkills")?;
    for skill in skills.into_iter().flatten() {
        if object(&vm.space, skill, "SkillInstigator")? == Some(mind) {
            return Err(NotYet::not_read(
                "the skill refresh of the WillowMind RecalculateAttributeInitializedState on a skill of this mind \
                 (the call on the skill is not read)",
                RECORD_MIND,
            )
            .into());
        }
    }
    Ok(())
}

/// Off mode: `NotYet` when step 3 applies (module documentation). Reads `CharacterClass` and, on a
/// `WillowPlayerController`, `MyWillowPawn` and its `StatusEffectComp`.
fn refuse_class_parts(
    space: &ow_object::heap::ObjectSpace,
    controller: ObjRef,
) -> Result<(), VmError> {
    // Step 1 is planned and written by `plan_class_part` (lane IMPL-LINKS-13-14); only step 3 refuses here.
    let class_def = object(space, controller, "CharacterClass")?;
    if is_a(space, controller, "WillowPlayerController")? {
        let component = match object(space, controller, "MyWillowPawn")? {
            Some(p) => object(space, p, "StatusEffectComp")?,
            None => None,
        };
        let willow_class = match class_def {
            Some(c) => is_a(space, c, "WillowCharacterClassDefinition")?,
            None => false,
        };
        if component.is_some() && willow_class {
            return Err(NotYet::not_read(
                "the ten status-effect resistances RecalculateAttributeInitializedState re-initialises (the \
                 component properties they land in are not named)",
                RECORD,
            )
            .into());
        }
    }
    Ok(())
}

/// The two `RecalculateAttributeInitializedState` keys as a [`NativePack`]. `World::new` registers it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        for key in NATIVE_KEYS {
            vm.register_native(
                key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let controller = c.this.ok_or_else(|| VmError::Script {
                        function: key.to_string(),
                        message: "called without a controller".to_string(),
                    })?;
                    recalculate_attribute_initialized_state(c.vm, controller)?;
                    Ok(Value::None)
                }),
            )?;
        }
        if vm.parity == ParityMode::Off {
            vm.register_native(
                MIND_KEY,
                Rc::new(|c: &mut NativeCall<'_>| {
                    let mind = c.this.ok_or_else(|| VmError::Script {
                        function: MIND_KEY.to_string(),
                        message: "called without a mind".to_string(),
                    })?;
                    recalculate_attribute_initialized_state(c.vm, mind)?;
                    Ok(Value::None)
                }),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "levelup_tests.rs"]
mod tests;
