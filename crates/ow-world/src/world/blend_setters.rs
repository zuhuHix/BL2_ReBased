//! Owner: lane IMPL-LINK1-TAIL-SETTERS. `AnimNode.FindAllAnimNodes`, `AnimNodeBlend.SetBlendTarget` and
//! `AnimNodeBlendList.SetActiveChild` in `ParityMode::Off`: `maya:8`'s `SetUpperBodyMovementBlendState` past the
//! aim-offset profile switch. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK1_TAIL_SETTERS.md sections 2.A to 2.C alone (lane
//! ANALYST-LINK1-TAIL, #734 with its review r1, on main); this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference keeps the stub policy.
//!
//! Every preflight is write-free and load-free (no load, no class-code load, nothing interned); a class test is by
//! name on loaded classes (`classes.by_path`).
//!
//! **`FindAllAnimNodes(out Nodes, NodeName)`** (2.A): the list is the **live** component's. With the receiver's
//! `SkelComponent` set and resident, its `AnimTickArray` filled and its `Animations` the receiver: that array, in
//! order, every node whose `NodeName` is the name appended to `Nodes` (not cleared first; no match leaves it as it
//! is). Otherwise (no component, an empty tick list, another tree) the gather step is not read: `NotYet { NotRead }`
//! with `anim_node_lookup::NO_TICK_LIST`'s text. A component (or a tick-list node) the store has not loaded:
//! `NotYet { NotRead }`. The out parameter is declared an array of object references, and `SkelComponent`,
//! `AnimTickArray` and `Animations` are declared with their kinds, else `VmError::Script`.
//!
//! **`SetBlendTarget(BlendTarget, BlendTime)`** (2.B): the target clamped to 0..1; `Child2WeightTarget` := it. The
//! blend is immediate when `BlendTime` is not above 0: `Child2Weight` := target, `Children[0].Weight` := 1 - target,
//! `Children[1].Weight` := target, `BlendTimeToGo` := 0. Otherwise `BlendTimeToGo` := `BlendTime`, except that with
//! `bSkipBlendWhenNotRendered` set the node's `SkelComponent`'s `bRecentlyRendered` is read: clear needs the
//! unidentified engine-wide flag ([`NOT_RENDERED_FLAG`], `NotRead`); a None or unloaded `SkelComponent` there is
//! `NotRead` too. `WillowAnimNodeFeatherBlend` then writes `Children[0].Weight` := 1. The body is chosen by the exact
//! class: `Engine.AnimNodeBlend` or `WillowGame.WillowAnimNodeFeatherBlend`; any other class's body is not read
//! ([`unread_body`], `NotRead`). The immediate path needs two children, else `VmError::Script`. All checks before the
//! first write.
//!
//! **`SetActiveChild`** (2.C): its body is not read: [`SET_ACTIVE_CHILD`] (`NotRead`).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::anim_node_lookup::NO_TICK_LIST;
use super::component_setters::{Want, bad, bool_of, object_of, schemas, write};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINK1_TAIL_SETTERS.md";
/// The native keys.
pub const FIND_ALL_ANIM_NODES: &str = "AnimNode.FindAllAnimNodes(array,name)";
pub const SET_BLEND_TARGET: &str = "AnimNodeBlend.SetBlendTarget(float,float)";
/// `WillowAnimNodeFeatherBlend`'s own declaration of the native, which shares `AnimNodeBlend`'s exec (2.B): the same
/// rule (lane IMPL-AIMOFFSET-REBUILD; the installed run showed #739 had bound only the base class's key).
pub const FEATHER_SET_BLEND_TARGET: &str = "WillowAnimNodeFeatherBlend.SetBlendTarget(float,float)";
pub const SET_ACTIVE_CHILD_KEY: &str = "AnimNodeBlendList.SetActiveChild(int,float)";
/// 2.A's component or node not loaded.
pub const FIND_NOT_LOADED: &str = "AnimNode.FindAllAnimNodes: an object the store has not loaded (a load is a write)";
/// 2.B's unidentified flag.
pub const NOT_RENDERED_FLAG: &str = "AnimNodeBlend.SetBlendTarget: the engine-wide flag of the not-rendered test";
/// 2.B's not-rendered test through a None or unloaded component.
pub const NOT_RENDERED_COMPONENT: &str =
    "AnimNodeBlend.SetBlendTarget: the not-rendered test reads through a SkelComponent that is None or not loaded";
