//! Owner: lane IMPL-BODYCOMP-RENDER-HOOKS (decision 2026-11-19-268, number pending). The Off rules of
//! `PrimitiveComponent.SetHidden` / `SetOwnerNoSee` ([`set_flag`]) and `PrimitiveComponent.SetLightEnvironment`
//! ([`set_light_environment`]), split into read-only planners and apply functions so the body composition's
//! rendering hooks (NATIVE_BODYCOMP_RENDER_HOOKS.md section 4) and the natives in `ow-inventory` share one
//! implementation. AI-assisted (Claude Code).
//!
//! **Moved, not changed.** The rules are NATIVE_ATTACH_MESH_TO_PICKUP.md sections 2, 3 and 6.2 as `ow-inventory`'s
//! `attach_mesh.rs` implemented them (lanes IMPL-ATTACH-MESH-TO-PICKUP parts 1 and 2, #511 and #520). They moved here
//! because `ow-inventory` depends on `ow-world`, so the body composition could not call them there (review r1 of #622,
//! item 1). The checks, their order and the error texts are unchanged; `ow-inventory`'s natives now call these. Every
//! rule is **UNVERIFIED** in the running game.
//!
//! **The slot view.** A planner reads a component through [`Slots`]: the object, plus values that stand in for some of
//! its slots. The body composition plans on a component template, with the slots the attach will write (`Owner` = the
//! pawn, `bNeedsReattach` and `bNeedsUpdateTransform` false) standing in for the template's own. The declarations are
//! always the object's class's: a template's class is the made component's.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record the setters follow.
pub const RECORD: &str = "docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md";
/// The deferred reattach's two refusals.
pub const REATTACH_OWNER: &str = "the deferred reattach: the owner's component update";
pub const REATTACH_SCENE: &str = "the deferred reattach: the component's detach and reattach";
/// The owner rule's refusal for an owner the store holds only as a handle (NATIVE_ATTACH_MESH_TO_PICKUP.md 2.1.1).
pub const REATTACH_OWNER_NOT_LOADED: &str = "the deferred reattach: an owner the store has not loaded (a load is a write)";
/// The owner flags the owner rule reads, by name, from the owner instance.
pub const OWNER_FLAGS: [(&str, Want); 3] = [("bStatic", Want::Bool), ("bTickIsDisabled", Want::Bool), ("bDeleteMe", Want::Bool)];

/// A data error of native `what`.
pub fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn declares_no(vm: &Vm, o: ObjRef, name: &str, what: &str) -> VmError {
    bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))
}

/// A component as a planner reads it (module documentation).
#[derive(Debug, Clone, Copy)]
pub struct Slots<'a> {
    pub object: ObjRef,
    pub overrides: &'a [(&'a str, Value)],
}

impl<'a> Slots<'a> {
    /// The object as it stands.
    pub fn of(object: ObjRef) -> Slots<'static> {
        Slots { object, overrides: &[] }
    }

    /// The value of `name`: its stand-in, else the object's slot. A class that declares no `name` is a data error.
    pub fn required(&self, vm: &Vm, name: &str, what: &str) -> Result<Value, VmError> {
        let class = vm.space.heap.get(self.object)?.class;
        let Some(p) = vm.space.prop(class, name) else {
            return Err(declares_no(vm, self.object, name, what));
        };
        if let Some((_, v)) = self.overrides.iter().find(|(n, _)| *n == name) {
            return Ok(v.clone());
        }
        Ok(vm.space.read(self.object, ow_object::ids::PropAddr::scalar(p))?.clone())
    }
}

/// `name`'s declared kind on `o`'s class, else a data error naming it.
pub fn declared(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<PropKind, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| declares_no(vm, o, name, what))?;
    vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} has no declaration")))
}

/// The kinds the setters check.
#[derive(Clone, Copy)]
pub enum Want {
    Object,
    Bool,
    Int,
    Float,
    Struct,
    Array,
}

/// Each `(name, kind)` declared on `o` with that kind, else a data error naming the field.
pub fn schemas(vm: &Vm, o: ObjRef, fields: &[(&str, Want)], what: &str) -> Result<(), VmError> {
    for &(name, want) in fields {
        let kind = declared(vm, o, name, what)?;
        let ok = match want {
            Want::Object => matches!(kind, PropKind::Object { .. }),
            Want::Bool => matches!(kind, PropKind::Bool),
            Want::Int => matches!(kind, PropKind::Int),
            Want::Float => matches!(kind, PropKind::Float),
            Want::Struct => matches!(kind, PropKind::Struct(_)),
            Want::Array => matches!(kind, PropKind::Array(_)),
        };
        if !ok {
            return Err(bad(what, format!("{name} is declared as {kind:?}, not the kind the note reads")));
        }
    }
    Ok(())
}

/// The component fields the deferred reattach reads and writes.
pub const REATTACH_FIELDS: [(&str, Want); 3] = [("Owner", Want::Object), ("Scene", Want::Struct), ("bNeedsReattach", Want::Bool)];

