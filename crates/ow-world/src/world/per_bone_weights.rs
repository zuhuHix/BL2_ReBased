//! Owner: lane IMPL-PER-BONE-WEIGHTS (decision 2026-11-19-194, number pending). The per-bone weight builders of
//! `AnimNodeBlendPerBone` and `WillowAnimNodeFeatherBlend` (implementation F of NATIVE_ANIM_NODE_INIT.md), in
//! `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_PER_BONE_WEIGHTS.md` (#580, accepted at 5b1a567,
//! analyst lane ANALYST-PER-BONE-WEIGHT-BUILDERS) section 6; this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.**
//!
//! [`plan`] is the build's write-free preflight and computation, in the note's order:
//! 1. **the mesh field:** the component's class declares `SkeletalMesh` as an object reference and holds one, else a
//!    `VmError::Script`. A real null (`Value::Object(None)`) skips the build ([`Build::Skip`]);
//! 2. **the skeleton:** #569's `reference_skeleton`, its errors passed through unchanged;
//! 3. **more than 256 bones:** `NotYet { NotRead }` ([`too_many_bones`]): the bone lists hold bytes;
//! 4. **the fields and their values** (a `VmError::Script` naming the class and the field or member);
//! 5. **the rules** (sections 2 to 4): the weights, the byte list and the parents pass.
//!
//! The caller writes the two arrays ([`Build::Write`]) after implementation B and before `Children[0].Weight`.
//! Nothing here writes, loads or draws (the layer's skeleton cache is not the heap).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::anim_tree_copy::{class_of, class_path, elements, read};
use super::body_composition::field;
use super::skeletal::ReferenceSkeleton;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_PER_BONE_WEIGHTS.md";
const UNIT: &str = "the per-bone weight build";

pub const PER_BONE: &str = "Engine.AnimNodeBlendPerBone";
pub const FEATHER: &str = "WillowGame.WillowAnimNodeFeatherBlend";
/// The most bones a byte list can index.
pub const MAX_BONES: usize = 256;
/// The feather blend's partial-weight bound (section 3).
pub const PARTIAL: f32 = 0.99999;

/// `"the per-bone weight build of <class>: a mesh with more than 256 bones (the bone lists hold bytes)"`.
pub fn too_many_bones(class: &str) -> String {
    format!("the per-bone weight build of {class}: a mesh with more than 256 bones (the bone lists hold bytes)")
}

/// What one F initialisation does to the two arrays.
#[derive(Debug, Clone, PartialEq)]
pub enum Build {
    /// A null mesh: no build, both arrays kept.
    Skip,
    /// `Child2PerBoneWeight` and `LocalToCompReqBones`.
    Write(Vec<f32>, Vec<u8>),
}

fn script(class: &str, message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{class}: {message}") }
}

#[cfg(test)]
thread_local! {
    static SKELETON: std::cell::RefCell<Option<Rc<ReferenceSkeleton>>> = const { std::cell::RefCell::new(None) };
}

/// For the tests only: answer every mesh's skeleton with `s` on this thread (invented skeletons; the layer itself is
/// tested in `skeletal_tests.rs`).
#[cfg(test)]
pub(crate) fn skeleton_for_tests(s: Option<ReferenceSkeleton>) {
    SKELETON.with(|c| *c.borrow_mut() = s.map(Rc::new));
}

/// The mesh's reference skeleton (#569's layer; its refusals passed through), or the tests' invented one.
pub(crate) fn skeleton(vm: &Vm, mesh: ObjRef) -> Result<Rc<ReferenceSkeleton>, VmError> {
    #[cfg(test)]
    if let Some(s) = SKELETON.with(|c| c.borrow().clone()) {
        return Ok(s);
    }
    super::skeletal::reference_skeleton(vm, mesh)
}

/// The declared kind of `class`'s property `name`, or a script error naming it.
fn declared(vm: &Vm, node: ObjRef, class: &str, name: &str, want: impl Fn(&PropKind) -> bool) -> Result<(), VmError> {
    let c = class_of(vm, node)?;
    let ok = vm.space.prop(c, name).and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| want(&d.kind));
    if ok { Ok(()) } else { Err(script(class, format!("{name} is not declared as the builder reads it"))) }
}