/// 2.C.
pub const SET_ACTIVE_CHILD: &str = "AnimNodeBlendList.SetActiveChild (its body is not read)";
const FIND: &str = "AnimNode.FindAllAnimNodes";
const BLEND: &str = "AnimNodeBlend.SetBlendTarget";
const BLEND_CLASS: &str = "Engine.AnimNodeBlend";
const FEATHER_CLASS: &str = "WillowGame.WillowAnimNodeFeatherBlend";

/// 2.B's refusal for a class whose body is not read.
pub fn unread_body(class: &str) -> String {
    format!("AnimNodeBlend.SetBlendTarget on a {class} (its body is not read)")
}

/// Whether `o` is resident: neither a handle the store has not loaded (or failed) nor a slotless stand-in.
fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(FIND, format!("no {name}")))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

/// 2.A's list: the matches to append, read only.
fn find_plan(c: &NativeCall<'_>, node: ObjRef) -> Result<Vec<Value>, VmError> {
    let vm: &Vm = c.vm;
    let out_ok = vm
        .function(c.function)
        .and_then(|d| d.params.first())
        .is_some_and(|p| matches!(&p.kind, PropKind::Array(inner) if matches!(**inner, PropKind::Object { .. })));
    if !out_ok {
        return Err(bad(FIND, "Nodes is not declared as an array of object references"));
    }
    let name = match c.arg(1)? {
        Value::Name(n) => *n,
        other => return Err(bad(FIND, format!("NodeName holds {}, not a name", other.kind_name()))),
    };
    schemas(vm, node, &[("SkelComponent", Want::Object)], FIND)?;
    let Some(component) = object_of(&read(vm, node, "SkelComponent")?, FIND)? else {
        return Err(NotYet::not_read(NO_TICK_LIST, RECORD).into());
    };
    if !resident(vm, component)? {
        return Err(NotYet::not_read(FIND_NOT_LOADED, RECORD).into());
    }
    schemas(vm, component, &[("AnimTickArray", Want::Array), ("Animations", Want::Object)], FIND)?;
    let tick = read(vm, component, "AnimTickArray")?;
    let items = tick.elements().unwrap_or(&[]);
    let tree = object_of(&read(vm, component, "Animations")?, FIND)?;
    if items.is_empty() || tree != Some(node) {
        return Err(NotYet::not_read(NO_TICK_LIST, RECORD).into());
    }
    let mut out = Vec::new();
    for item in items {
        let Some(n) = object_of(item, FIND)? else { continue };
        if !resident(vm, n)? {
            return Err(NotYet::not_read(FIND_NOT_LOADED, RECORD).into());
        }
        if !matches!(super::component_setters::declared(vm, n, "NodeName", FIND)?, PropKind::Name) {
            return Err(bad(FIND, "NodeName is not declared as a name"));
        }
        match read(vm, n, "NodeName")? {
            Value::Name(x) if x == name => out.push(Value::Object(Some(n))),
            Value::Name(_) => {}
            other => return Err(bad(FIND, format!("NodeName holds {}, not a name", other.kind_name()))),
        }
    }
    Ok(out)
}

/// `AnimNode.FindAllAnimNodes(out array<AnimNode> Nodes, name NodeName)` (module documentation).
pub fn find_all_anim_nodes(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let node = c.this.ok_or_else(|| bad(FIND, "called without a node"))?;
    let matches = find_plan(c, node)?;
    if !matches.is_empty() {
        // Review of #739, item 3: an out argument holding no array is a data error, not an empty list.
        let current = c.arg(0)?;
        let mut nodes = current
            .elements()
            .map(<[Value]>::to_vec)
            .ok_or_else(|| bad(FIND, format!("Nodes holds {}, not an array", current.kind_name())))?;
        nodes.extend(matches);
        c.set_out(0, Value::array(nodes))?;
    }
    Ok(Value::None)
}

