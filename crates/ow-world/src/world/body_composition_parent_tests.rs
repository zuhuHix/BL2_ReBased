//! The body composition's `Parent` link (lane IMPL-CUSTOMHEAD-PARENT-LINK), coded from
//! NATIVE_INITIALIZE_BODY_COMPOSITION.md section 8 alone, on the parent module's invented fixture (AGENTS.md rule 9:
//! invented values; the class and field names are the packages'). The tests follow the note's section 8.2 list. Every
//! refusal is checked write-free against #518's three counters and the name table's length.
//! AI-assisted (Claude Code). **UNVERIFIED** in the running game.

use super::*;
use crate::world::skeletal::{ReferenceSkeleton, SkeletalMeshes, SkeletonBone, SkeletonSource};

const BODY: &str = "ToyBodyMesh";
const HEAD: &str = "ToyHeadMesh";

/// The skeleton source: bone texts by the mesh's leaf name. A mesh it does not list has no data.
struct Bones(Vec<(&'static str, Vec<&'static str>)>);

impl SkeletonSource for Bones {
    fn skeleton(&self, _package: &str, _path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        let Some((_, names)) = self.0.iter().find(|(l, _)| *l == leaf) else {
            return Ok(None);
        };
        let bones = names
            .iter()
            .enumerate()
            .map(|(i, n)| SkeletonBone { name: (*n).to_string(), parent: if i == 0 { None } else { Some(0) }, bind: None })
            .collect();
        Ok(Some(ReferenceSkeleton { bones }))
    }
}

/// Installs `meshes` as the skeleton source, with `interned` in the name table as a package's names would be.
fn source(e: &mut Env, meshes: Vec<(&'static str, Vec<&'static str>)>, interned: &[&str]) {
    for n in interned {
        e.vm.space.names.intern(n);
    }
    SkeletalMeshes::install(&mut e.vm, Box::new(Bones(meshes))).unwrap();
}

/// A loaded mesh export of the fixture's template package.
fn mesh(e: &mut Env, leaf: &str) -> ObjRef {
    let path = ObjectPath::parse(&format!("GD_ToyBody.{leaf}")).unwrap();
    let m = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, m).unwrap();
    m
}

/// Attachment 0 a skeletal component on `parent_mesh`, then attachment 1, a component of `class` on `child_mesh`
/// under it with identifier `id`: (pawn, child template).
fn linked_as(e: &mut Env, class: ClassId, id: &str, parent_mesh: Option<ObjRef>, child_mesh: Option<ObjRef>) -> (ObjRef, ObjRef) {
    let skel = e.skeletal;
    let pt = make(&mut e.vm, skel, "Body_Template");
    put(&mut e.vm, pt, "SkeletalMesh", Value::Object(parent_mesh));
    let ct = make(&mut e.vm, class, "Head_Template");
    if child_mesh.is_some() {
        put(&mut e.vm, ct, "SkeletalMesh", Value::Object(child_mesh));
    }
    let a0 = attachment(e, A { template: Some(pt), ..A::default() });
    let a1 = attachment(e, A { base: 0, template: Some(ct), parent: id, index: 2, ..A::default() });
    (pawn_with(e, vec![a0, a1]), ct)
}

/// [`linked_as`] for a skeletal child with identifier `Parent`.
fn linked(e: &mut Env, parent_mesh: Option<ObjRef>, child_mesh: Option<ObjRef>) -> (ObjRef, ObjRef) {
    let skel = e.skeletal;
    linked_as(e, skel, PARENT_ID, parent_mesh, child_mesh)
}

/// The made (parent, child) after a build.
fn made_pair(e: &Env, p: ObjRef) -> (ObjRef, ObjRef) {
    let list = entries(e, p);
    (made_of(e, &list[0]), made_of(e, &list[1]))
}

