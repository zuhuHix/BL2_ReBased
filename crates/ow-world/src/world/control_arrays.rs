//! Owner: lane IMPL-COPY-COMPONENT-PASS. The tree initialisation's last writes in `ParityMode::Off`: the init tag and
//! the two per-node arrays (before the per-node loop), and step 5's component pass and per-bone control arrays (#551
//! section 2), from `docs/verification/NATIVE_COPY_COMPONENT_PASS.md` alone (#598 at 5196fcec, the analyst lane
//! ANALYST-COPY-COMPONENT-PASS; this lane did not read the natives). AI-assisted (Claude Code). **Every rule is
//! UNVERIFIED in the running game.**
//!
//! **Where it runs.** [`plan`] is called from the copy's preflight (`anim_tree_copy::plan`) after the per-node
//! initialisation is planned. The copies exist only in the copy's write phase, so it reads the **template** tree and
//! controls, which the copy maps one to one (a copy keeps its template's slots). It writes and loads nothing.
//! [`write_init`] runs in the initialisation's writes after the tick list and before the per-node loop;
//! [`write_arrays`] runs after the per-node loop, on the copies, before `bAnimTreeInitialised` is set.
//!
//! **The init tag and the per-node arrays** (the note's section 1 and section 6, writes 1): `InitTag` = the root's
//! `NodeInitTag` + 1; `AnimTickRelevancyArray` = n zeros and `AnimTickWeightsArray` = n entries of 0.0 (a port
//! convention: the engine does not set them there), n the tick list's length.
//!
//! **The component pass** (section 2): `MorphSets` must be declared a dynamic array of object references (else
//! `VmError::Script`); any set entry is `NotYet { NotPorted }` ([`MORPH_SETS`]), as the port keeps no value for the
//! native name map. An empty list writes nothing.
//!
//! **The per-bone control arrays** (section 4), planned by simulating the tags from the template controls' saved
//! `ControlTickTag`s. The new tag is the component's `TickTag` + 2: the tick list raises it once, the build once more.
//! 1. `SkelControlIndex`, `PostPhysSkelControlIndex` and `SkelControlTickArray` are emptied, always;
//! 2. the build runs only with a `SkeletalMesh`, the tree and at least one `SkelControlLists` entry. Then
//!    `SkelControlIndex` is one 255 per bone of the mesh (#569's reference skeleton, its refusals passed through), and
//!    `TickTag` goes up by one;
//! 3. each list in order, its `BoneName` found in the mesh and its bone not taken yet, sets the bone's byte to its
//!    index and walks its controls from `ControlHead` along `NextControl`:
//!    - a control whose tag is not the new tag gets it and is appended to the tick array;
//!    - then, if it is a post-physics control, `PostPhysSkelControlIndex` is sized to one 255 per bone (once), the
//!      bone's byte is set, and the walk ends, whether or not the control was newly listed.
//!
//! **Preflight refusals** (each before any write, so never marked "stopped after partial writes"):
//! - a field missing or of another kind: `VmError::Script` naming it;
//! - more than 255 lists: `NotYet { NotRead }` ([`TOO_MANY_LISTS`]);
//! - a reached template control whose saved `ControlTickTag` already is the new tag: `NotYet { NotRead }`
//!   ([`TAG_COLLISION`]), as #561's node rule.
//!
//! A looping chain and an unloaded control are the copy's own gather's refusals (#557's rule, before this runs).
//! The note's texts, kinds and the order of the checks are kept in this file only.

use std::rc::Rc;

use ow_core::{DetMap, NotYet};
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
/// A template control already carrying the next control tag (section 6, rule 3).
pub const TAG_COLLISION: &str = "the per-bone control arrays: a template control whose ControlTickTag equals the \
     component's next control tag (how the game keeps the copies' tags apart was not read)";
/// The most lists the byte index can hold.
pub const MAX_LISTS: usize = 255;
/// "No list" in either byte array.
pub const NO_LIST: u8 = 255;

const UNIT: &str = "the tree initialisation's last steps";

/// What the writes set, planned on the template tree (module notes).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrays {
    /// `InitTag`: the root's `NodeInitTag` + 1.
    pub init_tag: i32,
    /// The tick list's length (the two per-node arrays' length).
    pub ticks: usize,
    /// The build runs (a mesh, the tree and at least one list); otherwise only the three arrays are emptied.
    pub build: bool,
    /// `SkelControlIndex`: one byte per bone.
    pub index: Vec<u8>,
    /// `PostPhysSkelControlIndex`: empty, or one byte per bone once a post-physics control was met.
    pub post: Vec<u8>,
    /// The template controls tagged and appended, in order (their copies are written).
    pub listed: Vec<ObjRef>,
    /// The new control tag (the component's `TickTag` after the build).
    pub tag: i32,
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

