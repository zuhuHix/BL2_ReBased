//! Owner: lane FIX-RUN-BEHAVIORS (decision 2026-11-19-54). Running behavior lists outside the kernel, in
//! `ParityMode::Off`: `BehaviorBase.RunBehaviors`, the context resolvers its default `ApplyBehavior` and
//! `Behavior_RunBehaviorCollection` reach, `InstanceDataHelper.GetFirstInstanceDataObject`, and
//! `SpecialMoveComponent.RunBehaviors` with its `CanPlayBehaviorData` test. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_RUN_BEHAVIORS.md alone (analyst lane RUN-BEHAVIORS, decision 2026-11-19-47), with the context
//! resolver of NATIVE_BEHAVIOR_CONTEXT.md that note points at. No native code or analysis output was opened. **Every
//! rule is UNVERIFIED in the running game.** [`register`] binds them from [`super::natives::Pack`], in Off only:
//! under Reference the C++ has no such rule (its level-up stand-in is `ow-mission`'s, R3), so nothing changes there.
//!
//! **`BehaviorBase.RunBehaviors(out Behaviors, SelfObject, optional MyInstigatorObject, OtherEventParticipantObject,
//! EventData)`** (the note's section 1):
//! - one kernel-info record for the whole call: the zero value of the record parameter of `ApplyBehavior`, with only
//!   `bIsInitialRunOfThisBehavior` set; each behavior gets the record the previous one left (its `out` value);
//! - the list is walked in index order, its length read again at each step (the caller's storage, through the `out`
//!   alias); `None` entries are skipped; an empty or None list does nothing;
//! - each behavior's script `ApplyBehavior` runs with the self object, the record, the instigator, the other
//!   participant and a copy of the event data; nothing is returned.
//! - **Not a kernel thread.** An output link a behavior selects leads nowhere: each `ApplyBehavior` runs inside its own
//!   frame of the output-link collector ([`super::natives::collect`]) and the ids are dropped, so they never reach an
//!   outer behavior's list (the note's section 5 warning). A wait request (the record's `NextExecutionDelayTime`
//!   left non-zero) is `NotYet { NotRead }`: open question 1.
//! - A port error inside a behavior (a `NotYet`, an unbound native) stops the walk with that error. The game's "a
//!   behavior that fails inside its script does not stop the walk" is a script runtime warning, which the VM logs and
//!   goes past.
//!
//! **`StaticGetBehaviorContext` / `StaticGetAllBehaviorContexts`** (NATIVE_BEHAVIOR_CONTEXT.md, "Siblings sharing the
//! resolver"): the selector picks the base object (0 self, 1 instigator, 2 other participant, 4 the struct's own
//! `ContextObject`; 3 and anything else None). With no `InstancedDataContextName` the base object is the one result.
//! With a name, a base object that does not implement `Engine.IInstanceData` gives nothing, and one that does is
//! `NotYet { NotRead }` (the entry layout was read coarsely; the same open item as `GetFirstInstanceDataObject`'s).
//! The "all" form writes its results to its `out` array and answers whether there is one; an array that arrives
//! non-empty is `NotYet` (whether it is cleared first was not read).
//!
//! **`InstanceDataHelper.GetFirstInstanceDataObject(ContextObj, DataName, DefaultValue)`** (section 3): `DefaultValue`
//! when the name or the context object is missing, or when the context does not implement `Engine.IInstanceData`;
//! a context that does is `NotYet { NotRead }` (open question 2: which entry kinds count).
//!
//! **`SpecialMoveComponent.RunBehaviors(SMBehaviors)`** (section 4): nothing without an owner interface (`SMI`);
//! otherwise each entry, in order, that passes `CanPlayBehaviorData` has its `Behaviors` run by the loop above, with
//! the owner actor as self and the other arguments empty. The entries are read once at the start (the note: the walk
//! is not affected by a change to the list, UNVERIFIED). **`CanPlayBehaviorData(Data)`**: false without an owner
//! interface; else `PlayServer` when the owner's world is not a network client (`NetMode` is not `NM_Client`, 3),
//! `PlayClient` when it is, `PlayLocal` when the owner pawn's `Controller` answers `IsLocalController()` true (read as
//! the stock "locally controlled", UNVERIFIED); the flags are tried in that order.
//!
//! **Not bound:** `GearboxAnimDefinition.OnStart` / `OnServerStart` (a kernel event activation followed by a kernel
//! step the note leaves open, question 3).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::natives::{bind as bind_one, collect};