fn map_of(e: &Env, o: ObjRef) -> Vec<i32> {
    prop(&e.vm, o, "ParentBoneMap")
        .elements()
        .unwrap()
        .iter()
        .map(|v| match v {
            Value::Int(k) => *k,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn ints(v: &[i32]) -> Value {
    Value::array(v.iter().map(|&k| Value::Int(k)).collect())
}

/// Test 1: one mesh of 3 bones on both: `ParentAnimComponent` is the made parent and the map is the identity.
#[test]
fn the_same_mesh_links_with_the_identity_map() {
    let mut e = env();
    let m = mesh(&mut e, BODY);
    source(&mut e, vec![(BODY, vec!["Root", "Spine", "Head"])], &["Root", "Spine", "Head"]);
    let (p, _) = linked(&mut e, Some(m), Some(m));
    run(&mut e, p).unwrap();
    let (parent, child) = made_pair(&e, p);
    assert_eq!(prop(&e.vm, child, "ParentAnimComponent"), Value::Object(Some(parent)));
    assert_eq!(map_of(&e, child), vec![0, 1, 2]);
}

/// Test 2: different meshes, the flag clear: the child's `A, B, C` against the parent's `C, A` is `[1, -1, 0]`.
#[test]
fn different_meshes_map_by_bone_name() {
    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    source(&mut e, vec![(HEAD, vec!["A", "B", "C"]), (BODY, vec!["C", "A"])], &["A", "B", "C"]);
    let (p, _) = linked(&mut e, Some(body), Some(head));
    run(&mut e, p).unwrap();
    let (parent, child) = made_pair(&e, p);
    assert_eq!(prop(&e.vm, child, "ParentAnimComponent"), Value::Object(Some(parent)));
    assert_eq!(map_of(&e, child), vec![1, -1, 0]);
}

/// Test 3: different meshes with `bIsGestaltAccessoryMesh` set: the identity over the child's bones. The parent's
/// skeleton is not read (the source has none for it).
#[test]
fn the_gestalt_accessory_flag_forces_the_identity() {
    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    source(&mut e, vec![(HEAD, vec!["A", "B", "C"])], &["A", "B", "C"]);
    let (p, ct) = linked(&mut e, Some(body), Some(head));
    put(&mut e.vm, ct, "bIsGestaltAccessoryMesh", Value::Bool(true));
    run(&mut e, p).unwrap();
    let (_, child) = made_pair(&e, p);
    assert_eq!(map_of(&e, child), vec![0, 1, 2]);
}

/// Test 4: the child's or the parent's `SkeletalMesh` None: the link is set and the map is empty, a template map with
/// entries emptied too. No skeleton is read (no source is installed).
#[test]
fn a_missing_mesh_links_with_an_empty_map() {
    for child_has_mesh in [false, true] {
        let mut e = env();
        let m = mesh(&mut e, BODY);
        let (parent_mesh, child_mesh) = if child_has_mesh { (None, Some(m)) } else { (Some(m), None) };
        let (p, ct) = linked(&mut e, parent_mesh, child_mesh);
        put(&mut e.vm, ct, "ParentBoneMap", ints(&[5, 6]));
        run(&mut e, p).unwrap();
        let (parent, child) = made_pair(&e, p);
        assert_eq!(prop(&e.vm, child, "ParentAnimComponent"), Value::Object(Some(parent)), "child mesh {child_has_mesh}");
        assert_eq!(map_of(&e, child), Vec::<i32>::new(), "child mesh {child_has_mesh}");
    }
}

/// Test 5: a skeleton that cannot be read is its reader's own `LayerAbsent`, before any write and with nothing loaded:
/// no source installed, and a source without the parent's mesh (the by-name path reads it).
#[test]
fn an_unreadable_skeleton_is_the_reader_s_refusal() {
    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    let (p, _) = linked(&mut e, Some(body), Some(head));
    let (missing, what) = refused(&mut e, p);
    assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
    assert!(what.contains("no skeleton source is installed"), "{what}");

    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    source(&mut e, vec![(HEAD, vec!["A"])], &["A"]);
    let (p, _) = linked(&mut e, Some(body), Some(head));
    let (missing, what) = refused(&mut e, p);
    assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
    assert!(what.contains(BODY), "the parent mesh's reader: {what}");
}

/// Test 6: a child bone name the name table does not hold is -1 in its slot, and nothing is interned.
#[test]
fn a_bone_name_the_name_table_lacks_is_no_match() {
    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    source(&mut e, vec![(HEAD, vec!["Root", "NeverSeenHeadBone"]), (BODY, vec!["Root"])], &["Root"]);
    let (p, _) = linked(&mut e, Some(body), Some(head));
    let names = e.vm.space.names.len();
    run(&mut e, p).unwrap();
    let (_, child) = made_pair(&e, p);
    assert_eq!(map_of(&e, child), vec![0, -1]);
    assert_eq!(e.vm.space.names.len(), names, "nothing interned");
    assert!(e.vm.space.names.find("NeverSeenHeadBone").is_none());
}

/// Test 7: `ParentBoneMap` not an int array, or `bIsGestaltAccessoryMesh` not a bool: `VmError::Script` naming it,
/// before any write.
#[test]
fn malformed_link_fields_are_script_errors() {
    for (opts, needle) in [
        (Opts { parent_map_int: true, ..Opts::default() }, "ParentBoneMap"),
        (Opts { gestalt_flag_int: true, ..Opts::default() }, "bIsGestaltAccessoryMesh"),
    ] {
        let mut e = env_opts(opts);
        let (p, _) = linked(&mut e, None, None);
        script_refusal(&mut e, p, needle);
    }
}

/// Test 8: a non-skeletal child under a skeletal parent, or a skeletal child with another identifier: no link (as
/// before). With different meshes and no source installed, a link would refuse. A non-skeletal parent is the existing
/// other-target refusal ([`super::the_named_refusals_are_write_free`]).
#[test]
fn without_the_gate_there_is_no_link() {
    let mut e = env();
    let body = mesh(&mut e, BODY);
    let cylinder = e.cylinder;
    let (p, _) = linked_as(&mut e, cylinder, PARENT_ID, Some(body), None);
    run(&mut e, p).unwrap();

    let mut e = env();
    let (body, head) = (mesh(&mut e, BODY), mesh(&mut e, HEAD));
    let skel = e.skeletal;
    let (p, _) = linked_as(&mut e, skel, "None", Some(body), Some(head));
    run(&mut e, p).unwrap();
    let (_, child) = made_pair(&e, p);
    assert_eq!(prop(&e.vm, child, "ParentAnimComponent"), Value::Object(None));
    assert_eq!(map_of(&e, child), Vec::<i32>::new());
}

/// Test 9: the order of the writes on one attachment. The pawn's `HeadMesh` here takes only a cylinder, so the
/// custom-head write (the pawn's `PostInitBodyComposition`) fails in the write phase. By then the attach (with no
/// socket name, the owner retry: the child in the pawn's `Components`) and the link's two writes are in, and hook 2's planned `bOwnerNoSee`, which comes
/// after the custom head, is not. The failure is the heap's own error (only a `NotYet` gets #518's prefix).
#[test]
fn the_link_writes_after_the_attach_and_before_the_pawn_s_writes() {
    let mut e = env_opts(Opts { head_mesh_cylinder: true, ..Opts::default() });
    let m = mesh(&mut e, BODY);
    source(&mut e, vec![(BODY, vec!["Root", "Head"])], &["Root", "Head"]);
    // Both reattach-ready: hook 2 plans on each made component of a local player's pawn.
    let reattach = e.vm.find_class("Test.ToyReattachSkeletal").unwrap();
    let pt = make(&mut e.vm, reattach, "Body_Template");
    put(&mut e.vm, pt, "SkeletalMesh", Value::Object(Some(m)));
    let ct = make(&mut e.vm, reattach, "Head_Template");
    put(&mut e.vm, ct, "SkeletalMesh", Value::Object(Some(m)));
    let a0 = attachment(&mut e, A { template: Some(pt), ..A::default() });
    let a1 = attachment(&mut e, A { base: 0, template: Some(ct), parent: PARENT_ID, data_name: CUSTOM_HEAD_NAME, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    controller_for(&mut e, p);
    assert!(super::hook_2_plan(&e, p, ct).unwrap(), "hook 2 plans its write on the child");
    let err = run(&mut e, p).unwrap_err();
    assert!(matches!(err, VmError::Heap(_)), "the custom-head write fails: {err:?}");
    let (parent, child) = made_pair(&e, p);
    // No socket name: the socket attach answers false and the owner retry attaches the child to the pawn.
    let attached = prop(&e.vm, p, "Components").elements().unwrap().contains(&Value::Object(Some(child)));
    assert!(attached, "the attach came first");
    assert_eq!(prop(&e.vm, child, "ParentAnimComponent"), Value::Object(Some(parent)), "the link's first write");
    assert_eq!(map_of(&e, child), vec![0, 1], "the link's second write");
    assert_eq!(prop(&e.vm, child, "bOwnerNoSee"), Value::Bool(false), "hook 2 comes after the pawn's writes");
}

/// The link reads the new component's mesh from its template, so a child template the store has not loaded is the
/// walk's existing lazy-template `NotRead` (not in the note's list: this lane's guard, so nothing is loaded).
#[test]
fn an_unloaded_child_template_is_not_read() {
    let mut e = env();
    let (skel_handle, _, _) = super::store_templates(&mut e);
    let skel = e.skeletal;
    let pt = make(&mut e.vm, skel, "Body_Template");
    let a0 = attachment(&mut e, A { template: Some(pt), ..A::default() });
    let a1 = attachment(&mut e, A { base: 0, template: Some(skel_handle), parent: PARENT_ID, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    let (missing, what) = refused(&mut e, p);
    assert_eq!((missing, what), (Missing::NotRead, lazy_template("GD_ToyBody.SkelTemplate")));
    assert!(e.vm.defs.is_unloaded(&e.vm.space, skel_handle), "nothing loaded");
}
