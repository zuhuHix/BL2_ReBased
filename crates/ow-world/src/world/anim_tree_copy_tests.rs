//! The animation tree's copy and initialisation on invented classes and objects (AGENTS.md rule 9: invented values;
//! the class, struct and field names are the packages'). Graded by the `VmError` variant and the `NotYet` kind. The
//! note is NATIVE_COPY_ANIM_TREE.md, accepted at 4a363c0c. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;
use crate::world::body_composition::{ANIM_LAYER, ANIM_NODES};

struct Fx {
    vm: Vm,
    child: StructId,
    group: StructId,
    list_head: StructId,
    n: u32,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn class(vm: &mut Vm, path: &str, base: ClassId, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, Some(base), props).unwrap()
}

fn strukt(vm: &mut Vm, path: &str, props: &[(&str, PropKind)]) -> StructId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define_struct(path, None, props).unwrap()
}

fn arr(k: PropKind) -> PropKind {
    PropKind::Array(Box::new(k))
}

/// What a fixture changes from the declared classes.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Good,
    /// The node class declares no `TickArrayIndex`.
    NoTickIndex,
}

impl Fx {
    fn new() -> Fx {
        Fx::with(Shape::Good)
    }

    fn with(shape: Shape) -> Fx {
        let vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        Fx::on(vm, shape)
    }

