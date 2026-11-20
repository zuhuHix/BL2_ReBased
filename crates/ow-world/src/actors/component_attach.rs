//! Owner: lane IMPL-SPAWN-COMPONENT-ATTACH (decision 2026-11-19-NNN). The spawn's component update in Off: each of a
//! new actor's `Components` is attached to the world's scene and gets the actor as its `Owner`. AI-assisted (Claude
//! Code).
//!
//! Coded from `docs/verification/NATIVE_SPAWN_COMPONENT_ATTACH.md` alone (analyst lane ANALYST-SPAWN-COMPONENT-OWNER,
//! PR #605; clean room: this lane did not read the native) and the hub's final rules on its review r1. **Every rule is
//! UNVERIFIED in the running game.** Off only: Reference keeps the spawn as it was (R3).
//!
//! **The scene layer** ([`SceneLayer`]) moved here from `ow_inventory::attach_mesh`, which re-exports it: it is the
//! world's scene that components attach to, an explicit, empty VM service, shared by `Actor.AttachComponent` and the
//! spawn's step. The standard Off world set-up (`crate::world::World::new`, which the map load, the slice, the combat
//! run and link 9's world all build on) installs it, logged as an UNVERIFIED harness input ([`HARNESS_LINE`]). A bare
//! VM without it refuses the step with `NotYet { LayerAbsent }` ([`SCENE_LAYER`]), and only when the step has a
//! component to attach.
//!
//! **Two halves.**
//! - **The plan** ([`plan`]) is the preflight. The spawn runs it before the actor is made (after the instancing plan
//!   and the actor's own definition loads, before `vm.instantiate`), on the spawn's source, the class default or the
//!   archetype. Its `Components` are mapped through the instancing plan, so a refusal leaves no actor. It writes,
//!   loads and draws nothing, and reads with `vm.space.prop` and `vm.space.read` only. In order:
//!   1. the source's class declares `Components` and `AllComponents` as arrays: else a `VmError::Script`;
//!   2. the source's `AllComponents` is empty (the actor's copy of it starts as the source's): else
//!      `NotYet { NotRead }` ([`PRE_PASS`]);
//!   3. for each non-null entry of `Components`, in order:
//!      - it is one of this spawn's component copies: a template component of the instancing plan, or an inherited
//!        reference the plan stands the template's own sub-object for. Else `NotYet { NotRead }` ([`NOT_A_COPY`]);
//!      - that template component is resident: an unloaded definition handle, or a stand-in with no slots, is
//!        `NotYet { NotRead }` naming it;
//!      - its class declares `Owner`, `Scene`, `bAttached`, `bNeedsReattach`, `bNeedsUpdateTransform` and
//!        `bSkipChildComponentUpdate` with the right kinds: else a `VmError::Script`;
//!      - `bAttached` is false: else `NotYet { NotRead }` ([`ATTACHED`]);
//!      - a subclass of `GearboxFramework.AIComponent` other than the two exact classes read is `NotYet { NotRead }`:
//!        its own attach entry was not read;
//!   4. when at least one entry is to be attached, the scene layer is present: else `NotYet { LayerAbsent }`.
//! - **The writes** ([`apply`]) run in `crate::actors::after_insertion`, between `Rotation` and the owner
//!   (`GainedChild`), before `Instigator` and any script event, before and after play began. They go to this spawn's
//!   copies, in `Components` order:
//!   - every entry gets the generic attach: `bNeedsReattach` and `bNeedsUpdateTransform` false, `Owner` the actor
//!     (`Scene` stays at its zero value, the empty layer), and `bSkipChildComponentUpdate` false (its net value);
//!   - exactly `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent` also get their own attach entry:
//!     `bAttached` true, and the copy appended to the actor's `AllComponents`;
//!   - every other class gets nothing more (registration is not modelled, the port's answer, UNVERIFIED).
//!
//!   Nothing is loaded or drawn. The actor is in the level list by then, so a failure there is marked "stopped after
//!   partial writes" by the spawn's post-insertion boundary.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_SPAWN_COMPONENT_ATTACH.md";

/// The absent layer, for a world without a [`SceneLayer`] (the text `Actor.AttachComponent` names too).
pub const SCENE_LAYER: &str = "a scene that components attach to: render, collision and physics registration (the port has none)";
/// `Actor.AttachComponent`'s refusal when the layer is absent; the spawn's step refuses with the same text (the note).
pub const ATTACH: &str = "Actor.AttachComponent: the component's attach to the world's scene";
/// The step's refusal when the layer is absent: [`ATTACH`].
pub const NO_LAYER: &str = ATTACH;
/// The refusal for a non-empty `AllComponents`.
pub const PRE_PASS: &str = "the spawn's component update: the pre-pass over a non-empty AllComponents is not read";
/// The refusal for an attached component.
pub const ATTACHED: &str =
    "the spawn's component update on an attached component: the reattach and transform refresh";
