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
//! lookup and the vector constant remain unresolved, so the call then returns NotYet without changing motion. That
//! NotYet follows the append, so it is terminal: prefixed [`PARTIAL_WRITES`], its kind kept (review r1 of #456).
//! With no owner there is no motion side effect. A duplicate returns before this lookup.
//!
//! **Which components.** An exact `AIComponent`, and an exact `WillowAIComponent`: the note's second reading
//! (NATIVE_AI_HOLDS.md, read 2026-11-19 by lane LINK-1-NEXT; implemented by lane FIX-AI-HOLDS-SUBCLASS, decision
//! 2026-11-19-43) found that `WillowAIComponent` keeps `AIComponent`'s routines for the movement, demigod, god and
//! dialog kinds (and the target kind, `ow-ai`'s), replacing only the base kind. Any other subclass's table was not
//! read: `NotYet { NotRead }`, as before. Damage-reader behavior is not proven.
//!
//! **`GearboxMind.GetAIComponent`** (lane IMPL-GEARBOXMIND-GETAICOMPONENT, decision 2026-11-19-NNN), coded from
//! `docs/verification/NATIVE_GEARBOXMIND_GETAICOMPONENT.md` alone (the note at 4b83f1c5; clean room: lane
//! ANALYST-GEARBOXMIND-GETAICOMPONENT wrote it). In Off it is [`get_ai_component`]: the mind's own `AIComponent`, read
//! by name and returned as it is, None included. It writes, loads and draws nothing, and the returned handle is not
//! loaded either. UNVERIFIED in the running game.
//!
//! **A pawn context** (lane IMPL-PAWN-AI-INTERFACE, decision 2026-11-19-62): in Off the script's `QueryInterface`
//! answers by `Pawn`'s rule ([`crate::pawn_interface`], registered here), so the hold behavior runs on a pawn when the
//! rule gives an answer or none, and stops at the rule's `NotYet` (a controller, a vehicle's controller or a mesh whose
//! own answer is not read). In Off every context's question is asked before any script runs
//! ([`apply_hold_behavior`]), so such a refusal leaves every hold list as it was; a script's own refusal after an
//! earlier context's script ran is terminal. Reference keeps its per-context order.
//!
//! **Reference** (R3) binds none of this: `crate::hit` keeps the C++ stand-ins there (the demigod hold as a list of
//! reasons, `GetAIComponent` as the mind's property, the pawn-to-mind context). The Off answer of `GetAIComponent`
//! gives the same value as that stand-in for a mind with its own component.

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

/// The mind's script native, answered in Off by [`get_ai_component`].
pub const GET_AI_COMPONENT: &str = "GearboxFramework.GearboxMind.GetAIComponent";

/// The analyst note of [`get_ai_component`].
pub const RECORD_GET_AI_COMPONENT: &str = "docs/verification/NATIVE_GEARBOXMIND_GETAICOMPONENT.md";

/// The class the native belongs to.
pub const MIND: &str = "GearboxFramework.GearboxMind";


fn error(message: String) -> VmError {
    VmError::Script {
        function: "ai holds".to_string(),
        message,
    }
}

/// Binds the four kinds' twelve natives, [`GET_AI_COMPONENT`] ([`get_ai_component`]) and the pawn `QueryInterface` rule
/// ([`crate::pawn_interface::register`]) in Off; nothing under Reference. A key another pack bound first keeps its
/// binding.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    crate::pawn_interface::register(vm)?;
    for (kind, list) in KINDS {
        bind(vm, &format!("{COMPONENT}.Hold{kind}"), hold(list, kind == "Movement"))?;
        bind(vm, &format!("{COMPONENT}.Release{kind}"), release(list))?;
        bind(vm, &format!("{COMPONENT}.{kind}OnHold"), on_hold(list))?;
    }
    bind(vm, GET_AI_COMPONENT, Rc::new(get_ai_component))
}

/// `GearboxMind.GetAIComponent()` in Off (NATIVE_GEARBOXMIND_GETAICOMPONENT.md section 3): the receiver's
/// `AIComponent`, read by name with a plain read and returned as it is, the component handle or None. `WillowMind` and
/// any other subclass answer the same. It writes, loads and draws nothing; an unloaded component handle is returned
/// as the handle, not loaded. Classes are looked up only in the class table (`by_path`), never loaded.
///
/// Refusals, all before any effect: no receiver; a receiver whose class is not `GearboxMind` or a subclass (a class
/// not in the table counts as not one); a class that declares no `AIComponent`; a value that is not an object: each a
/// `VmError::Script`. The field is read straight from the heap (no `access::get`, which loads an unloaded handle).
///
/// **Pending the note's review r1 of #594** (two clarifications, not coded yet): how the note's template refusal (an
/// `AIComponent` that is the class default's template component) is detected without a load, and what a receiver
/// that is itself an unloaded handle answers. Until then neither case is special-cased here.
pub fn get_ai_component(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let vm = &*c.vm;
    let fail = |message: String| VmError::Script {
        function: GET_AI_COMPONENT.to_string(),
        message,
    };
    let this = c.this.ok_or_else(|| fail("called with no mind".to_string()))?;
    let class = vm.space.heap.get(this)?.class;
    let mind = vm.space.classes.by_path(MIND);
    if !mind.is_some_and(|m| vm.space.classes.is_child_of(class, m)) {
        return Err(fail(format!("{} is not a GearboxMind", ow_attr::eval::class_path(&vm.space, this))));
    }
    let Some(p) = vm.space.prop(class, "AIComponent") else {
        return Err(fail(format!("{} declares no AIComponent", ow_attr::eval::class_path(&vm.space, this))));
    };
    let component = match vm.space.read(this, ow_object::ids::PropAddr::scalar(p))? {
        Value::Object(r) => *r,
        other => return Err(fail(format!("AIComponent holds {}, not an object", other.kind_name()))),
    };
    Ok(Value::Object(component))
}