    /// The fixture's classes on `vm` (its `Core.Object` taken when a store already has it).
    fn on(mut vm: Vm, shape: Shape) -> Fx {
        let object = match vm.find_class("Core.Object") {
            Ok(c) => c,
            Err(_) => vm.space.classes.define("Core.Object", None, vec![]).unwrap(),
        };
        let o = PropKind::Object { class: object };
        let anim_object = class(&mut vm, "Engine.AnimObject", object, &[("SkelComponent", o.clone())]);
        let mut node_props = vec![
            ("NodeTickTag", PropKind::Int),
            ("ParentNodes", arr(o.clone())),
            ("NodeName", PropKind::Name),
        ];
        if shape != Shape::NoTickIndex {
            node_props.push(("TickArrayIndex", PropKind::Int));
        }
        let node = class(&mut vm, NODE, anim_object, &node_props);
        let child = strukt(&mut vm, "Engine.AnimNodeBlendBase.AnimBlendChild", &[("Name", PropKind::Name), ("Anim", o.clone())]);
        let blend = class(&mut vm, BLEND, node, &[("Children", arr(PropKind::Struct(child)))]);
        class(&mut vm, SEQUENCE, node, &[]);
        class(&mut vm, "Test.OtherBlend", blend, &[]);
        class(&mut vm, "GearboxFramework.AnimNodeSpecialMoveBlend", blend, &[]);
        class(&mut vm, AIM_OFFSET_CLASS, blend, &[]);
        let group = strukt(&mut vm, "Engine.AnimTree.AnimGroup", &[
            ("SeqNodes", arr(o.clone())),
            ("SynchMaster", o.clone()),
            ("NotifyMaster", o.clone()),
            ("GroupName", PropKind::Name),
        ]);
        let list_head = strukt(&mut vm, "Engine.AnimTree.SkelControlListHead", &[("BoneName", PropKind::Name), ("ControlHead", o.clone())]);
        class(&mut vm, "Engine.SkelControlBase", object, &[("ControlName", PropKind::Name), ("NextControl", o.clone())]);
        class(&mut vm, "Engine.MorphNodeBase", object, &[]);
        let tree = class(&mut vm, TREE, blend, &[
            ("AnimTreeTemplate", o.clone()),
            ("bParentNodeArrayBuilt", PropKind::Bool),
            ("bRebuildAnimTickArray", PropKind::Bool),
            ("AnimGroups", arr(PropKind::Struct(group))),
            ("RootMorphNodes", arr(o.clone())),
            ("SkelControlLists", arr(PropKind::Struct(list_head))),
            ("AnimTickArray", arr(o.clone())),
        ]);
        class(&mut vm, "Test.WillowAnimTree", tree, &[]);
        class(&mut vm, "Engine.SkeletalMeshComponent", object, &[
            ("AnimTreeTemplate", o.clone()),
            ("Animations", o.clone()),
            ("AnimTickArray", arr(o.clone())),
            ("TickTag", PropKind::Int),
            ("bAnimTreeInitialised", PropKind::Bool),
        ]);
        Fx { vm, child, group, list_head, n: 0 }
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        self.n += 1;
        let c = self.vm.space.classes.by_path(class).unwrap();
        let outer = self.vm.space.classes.by_path("Core.Object").unwrap();
        let pkg = self.vm.space.alloc(outer, &format!("TestPackage_{}", self.n), None, Origin::Runtime).unwrap();
        self.vm.space.alloc(c, &format!("Test_{}", self.n), Some(pkg), Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap_or_else(|| panic!("no {name}"));
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn objects(l: &[ObjRef]) -> Value {
        Value::array(l.iter().map(|o| Value::Object(Some(*o))).collect())
    }

    /// Sets `o`'s child slots to `children` (None an empty slot).
    fn children(&mut self, o: ObjRef, children: &[Option<ObjRef>]) {
        let ty = self.child;
        let slots = children
            .iter()
            .map(|c| {
                let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
                set_field(&self.vm, &mut v, "Anim", Value::Object(*c), "test").unwrap();
                v
            })
            .collect();
        self.set(o, "Children", Value::array(slots));
    }

    fn parents(&mut self, o: ObjRef, parents: &[ObjRef]) {
        self.set(o, "ParentNodes", Fx::objects(parents));
    }

    fn component(&mut self) -> ObjRef {
        self.obj("Engine.SkeletalMeshComponent")
    }

    fn list(v: &Value) -> Vec<Option<ObjRef>> {
        v.elements().unwrap().iter().map(|e| match e {
            Value::Object(o) => *o,
            other => panic!("{other:?}"),
        }).collect()
    }

    fn anims(&self, o: ObjRef) -> Vec<Option<ObjRef>> {
        let v = self.get(o, "Children");
        v.elements().unwrap().iter().map(|s| match field(&self.vm, s, "Anim", "t").unwrap() {
            Value::Object(o) => o,
            other => panic!("{other:?}"),
        }).collect()
    }

    fn outer(&self, o: ObjRef) -> Option<ObjRef> {
        self.vm.space.heap.get(o).unwrap().outer
    }

    fn name_of(&self, o: ObjRef) -> String {
        let n = self.vm.space.heap.get(o).unwrap().name;
        self.vm.space.names.text(n).to_string()
    }
}

/// The diamond: root → A; A → B, C; B → D; C → D (D has two parents). Saved parent lists, a built parent array, a
/// group with run-time state, one control list of two controls, and a saved tick list.
struct Diamond {
    root: ObjRef,
    a: ObjRef,
    b: ObjRef,
    c: ObjRef,
    d: ObjRef,
    k1: ObjRef,
    k2: ObjRef,
}

fn diamond(x: &mut Fx) -> Diamond {
    let root = x.obj("Test.WillowAnimTree");
    let a = x.obj(BLEND);
    let b = x.obj(BLEND);
    let c = x.obj(BLEND);
    let d = x.obj(SEQUENCE);
    x.children(root, &[Some(a)]);
    x.children(a, &[Some(b), None, Some(c)]);
    x.children(b, &[Some(d)]);
    x.children(c, &[Some(d)]);
    x.parents(a, &[root]);
    x.parents(b, &[a]);
    x.parents(c, &[a]);
    x.parents(d, &[b, c]);
    x.set(root, "bParentNodeArrayBuilt", Value::Bool(true));
    x.set(root, "AnimTickArray", Fx::objects(&[root, a, b, c, d]));
    let gt = x.group;
    let mut g = x.vm.space.classes.struct_defaults(gt).unwrap();
    set_field(&x.vm, &mut g, "SeqNodes", Fx::objects(&[d]), "t").unwrap();
    set_field(&x.vm, &mut g, "SynchMaster", Value::Object(Some(d)), "t").unwrap();
    set_field(&x.vm, &mut g, "NotifyMaster", Value::Object(Some(d)), "t").unwrap();
    x.set(root, "AnimGroups", Value::array(vec![g]));
    let k1 = x.obj("Engine.SkelControlBase");
    let k2 = x.obj("Engine.SkelControlBase");
    x.set(k1, "NextControl", Value::Object(Some(k2)));
    let lt = x.list_head;
    let mut l = x.vm.space.classes.struct_defaults(lt).unwrap();
    set_field(&x.vm, &mut l, "ControlHead", Value::Object(Some(k1)), "t").unwrap();
    x.set(root, "SkelControlLists", Value::array(vec![l]));
    Diamond { root, a, b, c, d, k1, k2 }
}

fn not_yet(e: VmError) -> ow_core::NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn refused(e: VmError, what: &str) {
    let n = not_yet(e);
    assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, RECORD));
}

