//! Owner: lane IMPL-INIT-PAWN-MESHES (decision 2026-11-19-NNN, number pending). `WillowPawn.InitializePawnMeshes()`
//! and `WillowPlayerPawn.InitializePawnMeshes()`, slice link 1's stop in `maya:8` (`ApplyBodyClassStartingValues`).
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_INIT_PAWN_MESHES.md` alone (analyst lane
//! ANALYST-INIT-PAWN-MESHES, PR #517); this lane did not read the executable. **Every rule is UNVERIFIED in the
//! running game.** Off only: `World::new` binds both keys outside Reference, first-wins, and Reference keeps the VM's
//! stub policy.
//!
//! **One rule under both keys** (the note's section 1: the player pawn's override only forwards). In order, writing
//! nothing at any step:
//! 1. **The receiver** must be a `WillowGame.WillowPawn`, or a subclass; otherwise `VmError::Script`.
//! 2. **The declarations** the gate reads: `BodyClass` on the receiver's class, an object reference; otherwise
//!    `VmError::Script` (the [`no_field`] / [`wrong_field`] texts).
//! 3. **No helper:** no `Engine.InstanceDataHelper` class in the class table: nothing (the native's own step 1).
//! 4. **`BodyClass` None:** nothing.
//! 5. **`BodyClass` unloaded** (a lazy definition handle): `NotYet { NotRead }`, [`LAZY_BODY_CLASS`]. The hub's ruling
//!    on #517 r1: a load is a store write, so the native loads nothing. A handle whose load failed earlier
//!    (`DefStore::is_failed`) has no list to read either, and is named the same way.
//! 6. **The attachment list:** `BodyComposition` (a struct) on the body class's class, holding `Attachments` (an
//!    array). Empty: nothing. That covers the root-motion writes, which sit inside the gate.
//! 7. **A non-empty list:** the remaining fields the note names are checked (the receiver's `InstanceDataState`,
//!    `ReplicatedInstanceDataState` and `Mesh`; the body class's `UseRootMotion` and `UseRootRotation`), then
//!    `NotYet { LayerAbsent }` with the note's text ([`BUILD`], [`LAYER`]).
//!
//! **Nothing is written before the `LayerAbsent`** (hub ruling): no switch entry, no network flag, no root-motion
//! write (they come after the composition build in the note's order), so the native's refusal carries no
//! "stopped after partial writes" prefix of its own. A caller that wrote before adds it.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The two native keys, as the installed declarations carry them.
pub const KEYS: [&str; 2] = ["WillowPawn.InitializePawnMeshes()", "WillowPlayerPawn.InitializePawnMeshes()"];
/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_INIT_PAWN_MESHES.md";
/// The receiver's class.
pub const WILLOW_PAWN: &str = "WillowGame.WillowPawn";
/// The helper class whose presence the native checks first.
pub const HELPER: &str = "Engine.InstanceDataHelper";
/// The `NotYet { LayerAbsent }` of a non-empty attachment list (the note's section 6, step 4).
pub const BUILD: &str = "WillowPawn.InitializePawnMeshes: the body composition build (InstanceDataHelper.InitializeBodyComposition: \
                         component instancing and attachment to the pawn's mesh)";
/// Its layer.
pub const LAYER: &str = "a component and mesh layer (instancing components from templates, socket attachment)";
/// The unit named in the `VmError::Script` texts.
pub const UNIT: &str = "WillowPawn.InitializePawnMeshes";

/// The fields of a non-empty list's preflight: (owner, field, expected kind).
const RECEIVER_FIELDS: [(&str, Kind); 3] =
    [("InstanceDataState", Kind::Struct), ("ReplicatedInstanceDataState", Kind::Struct), ("Mesh", Kind::Object)];
const BODY_CLASS_FIELDS: [(&str, Kind); 2] = [("UseRootMotion", Kind::Bool), ("UseRootRotation", Kind::Bool)];

/// The kinds the preflight checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Object,
    Struct,
    Array,
    Bool,
}

impl Kind {
    fn matches(self, k: &PropKind) -> bool {
        matches!(
            (self, k),
            (Kind::Object, PropKind::Object { .. })
                | (Kind::Struct, PropKind::Struct(_))
                | (Kind::Array, PropKind::Array(_))
                | (Kind::Bool, PropKind::Bool)
        )
    }

    fn words(self) -> &'static str {
        match self {
            Kind::Object => "an object reference",
            Kind::Struct => "a struct",
            Kind::Array => "a dynamic array",
            Kind::Bool => "a bool",
        }
    }
}

/// `<unit>: <class path> has no <field>`.
pub fn no_field(class_path: &str, field: &str) -> String {
    format!("{UNIT}: {class_path} has no {field}")
}

/// `<unit>: <class path>.<field> is not <kind>`.
pub fn wrong_field(class_path: &str, field: &str, kind: &str) -> String {
    format!("{UNIT}: {class_path}.{field} is not {kind}")
}

/// The lazy body class's `NotYet { NotRead }` text.
pub fn lazy_body_class(path: &str) -> String {
    format!(
        "{UNIT}: the attachment list of the body class {path}, an unloaded definition handle (loading it would be a \
         store write before the refusal; hub ruling, review r1 of #517)"
    )
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

/// The declared property `name` of `class`, checked against `kind`; its id.
fn declared(vm: &Vm, class: ClassId, name: &str, kind: Kind) -> Result<ow_object::ids::PropId, VmError> {
    let Some(p) = vm.space.prop(class, name) else {
        return Err(script(no_field(&class_path(vm, class), name)));
    };
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(k) if kind.matches(k) => Ok(p),
        _ => Err(script(wrong_field(&class_path(vm, class), name, kind.words()))),
    }
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

/// The rule (module documentation, steps 1 to 7). Writes nothing.
pub fn initialize_pawn_meshes(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| script("called with no pawn".to_string()))?;
    let vm: &mut Vm = c.vm;
    let class = vm.space.heap.get(me)?.class;
    let pawn = vm.find_class(WILLOW_PAWN)?;
    if !vm.space.classes.is_child_of(class, pawn) {
        return Err(script(format!("{} is not a {WILLOW_PAWN}", class_path(vm, class))));
    }
    let body_prop = declared(vm, class, "BodyClass", Kind::Object)?;
    if vm.find_class_if_present(HELPER)?.is_none() {
        return Ok(Value::None);
    }
    let Value::Object(body) = vm.space.read(me, PropAddr::scalar(body_prop))?.clone() else {
        return Err(script(wrong_field(&class_path(vm, class), "BodyClass", Kind::Object.words())));
    };
    let Some(body) = body else {
        return Ok(Value::None);
    };
    if vm.defs.is_unloaded(&vm.space, body) || vm.defs.is_failed(&vm.space, body) {
        let path = vm.space.heap.get(body)?.name;
        let path = format!("{} ({})", vm.space.names.text(path), class_path(vm, vm.space.heap.get(body)?.class));
        return Err(NotYet::not_read(lazy_body_class(&path), RECORD).into());
    }
    let body_class = vm.space.heap.get(body)?.class;
    let comp_prop = declared(vm, body_class, "BodyComposition", Kind::Struct)?;
    let composition = vm.space.read(body, PropAddr::scalar(comp_prop))?.clone();
    let Some(s) = composition.as_struct() else {
        return Err(script(wrong_field(&class_path(vm, body_class), "BodyComposition", Kind::Struct.words())));
    };
    let layout = vm
        .space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| script(wrong_field(&class_path(vm, body_class), "BodyComposition", Kind::Struct.words())))?;
    let attachments = vm.space.names.find("Attachments").and_then(|n| layout.field(n));
    let Some((index, decl)) = attachments else {
        return Err(script(no_field(&format!("{}.BodyComposition", class_path(vm, body_class)), "Attachments")));
    };
    if !Kind::Array.matches(&decl.kind) {
        return Err(script(wrong_field(
            &format!("{}.BodyComposition", class_path(vm, body_class)),
            "Attachments",
            Kind::Array.words(),
        )));
    }
    let list = usize::try_from(index).ok().and_then(|i| s.fields.get(i)).and_then(Value::elements);
    let Some(list) = list else {
        return Err(script(wrong_field(
            &format!("{}.BodyComposition", class_path(vm, body_class)),
            "Attachments",
            Kind::Array.words(),
        )));
    };
    if list.is_empty() {
        return Ok(Value::None);
    }
    for (name, kind) in RECEIVER_FIELDS {
        declared(vm, class, name, kind)?;
    }
    for (name, kind) in BODY_CLASS_FIELDS {
        declared(vm, body_class, name, kind)?;
    }
    Err(NotYet::layer_absent(BUILD, LAYER, RECORD).into())
}

#[cfg(test)]
#[path = "pawn_meshes_tests.rs"]
mod tests;