fn array_of(inner: fn(&PropKind) -> bool) -> impl Fn(&PropKind) -> bool {
    move |k| matches!(k, PropKind::Array(i) if inner(i))
}

fn name_of(_vm: &Vm, v: &Value, class: &str, what: &str) -> Result<NameId, VmError> {
    match v {
        Value::Name(n) => Ok(*n),
        other => Err(script(class, format!("{what} holds {}, not a name", other.kind_name()))),
    }
}

/// The bone a name names (a None name names none). A name index outside the table is a script error naming the field
/// (review r1 of #585, item 4).
fn bone(vm: &Vm, s: &ReferenceSkeleton, n: NameId, class: &str, what: &str) -> Result<Option<usize>, VmError> {
    if n == NameId::NONE {
        return Ok(None);
    }
    let text = vm.space.names.get(n).ok_or_else(|| script(class, format!("{what} holds a name outside the name table")))?;
    Ok(s.bone_index(&text))
}

/// The parent of bone `i`, the root counting as its own.
fn parent(s: &ReferenceSkeleton, i: usize) -> usize {
    s.bones[i].parent.unwrap_or(i)
}

/// The weights of section 2 and 3: a bone named by a target takes its weight, else its parent's, the root 0.
pub fn weights(s: &ReferenceSkeleton, targets: &[(usize, f32)]) -> Vec<f32> {
    let mut w = vec![0.0f32; s.len()];
    for i in 0..s.len() {
        w[i] = match targets.iter().find(|(b, _)| *b == i) {
            Some((_, x)) => *x,
            None if i > 0 => w[parent(s, i)],
            None => 0.0,
        };
    }
    w
}

/// The feather clamp (section 3): below 0 is 0, 1 or above is 1, a value that is not a number is 0.
pub fn clamp(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        x
    }
}

/// The byte list before the parents pass: bones whose weight differs from their parent's, and, with `partial`
/// (the feather blend), also bones of equal, partial weight.
pub fn listed(s: &ReferenceSkeleton, w: &[f32], partial: bool) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..s.len() {
        let (a, b) = (w[i], w[parent(s, i)]);
        #[allow(clippy::float_cmp)]
        let keep = a != b || (partial && a > 0.0 && b <= PARTIAL);
        if keep {
            out.push(u8::try_from(i).unwrap_or(u8::MAX));
        }
    }
    out
}

/// The parents pass (section 4): each listed bone's missing parent is inserted just before it, and looked at next.
pub fn parents_pass(s: &ReferenceSkeleton, list: &mut Vec<u8>) {
    let mut at = 0;
    while at < list.len() {
        let b = usize::from(list[at]);
        if b == 0 || b >= s.len() {
            at += 1;
            continue;
        }
        let p = parent(s, b);
        if list.iter().any(|&x| usize::from(x) == p) {
            at += 1;
        } else {
            list.insert(at, u8::try_from(p).unwrap_or(u8::MAX));
        }
    }
}