/// A float argument. A NaN or an infinity is refused (review of #739, item 2): **the port's choice**, UNVERIFIED (the
/// game's behaviour on a non-finite target or time is not read).
fn float_arg(c: &NativeCall<'_>, i: usize, name: &str) -> Result<f32, VmError> {
    match c.arg(i)? {
        Value::Float(f) if !f.is_finite() => {
            Err(bad(BLEND, format!("{name} is {f}, not finite (the port refuses it; the game's answer is not read)")))
        }
        Value::Float(f) => Ok(*f),
        other => Err(bad(BLEND, format!("{name} holds {}, not a float", other.kind_name()))),
    }
}

/// `Children` with element `i`'s `Weight` set to `w`.
fn with_weight(vm: &Vm, children: &[Value], i: usize, w: f32) -> Result<Vec<Value>, VmError> {
    let mut out = children.to_vec();
    let Some(Value::Struct(s)) = out.get(i) else {
        return Err(bad(BLEND, format!("Children[{i}] is not a struct")));
    };
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(BLEND, "a child struct with no layout"))?;
    let (at, decl) = vm
        .space
        .names
        .find("Weight")
        .and_then(|n| layout.field(n))
        .ok_or_else(|| bad(BLEND, "a child struct with no Weight"))?;
    if !matches!(decl.kind, PropKind::Float) {
        return Err(bad(BLEND, "a child's Weight is not a float"));
    }
    let mut fields = s.fields.clone();
    fields[at as usize] = Value::Float(w);
    out[i] = Value::Struct(Rc::new(StructValue { ty: s.ty, fields }));
    Ok(out)
}

/// `Children`'s declaration (an array of a struct with a float `Weight`) and every element (that struct, its `Weight` a
/// float), on every path and before any write (review of #739, item 1). Else `VmError::Script`.
fn check_children(vm: &Vm, node: ObjRef, children: &[Value]) -> Result<(), VmError> {
    let ty = match super::component_setters::declared(vm, node, "Children", BLEND)? {
        PropKind::Array(inner) => match *inner {
            PropKind::Struct(ty) => ty,
            other => return Err(bad(BLEND, format!("Children is declared as an array of {other:?}, not of a struct"))),
        },
        _ => return Err(bad(BLEND, "Children is not declared as an array")),
    };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(BLEND, "the child struct has no layout"))?;
    let (at, decl) = vm
        .space
        .names
        .find("Weight")
        .and_then(|n| layout.field(n))
        .ok_or_else(|| bad(BLEND, "the child struct has no Weight"))?;
    if !matches!(decl.kind, PropKind::Float) {
        return Err(bad(BLEND, "the child struct's Weight is not a float"));
    }
    for (i, child) in children.iter().enumerate() {
        match child {
            Value::Struct(s) if s.ty == ty && matches!(s.fields.get(at as usize), Some(Value::Float(_))) => {}
            other => return Err(bad(BLEND, format!("Children[{i}] holds {}, not a child struct with a float Weight", other.kind_name()))),
        }
    }
    Ok(())
}