/// The text a hold refusal carries once a context's script has run (review r1 of #456, item 2), the kernel's
/// `PARTIAL_WRITES` wording.
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

/// Runs the hold behavior `behavior`'s installed script (`ApplyBehaviorToContext`) on each of `contexts`, with the
/// context itself as `ContextObject` (decision 2026-11-16-74). What may refuse:
/// - a pawn, in Off: the script's question, `QueryInterface(IAIInterface)`, answered by `Pawn`'s rule
///   ([`crate::pawn_interface`], which a world registers through [`register`]); its `NotYet` stops the run. A VM
///   whose pawn class has no rule is an error, not a quiet plain answer;
/// - another context, in Off: the same question by its class's own answer (an open answer bound as `NotYet`, such as
///   a mind's), when the VM holds `IAIInterface`;
/// - a pawn, under Reference: `NotYet { NotRead }`, as before lane IMPL-PAWN-AI-INTERFACE (Reference runs the C++
///   stand-in instead);
/// - a context that is not a pawn and whose `Controller` or `MyWillowMind` is set, in both modes: `NotYet { NotRead }`
///   (its class's answer is not read); a context with neither runs the script.
///
/// **Order.** In Off every context is judged first ([`preflight_hold_contexts`]) and only then is any script run, so
/// those refusals change nothing (the hub's rule: refusals before any write). A refusal from a script itself (an
/// `GetAIComponent` refusal, a hold override, a release out of order) after an earlier context's script has run may
/// follow that script's hold writes, so it is terminal: a `NotYet` is prefixed [`PARTIAL_WRITES`], its kind kept, and
/// other errors are returned unchanged (review r1 of #456, item 2). "Has run" is the honest bound: the port does not
/// tell a script that held something from one that held nothing. Under Reference each context is judged just before
/// its own script, as before this lane, so a plain context ahead of a pawn runs and the pawn then refuses (review r1
/// of #456, item 5).
///
/// An action class without the event is an error.
pub fn apply_hold_behavior(
    vm: &mut Vm,
    behavior: ObjRef,
    contexts: &[ObjRef],
) -> Result<(), VmError> {
    let off = vm.parity == ParityMode::Off;
    if off {
        preflight_hold_contexts(vm, contexts)?;
    }
    let mut ran = false;
    for &context in contexts {
        if !off {
            judge_context(vm, context)?;
        }
        let answer = match call_method(
            vm,
            behavior,
            "ApplyBehaviorToContext",
            vec![Value::Object(Some(context))],
        ) {
            Err(e) if off && ran => return Err(after_partial_writes(e)),
            other => other?,
        };
        if answer.is_none() {
            return Err(error(format!(
                "the hold behavior {} has no ApplyBehaviorToContext",
                class_name(vm, behavior)
            )));
        }
        ran = true;
    }
    Ok(())
}

/// The refusals [`apply_hold_behavior`] checks before any script runs, for each of `contexts` in turn; writes nothing.
/// A caller that knows a hold's contexts before an event writes anything (the mission slice, review r1 of #456, item
/// 1) asks this first.
pub fn preflight_hold_contexts(vm: &mut Vm, contexts: &[ObjRef]) -> Result<(), VmError> {
    for &context in contexts {
        judge_context(vm, context)?;
    }
    Ok(())
}

/// A refusal after an earlier context's script ran ([`apply_hold_behavior`]): a `NotYet` is prefixed
/// [`PARTIAL_WRITES`] once, its kind kept; any other error is returned unchanged.
pub fn after_partial_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) if !n.what.starts_with(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

/// Whether the hold behavior may run on `context` ([`apply_hold_behavior`]); writes nothing.
fn judge_context(vm: &mut Vm, context: ObjRef) -> Result<(), VmError> {
    if is_a(vm, context, crate::pawn_interface::PAWN)? {
        if vm.parity != ParityMode::Off {
            return Err(NotYet::not_read("Pawn IAIInterface routing is unread", RECORD_HOLDS).into());
        }
        let class = vm.space.heap.get(context)?.class;
        if vm.interface_rule_for(class).is_none() {
            return Err(error(format!(
                "the pawn {} has no QueryInterface rule registered (ow_combat::pawn_interface::register)",
                class_name(vm, context)
            )));
        }
        let iface = vm.find_class(crate::pawn_interface::AI_INTERFACE)?;
        vm.query_interface(context, iface)?;
        return Ok(());
    }
    if vm.parity == ParityMode::Off
        && let Some(iface) = vm.find_class_if_present(crate::pawn_interface::AI_INTERFACE)?
    {
        vm.query_interface(context, iface)?;
    }
    for link in ["Controller", "MyWillowMind"] {
        if object(vm, context, link)?.is_some() {
            return Err(NotYet::not_read(
                format!(
                    "Behavior_AIHold on {}, which is not a pawn and whose {link} is set: its class's QueryInterface \
                     answer is not read",
                    class_name(vm, context)
                ),
                RECORD_HOLDS,
            )
            .into());
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
            // After the append: terminal, as the hub's rule has it for a NotYet that follows a write.
            return Err(NotYet::not_read(
                format!(
                    "{PARTIAL_WRITES}: HoldMovement appended the pair; owner controller virtual and motion vector                      constant unresolved"
                ),
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