/// A refusal of `instance`, with nothing written and nothing made.
fn refused_clean(x: &mut Fx, component: ObjRef, template: ObjRef) -> VmError {
    let (writes, objects) = (x.vm.space.heap.committed_writes(), x.vm.space.heap.handles().len());
    let e = instance(&mut x.vm, component, template).unwrap_err();
    assert_eq!((x.vm.space.heap.committed_writes(), x.vm.space.heap.handles().len()), (writes, objects), "write-free");
    e
}

#[test]
fn the_gather_order_is_depth_first_and_takes_a_node_once() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    let p = plan(&x.vm, comp, t.root).unwrap();
    assert_eq!(p.nodes, vec![t.a, t.b, t.d, t.c], "D under B, then C; the root excluded");
    assert_eq!(p.controls, vec![t.k1, t.k2]);
}

/// The diamond's tick order: the root, A, B; D waits for its second parent; then C, then D. Not the gather order.
#[test]
fn the_tick_list_appends_a_node_once_all_its_parents_are_listed() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    let tick = Fx::list(&x.get(comp, "AnimTickArray"));
    let names: Vec<String> = tick.iter().map(|o| x.name_of(o.unwrap())).collect();
    let want: Vec<String> = [t.root, t.a, t.b, t.c, t.d].iter().map(|o| x.name_of(*o)).collect();
    assert_eq!(names, want, "root, A, B, C, D");
    assert_eq!(tick[0], Some(root));
    for (i, n) in tick.iter().enumerate() {
        let n = n.unwrap();
        assert_eq!(x.get(n, "TickArrayIndex"), Value::Int(i32::try_from(i).unwrap()), "index {i}");
        assert_eq!(x.get(n, "SkelComponent"), Value::Object(Some(comp)));
        assert_eq!(x.get(n, "NodeTickTag"), Value::Int(1), "the component's counter, from 0 to 1");
    }
    assert_eq!(x.get(comp, "TickTag"), Value::Int(1));
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)));
    assert_eq!(x.get(comp, "bAnimTreeInitialised"), Value::Bool(true));
    assert_eq!(x.get(root, "AnimTickArray"), x.get(comp, "AnimTickArray"), "the tree's cache holds the built list");
}