/// The record these rules come from.
pub const RECORD: &str = "NATIVE_RUN_BEHAVIORS.md";
/// The record of the context resolver.
pub const RECORD_CONTEXT: &str = "NATIVE_BEHAVIOR_CONTEXT.md";

/// `BehaviorBase.RunBehaviors`.
pub const RUN_BEHAVIORS: &str = "Engine.BehaviorBase.RunBehaviors";
/// `BehaviorBase.StaticGetBehaviorContext`.
pub const STATIC_CONTEXT: &str = "Engine.BehaviorBase.StaticGetBehaviorContext";
/// `BehaviorBase.StaticGetAllBehaviorContexts`.
pub const STATIC_ALL_CONTEXTS: &str = "Engine.BehaviorBase.StaticGetAllBehaviorContexts";
/// `InstanceDataHelper.GetFirstInstanceDataObject`: the class's package is not named by the note, so both packages
/// that hold behavior helpers are tried (a path the package set does not declare binds nothing).
pub const FIRST_INSTANCE_DATA: [&str; 2] = [
    "Engine.InstanceDataHelper.GetFirstInstanceDataObject",
    "GearboxFramework.InstanceDataHelper.GetFirstInstanceDataObject",
];
/// `SpecialMoveComponent.RunBehaviors`.
pub const SM_RUN_BEHAVIORS: &str = "GearboxFramework.SpecialMoveComponent.RunBehaviors";
/// `SpecialMoveComponent.CanPlayBehaviorData`.
pub const SM_CAN_PLAY: &str = "GearboxFramework.SpecialMoveComponent.CanPlayBehaviorData";
/// The instance-data interface (NATIVE_BEHAVIOR_CONTEXT.md: the Engine `IInstanceData` interface).
pub const INSTANCE_DATA_INTERFACE: &str = "Engine.IInstanceData";
/// `WorldInfo.NetMode` of a network client (`NM_Client`).
pub const NM_CLIENT: u8 = 3;

/// Binds this module's natives (Off only; module documentation).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let binds: [(&str, NativeFn); 5] = [
        (RUN_BEHAVIORS, Rc::new(run_behaviors)),
        (STATIC_CONTEXT, Rc::new(static_context)),
        (STATIC_ALL_CONTEXTS, Rc::new(static_all_contexts)),
        (SM_RUN_BEHAVIORS, Rc::new(sm_run_behaviors)),
        (SM_CAN_PLAY, Rc::new(sm_can_play)),
    ];
    for (path, f) in binds {
        bind_one(vm, path, f)?;
    }
    for path in FIRST_INSTANCE_DATA {
        bind_one(vm, path, Rc::new(first_instance_data))?;
    }
    Ok(())
}

fn script(function: &str, message: impl Into<String>) -> VmError {
    VmError::Script {
        function: function.to_string(),
        message: message.into(),
    }
}

fn object_of(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

/// Argument `i` as an object; an omitted or absent one is None.
fn arg_object(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    Ok(object_of(c.arg(i)?))
}

/// The field `name` of struct value `v`, or None when it is not a struct or has no such field.
fn struct_field<'v>(vm: &Vm, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let layout = vm.space.classes.struct_layout(s.ty)?;
    let (entry, _) = layout.field(vm.space.names.find(name)?)?;
    s.fields.get(usize::try_from(entry).ok()?)
}

fn set_flag(vm: &Vm, v: &mut Value, name: &str) -> Result<(), VmError> {
    let Value::Struct(s) = v else {
        return Err(script(RUN_BEHAVIORS, "the kernel-info record is not a struct"));
    };
    let layout = vm
        .space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| script(RUN_BEHAVIORS, "the kernel-info record has no layout"))?;
    let (entry, _) = vm
        .space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .ok_or_else(|| script(RUN_BEHAVIORS, format!("the kernel-info record declares no {name}")))?;
    let mut fields = StructValue::clone(s);
    let at = usize::try_from(entry).map_err(|_| script(RUN_BEHAVIORS, "a field entry out of range"))?;
    fields.fields[at] = Value::Bool(true);
    *v = Value::Struct(Rc::new(fields));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// BehaviorBase.RunBehaviors
