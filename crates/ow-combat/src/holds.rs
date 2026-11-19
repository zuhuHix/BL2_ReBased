//! Owner: lane FIX-AIHOLD-NATIVES. The AI component's movement, demigod, god and dialog hold natives in Off, and the
//! hold behavior's run on its contexts. AI-assisted (Claude Code). Decision 2026-11-16-83.
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_AI_HOLDS.md` alone, by a lane that did not read these
//! natives (the analyst was lane FIX-AIHOLD, decision 2026-11-16-74). Every rule is **UNVERIFIED** in the running game.
//!
//! **Why here.** `ow-ai` owns the AI component's natives (`ow_ai::awareness::holds::Pack` registers these), but
//! `ow-combat`'s pawns reach the demigod kind through their script (`IsDemigodMode`), and `ow-combat` sits below
//! `ow-ai`, so the rule lives here and both crates' packs bind it: [`crate::hit::Pack`] and `ow-ai`'s pack.
//!
//! **The kinds** ([`register`], Off only). Each kind keeps its own `AIHoldData` array of (`HoldName`, `Tag`) pairs on
//! the component ([`KINDS`]); the base and target kinds are `ow-ai`'s (part 2, `ow_ai::awareness::actions`), and this
//! is the same rule on the other four lists:
//! - `Hold<Kind>(HoldName, Tag)`: a None tag does nothing; a pair already in the list does nothing; otherwise the pair
//!   is appended at the end;
//! - `Release<Kind>(HoldName, Tag)`: a None tag does nothing; a pair not in the list does nothing; otherwise the pair
//!   is removed. Whether the entries after it keep their order is not read, so removing a pair that is not the last
//!   entry is `NotYet { NotRead }`, with the list left as it was;
//! - `<Kind>OnHold()`: whether the list has an entry.
//!
//! **Movement side effect.** A new pair is appended first. With a non-null Owner the owner's virtual controller
//! lookup and the vector constant remain unresolved, so the call then returns NotYet without changing motion.
//! With no owner there is no motion side effect. A duplicate returns before this lookup.
//!
//! **Which components.** An exact `AIComponent`, and an exact `WillowAIComponent`: the note's second reading
//! (NATIVE_AI_HOLDS.md, read 2026-11-19 by lane LINK-1-NEXT; implemented by lane FIX-AI-HOLDS-SUBCLASS, decision
//! 2026-11-19-43) found that `WillowAIComponent` keeps `AIComponent`'s routines for the movement, demigod, god and
//! dialog kinds (and the target kind, `ow-ai`'s), replacing only the base kind. Any other subclass's table was not
//! read: `NotYet { NotRead }`, as before. `GearboxMind.GetAIComponent` and the pawn IAIInterface routing remain NotYet.
//! Damage-reader behavior is not proven.
//!
//! **Reference** (R3) binds none of this: `crate::hit` keeps the C++ stand-ins there (the demigod hold as a list of
//! reasons, `GetAIComponent` as the mind's property, the pawn-to-mind context).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::hit::access::{class_name, field, get, is_a, object, set, set_field};
use crate::hit::{bind, call_method};

/// The analyst note of the hold kinds and of the pawn question.
pub const RECORD_HOLDS: &str = "docs/verification/NATIVE_AI_HOLDS.md";

/// The four kinds: the natives' kind word and the component's list of `AIHoldData` pairs.
pub const KINDS: [(&str, &str); 4] = [
    ("Movement", "MovementHolds"),
    ("Demigod", "DemigodHolds"),
    ("God", "GodHolds"),
    ("Dialog", "DialogHolds"),
];

/// The class whose script natives these are.
pub const COMPONENT: &str = "GearboxFramework.AIComponent";

/// The subclass that keeps these four kinds' routines (NATIVE_AI_HOLDS.md, read 2026-11-19).
pub const WILLOW_COMPONENT: &str = "WillowGame.WillowAIComponent";

/// The script native the note does not resolve (`NotYet` in Off).
pub const GET_AI_COMPONENT: &str = "GearboxFramework.GearboxMind.GetAIComponent";

fn error(message: String) -> VmError {
    VmError::Script {
        function: "ai holds".to_string(),
        message,
    }
}

/// Binds the four kinds' twelve natives and [`GET_AI_COMPONENT`] (`NotYet`) in Off; nothing under Reference. A key
/// another pack bound first keeps its binding.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    for (kind, list) in KINDS {
        bind(vm, &format!("{COMPONENT}.Hold{kind}"), hold(list, kind == "Movement"))?;
        bind(vm, &format!("{COMPONENT}.Release{kind}"), release(list))?;
        bind(vm, &format!("{COMPONENT}.{kind}OnHold"), on_hold(list))?;
    }
    bind(
        vm,
        GET_AI_COMPONENT,
        Rc::new(|_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
            Err(NotYet::not_read(
                "GearboxMind.GetAIComponent: which object of the mind answers it is not resolved (the mind's \
                 AIComponent property is plausible and unproven)",
                RECORD_HOLDS,
            )
            .into())
        }),
    )
}

/// Off: runs the hold behavior `behavior`'s installed script (`ApplyBehaviorToContext`) on each of `contexts`, with
/// the context itself as `ContextObject` (decision 2026-11-16-74). How a pawn context reaches its AI interface is not
/// read (NATIVE_AI_HOLDS.md, "Not read": the class tables list `IAIInterface` for the mind only, yet the live hold
/// reached the dummy's AI), so a context whose `Controller` or `MyWillowMind` is set is `NotYet { NotRead }` with
/// nothing run; a context with neither runs the script, which can reach no AI component either way. An action class
/// without the event is an error.
pub fn apply_hold_behavior(
    vm: &mut Vm,
    behavior: ObjRef,
    contexts: &[ObjRef],
) -> Result<(), VmError> {
    for &context in contexts {
        if is_a(vm, context, "Engine.Pawn")? {
            return Err(NotYet::not_read("Pawn IAIInterface routing is unread", RECORD_HOLDS).into());
        }
        for link in ["Controller", "MyWillowMind"] {
            if object(vm, context, link)?.is_some() {
                return Err(NotYet::not_read(
                    format!(
                        "Behavior_AIHold on {}, whose {link} is set: how a pawn context reaches its AI interface (the \
                         class tables list IAIInterface for the mind only, and the live hold reached the dummy's AI)",
                        class_name(vm, context)
                    ),
                    RECORD_HOLDS,
                )
                .into());
            }
        }
        if call_method(
            vm,
            behavior,
            "ApplyBehaviorToContext",
            vec![Value::Object(Some(context))],
        )?
        .is_none()
        {
            return Err(error(format!(
                "the hold behavior {} has no ApplyBehaviorToContext",
                class_name(vm, behavior)
            )));
        }
    }
    Ok(())
}

fn this(c: &mut NativeCall<'_>) -> Result<ObjRef, VmError> {
    let me = c.this.ok_or_else(|| error("a hold native called without an object".to_string()))?;
    crate::hit::access::loaded(c.vm, me)?;
    let class = c.vm.space.heap.get(me)?.class;
    let willow = c.vm.find_class(WILLOW_COMPONENT).ok();
    if class != c.vm.find_class(COMPONENT)? && Some(class) != willow {
        return Err(NotYet::not_read(
            "AI hold virtual overrides of a component class other than AIComponent and WillowAIComponent are unread",
            RECORD_HOLDS,
        )
        .into());
    }
    Ok(me)
}

/// The hold name and the tag of a hold native's call (arguments 0 and 1; a missing tag is None).
fn pair(c: &NativeCall<'_>) -> Result<(Value, Option<ObjRef>), VmError> {
    let name = c.arg(0)?.clone();
    let tag = if c.arg_count() > 1 {
        match c.arg(1)? {
            Value::Object(o) => *o,
            _ => None,
        }
    } else {
        None
    };
    Ok((name, tag))
}

fn read_list(vm: &mut Vm, me: ObjRef, list: &str) -> Result<Vec<Value>, VmError> {
    match get(vm, me, list)? {
        Some(Value::Array(items)) => Ok((*items).clone()),
        _ => Err(error(format!("AIComponent has no {list} array"))),
    }
}

/// The component's `list` and where the pair (`name`, `tag`) is in it.
fn find(
    vm: &mut Vm,
    me: ObjRef,
    list: &str,
    name: &Value,
    tag: ObjRef,
) -> Result<(Vec<Value>, Option<usize>), VmError> {
    let items = read_list(vm, me, list)?;
    let at = items.iter().position(|v| {
        field(vm, v, "HoldName").as_ref() == Some(name)
            && field(vm, v, "Tag") == Some(Value::Object(Some(tag)))
    });
    Ok((items, at))
}

/// A new `AIHoldData` entry of the component's `list` (the array's element type, defaults, then the pair).
fn entry(vm: &mut Vm, me: ObjRef, list: &str, name: Value, tag: ObjRef) -> Result<Value, VmError> {
    let class = vm.space.heap.get(me)?.class;
    let kind = vm
        .space
        .classes
        .props(class)
        .map_err(ow_object::heap::HeapError::from)?
        .into_iter()
        .find(|(_, d)| vm.space.names.text(d.name).eq_ignore_ascii_case(list))
        .and_then(|(_, d)| match &d.kind {
            ow_object::layout::PropKind::Array(inner) => Some((**inner).clone()),
            _ => None,
        })
        .ok_or_else(|| {
            error(format!(
                "the component {} has no {list} array",
                class_name(vm, me)
            ))
        })?;
    let mut v = vm
        .space
        .classes
        .default_value(&kind)
        .map_err(ow_object::heap::HeapError::from)?;
    if !set_field(vm, &mut v, "HoldName", name) || !set_field(vm, &mut v, "Tag", Value::Object(Some(tag))) {
        return Err(error(format!(
            "the entries of {list} have no HoldName and Tag"
        )));
    }
    Ok(v)
}

/// `Hold<Kind>` on `list`; with `movement`, the side effect of a newly added pair (module documentation).
fn hold(list: &'static str, movement: bool) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| {
        let me = this(c)?;
        let (name, tag) = pair(c)?;
        let Some(tag) = tag else {
            return Ok(Value::None);
        };
        let (mut items, at) = find(c.vm, me, list, &name, tag)?;
        if at.is_some() {
            return Ok(Value::None);
        }
        items.push(entry(c.vm, me, list, name, tag)?);
        set(c.vm, me, list, Value::Array(Rc::new(items)))?;
        if movement && object(c.vm, me, "Owner")?.is_some() {
            return Err(NotYet::not_read(
                "HoldMovement appended the pair; owner controller virtual and motion vector constant unresolved",
                RECORD_HOLDS,
            ).into());
        }
        Ok(Value::None)
    })
}

/// `Release<Kind>` on `list` (module documentation).
fn release(list: &'static str) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| {
        let me = this(c)?;
        let (name, tag) = pair(c)?;
        let Some(tag) = tag else {
            return Ok(Value::None);
        };
        let (mut items, at) = find(c.vm, me, list, &name, tag)?;
        let Some(at) = at else {
            return Ok(Value::None);
        };
        if at + 1 != items.len() {
            return Err(NotYet::not_read(
                format!(
                    "{list}: removing entry {at} of {} (whether the entries after a removed hold keep their order is \
                     not read)",
                    items.len()
                ),
                RECORD_HOLDS,
            )
            .into());
        }
        items.pop();
        set(c.vm, me, list, Value::Array(Rc::new(items)))?;
        Ok(Value::None)
    })
}

/// `<Kind>OnHold` on `list`.
fn on_hold(list: &'static str) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| {
        let me = this(c)?;
        Ok(Value::Bool(!read_list(c.vm, me, list)?.is_empty()))
    })
}

#[cfg(test)]
mod tests;