/// The declared kind of `o`'s property `name` satisfies `want` (one element), else a script error naming it.
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

fn is_int(k: &PropKind) -> bool {
    matches!(k, PropKind::Int)
}

fn is_float(k: &PropKind) -> bool {
    matches!(k, PropKind::Float)
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

/// The preflight (module notes) for `component`, whose tree is the template `tree`, with `tick_tag` the component's
/// `TickTag` before the initialisation and `ticks` the planned tick list's length. Reads only.
pub fn plan(vm: &Vm, component: ObjRef, tree: ObjRef, tick_tag: i32, ticks: usize) -> Result<Arrays, VmError> {
    // The init tag and the per-node arrays' fields.
    declared(vm, component, "InitTag", is_int)?;
    declared(vm, component, "AnimTickRelevancyArray", array_of(is_int))?;
    declared(vm, component, "AnimTickWeightsArray", array_of(is_float))?;
    let init_tag = int_of(vm, tree, "NodeInitTag")?.wrapping_add(1);
    // Step 5.
    component_pass(vm, component)?;
    declared(vm, component, "SkelControlIndex", array_of(is_byte))?;
    declared(vm, component, "PostPhysSkelControlIndex", array_of(is_byte))?;
    declared(vm, component, "SkelControlTickArray", array_of(is_object))?;
    declared(vm, component, "TickTag", is_int)?;
    declared(vm, component, "SkeletalMesh", is_object)?;
    let tag = tick_tag.wrapping_add(2);
    let base = Arrays { init_tag, ticks, tag: tick_tag.wrapping_add(1), ..Arrays::default() };
    let mesh = object_of(&read(vm, component, "SkeletalMesh")?);
    let lists = elements(&read(vm, tree, "SkelControlLists")?, "SkelControlLists")?;
    let (Some(mesh), false) = (mesh, lists.is_empty()) else {
        return Ok(base);
    };
    let skeleton = crate::world::per_bone_weights::skeleton(vm, mesh)?;
    if lists.len() > MAX_LISTS {
        return Err(NotYet::not_read(TOO_MANY_LISTS, RECORD).into());
    }
    build(vm, &skeleton, &lists, Arrays { tag, build: true, ..base })
}

/// The walk of section 4 over the template's `lists`, simulating the controls' tags (module notes).
fn build(vm: &Vm, skeleton: &Rc<ReferenceSkeleton>, lists: &[Value], mut out: Arrays) -> Result<Arrays, VmError> {
    let bones = skeleton.len();
    let tag = out.tag;
    out.index = vec![NO_LIST; bones];
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
        // The copy's gather refused a loop before this runs (#557's rule), so the chain ends.
        let mut next = head;
        while let Some(c) = next {
            control_fields(vm, c)?;
            let now = match tags.get(&c) {
                Some(&t) => t,
                None => {
                    let saved = int_of(vm, c, "ControlTickTag")?;
                    if saved == tag {
                        return Err(NotYet::not_read(TAG_COLLISION, RECORD).into());
                    }
                    saved
                }
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

/// A template control reached must declare the three control fields with their kinds (its residency is the copy's
/// gather's check).
fn control_fields(vm: &Vm, c: ObjRef) -> Result<(), VmError> {
    declared(vm, c, "ControlTickTag", is_int)?;
    declared(vm, c, "NextControl", is_object)?;
    declared(vm, c, "bPostPhysicsController", |k| matches!(k, PropKind::Bool))?;
    Ok(())
}

/// The writes before the per-node loop (section 6, writes 1): `InitTag` and the two per-node arrays, each emptied
/// first. Called only after the whole preflight passed.
pub fn write_init(vm: &mut Vm, component: ObjRef, arrays: &Arrays) -> Result<(), VmError> {
    write(vm, component, "InitTag", Value::Int(arrays.init_tag))?;
    for name in ["AnimTickRelevancyArray", "AnimTickWeightsArray"] {
        write(vm, component, name, Value::array(Vec::new()))?;
    }
    write(vm, component, "AnimTickRelevancyArray", Value::array(vec![Value::Int(0); arrays.ticks]))?;
    write(vm, component, "AnimTickWeightsArray", Value::array(vec![Value::Float(0.0); arrays.ticks]))
}

/// The writes of step 5 (section 6, writes 2 and 3) on `component`, with `copy_of` mapping a template control to its
/// copy. Called only after the whole preflight passed.
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