// ---------------------------------------------------------------------------------------------------------------

/// The arguments every behavior of one walk gets.
struct Walk {
    self_object: Option<ObjRef>,
    instigator: Option<ObjRef>,
    other: Option<ObjRef>,
    /// The event data, or None when the caller gave none (the parameter's zero value is used).
    event_data: Option<Value>,
    /// The shared record, made on the first behavior from its `ApplyBehavior`'s parameter.
    record: Option<Value>,
}

impl Walk {
    /// Runs `behavior`'s script `ApplyBehavior` with this walk's arguments and keeps the record it leaves.
    fn apply(&mut self, vm: &mut Vm, behavior: ObjRef) -> Result<(), VmError> {
        let (class, state) = {
            let o = vm.space.heap.get(behavior)?;
            (o.class, o.state)
        };
        let f = vm.find_method(class, "ApplyBehavior", state)?.ok_or_else(|| {
            script(
                RUN_BEHAVIORS,
                format!("{} has no ApplyBehavior", ow_attr::eval::class_path(&vm.space, behavior)),
            )
        })?;
        let params = vm
            .function(f)
            .map(|d| d.params.clone())
            .ok_or_else(|| script(RUN_BEHAVIORS, "ApplyBehavior is not in this VM"))?;
        if params.len() < 5 {
            return Err(script(
                RUN_BEHAVIORS,
                format!("ApplyBehavior has {} parameters, the note's form has 5", params.len()),
            ));
        }
        if self.record.is_none() {
            let PropKind::Struct(_) = &params[1].kind else {
                return Err(script(RUN_BEHAVIORS, "ApplyBehavior's record parameter is not a struct"));
            };
            let mut record = vm
                .space
                .classes
                .default_value(&params[1].kind)
                .map_err(|e| script(RUN_BEHAVIORS, e.to_string()))?;
            set_flag(vm, &mut record, "bIsInitialRunOfThisBehavior")?;
            self.record = Some(record);
        }
        let event = match &self.event_data {
            Some(v) => v.clone(),
            None => vm
                .space
                .classes
                .default_value(&params[4].kind)
                .map_err(|e| script(RUN_BEHAVIORS, e.to_string()))?,
        };
        let args = vec![
            Value::Object(self.self_object),
            self.record.clone().unwrap_or(Value::None),
            Value::Object(self.instigator),
            Value::Object(self.other),
            event,
        ];
        // A frame of its own: the output links the behavior selects lead nowhere (module documentation).
        let (ran, _dropped) = collect(vm, |vm| vm.call(f, Some(behavior), args))?;
        let ran = ran?;
        if let Some(record) = ran.outs.get(1) {
            self.record = Some(record.clone());
        }
        if let Some(record) = &self.record
            && let Some(Value::Float(delay)) = struct_field(vm, record, "NextExecutionDelayTime")
            && *delay != 0.0
        {
            return Err(NotYet::not_read(
                "BehaviorBase.RunBehaviors: a behavior asked to wait (NextExecutionDelayTime) outside a kernel thread \
                 (NATIVE_RUN_BEHAVIORS.md open question 1)",
                RECORD,
            )
            .into());
        }
        Ok(())
    }
}