#[test]
fn the_copy_keeps_class_and_name_takes_the_component_as_outer_and_rewires_links() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_ne!(root, t.root);
    assert_eq!(x.outer(root), Some(comp));
    assert_eq!(x.get(root, "AnimTreeTemplate"), Value::Object(Some(t.root)), "names the original");
    let a = x.anims(root)[0].unwrap();
    assert_ne!(a, t.a);
    assert_eq!((x.name_of(a), x.outer(a)), (x.name_of(t.a), Some(comp)));
    assert_eq!(x.vm.space.heap.get(a).unwrap().class, x.vm.space.heap.get(t.a).unwrap().class);
    let kids = x.anims(a);
    assert_eq!(kids.len(), 3);
    assert_eq!(kids[1], None, "an empty slot stays empty");
    let (b, c) = (kids[0].unwrap(), kids[2].unwrap());
    let d = x.anims(b)[0].unwrap();
    assert_eq!(x.anims(c)[0], Some(d), "one copy of D for both parents");
    assert_eq!(Fx::list(&x.get(d, "ParentNodes")), vec![Some(b), Some(c)]);
    assert_eq!(Fx::list(&x.get(a, "ParentNodes")), vec![Some(root)], "the root is in the map (port reading)");
    // The groups' run-time state is cleared; the controls are copied with their chain and the list head.
    let groups = x.get(root, "AnimGroups");
    let g = &groups.elements().unwrap()[0];
    assert_eq!(field(&x.vm, g, "SeqNodes", "t").unwrap(), Value::array(Vec::new()));
    assert_eq!(field(&x.vm, g, "SynchMaster", "t").unwrap(), Value::Object(None));
    let lists = x.get(root, "SkelControlLists");
    let k1 = match field(&x.vm, &lists.elements().unwrap()[0], "ControlHead", "t").unwrap() {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    assert_ne!(k1, t.k1);
    assert_eq!(x.outer(k1), Some(comp));
    let k2 = match x.get(k1, "NextControl") {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    assert_ne!(k2, t.k2);
    assert_eq!(x.get(k2, "NextControl"), Value::Object(None));
    // The template is untouched.
    assert_eq!(x.anims(t.root), vec![Some(t.a)]);
    assert_eq!(Fx::list(&x.get(t.d, "ParentNodes")), vec![Some(t.b), Some(t.c)]);
    assert_eq!(x.get(t.a, "SkelComponent"), Value::Object(None));
}

/// A copy of a copy names the original template.
#[test]
fn a_copy_of_a_copy_names_the_original() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    let first = instance(&mut x.vm, comp, t.root).unwrap();
    // A second component with another counter, so the first copy's tags (1) do not read as already listed.
    let other = x.component();
    x.set(other, "TickTag", Value::Int(5));
    let second = instance(&mut x.vm, other, first).unwrap();
    assert_eq!(x.get(second, "AnimTreeTemplate"), Value::Object(Some(t.root)));
}

/// Review r1 of #561, item 1: a copy of a copy onto a component whose next tick tag is the first copy's nodes' tag (both
/// components at 0) would list only the root; it is the tag-collision refusal, write-free.
#[test]
fn a_copy_whose_node_tags_collide_with_the_next_tick_tag_is_refused() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    let first = instance(&mut x.vm, comp, t.root).unwrap();
    let other = x.component();
    refused(refused_clean(&mut x, other, first), TAG_COLLISION);
}

/// Review r1 of #561, item 6: with `bAnimTreeInitialised` set (the call does not force it), the copy is made and
/// `Animations` set, and the initialisation is skipped: no tick list, no counter, no unread step refused.
#[test]
fn an_initialised_component_gets_the_copy_and_no_initialisation() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(false);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "bAnimTreeInitialised", Value::Bool(true));
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)));
    assert_eq!(x.get(comp, "TickTag"), Value::Int(0));
    assert_eq!(x.get(comp, "AnimTickArray"), Value::array(Vec::new()));
}

