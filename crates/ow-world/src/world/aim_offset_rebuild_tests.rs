//! The required-bones rebuild (NATIVE_AIMOFFSET_REBUILD.md section 5; lane IMPL-AIMOFFSET-REBUILD) on invented classes
//! and a toy skeleton source (AGENTS.md rule 9: invented objects and values, the class and field names the packages').
//! The meshes are exports of an invented `Engine.upk`, so the skeletal layer reads them by identity. Every refusal
//! leaves #518's counters, `class_code_loads` and the name table unchanged.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{DetMap, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::skeletal::{ReferenceSkeleton, SkeletalMeshes, SkeletonBone, SkeletonSource};

static NEXT: AtomicU32 = AtomicU32::new(0);
const MESHES: usize = 8;

/// A source answering `Mesh_<i>`'s skeleton from a map by leaf.
struct Toy(DetMap<String, ReferenceSkeleton>);

impl SkeletonSource for Toy {
    fn skeleton(&self, _package: &str, _path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        Ok(self.0.get(leaf).cloned())
    }
}

/// A skeleton of `parents.len()` bones named `Bone<i>` (or `names[i]` when given).
fn skeleton(parents: &[Option<usize>], names: Option<&[&str]>) -> ReferenceSkeleton {
    ReferenceSkeleton {
        bones: parents
            .iter()
            .enumerate()
            .map(|(i, &p)| SkeletonBone {
                name: names.map_or_else(|| format!("Bone{i}"), |n| n[i].to_string()),
                parent: p,
                bind: None,
            })
            .collect(),
    }
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    let mesh = t.class("SkeletalMesh", object);
    for i in 0..MESHES {
        let mut p = ints(&[0]);
        p.extend_from_slice(&t.none);
        t.p.add_export(mesh, &format!("Mesh_{i}"), p);
    }
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    by_index: FuncId,
    node: ClassId,
    component: ClassId,
    profile: StructId,
    aim_component: StructId,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

/// The fixture, with `skeletons[i]` as `Mesh_<i>`'s skeleton (None: no skeleton source installed).
fn fx(skeletons: Option<Vec<ReferenceSkeleton>>) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-aimrebuild-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Engine.Object").unwrap();
    vm.find_class("Engine.SkeletalMesh").unwrap();
    let o = PropKind::Object { class: object };
    let bone = vec![decl(&mut vm, "BoneName", PropKind::Name)];
    let aim_component = vm.space.classes.define_struct("Engine.AnimNodeAimOffset.AimComponent", None, bone).unwrap();
    let fields = vec![
        decl(&mut vm, "ProfileName", PropKind::Name),
        decl(&mut vm, "AimComponents", PropKind::Array(Box::new(PropKind::Struct(aim_component)))),
    ];
    let profile = vm.space.classes.define_struct("Engine.AnimNodeAimOffset.AimOffsetProfile", None, fields).unwrap();
    let byte_array = PropKind::Array(Box::new(PropKind::Byte { enum_: None }));
    let props = vec![
        decl(&mut vm, "TemplateNode", o.clone()),
        decl(&mut vm, "SkelComponent", o.clone()),
        decl(&mut vm, "Profiles", PropKind::Array(Box::new(PropKind::Struct(profile)))),
        decl(&mut vm, "CurrentProfileIndex", PropKind::Int),
        decl(&mut vm, "RequiredBones", byte_array.clone()),
        decl(&mut vm, "AimCpntIndexLUT", byte_array),
    ];
    let aim = vm.space.classes.define("Engine.AnimNodeAimOffset", Some(object), props).unwrap();
    let node = vm.space.classes.define("Engine.WillowAnimNodeAimOffset", Some(aim), vec![]).unwrap();
    let props = vec![decl(&mut vm, "SkeletalMesh", o)];
    let component = vm.space.classes.define("Engine.ToyMeshComponent", Some(object), props).unwrap();
    let p = decl(&mut vm, "ProfileIndex", PropKind::Int).with_flags(PropertyDecl::PARAM);
    let path = ObjectPath::parse("Engine.AnimNodeAimOffset.SetActiveProfileByIndex").unwrap();
    let by_index = vm.declare_function(FunctionDecl::native(path, BY_INDEX).with_owner(aim).with_params(vec![p])).unwrap();
    register(&mut vm).unwrap();
    if let Some(list) = skeletons {
        let map = list.into_iter().enumerate().map(|(i, s)| (format!("Mesh_{i}"), s)).collect();
        SkeletalMeshes::install(&mut vm, Box::new(Toy(map))).unwrap();
    }
    Fx { vm, dir, by_index, node, component, profile, aim_component }
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn mesh(&mut self, i: usize) -> ObjRef {
        let path = ObjectPath::parse(&format!("Engine.Mesh_{i}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// A profile named `name` whose components name `bones` (`None` for a `None` name).
    fn profile(&mut self, name: &str, bones: &[Option<&str>]) -> Value {
        let comps = bones
            .iter()
            .map(|b| {
                let id = b.map_or(ow_object::ids::NameId::NONE, |t| self.vm.space.names.intern(t));
                Value::structure(self.aim_component, vec![Value::Name(id)])
            })
            .collect();
        let n = self.vm.space.names.intern(name);
        Value::structure(self.profile, vec![Value::Name(n), Value::array(comps)])
    }

    /// A node over `profiles`, `CurrentProfileIndex` 0, whose component holds `Mesh_<mesh>`.
    fn node(&mut self, profiles: Vec<Value>, mesh: usize) -> ObjRef {
        let (node_class, comp_class) = (self.node, self.component);
        let n = self.vm.space.alloc(node_class, "AimOffset", None, Origin::Runtime).unwrap();
        let c = self.vm.space.alloc(comp_class, "Mesh", None, Origin::Runtime).unwrap();
        let m = self.mesh(mesh);
        self.set(c, "SkeletalMesh", Value::Object(Some(m)));
        self.set(n, "SkelComponent", Value::Object(Some(c)));
        // Through the raw slot, so a test can give a malformed list (the heap's typed write would refuse it).
        let p = self.vm.space.prop(node_class, "Profiles").unwrap();
        *self.vm.space.slot_mut(n, PropAddr::scalar(p)).unwrap() = Value::array(profiles);
        n
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    fn by_index(&mut self, n: ObjRef, i: i32) -> Result<(), VmError> {
        let f = self.by_index;
        self.vm.call(f, Some(n), vec![Value::Int(i)]).map(|_| ())
    }

    fn refusal(&mut self, n: ObjRef, i: i32) -> VmError {
        let before = self.marks();
        let e = self.by_index(n, i).unwrap_err();
        assert_eq!(self.marks(), before, "a refusal writes nothing");
        e
    }

    fn bytes(&self, n: ObjRef, name: &str) -> Vec<u8> {
        self.get(n, name)
            .elements()
            .unwrap()
            .iter()
            .map(|v| match v {
                Value::Byte(b) => *b,
                other => panic!("{other:?}"),
            })
            .collect()
    }
}

/// Bones 0..5: 1 and 3 children of the root, 2 a child of 1, 4 a child of 1.
fn small() -> ReferenceSkeleton {
    skeleton(&[None, Some(0), Some(1), Some(0), Some(1)], None)
}

/// Tests 1, 2 and 4: two components on bones 2 and 4 give both and their missing ancestors (0, 1, 2, 4), and a LUT
/// with one 255 per bone except the components' positions; a bone the mesh lacks and a `None` name are skipped; two
/// components on one bone give it twice, the LUT keeping the later position.
#[test]
fn the_components_their_ancestors_and_the_lookup_table() {
    let mut x = fx(Some(vec![small()]));
    let a = x.profile("A", &[]);
    let b = x.profile("B", &[Some("Bone2"), Some("NoSuchBone"), None, Some("Bone4")]);
    let n = x.node(vec![a, b], 0);
    x.by_index(n, 1).unwrap();
    assert_eq!(x.get(n, "CurrentProfileIndex"), Value::Int(1));
    assert_eq!(x.bytes(n, "RequiredBones"), [0, 1, 2, 4]);
    assert_eq!(x.bytes(n, "AimCpntIndexLUT"), [255, 255, 0, 255, 3]);

    let mut x = fx(Some(vec![small()]));
    let a = x.profile("A", &[]);
    let b = x.profile("B", &[Some("Bone3"), Some("Bone3")]);
    let n = x.node(vec![a, b], 0);
    x.by_index(n, 1).unwrap();
    assert_eq!(x.bytes(n, "RequiredBones"), [0, 3, 3], "the bone twice, its parent once");
    assert_eq!(x.bytes(n, "AimCpntIndexLUT"), [255, 255, 255, 1, 255], "the later position");
}

/// Test 3: the parent pass's order. Bones 3 and 4 required, with 1 and 3 children of the root and 4 a child of 1:
/// 0, 3, 1, 4 (not sorted).
#[test]
fn the_parent_pass_puts_an_ancestor_before_the_first_bone_needing_it() {
    let parents = [None, Some(0), Some(1), Some(0), Some(1)];
    assert_eq!(with_parents(&[3, 4], &parents), [0, 3, 1, 4]);
    assert_eq!(with_parents(&[2], &parents), [0, 1, 2], "an ancestor's ancestors before it");
    assert_eq!(with_parents(&[0, 1], &parents), [0, 1], "nothing missing");
    let mut x = fx(Some(vec![small()]));
    let a = x.profile("A", &[]);
    let b = x.profile("B", &[Some("Bone4"), Some("Bone3")]);
    let n = x.node(vec![a, b], 0);
    x.by_index(n, 1).unwrap();
    assert_eq!(x.bytes(n, "RequiredBones"), [0, 3, 1, 4]);
}

/// Test 7: the limits and their order. 257 bones: the byte `NotRead`; 256 answered. 256 components: the byte
/// `NotRead`; 255 answered. 257 bones with a malformed component list: the bone limit, so the components were not
/// read yet. All refusals write-free.
#[test]
fn the_byte_limits_in_the_note_s_order() {
    let wide = |n: usize| skeleton(&(0..n).map(|i| if i == 0 { None } else { Some(0) }).collect::<Vec<_>>(), None);
    let mut x = fx(Some(vec![wide(257), wide(256), small()]));
    let a = x.profile("A", &[]);
    let b = x.profile("B", &[Some("Bone1")]);
    let n = x.node(vec![a.clone(), b.clone()], 0);
    match x.refusal(n, 1) {
        VmError::NotYet(e) => assert_eq!((e.missing, &*e.what), (Missing::NotRead, PAST_A_BYTE)),
        other => panic!("{other:?}"),
    }
    let n = x.node(vec![a.clone(), b], 1);
    x.by_index(n, 1).unwrap();
    assert_eq!(x.bytes(n, "AimCpntIndexLUT").len(), 256, "256 bones answered");
    let names: Vec<Option<&str>> = vec![Some("Bone1"); 256];
    let c256 = x.profile("C", &names);
    let n = x.node(vec![a.clone(), c256], 2);
    match x.refusal(n, 1) {
        VmError::NotYet(e) => assert_eq!(&*e.what, PAST_A_BYTE),
        other => panic!("{other:?}"),
    }
    let c255 = x.profile("D", &names[..255]);
    let n = x.node(vec![a.clone(), c255], 2);
    x.by_index(n, 1).unwrap();
    assert_eq!(x.bytes(n, "AimCpntIndexLUT")[1], 254, "the last of 255 positions");
    // Review of #745: the literal case, 257 bones with a 256-component profile, is refused past a byte. Both limits
    // share one text, so this case does not show which one fired; the malformed-list case below proves the order.
    let e256 = x.profile("E", &names);
    let n = x.node(vec![a.clone(), e256], 0);
    match x.refusal(n, 1) {
        VmError::NotYet(e) => assert_eq!((e.missing, &*e.what), (Missing::NotRead, PAST_A_BYTE)),
        other => panic!("{other:?}"),
    }
    // The order: 257 bones and a malformed list is the bone limit, not the list's script error.
    let broken = Value::structure(x.profile, vec![Value::Name(ow_object::ids::NameId::NONE), Value::array(vec![Value::Int(1)])]);
    let n = x.node(vec![a, broken], 0);
    match x.refusal(n, 1) {
        VmError::NotYet(e) => assert_eq!(&*e.what, PAST_A_BYTE),
        other => panic!("{other:?}"),
    }
}

/// Test 8: the profiles layer's text names and the decoded name ids give the same lists, a numbered name included
/// (`Spine_2` as text and as an id); nothing is interned.
#[test]
fn layer_text_and_decoded_ids_match_the_same_bones() {
    let names = ["Root", "Spine_2", "Arm", "Hand"];
    let mut x = fx(Some(vec![skeleton(&[None, Some(0), Some(1), Some(2)], Some(&names))]));
    let mesh = x.mesh(0);
    let spine = x.vm.space.names.intern("Spine_2");
    let hand = x.vm.space.names.intern("Hand");
    let before = x.marks();
    let text = rebuild_lists(&x.vm, mesh, || Ok(vec![Bone::Text("spine_2".into()), Bone::Text("Hand".into())])).unwrap();
    let ids = rebuild_lists(&x.vm, mesh, || Ok(vec![Bone::Id(spine), Bone::Id(hand)])).unwrap();
    assert_eq!(text, ids);
    assert_eq!(text.0, [0, 1, 2, 3]);
    assert_eq!(text.1, [255, 0, 255, 1]);
    assert_eq!(x.marks(), before, "nothing interned or written");
    let none = rebuild_lists(&x.vm, mesh, || Ok(vec![Bone::Text("None".into())])).unwrap();
    assert!(none.0.is_empty(), "a None text is no bone");
}

/// Test 9: a malformed `AimComponents` (an element that is not a struct) is `VmError::Script`; a skeleton with two
/// bones of one name (case-folded) is the layer's `VmError::Script`; nothing written.
#[test]
fn malformed_components_and_a_repeated_bone_name_are_script_errors() {
    let names = ["Root", "Arm", "ARM"];
    let mut x = fx(Some(vec![small(), skeleton(&[None, Some(0), Some(0)], Some(&names))]));
    let a = x.profile("A", &[]);
    let broken = Value::structure(x.profile, vec![Value::Name(ow_object::ids::NameId::NONE), Value::array(vec![Value::Int(1)])]);
    let n = x.node(vec![a.clone(), broken], 0);
    assert!(matches!(x.refusal(n, 1), VmError::Script { .. }));
    let b = x.profile("B", &[Some("Arm")]);
    let n = x.node(vec![a, b], 1);
    assert!(matches!(x.refusal(n, 1), VmError::Script { .. }));
}

/// Test 10, link 1's shape: a template node with 7 profiles and `Unarmed` at index 3, with 11 components on a 27-bone
/// toy skeleton (every parent before its child): `CurrentProfileIndex` 3; every component bone and all its ancestors
/// in `RequiredBones`, each once, every parent before its child; a 27-entry table with the 11 positions set.
#[test]
fn link_1_s_shape() {
    let parents: Vec<Option<usize>> = (0..27).map(|i| if i == 0 { None } else { Some((i - 1) / 2) }).collect();
    let mut x = fx(Some(vec![skeleton(&parents, None)]));
    let bones: Vec<String> = (12..23).map(|i| format!("Bone{i}")).collect();
    let bone_refs: Vec<Option<&str>> = bones.iter().map(|b| Some(b.as_str())).collect();
    let mut profiles = Vec::new();
    for (i, name) in ["Default", "Soldier", "Merc", "Unarmed", "Siren", "Assassin", "Mechro"].iter().enumerate() {
        profiles.push(if i == 3 { x.profile(name, &bone_refs) } else { x.profile(name, &[]) });
    }
    let template = x.vm.space.alloc(x.node, "Template", None, Origin::Runtime).unwrap();
    x.set(template, "Profiles", Value::array(profiles));
    let copy = x.node(vec![], 0);
    x.set(copy, "TemplateNode", Value::Object(Some(template)));
    x.by_index(copy, 3).unwrap();
    assert_eq!(x.get(copy, "CurrentProfileIndex"), Value::Int(3));
    let required = x.bytes(copy, "RequiredBones");
    let mut wanted = std::collections::BTreeSet::new();
    for b in 12..23usize {
        let mut cur = Some(b);
        while let Some(c) = cur {
            wanted.insert(c);
            cur = parents[c];
        }
    }
    let got: std::collections::BTreeSet<usize> = required.iter().map(|&b| usize::from(b)).collect();
    assert_eq!(got, wanted);
    assert_eq!(required.len(), wanted.len(), "each once");
    for (i, &b) in required.iter().enumerate() {
        if let Some(p) = parents[usize::from(b)] {
            assert!(required[..i].contains(&u8::try_from(p).unwrap()), "{p} before {b}");
        }
    }
    let lut = x.bytes(copy, "AimCpntIndexLUT");
    assert_eq!(lut.len(), 27);
    assert_eq!(lut.iter().filter(|&&v| v != LUT_NONE).count(), 11);
    assert_eq!(lut[12], 0);
    assert_eq!(lut[22], 10);
}