fn run_behaviors(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let mut walk = Walk {
        self_object: arg_object(c, 1)?,
        instigator: arg_object(c, 2)?,
        other: arg_object(c, 3)?,
        // An omitted event data arrives as no value: the parameter's zero value is used instead.
        event_data: if c.arg_count() > 4 { Some(c.arg(4)?.clone()).filter(|v| matches!(v, Value::Struct(_))) } else { None },
        record: None,
    };
    let mut i = 0usize;
    loop {
        // The length is read again at every step: a behavior that changes the list changes the walk.
        let entry = match c.arg(0)? {
            Value::Array(items) => match items.get(i) {
                Some(v) => object_of(v),
                None => break,
            },
            Value::None => break,
            other => return Err(script(RUN_BEHAVIORS, format!("Behaviors holds {}, not an array", other.kind_name()))),
        };
        i += 1;
        if let Some(behavior) = entry {
            walk.apply(c.vm, behavior)?;
        }
    }
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------------------------------------
// The context resolvers
// ---------------------------------------------------------------------------------------------------------------

/// What the resolver gives for `data` and the three objects (NATIVE_BEHAVIOR_CONTEXT.md).
fn resolve(
    vm: &mut Vm,
    function: &str,
    data: &Value,
    objects: [Option<ObjRef>; 3],
) -> Result<Vec<ObjRef>, VmError> {
    let selector = match struct_field(vm, data, "BehaviorContext") {
        Some(Value::Byte(b)) => i32::from(*b),
        Some(Value::Int(i)) => *i,
        _ => return Err(script(function, "the context data has no BehaviorContext byte")),
    };
    let base = match selector {
        0 => objects[0],
        1 => objects[1],
        2 => objects[2],
        4 => struct_field(vm, data, "ContextObject").and_then(object_of),
        _ => None,
    };
    let named = match struct_field(vm, data, "InstancedDataContextName") {
        Some(Value::Name(n)) => {
            let t = vm.space.names.text(*n);
            !t.is_empty() && !t.eq_ignore_ascii_case("None")
        }
        _ => false,
    };
    let Some(base) = base else {
        return Ok(Vec::new());
    };
    if !named {
        return Ok(vec![base]);
    }
    if !implements_instance_data(vm, base)? {
        return Ok(Vec::new());
    }
    Err(NotYet::not_read(
        format!("{function} with an InstancedDataContextName on an object with instance data (the entry layout)"),
        RECORD_CONTEXT,
    )
    .into())
}

fn implements_instance_data(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
    let Ok(iface) = vm.find_class(INSTANCE_DATA_INTERFACE) else {
        return Ok(false);
    };
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.implements(class, iface))
}

fn static_context(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let data = c.arg(1)?.clone();
    let objects = [arg_object(c, 2)?, arg_object(c, 3)?, arg_object(c, 4)?];
    let found = resolve(c.vm, STATIC_CONTEXT, &data, objects)?;
    Ok(Value::Object(found.first().copied()))
}

fn static_all_contexts(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const CONTEXTS: usize = 6;
    if c.arg_count() <= CONTEXTS {
        return Err(script(STATIC_ALL_CONTEXTS, "no Contexts out parameter"));
    }
    if matches!(c.arg(CONTEXTS)?, Value::Array(a) if !a.is_empty()) {
        return Err(NotYet::not_read(
            "StaticGetAllBehaviorContexts with a Contexts array that is not empty (whether it is cleared first)",
            RECORD_CONTEXT,
        )
        .into());
    }
    let data = c.arg(1)?.clone();
    let objects = [arg_object(c, 2)?, arg_object(c, 3)?, arg_object(c, 4)?];
    let found = resolve(c.vm, STATIC_ALL_CONTEXTS, &data, objects)?;
    let any = !found.is_empty();
    let array = Value::Array(Rc::new(found.into_iter().map(|o| Value::Object(Some(o))).collect()));
    c.set_out(CONTEXTS, array)?;
    Ok(Value::Bool(any))
}

fn first_instance_data(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let default = if c.arg_count() > 2 { c.arg(2)?.clone() } else { Value::Object(None) };
    let context = arg_object(c, 0)?;
    let named = match c.arg(1)? {
        Value::Name(n) => {
            let t = c.vm.space.names.text(*n);
            !t.is_empty() && !t.eq_ignore_ascii_case("None")
        }
        _ => false,
    };
    let Some(context) = context.filter(|_| named) else {
        return Ok(default);
    };
    if !implements_instance_data(c.vm, context)? {
        return Ok(default);
    }
    Err(NotYet::not_read(
        "InstanceDataHelper.GetFirstInstanceDataObject on a context with instance data (which entry kinds count: \
         NATIVE_RUN_BEHAVIORS.md open question 2)",
        RECORD,
    )
    .into())
}

