//! Owner: lane IMPL-COPY-COMPONENT-PASS. The tree initialisation's component pass and per-bone control arrays in
//! `ParityMode::Off` (#551 section 2 step 5), from `docs/verification/NATIVE_COPY_COMPONENT_PASS.md` alone (#598, the
//! analyst lane ANALYST-COPY-COMPONENT-PASS; this lane did not read the natives). AI-assisted (Claude Code). **Every
//! rule is UNVERIFIED in the running game.**
//!
//! **Where it runs.** [`plan`] is called from the copy's preflight (`anim_tree_copy::plan`) after the per-node
//! initialisation is planned, on the template tree, with nothing written or loaded; [`write_arrays`] is called from the
//! initialisation's writes after the per-node steps, on the copies, before `bAnimTreeInitialised` is set.
//!
//! **The component pass** (the note's section 2): `MorphSets` must be declared a dynamic array of object references
//! (else `VmError::Script`); any set entry is `NotYet { NotPorted }` ([`MORPH_SETS`]): the port keeps no value for the
//! native name map. An empty list writes nothing.
//!
//! **The per-bone control arrays** (section 4), planned by simulating the tags (the copies take their templates'
//! `ControlTickTag`, so the template's tags are the copies' tags before the build):
//! 1. `SkelControlIndex`, `PostPhysSkelControlIndex` and `SkelControlTickArray` are emptied, always;
//! 2. the build runs only with a `SkeletalMesh`, the tree, and at least one `SkelControlLists` entry; then
//!    `SkelControlIndex` is one 255 per bone of the mesh (#569's reference skeleton, its refusals passed through),
//!    `TickTag` goes up by one, and each list in order, its `BoneName` found in the mesh and its bone not taken yet,
//!    sets the bone's byte to its index and walks its controls from `ControlHead` along `NextControl`: a control whose
//!    tag is not the new `TickTag` gets it and is appended to the tick array; a post-physics control first sizes
//!    `PostPhysSkelControlIndex` to one 255 per bone (once), sets the bone's byte and ends the walk.
//!
//! **Preflight refusals** (each before any write, so never marked "stopped after partial writes"):
//! - a field missing or of another kind: `VmError::Script` naming it;
//! - more than 255 lists: `NotYet { NotRead }` ([`TOO_MANY_LISTS`]);
//! - a chain that reaches a control twice: `NotYet { NotRead }` ([`LOOP`]). The copy's own control gather (#557's
//!   rule) refuses such a chain earlier, as a script error, so on the copy's path this is a second guard;
//! - an unloaded control: #543's refusal, as the copy's.
//!
//! The note's texts, kinds and the order of the checks are kept in this file only, so a review of the note changes
//! one place.

use std::rc::Rc;

use ow_core::{DetMap, DetSet, NotYet};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::world::anim_tree_copy::{class_of, class_path, elements, object_of, read, truth, write};
use crate::world::body_composition::field;
use crate::world::skeletal::ReferenceSkeleton;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_COPY_COMPONENT_PASS.md";
/// The component pass's refusal for a set `MorphSets` entry (section 6, rule 1).
pub const MORPH_SETS: &str = "the tree initialisation's component pass: the morph-target map of MorphSets";
/// More lists than a byte index can tell apart from 255 (section 6, rule 3).
pub const TOO_MANY_LISTS: &str =
    "the per-bone control arrays: a tree with more than 255 control lists (the list index is a byte and 255 means none)";
/// A `NextControl` chain that comes back to a control (section 6, rule 3).
pub const LOOP: &str = "the per-bone control arrays: a NextControl chain that loops";
/// The most lists the byte index can hold.
pub const MAX_LISTS: usize = 255;
/// "No list" in either byte array.
pub const NO_LIST: u8 = 255;

const UNIT: &str = "the tree initialisation's per-bone control arrays";

/// What the build writes, planned on the template tree (module notes).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrays {
    /// The build runs (a mesh, the tree and at least one list); otherwise only the three arrays are emptied.
    pub build: bool,
    /// `SkelControlIndex`: one byte per bone.
    pub index: Vec<u8>,
    /// `PostPhysSkelControlIndex`: empty, or one byte per bone once a post-physics control was met.
    pub post: Vec<u8>,
    /// The template controls tagged and appended, in order (their copies are written).
    pub listed: Vec<ObjRef>,
    /// The new `TickTag` (the component's tag after the build).
    pub tag: i32,
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

/// The declared kind of `o`'s property `name` satisfies `want`, else a script error naming it.
fn declared(vm: &Vm, o: ObjRef, name: &str, want: impl Fn(&PropKind) -> bool) -> Result<(), VmError> {
    let c = class_of(vm, o)?;
    let ok = vm.space.prop(c, name).and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| d.array_dim == 1 && want(&d.kind));
    if ok {
        Ok(())
    } else {
        Err(script(format!("{}.{name} is not declared as the initialisation reads it", class_path(vm, c))))
    }
}

fn array_of(inner: fn(&PropKind) -> bool) -> impl Fn(&PropKind) -> bool {
    move |k| matches!(k, PropKind::Array(i) if inner(i))
}

fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

fn is_byte(k: &PropKind) -> bool {
    matches!(k, PropKind::Byte { .. })
}

fn int_of(vm: &Vm, o: ObjRef, name: &str) -> Result<i32, VmError> {
    match read(vm, o, name)? {
        Value::Int(i) => Ok(i),
        other => Err(script(format!("{}.{name} holds {}, not an int", class_path(vm, class_of(vm, o)?), other.kind_name()))),
    }
}