/// Review r1 of #561, item 2: the four unread steps are raised in order, the per-bone arrays as `LayerAbsent`.
#[test]
fn the_initialisation_s_unread_steps_refuse_in_order() {
    for (answered, what) in [(0, PER_NODE_INIT), (1, COMPONENT_PASS), (2, BONE_ARRAYS), (3, OWNER_EVENT)] {
        let mut x = Fx::new();
        answer_init_steps_up_to_for_tests(answered);
        let t = diamond(&mut x);
        let comp = x.component();
        let n = not_yet(refused_clean(&mut x, comp, t.root));
        assert_eq!((&*n.what, n.record), (what, RECORD), "{answered}");
        if what == BONE_ARRAYS {
            assert_eq!(n.missing, Missing::LayerAbsent(crate::world::body_composition::MESH_LAYER));
        } else {
            assert_eq!(n.missing, Missing::NotRead, "{answered}");
        }
    }
}

/// Review r1 of #561, item 3: a component that is not a skeletal mesh component is a script error before any write.
#[test]
fn a_component_that_is_not_skeletal_is_a_script_error() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let other = x.obj(BLEND);
    match refused_clean(&mut x, other, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("not a SkeletalMeshComponent"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// Review r1 of #561, item 5 (#557's chain rule): a loop within one chain is a script error; a control shared with an
/// earlier list keeps its first place.
#[test]
fn a_looping_control_chain_is_a_script_error_and_a_shared_control_keeps_its_place() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let lt = x.list_head;
    let head = |x: &Fx, c: ObjRef| {
        let mut l = x.vm.space.classes.struct_defaults(lt).unwrap();
        set_field(&x.vm, &mut l, "ControlHead", Value::Object(Some(c)), "t").unwrap();
        l
    };
    let lists = vec![head(&x, t.k1), head(&x, t.k2)];
    x.set(t.root, "SkelControlLists", Value::array(lists));
    let comp = x.component();
    assert_eq!(plan(&x.vm, comp, t.root).unwrap().controls, vec![t.k1, t.k2], "k2 keeps its place from the first list");
    x.set(t.k2, "NextControl", Value::Object(Some(t.k1)));
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("loops back"), "{message}"),
        e => panic!("{e:?}"),
    }
}

#[test]
fn the_per_node_initialisation_refuses_every_tree_before_any_write() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(false);
    let t = diamond(&mut x);
    let comp = x.component();
    refused(refused_clean(&mut x, comp, t.root), PER_NODE_INIT);
}

#[test]
fn a_node_class_whose_fix_up_was_not_read_is_refused() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let other = x.obj("Test.OtherBlend");
    x.children(t.c, &[Some(t.d), Some(other)]);
    x.parents(other, &[t.c]);
    let comp = x.component();
    refused(refused_clean(&mut x, comp, t.root), &fix_up("Test.OtherBlend"));
}

/// A class of Maya's tree with a checked, empty fix-up passes; the aim-offset node is its own refusal (her next stop).
#[test]
fn a_checked_class_passes_and_the_aim_offset_node_is_refused() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let slot = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    x.children(t.c, &[Some(t.d), Some(slot)]);
    x.parents(slot, &[t.c]);
    let comp = x.component();
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(Fx::list(&x.get(comp, "AnimTickArray")).len(), 6, "the checked class is copied and listed");
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)));
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let aim = x.obj(AIM_OFFSET_CLASS);
    x.children(t.c, &[Some(t.d), Some(aim)]);
    x.parents(aim, &[t.c]);
    let comp = x.component();
    refused(refused_clean(&mut x, comp, t.root), AIM_OFFSET);
}

/// The body composition's attach check: no tree has nothing to copy; a tree runs the copy's preflight, write-free
/// (here the aim-offset refusal, Maya's expected stop), with the component template standing for the component.
#[test]
fn the_attach_check_runs_the_copy_preflight_on_the_template_s_tree() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let template = x.component();
    attach_check(&x.vm, template).unwrap();
    x.set(template, "AnimTreeTemplate", Value::Object(Some(t.root)));
    attach_check(&x.vm, template).unwrap();
    let aim = x.obj(AIM_OFFSET_CLASS);
    x.children(t.c, &[Some(t.d), Some(aim)]);
    x.parents(aim, &[t.c]);
    let writes = x.vm.space.heap.committed_writes();
    refused(attach_check(&x.vm, template).unwrap_err(), AIM_OFFSET);
    assert_eq!(x.vm.space.heap.committed_writes(), writes);
}