/// `v` as a bool, else a data error naming `name`.
pub fn bool_of(v: &Value, name: &str, what: &str) -> Result<bool, VmError> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(bad(what, format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

/// `v` as an object reference, else a data error.
pub fn object_of(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        other => Err(bad(what, format!("expected an object, found {}", other.kind_name()))),
    }
}

/// Whether `v`, the `Scene` pointer struct, is zero: every field an int 0, at any depth.
pub fn is_zero(v: &Value, what: &str) -> Result<bool, VmError> {
    match v {
        Value::Int(i) => Ok(*i == 0),
        Value::Struct(s) => {
            for f in &s.fields {
                if !is_zero(f, what)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        other => Err(bad(what, format!("Scene holds {}, not a pointer struct of ints", other.kind_name()))),
    }
}

/// The deferred reattach's owner rule (NATIVE_ATTACH_MESH_TO_PICKUP.md section 2.1.1, lane IMPL-DEFERRED-REATTACH-OWNER;
/// UNVERIFIED in game), one rule for every caller:
/// 1. the owner must be resident (not a store handle, not a stand-in with no slots), else
///    [`REATTACH_OWNER_NOT_LOADED`] (`NotRead`), with nothing loaded;
/// 2. its `bStatic`, `bTickIsDisabled` and `bDeleteMe` are read by name from the **instance**, each declared as a bool,
///    else a data error naming it;
/// 3. `bStatic` or `bTickIsDisabled` set, and `bDeleteMe` clear: [`REATTACH_OWNER`] (`NotRead`), whatever the owner's
///    pending-kill and template-object states, which the port does not model here (it refuses on the side where the
///    game may update);
/// 4. otherwise: no refusal, and the caller writes as in the no-owner, no-scene case.
///
/// Reads only.
pub fn owner_rule(vm: &Vm, owner: ObjRef, what: &str) -> Result<(), VmError> {
    let obj = vm.space.heap.get(owner)?;
    let full = u32::try_from(obj.slots.len()).ok() == vm.space.classes.get_layout(obj.class).map(|l| l.slot_count());
    if vm.defs.is_unloaded(&vm.space, owner) || !full {
        return Err(NotYet::not_read(REATTACH_OWNER_NOT_LOADED, RECORD).into());
    }
    schemas(vm, owner, &OWNER_FLAGS, what)?;
    let flag = |name: &str| -> Result<bool, VmError> { bool_of(&Slots::of(owner).required(vm, name, what)?, name, what) };
    if (flag("bStatic")? || flag("bTickIsDisabled")?) && !flag("bDeleteMe")? {
        return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into());
    }
    Ok(())
}

/// The deferred reattach's preflight: with `Owner` set, the [`owner_rule`]; with none, a `Scene` not zero is
/// [`REATTACH_SCENE`] (`NotYet { NotRead }`). Reads only; `Owner` is read through the caller's slots, so a template
/// plan sees the attach's stand-in.
pub fn reattach_check(vm: &Vm, c: Slots<'_>, what: &str) -> Result<(), VmError> {
    if let Some(owner) = object_of(&c.required(vm, "Owner", what)?, what)? {
        return owner_rule(vm, owner, what);
    }
    if !is_zero(&c.required(vm, "Scene", what)?, what)? {
        return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
    }
    Ok(())
}

/// `e` after a native's first write: a `NotYet` marked once, its kind and record kept; any other error unchanged.
pub fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
        other => other,
    }
}

/// Writes `v` to `name` on `o`, else a data error naming the field.
pub fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| declares_no(vm, o, name, what))?;
    vm.space.write(o, ow_object::ids::PropAddr::scalar(p), v)?;
    Ok(())
}

/// `v` against `name`'s declaration on `o` by the heap's own write rules, else a data error naming the field.
pub fn fits(vm: &Vm, o: ObjRef, name: &str, v: &Value, what: &str) -> Result<(), VmError> {
    let kind = declared(vm, o, name, what)?;
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, v)
        .map_err(|e| bad(what, format!("{name} cannot take the value: {e}")))
}

/// `set_flag`'s checks before the deferred reattach, in its order: the reattach's declarations, then the flag's,
/// then the equal-value shortcut. `false`: the flag already equals `value`, nothing happens. `true`: the setter goes
/// on to [`reattach_check`] and then writes. Reads only.
pub fn plan_flag(vm: &Vm, c: Slots<'_>, flag: &str, value: bool, what: &str) -> Result<bool, VmError> {
    schemas(vm, c.object, &REATTACH_FIELDS, what)?;
    schemas(vm, c.object, &[(flag, Want::Bool)], what)?;
    Ok(bool_of(&c.required(vm, flag, what)?, flag, what)? != value)
}

/// `set_flag`'s writes, after its planner and the reattach's preflight passed: the flag, then `bNeedsReattach`.
pub fn apply_flag(vm: &mut Vm, component: ObjRef, flag: &str, value: bool, what: &str) -> Result<(), VmError> {
    write(vm, component, flag, Value::Bool(value), what)?;
    write(vm, component, "bNeedsReattach", Value::Bool(true), what).map_err(after_writes)
}