/// The start of the refusal for an entry that is not this spawn's own copy (the note's text, with the path).
pub const NOT_A_COPY: &str = "the spawn's component update: the component";
/// The end of that refusal.
pub const NOT_A_COPY_WHY: &str = "is not this spawn's own copy (writing Owner onto it would write a shared object)";
/// The refusal for a component listed twice in `Components` (directly, or through an inherited reference).
pub const LISTED_TWICE: &str = "the spawn's component update: a component listed twice in Components";
/// The start of the refusal for an `AIComponent` subclass whose own attach entry was not read.
pub const OTHER_AI_CLASS: &str = "the spawn's component update: the own attach entry of";

/// The classes whose own attach entry was read (the note's section 2): `bAttached` and the `AllComponents` append.
pub const AI_COMPONENT_CLASSES: [&str; 2] = ["GearboxFramework.AIComponent", "WillowGame.WillowAIComponent"];

/// The harness line [`crate::world::World::new`] logs when it installs the empty layer in Off.
pub const HARNESS_LINE: &str = "harness input: an empty scene layer (the port has no scene: components attach with \
                                their script-visible writes only; UNVERIFIED stand-in for the game's scene)";

/// The world's scene that components attach to, as an explicit layer on the world (a VM service). The hub's ruling
/// (NATIVE_ATTACH_MESH_TO_PICKUP.md section 3.4): a world that holds it, empty, gets the attach's script-visible writes
/// and no registration (a port Off answer, UNVERIFIED in game). A world without it is `NotYet { LayerAbsent }`.
///
/// **A lane that adds a real scene feeds this layer**; the attach then registers there. It holds nothing today.
#[derive(Debug, Default)]
pub struct SceneLayer {}

/// Adds an empty [`SceneLayer`] to `vm`'s world unless one is there (world set-up). Answers whether it added one.
pub fn add_empty_scene(vm: &mut Vm) -> Result<bool, VmError> {
    if vm.service::<SceneLayer>().is_some() {
        return Ok(false);
    }
    vm.add_service(SceneLayer::default())?;
    Ok(true)
}

/// One component's writes, decided by [`plan`] on the source's entry and applied to this spawn's copy of it.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentWrites {
    /// The template component of the instancing plan whose copy is written.
    pub original: ObjRef,
    /// `bNeedsReattach`, `bNeedsUpdateTransform`, `Owner`, `bSkipChildComponentUpdate`.
    props: [PropId; 4],
    /// `bAttached`, for the two exact AI component classes only.
    attached: Option<PropId>,
}

/// The step's writes, decided before the actor is made.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub components: Vec<ComponentWrites>,
    all_components: PropId,
}

fn script(message: String) -> VmError {
    VmError::Script { function: "the spawn's component update".to_string(), message }
}

fn class_path(vm: &Vm, o: ObjRef) -> String {
    ow_attr::eval::class_path(&vm.space, o)
}

/// Property `name` of `o`'s class, declared with a kind `ok` accepts: else a `VmError::Script` naming it.
fn declared(vm: &Vm, o: ObjRef, name: &str, want: &str, ok: impl Fn(&PropKind) -> bool) -> Result<PropId, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| script(format!("{} declares no {name}", class_path(vm, o))))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(k) if ok(k) => Ok(p),
        _ => Err(script(format!("{}.{name} is not {want}", class_path(vm, o)))),
    }
}

fn read(vm: &Vm, o: ObjRef, p: PropId) -> Result<&Value, VmError> {
    Ok(vm.space.read(o, PropAddr::scalar(p))?)
}

fn is_array(k: &PropKind) -> bool {
    matches!(k, PropKind::Array(_))
}

fn is_bool(k: &PropKind) -> bool {
    matches!(k, PropKind::Bool)
}

fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

/// The objects of an array value (None entries kept), or a `VmError::Script` naming `name`.
fn objects_of(vm: &Vm, o: ObjRef, name: &str, v: &Value) -> Result<Vec<Option<ObjRef>>, VmError> {
    let items = v
        .elements()
        .ok_or_else(|| script(format!("{}.{name} holds {}, not an array", class_path(vm, o), v.kind_name())))?;
    items
        .iter()
        .map(|x| match x {
            Value::Object(r) => Ok(*r),
            other => Err(script(format!("{}.{name} holds {}, not an object", class_path(vm, o), other.kind_name()))),
        })
        .collect()
}