/// The component pass (section 2; module notes): checks only, it writes nothing for an empty list.
fn component_pass(vm: &Vm, component: ObjRef) -> Result<(), VmError> {
    declared(vm, component, "MorphSets", array_of(is_object))?;
    let sets = elements(&read(vm, component, "MorphSets")?, "MorphSets")?;
    if sets.iter().any(|s| object_of(s).is_some()) {
        return Err(NotYet::not_ported(MORPH_SETS, RECORD).into());
    }
    Ok(())
}

/// The preflight of step 5 (module notes) for `component`, whose tree is the template `tree` and whose tick counter
/// after the tick list (#551 step 3) is `next_tag`. Reads only.
pub fn plan(vm: &Vm, component: ObjRef, tree: ObjRef, next_tag: i32) -> Result<Arrays, VmError> {
    component_pass(vm, component)?;
    declared(vm, component, "SkelControlIndex", array_of(is_byte))?;
    declared(vm, component, "PostPhysSkelControlIndex", array_of(is_byte))?;
    declared(vm, component, "SkelControlTickArray", array_of(is_object))?;
    declared(vm, component, "TickTag", |k| matches!(k, PropKind::Int))?;
    declared(vm, component, "SkeletalMesh", is_object)?;
    let mesh = object_of(&read(vm, component, "SkeletalMesh")?);
    let lists = elements(&read(vm, tree, "SkelControlLists")?, "SkelControlLists")?;
    let (Some(mesh), false) = (mesh, lists.is_empty()) else {
        return Ok(Arrays { tag: next_tag, ..Arrays::default() });
    };
    let skeleton = crate::world::per_bone_weights::skeleton(vm, mesh)?;
    if lists.len() > MAX_LISTS {
        return Err(NotYet::not_read(TOO_MANY_LISTS, RECORD).into());
    }
    build(vm, &skeleton, &lists, next_tag.wrapping_add(1))
}

/// The walk of section 4 over the template's `lists`, simulating the controls' tags (module notes).
fn build(vm: &Vm, skeleton: &Rc<ReferenceSkeleton>, lists: &[Value], tag: i32) -> Result<Arrays, VmError> {
    let bones = skeleton.len();
    let mut out = Arrays { build: true, index: vec![NO_LIST; bones], tag, ..Arrays::default() };
    let mut tags: DetMap<ObjRef, i32> = DetMap::new();
    for (i, list) in lists.iter().enumerate() {
        let byte = u8::try_from(i).map_err(|_| NotYet::not_read(TOO_MANY_LISTS, RECORD))?;
        let name = match field(vm, list, "BoneName", "SkelControlListHead")? {
            Value::Name(n) => n,
            other => return Err(script(format!("SkelControlLists.BoneName holds {}, not a name", other.kind_name()))),
        };
        let head = match field(vm, list, "ControlHead", "SkelControlListHead")? {
            Value::Object(o) => o,
            other => return Err(script(format!("SkelControlLists.ControlHead holds {}, not an object", other.kind_name()))),
        };
        let text = vm.space.names.text(name);
        let found = if text.eq_ignore_ascii_case("None") { None } else { skeleton.bone_index(&text) };
        let Some(bone) = found.filter(|&b| b < bones) else {
            continue;
        };
        if out.index[bone] != NO_LIST {
            continue;
        }
        out.index[bone] = byte;
        let mut chain = DetSet::new();
        let mut next = head;
        while let Some(c) = next {
            if !chain.insert(c) {
                return Err(NotYet::not_read(LOOP, RECORD).into());
            }
            control_fields(vm, c)?;
            let now = match tags.get(&c) {
                Some(&t) => t,
                None => int_of(vm, c, "ControlTickTag")?,
            };
            if now != tag {
                tags.insert(c, tag);
                out.listed.push(c);
            }
            if truth(&read(vm, c, "bPostPhysicsController")?) {
                if out.post.is_empty() {
                    out.post = vec![NO_LIST; bones];
                }
                out.post[bone] = byte;
                break;
            }
            next = object_of(&read(vm, c, "NextControl")?);
        }
    }
    Ok(out)
}

/// A control reached must be resident and declare the three control fields with their kinds.
fn control_fields(vm: &Vm, c: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, c) || vm.defs.is_failed(&vm.space, c) {
        return Err(crate::world::anim_node_lookup::refusal());
    }
    declared(vm, c, "ControlTickTag", |k| matches!(k, PropKind::Int))?;
    declared(vm, c, "NextControl", is_object)?;
    declared(vm, c, "bPostPhysicsController", |k| matches!(k, PropKind::Bool))?;
    Ok(())
}

/// The writes of step 5 (section 6, "Writes, in order") on `component`, with `copy_of` mapping a template control to
/// its copy. Called only after the whole preflight passed.
pub fn write_arrays(
    vm: &mut Vm,
    component: ObjRef,
    arrays: &Arrays,
    copy_of: impl Fn(ObjRef) -> ObjRef,
) -> Result<(), VmError> {
    for name in ["SkelControlIndex", "PostPhysSkelControlIndex", "SkelControlTickArray"] {
        write(vm, component, name, Value::array(Vec::new()))?;
    }
    if !arrays.build {
        return Ok(());
    }
    let bytes = |b: &[u8]| Value::array(b.iter().map(|&x| Value::Byte(x)).collect());
    write(vm, component, "SkelControlIndex", bytes(&arrays.index))?;
    write(vm, component, "TickTag", Value::Int(arrays.tag))?;
    let mut tick = Vec::with_capacity(arrays.listed.len());
    for &t in &arrays.listed {
        let c = copy_of(t);
        write(vm, c, "ControlTickTag", Value::Int(arrays.tag))?;
        tick.push(Value::Object(Some(c)));
    }
    write(vm, component, "SkelControlTickArray", Value::array(tick))?;
    if !arrays.post.is_empty() {
        write(vm, component, "PostPhysSkelControlIndex", bytes(&arrays.post))?;
    }
    Ok(())
}