#[test]
fn a_morph_node_is_refused() {
    let mut x = Fx::new();
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let m = x.obj("Engine.MorphNodeBase");
    x.set(t.root, "RootMorphNodes", Fx::objects(&[m]));
    let comp = x.component();
    refused(refused_clean(&mut x, comp, t.root), &fix_up("Engine.MorphNodeBase"));
}

/// A change made to the diamond before the run.
type Change = fn(&mut Fx, &Diamond, ObjRef);

/// The hub's preflight additions (#551 r1).
#[test]
fn the_hub_s_preflight_additions_refuse_before_any_write() {
    let cases: [(&str, Change); 3] = [
        (REBUILD, |x, t, _| x.set(t.root, "bRebuildAnimTickArray", Value::Bool(true))),
        (PARENTS, |x, t, _| x.set(t.root, "bParentNodeArrayBuilt", Value::Bool(false))),
        (ROOT_CHILDREN, |x, t, _| x.children(t.root, &[Some(t.a), None])),
    ];
    for (what, change) in cases {
        let mut x = Fx::new();
        answer_init_steps_for_tests(true);
        let t = diamond(&mut x);
        let comp = x.component();
        change(&mut x, &t, comp);
        refused(refused_clean(&mut x, comp, t.root), what);
    }
}

#[test]
fn a_missing_field_is_a_script_error_before_any_write() {
    let mut x = Fx::with(Shape::NoTickIndex);
    answer_init_steps_for_tests(true);
    let t = diamond(&mut x);
    let comp = x.component();
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("TickArrayIndex"), "{message}"),
        other => panic!("{other:?}"),
    }
    // A template that is not an AnimTree.
    let mut x = Fx::new();
    let n = x.obj(BLEND);
    let comp = x.component();
    assert!(matches!(refused_clean(&mut x, comp, n), VmError::Script { .. }));
}