/// The preflight (module documentation, steps 1 to 4) on the spawn's `source`, over its `Components` mapped through
/// the instancing plan `instancing`. Writes, loads and draws nothing.
pub fn plan(vm: &Vm, source: ObjRef, instancing: &super::instancing::Plan) -> Result<Plan, VmError> {
    let components_prop = declared(vm, source, "Components", "an array", is_array)?;
    let all_components = declared(vm, source, "AllComponents", "an array", is_array)?;
    let components = objects_of(vm, source, "Components", read(vm, source, components_prop)?)?;
    if !objects_of(vm, source, "AllComponents", read(vm, source, all_components)?)?.is_empty() {
        return Err(NotYet::not_read(PRE_PASS, RECORD).into());
    }
    let ai_base = vm.space.classes.by_path(AI_COMPONENT_CLASSES[0]);
    let ai_classes: Vec<_> = AI_COMPONENT_CLASSES.iter().filter_map(|p| vm.space.classes.by_path(p)).collect();
    let mut planned = Vec::new();
    for entry in components.into_iter().flatten() {
        let original = if instancing.originals.contains(&entry) {
            entry
        } else if let Some(&(_, own)) = instancing.inherited.iter().find(|(r, _)| *r == entry) {
            own
        } else {
            let name = vm.space.heap.get(entry).map(|o| vm.space.names.text(o.name).to_string()).unwrap_or_default();
            return Err(NotYet::not_read(
                format!("{NOT_A_COPY} {name} ({}) {NOT_A_COPY_WHY}", class_path(vm, entry)),
                RECORD,
            )
            .into());
        };
        // The same template component reached twice (the same entry, or an inherited reference and the own sub-object
        // it maps to): the hub's ruling on #610 r1, a refusal (the game's double attach is not read).
        if planned.iter().any(|w: &ComponentWrites| w.original == original) {
            return Err(NotYet::not_read(format!("{LISTED_TWICE}: {}", class_path(vm, original)), RECORD).into());
        }
        let obj = vm.space.heap.get(original)?;
        let full = u32::try_from(obj.slots.len()).ok() == vm.space.classes.get_layout(obj.class).map(|l| l.slot_count());
        if vm.defs.is_unloaded(&vm.space, original) || !full {
            return Err(NotYet::not_read(
                format!(
                    "the spawn's component update: the component {} ({}) is not loaded (a load is a write)",
                    vm.space.names.text(obj.name),
                    class_path(vm, original)
                ),
                RECORD,
            )
            .into());
        }
        let class = obj.class;
        let owner = declared(vm, original, "Owner", "an object", is_object)?;
        declared(vm, original, "Scene", "declared", |_| true)?;
        let attached = declared(vm, original, "bAttached", "a bool", is_bool)?;
        let reattach = declared(vm, original, "bNeedsReattach", "a bool", is_bool)?;
        let transform = declared(vm, original, "bNeedsUpdateTransform", "a bool", is_bool)?;
        let skip = declared(vm, original, "bSkipChildComponentUpdate", "a bool", is_bool)?;
        match read(vm, original, attached)? {
            Value::Bool(false) => {}
            Value::Bool(true) => return Err(NotYet::not_read(ATTACHED, RECORD).into()),
            other => return Err(script(format!("{}.bAttached holds {}", class_path(vm, original), other.kind_name()))),
        }
        let own_entry = ai_classes.contains(&class);
        if !own_entry && ai_base.is_some_and(|b| vm.space.classes.is_child_of(class, b)) {
            return Err(NotYet::not_read(format!("{OTHER_AI_CLASS} {} is not read", class_path(vm, original)), RECORD).into());
        }
        planned.push(ComponentWrites {
            original,
            props: [reattach, transform, owner, skip],
            attached: own_entry.then_some(attached),
        });
    }
    // The layer is asked for only when a component is to be attached.
    if !planned.is_empty() && vm.service::<SceneLayer>().is_none() {
        return Err(NotYet::layer_absent(NO_LAYER, SCENE_LAYER, RECORD).into());
    }
    Ok(Plan { components: planned, all_components })
}

/// The writes (module documentation), in `Components` order, on this spawn's copies: `pairs` maps each template
/// component to its copy (`apply_plan`'s answer). A planned component with no copy is a `VmError::Script` (the plan and
/// the copies disagree, a caller's bug). Nothing is loaded or drawn.
pub fn apply(vm: &mut Vm, actor: ObjRef, plan: &Plan, pairs: &[(ObjRef, ObjRef)]) -> Result<(), VmError> {
    let mut appended = Vec::new();
    for w in &plan.components {
        let Some(&(_, copy)) = pairs.iter().find(|(o, _)| *o == w.original) else {
            return Err(script(format!("no copy of the component {} was made", class_path(vm, w.original))));
        };
        let [reattach, transform, owner, skip] = w.props;
        vm.space.write(copy, PropAddr::scalar(reattach), Value::Bool(false))?;
        vm.space.write(copy, PropAddr::scalar(transform), Value::Bool(false))?;
        vm.space.write(copy, PropAddr::scalar(owner), Value::Object(Some(actor)))?;
        if let Some(attached) = w.attached {
            vm.space.write(copy, PropAddr::scalar(attached), Value::Bool(true))?;
            appended.push(Value::Object(Some(copy)));
            vm.space.write(actor, PropAddr::scalar(plan.all_components), Value::array(appended.clone()))?;
        }
        vm.space.write(copy, PropAddr::scalar(skip), Value::Bool(false))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_attach_tests.rs"]
mod tests;
