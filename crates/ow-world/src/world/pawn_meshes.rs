//! Owner: lane IMPL-INIT-PAWN-MESHES (decision 2026-11-19-134, number pending). `WillowPawn.InitializePawnMeshes()`
//! and `WillowPlayerPawn.InitializePawnMeshes()`, slice link 1's stop in `maya:8` (`ApplyBodyClassStartingValues`).
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_INIT_PAWN_MESHES.md` alone (analyst lane
//! ANALYST-INIT-PAWN-MESHES, PR #517, at its review-r1 head 3217b58b); this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.** Off only: `World::new` binds both keys outside Reference, first-wins, and
//! Reference keeps the VM's stub policy.
//!
//! **One rule under both keys** (the note's section 1: the player pawn's override only forwards). The note's
//! section 6 preflight, in order, writing nothing and loading nothing (class lookups read the class table alone):
//! 1. **The receiver:** no receiver, or a class that is not `WillowGame.WillowPawn` or a subclass, is a
//!    `VmError::Script` ([`UNIT`], "called with no pawn" / "<class path> is not a WillowPawn"). A receiver no longer in
//!    the heap is the heap's own error.
//! 2. **The helper class** `Engine.InstanceDataHelper` not in the class table: `NotYet { NotRead }`, [`NO_HELPER`] (a
//!    port state; the game always has the class).
//! 3. **The body class:** the class must declare `BodyClass` ([`no_body_class`]), holding an object reference
//!    ([`body_class_kind`]). None: nothing. An unloaded or failed definition handle: `NotYet { NotRead }`,
//!    [`lazy_body_class`], and nothing is loaded (hub ruling, review r1 of #517: a load is a store write).
//! 4. **The attachments:** `BodyComposition` must be a struct holding an `Attachments` array ([`no_attachments`]).
//!    Empty: nothing, which also covers the root-motion writes inside the gate.
//! 5. **A non-empty list:** since lane IMPL-BODY-COMPOSITION, the build itself
//!    (`crate::world::body_composition`, NATIVE_INITIALIZE_BODY_COMPOSITION.md) on the pawn's `InstanceDataState` and
//!    `ReplicatedInstanceDataState`, mode 0, with one more write-free check after the build's preflight and before its
//!    first write (review r1 of #542): a body class that sets `UseRootMotion` or `UseRootRotation` is
//!    `NotYet { NotRead }`, [`ROOT_MOTION`] (the two mesh fields the native then writes are named by fit only,
//!    NATIVE_INIT_PAWN_MESHES.md open question 1). The note's `LayerAbsent` ([`BUILD`], [`LAYER`])
//!    is kept for reference but no longer answered.
//!
//! **Writes:** only the build's, in its order, under its write tracker; steps 1 to 4 and the root-motion check write
//! and load nothing, so a refusal there carries no "stopped after partial writes" prefix.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use ow_object::heap::Origin;

use crate::pools::object_path;

/// The two native keys, as the installed declarations carry them.
pub const KEYS: [&str; 2] = ["WillowPawn.InitializePawnMeshes()", "WillowPlayerPawn.InitializePawnMeshes()"];
/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_INIT_PAWN_MESHES.md";
/// The receiver's class.
pub const WILLOW_PAWN: &str = "WillowGame.WillowPawn";
/// The helper class the native checks first.
pub const HELPER: &str = "Engine.InstanceDataHelper";
/// The function named in the `VmError::Script` errors.
pub const UNIT: &str = "WillowPawn.InitializePawnMeshes";
/// Step 2's `NotYet { NotRead }`.
pub const NO_HELPER: &str = "WillowPawn.InitializePawnMeshes: the Engine.InstanceDataHelper class is not loaded";
/// Step 5's `NotYet { LayerAbsent }` text.
pub const BUILD: &str = "WillowPawn.InitializePawnMeshes: the body composition build (InstanceDataHelper.InitializeBodyComposition: \
                         component instancing and attachment to the pawn's mesh)";
/// A body class that asks for root motion (the module documentation, step 5).
pub const ROOT_MOTION: &str =
    "WillowPawn.InitializePawnMeshes: the root-motion fields of the pawn's Mesh (named by fit only)";
/// Its layer.
pub const LAYER: &str = "a component and mesh layer (instancing components from templates, socket attachment)";

/// Step 1's message for a receiver of another class.
pub fn not_a_pawn(class_path: &str) -> String {
    format!("{class_path} is not a WillowPawn")
}

/// Step 3's message for a class with no `BodyClass`.
pub fn no_body_class(class_path: &str) -> String {
    format!("{class_path} declares no BodyClass")
}

/// Step 3's message for a `BodyClass` that holds no object reference.
pub fn body_class_kind(kind: &str) -> String {
    format!("BodyClass holds {kind}, not an object")
}

/// Step 3's `NotYet { NotRead }` text for an unloaded body class.
pub fn lazy_body_class(object_path: &str) -> String {
    format!("WillowPawn.InitializePawnMeshes: the attachment list of the unloaded body class {object_path}")
}

/// Step 4's message.
pub fn no_attachments(class_path: &str) -> String {
    format!("{class_path} has no BodyComposition.Attachments array")
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn class_path_of(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string()))
}

/// The full object path of a definition handle, as the definition store names it (`ow_defs::identity::full_path` on
/// its package's export table, so a package cooked into another file is not prefixed with that file's stem), without
/// loading anything (an unloaded handle has no outer in the heap yet). Any other object: its heap outer chain.
pub fn handle_path(vm: &Vm, o: ObjRef) -> String {
    if let Ok(x) = vm.space.heap.get(o)
        && let Origin::Export { package, index } | Origin::Resource { package, index } = x.origin
        && index > 0
    {
        // The game's name for the export (review r2 of #522): a package cooked into another file (`Startup.upk`
        // holding `GD_*`) is named by its path in the file, any other export after the file's stem.
        if let Ok(path) = ow_defs::identity::full_path(vm.defs.packages().get(package), index) {
            return path;
        }
    }
    object_path(&vm.space, o)
}

/// Binds both keys (Off only; the caller checks the mode). A key another pack bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for key in KEYS {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(initialize_pawn_meshes))?;
        }
    }
    Ok(())
}

/// The rule (module documentation, steps 1 to 5). Writes nothing and loads nothing.
pub fn initialize_pawn_meshes(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| script("called with no pawn".to_string()))?;
    let vm: &Vm = c.vm;
    let class = vm.space.heap.get(me)?.class;
    let is_pawn = vm.space.classes.by_path(WILLOW_PAWN).is_some_and(|p| vm.space.classes.is_child_of(class, p));
    if !is_pawn {
        return Err(script(not_a_pawn(&class_path_of(vm, me)?)));
    }
    if vm.space.classes.by_path(HELPER).is_none() {
        return Err(NotYet::not_read(NO_HELPER, RECORD).into());
    }
    let Some(prop) = vm.space.prop(class, "BodyClass") else {
        return Err(script(no_body_class(&class_path_of(vm, me)?)));
    };
    let body = match vm.space.read(me, PropAddr::scalar(prop))? {
        Value::Object(o) => *o,
        other => return Err(script(body_class_kind(other.kind_name()))),
    };
    let Some(body) = body else {
        return Ok(Value::None);
    };
    if vm.defs.is_unloaded(&vm.space, body) || vm.defs.is_failed(&vm.space, body) {
        return Err(NotYet::not_read(lazy_body_class(&handle_path(vm, body)), RECORD).into());
    }
    let body_class = vm.space.heap.get(body)?.class;
    let bad = || -> Result<VmError, VmError> { Ok(script(no_attachments(&class_path_of(vm, body)?))) };
    let Some(comp) = vm.space.prop(body_class, "BodyComposition") else {
        return Err(bad()?);
    };
    let composition = vm.space.read(body, PropAddr::scalar(comp))?;
    let Some(s) = composition.as_struct() else {
        return Err(bad()?);
    };
    let field = vm
        .space
        .names
        .find("Attachments")
        .and_then(|n| vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(n)));
    let Some((index, decl)) = field else {
        return Err(bad()?);
    };
    if !matches!(decl.kind, PropKind::Array(_)) {
        return Err(bad()?);
    }
    let Some(list) = usize::try_from(index).ok().and_then(|i| s.fields.get(i)).and_then(Value::elements) else {
        return Err(bad()?);
    };
    if list.is_empty() {
        return Ok(Value::None);
    }
    let read = |name: &str| -> Result<Value, VmError> {
        let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path_of(vm, me).unwrap_or_default())))?;
        Ok(vm.space.read(me, PropAddr::scalar(p))?.clone())
    };
    let inputs = crate::world::body_composition::Inputs {
        owner: Some(me),
        composition: composition.clone(),
        set: read(crate::world::body_composition::SET)?,
        replicated: read(crate::world::body_composition::REPLICATED)?,
        mode: 0,
    };
    // The root-motion check runs after the build's preflight succeeds, still before any write (review r1 of #542,
    // item 7).
    crate::world::body_composition::build_then(c.vm, inputs, |vm| {
        for flag in ["UseRootMotion", "UseRootRotation"] {
            if let Some(p) = vm.space.prop(body_class, flag)
                && matches!(vm.space.read(body, PropAddr::scalar(p))?, Value::Bool(true))
            {
                return Err(NotYet::not_read(ROOT_MOTION, RECORD).into());
            }
        }
        Ok(())
    })?;
    Ok(Value::None)
}

#[cfg(test)]
#[path = "pawn_meshes_tests.rs"]
mod tests;