// ---------------------------------------------------------------------------------------------------------------
// SpecialMoveComponent.RunBehaviors and CanPlayBehaviorData
// ---------------------------------------------------------------------------------------------------------------

/// The component's owner actor, through its `SMI` field; None without one.
fn sm_owner(vm: &Vm, component: ObjRef, function: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let p = vm
        .space
        .prop(class, "SMI")
        .ok_or_else(|| script(function, "the special-move component declares no SMI"))?;
    Ok(object_of(vm.space.read(component, ow_object::ids::PropAddr::scalar(p))?))
}

fn field(vm: &Vm, o: ObjRef, name: &str, function: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| {
        script(function, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))
    })?;
    Ok(vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?.clone())
}

fn flag(vm: &Vm, data: &Value, name: &str, function: &str) -> Result<bool, VmError> {
    match struct_field(vm, data, name) {
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(script(function, format!("the behavior data has no {name} flag"))),
    }
}

/// `CanPlayBehaviorData` for the owner `owner` (module documentation).
fn can_play(vm: &mut Vm, owner: ObjRef, data: &Value, function: &str) -> Result<bool, VmError> {
    let client = || -> Result<bool, VmError> {
        let world = object_of(&field(vm, owner, "WorldInfo", function)?)
            .ok_or_else(|| script(function, "the owner has no WorldInfo"))?;
        match field(vm, world, "NetMode", function)? {
            Value::Byte(m) => Ok(m == NM_CLIENT),
            other => Err(script(function, format!("NetMode holds {}", other.kind_name()))),
        }
    };
    if flag(vm, data, "PlayServer", function)? && !client()? {
        return Ok(true);
    }
    if flag(vm, data, "PlayClient", function)? && client()? {
        return Ok(true);
    }
    if flag(vm, data, "PlayLocal", function)? {
        let Some(controller) = object_of(&field(vm, owner, "Controller", function)?) else {
            return Ok(false);
        };
        let (class, state) = {
            let o = vm.space.heap.get(controller)?;
            (o.class, o.state)
        };
        let Some(f) = vm.find_method(class, "IsLocalController", state)? else {
            return Err(NotYet::not_read(
                "CanPlayBehaviorData's PlayLocal on a controller with no IsLocalController",
                RECORD,
            )
            .into());
        };
        return Ok(matches!(vm.call(f, Some(controller), Vec::new())?.ret, Value::Bool(true)));
    }
    Ok(false)
}

fn sm_can_play(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| script(SM_CAN_PLAY, "no component"))?;
    let Some(owner) = sm_owner(c.vm, me, SM_CAN_PLAY)? else {
        return Ok(Value::Bool(false));
    };
    let data = c.arg(0)?.clone();
    Ok(Value::Bool(can_play(c.vm, owner, &data, SM_CAN_PLAY)?))
}

fn sm_run_behaviors(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| script(SM_RUN_BEHAVIORS, "no component"))?;
    let Some(owner) = sm_owner(c.vm, me, SM_RUN_BEHAVIORS)? else {
        return Ok(Value::None);
    };
    // Read once: a change to the definition's list during the walk does not change it (UNVERIFIED).
    let entries: Vec<Value> = match c.arg(0)? {
        Value::Array(items) => items.as_ref().clone(),
        Value::None => Vec::new(),
        other => {
            return Err(script(SM_RUN_BEHAVIORS, format!("SMBehaviors holds {}, not an array", other.kind_name())));
        }
    };
    for data in entries {
        if !can_play(c.vm, owner, &data, SM_RUN_BEHAVIORS)? {
            continue;
        }
        let behaviors = match struct_field(c.vm, &data, "Behaviors") {
            Some(Value::Array(items)) => items.as_ref().clone(),
            Some(Value::None) | None => Vec::new(),
            Some(other) => {
                return Err(script(SM_RUN_BEHAVIORS, format!("Behaviors holds {}, not an array", other.kind_name())));
            }
        };
        let mut walk = Walk {
            self_object: Some(owner),
            instigator: None,
            other: None,
            event_data: None,
            record: None,
        };
        for behavior in behaviors.iter().filter_map(object_of) {
            walk.apply(c.vm, behavior)?;
        }
    }
    Ok(Value::None)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
