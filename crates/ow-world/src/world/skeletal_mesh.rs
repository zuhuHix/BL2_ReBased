//! Owner: lane IMPL-SET-SKELETAL-MESH. `SkeletalMeshComponent.SetSkeletalMesh(NewMesh, optional bKeepSpaceBases)` in
//! `ParityMode::Off`, coded from docs/verification/NATIVE_SET_SKELETAL_MESH.md alone (#583 at 231bbaed; clean room:
//! lane ANALYST-SET-SKELETAL-MESH read the native, this lane did not). AI-assisted (Claude Code). **Every rule here is
//! UNVERIFIED in the running game.** Reference binds nothing, so the VM's stub policy answers there.
//!
//! **The answer** (note section 4), in this order. Every path writes nothing, loads nothing and draws nothing:
//! 1. **the receiver:** `this` must be set and resident; an unloaded definition handle is `NotYet { NotRead }`;
//! 2. **the current mesh:** `SkeletalMesh`, read straight from the component's slot. A class with no `SkeletalMesh`
//!    object field is `VmError::Script`;
//! 3. **the arguments:** `NewMesh` is an object (`Object(None)` is null), and `bKeepSpaceBases`, when supplied
//!    (`NativeCall::has`), a bool; an omitted one is false. Anything else is `VmError::Script`;
//! 4. **identity first,** before either mesh's residency is looked at: equal handles (both None, or the same handle,
//!    loaded or not) are the native's own "nothing to do"; unequal ones (a replacement or a clear) are
//!    `NotYet { LayerAbsent }`, the component's bone transforms, raised before any write. The port has no layer for
//!    the bones, the tree's per-bone state, physics or the render state the change touches (note section 3).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The native's registry key.
pub const SET_SKELETAL_MESH: &str = "SkeletalMeshComponent.SetSkeletalMesh(object,bool)";
/// The record of the rule.
pub const RECORD: &str = "NATIVE_SET_SKELETAL_MESH.md";
/// The layer a mesh change needs (note section 3).
pub const BONES_LAYER: &str = "a skeletal mesh component's bone transforms";
const UNIT: &str = "SetSkeletalMesh";

/// Binds the native (module documentation); the caller binds it in Off only.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(SET_SKELETAL_MESH, Rc::new(set_skeletal_mesh))
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn set_skeletal_mesh(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| script("called with no component".to_string()))?;
    // Rule 1 before the argument shapes (the note's section 4 order; review r1 of #599, item 4).
    resident(c.vm, component)?;
    let new_mesh = match c.arg(0)? {
        Value::Object(o) => *o,
        other => return Err(script(format!("NewMesh holds {}, not an object reference", other.kind_name()))),
    };
    let keep_space_bases = if c.has(1) {
        match c.arg(1)? {
            Value::Bool(b) => *b,
            other => return Err(script(format!("bKeepSpaceBases holds {}, not a bool", other.kind_name()))),
        }
    } else {
        false
    };
    // The flag only matters to a mesh change (note section 2), which is refused; its shape is still checked.
    let _keep_space_bases = keep_space_bases;
    answer(c.vm, component, new_mesh)?;
    Ok(Value::None)
}

/// Rule 1: the receiver is resident. An unloaded handle is `NotRead`; one whose load already failed says so
/// (review r1 of #599, item 6).
fn resident(vm: &Vm, component: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_failed(&vm.space, component) {
        return Err(NotYet::not_read(format!("{UNIT}: the component is a handle whose load failed"), RECORD).into());
    }
    if vm.defs.is_unloaded(&vm.space, component) {
        return Err(NotYet::not_read(
            format!("{UNIT}: the component is an unloaded handle (a load is a write)"),
            RECORD,
        )
        .into());
    }
    Ok(())
}

/// The rule on its inputs (module documentation): `Ok` when the mesh is already `new_mesh`.
pub fn answer(vm: &Vm, component: ObjRef, new_mesh: Option<ObjRef>) -> Result<(), VmError> {
    resident(vm, component)?;
    let current = current_mesh(vm, component)?;
    if current == new_mesh {
        return Ok(());
    }
    Err(NotYet::layer_absent(
        format!("{UNIT}: a mesh change (bones, the animation tree's per-bone state, physics and render state)"),
        BONES_LAYER,
        RECORD,
    )
    .into())
}

/// The component's `SkeletalMesh`, read from its slot without a load.
fn current_mesh(vm: &Vm, component: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let p = vm.space.prop(class, "SkeletalMesh").ok_or_else(|| script(format!("{} declares no SkeletalMesh", path())))?;
    if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Object { .. })) {
        return Err(script(format!("{}.SkeletalMesh is not an object field", path())));
    }
    match vm.space.read(component, PropAddr::scalar(p))? {
        Value::Object(o) => Ok(*o),
        other => Err(script(format!("{}.SkeletalMesh holds {}, not an object reference", path(), other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "skeletal_mesh_tests.rs"]
mod tests;