/// `SetHidden` or `SetOwnerNoSee` on `component` in Off: `flag` set to `value` (NATIVE_ATTACH_MESH_TO_PICKUP.md
/// section 3): when the flag already equals the argument, nothing; otherwise the deferred reattach's preflight, then
/// the flag, then `bNeedsReattach`.
pub fn set_flag(vm: &mut Vm, component: ObjRef, flag: &str, value: bool, what: &str) -> Result<(), VmError> {
    if !plan_flag(vm, Slots::of(component), flag, value, what)? {
        return Ok(());
    }
    reattach_check(vm, Slots::of(component), what)?;
    apply_flag(vm, component, flag, value, what)
}

/// What `SetLightEnvironment` will do, decided before any write.
#[derive(Debug, Clone, PartialEq)]
pub enum LightPlan {
    /// The argument equals `LightEnvironment`.
    Nothing,
    /// A component not attached: the one write `LightEnvironment`.
    Detached(Value),
    /// An attached one, past the deferred reattach's refusals: `PreviousLightEnvironment` = `previous`, then
    /// `LightEnvironment` = `new`, then `bNeedsReattach`.
    Attached { previous: Value, new: Value },
}

const LIGHT: &str = "PrimitiveComponent.SetLightEnvironment";

/// `SetLightEnvironment(new)`'s checks (NATIVE_ATTACH_MESH_TO_PICKUP.md section 6.2), each only on the path that
/// reaches it (review r1 of #520): the equal value first, then `bAttached`; on the attached path the deferred
/// reattach's refusals (owner, then scene), and only then the write targets, each destination's slot read (review r2
/// of #520). Reads only.
pub fn plan_light_environment(vm: &Vm, c: Slots<'_>, new: Option<ObjRef>) -> Result<LightPlan, VmError> {
    let o = c.object;
    schemas(vm, o, &[("LightEnvironment", Want::Object)], LIGHT)?;
    let current = object_of(&c.required(vm, "LightEnvironment", LIGHT)?, LIGHT)?;
    if current == new {
        return Ok(LightPlan::Nothing);
    }
    schemas(vm, o, &[("bAttached", Want::Bool)], LIGHT)?;
    let attached = bool_of(&c.required(vm, "bAttached", LIGHT)?, "bAttached", LIGHT)?;
    let value = Value::Object(new);
    if !attached {
        fits(vm, o, "LightEnvironment", &value, LIGHT)?;
        return Ok(LightPlan::Detached(value));
    }
    schemas(vm, o, &[("Owner", Want::Object)], LIGHT)?;
    if let Some(owner) = object_of(&c.required(vm, "Owner", LIGHT)?, LIGHT)? {
        // The same deferred reattach (section 6.2 step 4): the owner rule (lane IMPL-DEFERRED-REATTACH-OWNER).
        owner_rule(vm, owner, LIGHT)?;
    } else {
        schemas(vm, o, &[("Scene", Want::Struct)], LIGHT)?;
        if !is_zero(&c.required(vm, "Scene", LIGHT)?, LIGHT)? {
            return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
        }
    }
    schemas(vm, o, &[("PreviousLightEnvironment", Want::Object), ("bNeedsReattach", Want::Bool)], LIGHT)?;
    let previous = Value::Object(current);
    fits(vm, o, "PreviousLightEnvironment", &previous, LIGHT)?;
    fits(vm, o, "LightEnvironment", &value, LIGHT)?;
    for name in ["PreviousLightEnvironment", "LightEnvironment", "bNeedsReattach"] {
        c.required(vm, name, LIGHT)?;
    }
    Ok(LightPlan::Attached { previous, new: value })
}

/// `SetLightEnvironment`'s writes, from its plan. An error after the first write is marked once.
pub fn apply_light_environment(vm: &mut Vm, component: ObjRef, plan: &LightPlan) -> Result<(), VmError> {
    match plan {
        LightPlan::Nothing => Ok(()),
        LightPlan::Detached(v) => write(vm, component, "LightEnvironment", v.clone(), LIGHT),
        LightPlan::Attached { previous, new } => {
            write(vm, component, "PreviousLightEnvironment", previous.clone(), LIGHT)?;
            let rest = (|| {
                write(vm, component, "LightEnvironment", new.clone(), LIGHT)?;
                write(vm, component, "bNeedsReattach", Value::Bool(true), LIGHT)
            })();
            rest.map_err(after_writes)
        }
    }
}

/// `SetLightEnvironment(new)` on `component` in Off: its planner, then its writes.
pub fn set_light_environment(vm: &mut Vm, component: ObjRef, new: Option<ObjRef>) -> Result<(), VmError> {
    let plan = plan_light_environment(vm, Slots::of(component), new)?;
    apply_light_environment(vm, component, &plan)
}