/// 2.B's plan: the writes, in order, decided before any of them.
fn blend_plan(vm: &Vm, node: ObjRef, target: f32, time: f32) -> Result<Vec<(&'static str, Value)>, VmError> {
    let class = vm.space.heap.get(node)?.class;
    let path = vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
    let feather = vm.space.classes.by_path(FEATHER_CLASS) == Some(class);
    if !feather && vm.space.classes.by_path(BLEND_CLASS) != Some(class) {
        return Err(NotYet::not_read(unread_body(&path), RECORD).into());
    }
    schemas(
        vm,
        node,
        &[
            ("Child2Weight", Want::Float),
            ("Child2WeightTarget", Want::Float),
            ("BlendTimeToGo", Want::Float),
            ("bSkipBlendWhenNotRendered", Want::Bool),
            ("Children", Want::Array),
        ],
        BLEND,
    )?;
    let children = read(vm, node, "Children")?.elements().map(<[Value]>::to_vec).unwrap_or_default();
    check_children(vm, node, &children)?;
    let t = if target < 0.0 { 0.0 } else if target > 1.0 { 1.0 } else { target };
    let mut immediate = time <= 0.0;
    if !immediate && bool_of(&read(vm, node, "bSkipBlendWhenNotRendered")?, "bSkipBlendWhenNotRendered", BLEND)? {
        schemas(vm, node, &[("SkelComponent", Want::Object)], BLEND)?;
        let component = match object_of(&read(vm, node, "SkelComponent")?, BLEND)? {
            Some(c) if resident(vm, c)? => c,
            _ => return Err(NotYet::not_read(NOT_RENDERED_COMPONENT, RECORD).into()),
        };
        schemas(vm, component, &[("bRecentlyRendered", Want::Bool)], BLEND)?;
        if !bool_of(&read(vm, component, "bRecentlyRendered")?, "bRecentlyRendered", BLEND)? {
            // Clear: the blend is immediate only when the unidentified engine-wide flag is clear too.
            return Err(NotYet::not_read(NOT_RENDERED_FLAG, RECORD).into());
        }
        immediate = false;
    }
    let mut writes = vec![("Child2WeightTarget", Value::Float(t))];
    let mut current = children;
    if immediate {
        if current.len() < 2 {
            return Err(bad(BLEND, format!("the immediate blend needs two children, the node has {}", current.len())));
        }
        current = with_weight(vm, &current, 0, 1.0 - t)?;
        current = with_weight(vm, &current, 1, t)?;
        writes.push(("Child2Weight", Value::Float(t)));
        writes.push(("Children", Value::array(current.clone())));
        writes.push(("BlendTimeToGo", Value::Float(0.0)));
    } else {
        writes.push(("BlendTimeToGo", Value::Float(time)));
    }
    if feather {
        if current.is_empty() {
            return Err(bad(BLEND, "the feather blend's step needs a child"));
        }
        writes.push(("Children", Value::array(with_weight(vm, &current, 0, 1.0)?)));
    }
    for (name, v) in &writes {
        super::component_setters::fits(vm, node, name, v, BLEND)?;
    }
    Ok(writes)
}

/// `AnimNodeBlend.SetBlendTarget(float BlendTarget, float BlendTime)` (module documentation).
pub fn set_blend_target(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let node = c.this.ok_or_else(|| bad(BLEND, "called without a node"))?;
    let target = float_arg(c, 0, "BlendTarget")?;
    let time = float_arg(c, 1, "BlendTime")?;
    let writes = blend_plan(c.vm, node, target, time)?;
    for (i, (name, v)) in writes.into_iter().enumerate() {
        let done = write(c.vm, node, name, v, BLEND);
        if i > 0 {
            done.map_err(super::component_setters::after_writes)?;
        } else {
            done?;
        }
    }
    Ok(Value::None)
}

/// `AnimNodeBlendList.SetActiveChild` (module documentation).
pub fn set_active_child(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Err(NotYet::not_read(SET_ACTIVE_CHILD, RECORD).into())
}

/// Binds the three keys (Off only; the caller checks the mode). A key bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for (key, f) in [
        (FIND_ALL_ANIM_NODES, find_all_anim_nodes as fn(&mut NativeCall<'_>) -> Result<Value, VmError>),
        (SET_BLEND_TARGET, set_blend_target),
        (FEATHER_SET_BLEND_TARGET, set_blend_target),
        (SET_ACTIVE_CHILD_KEY, set_active_child),
    ] {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "blend_setters_tests.rs"]
mod tests;