// ---- An unloaded template (rule 1): #543's refusal, nothing loaded ------------------------------------------------

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Store {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn store() -> Store {
    let dir = std::env::temp_dir().join(format!("ow-world-animcopy-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        t.class("AnimTree", object);
        let holder = t.class("ToyTreeHolder", object);
        t.prop("Object", holder, "Tree", 0, object);
        t.class("SkeletalMeshComponent", object);
        t.build()
    };
    let templates = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let tree = t.p.add_import("Core", "Class", engine, "AnimTree");
        let holder = t.p.add_import("Core", "Class", engine, "ToyTreeHolder");
        let mut payload = ow_package::testkit::ints(&[0, 0]);
        payload.extend_from_slice(&t.none);
        let r = t.p.add_export(tree, "ToyTree", payload);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.object_tag("Tree", r));
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    let files: Vec<(&str, &[u8])> =
        vec![("Core.upk", core.as_slice()), ("Engine.upk", engine.as_slice()), ("GD_ToyTree.upk", templates.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    Store { vm, dir }
}

#[test]
fn an_unloaded_template_is_the_lookup_s_layer_absent_with_nothing_loaded() {
    let mut s = store();
    let path = ObjectPath::parse("GD_ToyTree.Holder").unwrap();
    let holder = s.vm.defs.find(&mut s.vm.space, &path).unwrap().unwrap();
    let class = s.vm.space.heap.get(holder).unwrap().class;
    let p = s.vm.space.prop(class, "Tree").unwrap();
    let tree = match s.vm.space.read(holder, PropAddr::scalar(p)).unwrap() {
        Value::Object(Some(o)) => *o,
        other => panic!("{other:?}"),
    };
    assert!(s.vm.defs.is_unloaded(&s.vm.space, tree));
    let comp_class = s.vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    let comp = s.vm.space.alloc(comp_class, "Comp", None, Origin::Runtime).unwrap();
    let (loads, writes) = (s.vm.defs.load_transitions(), s.vm.space.heap.committed_writes());
    let n = not_yet(instance(&mut s.vm, comp, tree).unwrap_err());
    assert_eq!(n.missing, Missing::LayerAbsent(ANIM_LAYER));
    assert_eq!(&*n.what, ANIM_NODES);
    assert!(!n.what.contains("partial writes"));
    assert_eq!((s.vm.defs.load_transitions(), s.vm.space.heap.committed_writes()), (loads, writes), "nothing loaded or written");
    assert!(s.vm.defs.is_unloaded(&s.vm.space, tree));
}

/// A store with two unloaded handles of a toy class (a node and a control stand-in), on which the fixture's classes are
/// then defined by hand.
fn handles_store() -> (Vm, DirGuard, ObjRef, ObjRef) {
    let dir = std::env::temp_dir().join(format!("ow-world-animcopy-h-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let classes = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        t.class("ToyHandle", object);
        let holder = t.class("ToyHandleHolder", object);
        t.prop("Object", holder, "Node", 0, object);
        t.prop("Object", holder, "Control", 0, object);
        t.build()
    };
    let data = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let pkg = t.p.add_import("Core", "Package", 0, "ToyClasses");
        let handle = t.p.add_import("Core", "Class", pkg, "ToyHandle");
        let holder = t.p.add_import("Core", "Class", pkg, "ToyHandleHolder");
        let mut payload = ow_package::testkit::ints(&[0, 0]);
        payload.extend_from_slice(&t.none);
        let node = t.p.add_export(handle, "ToyNode", payload.clone());
        let control = t.p.add_export(handle, "ToyControl", payload);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.object_tag("Node", node));
        payload.extend(t.p.object_tag("Control", control));
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    let files: Vec<(&str, &[u8])> =
        vec![("Core.upk", core.as_slice()), ("ToyClasses.upk", classes.as_slice()), ("GD_ToyHandles.upk", data.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("GD_ToyHandles.Holder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let class = vm.space.heap.get(holder).unwrap().class;
    let get = |vm: &Vm, n: &str| match vm.space.read(holder, PropAddr::scalar(vm.space.prop(class, n).unwrap())).unwrap() {
        Value::Object(Some(o)) => *o,
        other => panic!("{other:?}"),
    };
    let (node, control) = (get(&vm, "Node"), get(&vm, "Control"));
    assert!(vm.defs.is_unloaded(&vm.space, node) && vm.defs.is_unloaded(&vm.space, control));
    (vm, DirGuard(dir), node, control)
}

/// Review r1 of #561, item 8: in a loaded tree, an unloaded child node (reached by the gather) and an unloaded control
/// (reached through a chain) are rule 1's refusal, with nothing loaded or written.
#[test]
fn an_unloaded_node_or_control_in_a_loaded_tree_is_rule_1_s_refusal() {
    for which in ["node", "control"] {
        let (vm, _dir, node, control) = handles_store();
        let mut x = Fx::on(vm, Shape::Good);
        answer_init_steps_for_tests(true);
        let t = diamond(&mut x);
        if which == "node" {
            x.children(t.c, &[Some(t.d), Some(node)]);
        } else {
            x.set(t.k2, "NextControl", Value::Object(Some(control)));
        }
        let comp = x.component();
        let loads = x.vm.defs.load_transitions();
        let n = not_yet(refused_clean(&mut x, comp, t.root));
        assert_eq!((&*n.what, n.missing), (ANIM_NODES, Missing::LayerAbsent(ANIM_LAYER)), "{which}");
        assert_eq!(x.vm.defs.load_transitions(), loads, "{which}: nothing loaded");
    }
}

/// Removes a test directory when dropped.
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