/// The build of F node `node` initialised on `component` (module documentation). Reads only.
pub fn plan(vm: &Vm, component: ObjRef, node: ObjRef) -> Result<Build, VmError> {
    let class = class_path(vm, class_of(vm, node)?);
    // 1. The mesh field.
    let component_class = class_path(vm, class_of(vm, component)?);
    let c = class_of(vm, component)?;
    let mesh_object =
        vm.space.prop(c, "SkeletalMesh").and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| matches!(d.kind, PropKind::Object { .. }));
    if !mesh_object {
        return Err(script(&component_class, "SkeletalMesh is not declared as an object reference".into()));
    }
    let mesh = match read(vm, component, "SkeletalMesh")? {
        Value::Object(None) => return Ok(Build::Skip),
        Value::Object(Some(m)) => m,
        other => return Err(script(&component_class, format!("SkeletalMesh holds {}, not an object", other.kind_name()))),
    };
    // 2. The skeleton (#569's errors unchanged); 3. the byte lists' limit.
    let s = skeleton(vm, mesh)?;
    if s.len() > MAX_BONES {
        return Err(NotYet::not_read(too_many_bones(&class), RECORD).into());
    }
    // 4. The fields and their values.
    declared(vm, node, &class, "Child2PerBoneWeight", array_of(|k| matches!(k, PropKind::Float)))?;
    declared(vm, node, &class, "LocalToCompReqBones", array_of(|k| matches!(k, PropKind::Byte { .. })))?;
    let (targets, partial, force) = if class == FEATHER {
        declared(vm, node, &class, "BoneBlendTargets", array_of(|k| matches!(k, PropKind::Struct(_))))?;
        declared(vm, node, &class, "bForceLocalSpaceBlend", |k| matches!(k, PropKind::Bool))?;
        // The declared element struct's members, before any entry (review r1 of #585, item 1).
        let c = class_of(vm, node)?;
        let element = vm.space.prop(c, "BoneBlendTargets").and_then(|p| vm.space.classes.decl(p)).and_then(|d| match &d.kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(st) => Some(st),
                _ => None,
            },
            _ => None,
        });
        let layout = element
            .and_then(|st| vm.space.classes.struct_layout(st))
            .ok_or_else(|| script(&class, "BoneBlendTargets' element struct is not defined".into()))?;
        for (member, want) in [("StartBoneName", "a name"), ("BlendWeight", "a float")] {
            // One element each (review r2 of #585, item 1): a static array member would be read as its element zero.
            let decl = vm.space.names.find(member).and_then(|m| layout.field(m)).map(|(_, d)| (d.kind.clone(), d.array_dim));
            let ok = matches!(
                (member, &decl),
                ("StartBoneName", Some((PropKind::Name, 1))) | ("BlendWeight", Some((PropKind::Float, 1)))
            );
            if !ok {
                return Err(script(&class, format!("BoneBlendTargets.{member} is not declared as {want}")));
            }
        }
        let mut targets = Vec::new();
        let list = elements(&read(vm, node, "BoneBlendTargets")?, "BoneBlendTargets").map_err(|e| script(&class, e.to_string()))?;
        for e in list {
            let n = field(vm, &e, "StartBoneName", "BoneBlendTargets").map_err(|_| script(&class, "a BoneBlendTargets entry has no StartBoneName".into()))?;
            let n = name_of(vm, &n, &class, "BoneBlendTargets.StartBoneName")?;
            let w = match field(vm, &e, "BlendWeight", "BoneBlendTargets").map_err(|_| script(&class, "a BoneBlendTargets entry has no BlendWeight".into()))? {
                Value::Float(f) => f,
                other => return Err(script(&class, format!("BoneBlendTargets.BlendWeight holds {}, not a float", other.kind_name()))),
            };
            targets.push((n, w));
        }
        // A bool exactly (review r1 of #585, item 3).
        let force = match read(vm, node, "bForceLocalSpaceBlend")? {
            Value::Bool(b) => b,
            other => return Err(script(&class, format!("bForceLocalSpaceBlend holds {}, not a bool", other.kind_name()))),
        };
        let mut resolved: Vec<(usize, f32)> = Vec::new();
        for (n, w) in targets {
            if let Some(b) = bone(vm, &s, n, &class, "BoneBlendTargets.StartBoneName")? {
                resolved.push((b, clamp(w)));
            }
        }
        (resolved, true, force)
    } else {
        declared(vm, node, &class, "BranchStartBoneName", array_of(|k| matches!(k, PropKind::Name)))?;
        let mut targets = Vec::new();
        let list = elements(&read(vm, node, "BranchStartBoneName")?, "BranchStartBoneName").map_err(|e| script(&class, e.to_string()))?;
        for n in list {
            let n = name_of(vm, &n, &class, "BranchStartBoneName")?;
            if let Some(b) = bone(vm, &s, n, &class, "BranchStartBoneName")? {
                targets.push((b, 1.0));
            }
        }
        (targets, false, false)
    };
    // 5. The rules.
    let w = weights(&s, &targets);
    let mut list = Vec::new();
    if !force {
        list = listed(&s, &w, partial);
        parents_pass(&s, &mut list);
    }
    Ok(Build::Write(w, list))
}

// Tested with the copy's fixture (anim_tree_copy_tests.rs, "The per-bone weight builders").
