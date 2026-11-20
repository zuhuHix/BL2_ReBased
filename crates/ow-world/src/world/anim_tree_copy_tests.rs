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
    object: ClassId,
    /// The class `component()` makes: `Engine.SkeletalMeshComponent`, or on an owner store (whose Engine package
    /// declares it, with the two lookups and `bAnimTreeInitialised`) the hand-declared `Test.ToyMeshComponent` below it.
    component_class: &'static str,
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
    /// The component class declares no `bUpdateComposeSkeletonPasses` (lane IMPL-ANIM-NODE-INIT).
    NoComposeFlag,
    /// H's `AnimParameters` declared an array of ints (review r1 of #575, item 4).
    IntParameters,
    /// The Willow aim-offset class not derived from `Engine.AnimNodeAimOffset` (review r1 of #575, item 5).
    AimNoBase,
    /// `AnimBlendChild.Anim` declared interface-typed (review r2 of #575, item 2).
    InterfaceAnim,
    /// The component's `SkeletalMesh` declared an int (lane IMPL-PER-BONE-WEIGHTS).
    IntMesh,
    /// The feather blend's `BlendWeight` member declared an int (lane IMPL-PER-BONE-WEIGHTS).
    IntBlendWeight,
    /// The feather blend's target struct without `StartBoneName` (review r1 of #585, item 1).
    NoStartBone,
    /// The feather blend's `BlendWeight` member declared `float[2]` (review r2 of #585, item 1).
    ArrayBlendWeight,
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
        // The store's root class when it has one (the package-backed owner test's is GearboxFramework's).
        let object = match vm.find_class("Core.Object").or_else(|_| vm.find_class("GearboxFramework.Object")) {
            Ok(c) => c,
            Err(_) => vm.space.classes.define("Core.Object", None, vec![]).unwrap(),
        };
        let o = PropKind::Object { class: object };
        let anim_object = class(&mut vm, "Engine.AnimObject", object, &[("SkelComponent", o.clone())]);
        let mut node_props = vec![
            ("NodeTickTag", PropKind::Int),
            // The root's init tag source (lane IMPL-COPY-COMPONENT-PASS).
            ("NodeInitTag", PropKind::Int),
            ("ParentNodes", arr(o.clone())),
            ("NodeName", PropKind::Name),
            // The per-node initialisation's fields (lane IMPL-ANIM-NODE-INIT).
            ("bRelevant", PropKind::Bool),
            ("bJustBecameRelevant", PropKind::Bool),
            ("NodeTotalWeight", PropKind::Float),
            ("bCallScriptEventOnInit", PropKind::Bool),
        ];
        if shape != Shape::NoTickIndex {
            node_props.push(("TickArrayIndex", PropKind::Int));
        }
        let node = class(&mut vm, NODE, anim_object, &node_props);
        let anim = if shape == Shape::InterfaceAnim { PropKind::Interface { class: node } } else { o.clone() };
        let child = strukt(&mut vm, "Engine.AnimNodeBlendBase.AnimBlendChild", &[
            ("Name", PropKind::Name),
            ("Anim", anim),
            ("Weight", PropKind::Float),
            ("bIsAdditive", PropKind::Bool),
            ("AdditiveWeightScale", PropKind::Float),
        ]);
        let blend = class(&mut vm, BLEND, node, &[("Children", arr(PropKind::Struct(child)))]);
        class(&mut vm, SEQUENCE, node, &[
            ("AnimSeqName", PropKind::Name),
            ("AnimSeq", o.clone()),
            ("AnimLinkupIndex", PropKind::Int),
            ("CachedNumDesiredBones", PropKind::Int),
            ("bIsIssuingNotifies", PropKind::Bool),
            ("ActiveCameraAnimInstance", o.clone()),
        ]);
        class(&mut vm, "Test.OtherBlend", blend, &[]);
        class(&mut vm, "Engine.AnimNodeScaleRateBySpeed", blend, &[]);
        let weights = [("TargetWeight", arr(PropKind::Float)), ("ActiveChildIndex", PropKind::Int)];
        class(&mut vm, "Engine.AnimNodeBlendBySpeed", blend, &weights);
        // The per-bone weight builders' fields (lane IMPL-PER-BONE-WEIGHTS).
        let lists = [("Child2PerBoneWeight", arr(PropKind::Float)), ("LocalToCompReqBones", arr(PropKind::Byte { enum_: None }))];
        let mut per_bone = vec![("BranchStartBoneName", arr(PropKind::Name))];
        per_bone.extend(lists.iter().cloned());
        class(&mut vm, "Engine.AnimNodeBlendPerBone", blend, &per_bone);
        let weight = if shape == Shape::IntBlendWeight { PropKind::Int } else { PropKind::Float };
        let target_fields: Vec<(&str, PropKind)> = if shape == Shape::NoStartBone {
            vec![("BlendWeight", weight)]
        } else {
            vec![("StartBoneName", PropKind::Name), ("BlendWeight", weight)]
        };
        let target_path = "WillowGame.WillowAnimNodeFeatherBlend.BoneBlendTarget";
        let target = if shape == Shape::ArrayBlendWeight {
            let props = vec![
                decl(&mut vm, "StartBoneName", PropKind::Name),
                decl(&mut vm, "BlendWeight", PropKind::Float).with_array_dim(2),
            ];
            vm.space.classes.define_struct(target_path, None, props).unwrap()
        } else {
            strukt(&mut vm, target_path, &target_fields)
        };
        let mut feather = vec![("BoneBlendTargets", arr(PropKind::Struct(target))), ("bForceLocalSpaceBlend", PropKind::Bool)];
        feather.extend(lists.iter().cloned());
        class(&mut vm, "WillowGame.WillowAnimNodeFeatherBlend", blend, &feather);
        class(&mut vm, "Engine.SkeletalMesh", object, &[]);
        class(&mut vm, "Engine.AnimNodeAdditiveBlending", blend, &[]);
        let slot = class(&mut vm, "Engine.AnimNodeSlot", blend, &[("TargetWeight", arr(PropKind::Float)), ("SynchNode", o.clone())]);
        let params = strukt(&mut vm, "GearboxFramework.AnimNodeSpecialMoveBlend.SlotAnimParameters", &[("CustomAnimDef", o.clone())]);
        let smi = match vm.space.classes.by_path(crate::world::anim_node_init::SPECIAL_MOVE_INTERFACE) {
            Some(class) => PropKind::Interface { class },
            None => o.clone(),
        };
        class(&mut vm, "GearboxFramework.AnimNodeSpecialMoveBlend", slot, &[
            (
                "AnimParameters",
                if shape == Shape::IntParameters { arr(PropKind::Int) } else { arr(PropKind::Struct(params)) },
            ),
            // Interface-typed when the store holds the interface class (the package-backed owner test).
            ("SMI", smi),
        ]);
        let aim = class(&mut vm, "Engine.AnimNodeAimOffset", blend, &[("TemplateNode", o.clone()), ("Profiles", arr(o.clone()))]);
        if shape == Shape::AimNoBase {
            class(&mut vm, AIM_OFFSET_CLASS, blend, &[("TemplateNode", PropKind::Object { class: aim }), ("Profiles", arr(o.clone()))]);
        } else {
            class(&mut vm, AIM_OFFSET_CLASS, aim, &[]);
        }
        let group = strukt(&mut vm, "Engine.AnimTree.AnimGroup", &[
            ("SeqNodes", arr(o.clone())),
            ("SynchMaster", o.clone()),
            ("NotifyMaster", o.clone()),
            ("GroupName", PropKind::Name),
        ]);
        let list_head = strukt(&mut vm, "Engine.AnimTree.SkelControlListHead", &[("BoneName", PropKind::Name), ("ControlHead", o.clone())]);
        // The control fields of the per-bone control arrays (lane IMPL-COPY-COMPONENT-PASS).
        let control_props = [
            ("ControlName", PropKind::Name),
            ("NextControl", o.clone()),
            ("ControlTickTag", PropKind::Int),
            ("bPostPhysicsController", PropKind::Bool),
        ];
        class(&mut vm, "Engine.SkelControlBase", object, &control_props);
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
        class(&mut vm, "WillowGame.WillowAnimTree", tree, &[]);
        class(&mut vm, "Test.OtherTree", tree, &[]);
        let mut component_props = vec![
            ("AnimTreeTemplate", o.clone()),
            ("Animations", o.clone()),
            ("AnimTickArray", arr(o.clone())),
            ("TickTag", PropKind::Int),
            ("bAnimTreeInitialised", PropKind::Bool),
            // The initialisation's own gate (review r1 of #631, item 1).
            ("ParentAnimComponent", o.clone()),
            ("SkeletalMesh", if shape == Shape::IntMesh { PropKind::Int } else { o.clone() }),
            ("Owner", o.clone()),
            // The component pass and the per-bone control arrays (lane IMPL-COPY-COMPONENT-PASS).
            ("MorphSets", arr(o.clone())),
            ("SkelControlIndex", arr(PropKind::Byte { enum_: None })),
            ("PostPhysSkelControlIndex", arr(PropKind::Byte { enum_: None })),
            ("SkelControlTickArray", arr(o.clone())),
            ("InitTag", PropKind::Int),
            ("AnimTickRelevancyArray", arr(PropKind::Int)),
            ("AnimTickWeightsArray", arr(PropKind::Float)),
        ];
        if shape != Shape::NoComposeFlag {
            component_props.push(("bUpdateComposeSkeletonPasses", PropKind::Bool));
        }
        // An owner store's Engine declares the class (review r1 of #631, item 6: its event calls the two natives through
        // bytecode); the rest of the fields go on a hand-declared class below it.
        let component_class = match vm.find_class_if_present(SKELETAL).ok().flatten() {
            Some(package) => {
                component_props.retain(|(n, _)| *n != "bAnimTreeInitialised");
                class(&mut vm, "Test.ToyMeshComponent", package, &component_props);
                "Test.ToyMeshComponent"
            }
            None => {
                class(&mut vm, SKELETAL, object, &component_props);
                SKELETAL
            }
        };
        Fx { vm, child, group, list_head, n: 0, object, component_class }
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        self.n += 1;
        let c = self.vm.space.classes.by_path(class).unwrap();
        let outer = self.object;
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
                // An interface-typed link (the InterfaceAnim shape) holds an interface value.
                let interface = matches!(field(&self.vm, &v, "Anim", "t").unwrap(), Value::Interface(_));
                let link = if interface { Value::Interface(*c) } else { Value::Object(*c) };
                set_field(&self.vm, &mut v, "Anim", link, "test").unwrap();
                v
            })
            .collect();
        self.set(o, "Children", Value::array(slots));
    }

    fn parents(&mut self, o: ObjRef, parents: &[ObjRef]) {
        self.set(o, "ParentNodes", Fx::objects(parents));
    }

    fn component(&mut self) -> ObjRef {
        self.obj(self.component_class)
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
    let root = x.obj("WillowGame.WillowAnimTree");
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
    let counts = |x: &Fx| {
        (
            x.vm.space.heap.committed_writes(),
            x.vm.space.heap.handles().len(),
            x.vm.rng.draws(),
            x.vm.defs.load_transitions(),
            x.vm.space.names.len(),
        )
    };
    let before = counts(x);
    let e = instance(&mut x.vm, component, template).unwrap_err();
    assert_eq!(counts(x), before, "write-free: no heap write, no object, no draw, no load, no name interned");
    e
}

#[test]
fn the_gather_order_is_depth_first_and_takes_a_node_once() {
    let mut x = Fx::new();
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
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "bAnimTreeInitialised", Value::Bool(true));
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)));
    assert_eq!(x.get(comp, "TickTag"), Value::Int(0));
    assert_eq!(x.get(comp, "AnimTickArray"), Value::array(Vec::new()));
}

/// Review r1 of #561, item 3: a component that is not a skeletal mesh component is a script error before any write.
#[test]
fn a_component_that_is_not_skeletal_is_a_script_error() {
    let mut x = Fx::new();
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
fn a_node_class_whose_fix_up_was_not_read_is_refused() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let other = x.obj("Test.OtherBlend");
    x.children(t.c, &[Some(t.d), Some(other)]);
    x.parents(other, &[t.c]);
    let comp = x.component();
    refused(refused_clean(&mut x, comp, t.root), &fix_up("Test.OtherBlend"));
}

/// A class of Maya's tree with a checked, empty fix-up passes; the aim-offset node gets its fix-up (lane
/// IMPL-ANIM-NODE-INIT: `TemplateNode` its source, its own `Profiles` emptied).
#[test]
fn a_checked_class_passes_and_the_aim_offset_node_gets_its_fix_up() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let slot = x.obj("Engine.AnimNodeScaleRateBySpeed");
    x.children(t.c, &[Some(t.d), Some(slot)]);
    x.parents(slot, &[t.c]);
    let comp = x.component();
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(Fx::list(&x.get(comp, "AnimTickArray")).len(), 6, "the checked class is copied and listed");
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)));
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let aim = x.obj(AIM_OFFSET_CLASS);
    let profile = x.obj("Engine.SkelControlBase");
    x.set(aim, "Profiles", Fx::objects(&[profile]));
    x.children(t.c, &[Some(t.d), Some(aim)]);
    x.parents(aim, &[t.c]);
    let comp = x.component();
    instance(&mut x.vm, comp, t.root).unwrap();
    let copy = copy_of(&x, comp, aim);
    assert_eq!(x.get(copy, "TemplateNode"), Value::Object(Some(aim)), "the source node");
    assert_eq!(x.get(copy, "Profiles"), Value::array(Vec::new()), "the copy's own profiles emptied");
    assert_eq!(x.get(aim, "Profiles"), Fx::objects(&[profile]), "the template keeps its own");
}

/// The aim-offset fix-up does nothing when the copy's outer is its source's (the gate of NATIVE_ANIM_NODE_INIT.md
/// section 1): a source node outered to the component itself.
#[test]
fn the_aim_offset_fix_up_does_nothing_when_the_outers_match() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let comp = x.component();
    let class = x.vm.space.classes.by_path(AIM_OFFSET_CLASS).unwrap();
    let aim = x.vm.space.alloc(class, "SameOuterAim", Some(comp), Origin::Runtime).unwrap();
    let profile = x.obj("Engine.SkelControlBase");
    x.set(aim, "Profiles", Fx::objects(&[profile]));
    x.children(t.c, &[Some(t.d), Some(aim)]);
    x.parents(aim, &[t.c]);
    instance(&mut x.vm, comp, t.root).unwrap();
    let copy = copy_of(&x, comp, aim);
    assert_eq!(x.get(copy, "TemplateNode"), Value::Object(None));
    assert_eq!(x.get(copy, "Profiles"), Fx::objects(&[profile]));
}

/// The body composition's attach check: no tree has nothing to copy; a tree runs the copy's preflight, write-free
/// (here a fix-up refusal), with the component template standing for the component.
#[test]
fn the_attach_check_runs_the_copy_preflight_on_the_template_s_tree() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let template = x.component();
    attach_check(&x.vm, template, None).unwrap();
    x.set(template, "AnimTreeTemplate", Value::Object(Some(t.root)));
    attach_check(&x.vm, template, None).unwrap();
    let other = x.obj("Test.OtherBlend");
    x.children(t.c, &[Some(t.d), Some(other)]);
    x.parents(other, &[t.c]);
    let writes = x.vm.space.heap.committed_writes();
    refused(attach_check(&x.vm, template, None).unwrap_err(), &fix_up("Test.OtherBlend"));
    assert_eq!(x.vm.space.heap.committed_writes(), writes);
}

#[test]
fn a_morph_node_is_refused() {
    let mut x = Fx::new();
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
        let t = diamond(&mut x);
        let comp = x.component();
        change(&mut x, &t, comp);
        refused(refused_clean(&mut x, comp, t.root), what);
    }
}

#[test]
fn a_missing_field_is_a_script_error_before_any_write() {
    let mut x = Fx::with(Shape::NoTickIndex);
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

// ---- The per-node initialisation (NATIVE_ANIM_NODE_INIT.md section 2; lane IMPL-ANIM-NODE-INIT) -------------------

use crate::world::anim_node_init::{
    CAMERA, ENTRY_WEIGHTS, ON_INIT, OWNER_NOT_SMI, RECORD as INIT_RECORD, SEQUENCE_CHANGE, answer_owner_for_tests,
    set_active_child, unknown_class,
};

/// The copy of template node `n` on `comp`'s instance (by name, in the component's tick list).
fn copy_of(x: &Fx, comp: ObjRef, n: ObjRef) -> ObjRef {
    let name = x.name_of(n);
    Fx::list(&x.get(comp, "AnimTickArray"))
        .into_iter()
        .flatten()
        .find(|&o| x.name_of(o) == name && o != n)
        .unwrap_or_else(|| panic!("no copy of {name}"))
}

/// The diamond with `n` as C's second child (its one parent C), and a component.
fn with_node(x: &mut Fx, n: ObjRef) -> (Diamond, ObjRef) {
    let t = diamond(x);
    x.children(t.c, &[Some(t.d), Some(n)]);
    x.parents(n, &[t.c]);
    let comp = x.component();
    (t, comp)
}

/// Child slots of `o` with these `(anim, weight, additive, additive weight scale)`.
fn slots_of(x: &mut Fx, o: ObjRef, slots: &[(Option<ObjRef>, f32, bool, f32)]) {
    let ty = x.child;
    let list = slots
        .iter()
        .map(|&(a, w, additive, scale)| {
            let mut v = x.vm.space.classes.struct_defaults(ty).unwrap();
            for (f, val) in [
                ("Anim", Value::Object(a)),
                ("Weight", Value::Float(w)),
                ("bIsAdditive", Value::Bool(additive)),
                ("AdditiveWeightScale", Value::Float(scale)),
            ] {
                set_field(&x.vm, &mut v, f, val, "test").unwrap();
            }
            v
        })
        .collect();
    x.set(o, "Children", Value::array(list));
}

fn slot_field(x: &Fx, o: ObjRef, i: usize, f: &str) -> Value {
    field(&x.vm, &x.get(o, "Children").elements().unwrap()[i], f, "t").unwrap()
}

fn init_refused(e: VmError, what: &str) {
    let n = not_yet(e);
    assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, INIT_RECORD));
}

/// A and B: the relevance flags and the total weight cleared, the tick tag the component's; a child slot's additive
/// weight scale of exactly 0 becomes 1, another stays.
#[test]
fn a_and_b_write_their_fields() {
    let mut x = Fx::new();
    let n = x.obj(BLEND);
    let leaf = x.obj(NODE);
    x.parents(leaf, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(leaf), 1.0, false, 0.0), (None, 0.0, false, 0.5)]);
    for o in [n, leaf] {
        x.set(o, "bRelevant", Value::Bool(true));
        x.set(o, "NodeTotalWeight", Value::Float(5.0));
    }
    instance(&mut x.vm, comp, t.root).unwrap();
    let (c, l) = (copy_of(&x, comp, n), copy_of(&x, comp, leaf));
    for o in [c, l] {
        assert_eq!((x.get(o, "bRelevant"), x.get(o, "NodeTotalWeight")), (Value::Bool(false), Value::Float(0.0)));
        assert_eq!(x.get(o, "NodeTickTag"), x.get(comp, "TickTag"));
    }
    assert_eq!(slot_field(&x, c, 0, "AdditiveWeightScale"), Value::Float(1.0));
    assert_eq!(slot_field(&x, c, 1, "AdditiveWeightScale"), Value::Float(0.5));
}

/// Refusal 1: a node with `bCallScriptEventOnInit` set.
#[test]
fn a_node_with_the_on_init_event_is_refused() {
    let mut x = Fx::new();
    let n = x.obj(NODE);
    let (t, comp) = with_node(&mut x, n);
    x.set(n, "bCallScriptEventOnInit", Value::Bool(true));
    init_refused(refused_clean(&mut x, comp, t.root), ON_INIT);
}

/// C: `TargetWeight` rebuilt to one entry per child, the first 1.0; refusal 3 when the non-additive weights sum to
/// at most 1e-5 (an additive slot does not count).
#[test]
fn c_rebuilds_its_target_weights_and_refuses_a_zero_sum() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendBySpeed");
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(None, 1.0, false, 1.0), (None, 0.0, false, 1.0)]);
    instance(&mut x.vm, comp, t.root).unwrap();
    let c = copy_of(&x, comp, n);
    assert_eq!(x.get(c, "TargetWeight"), Value::array(vec![Value::Float(1.0), Value::Float(0.0)]));
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendBySpeed");
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(None, 1.0, true, 1.0), (None, 0.0, false, 1.0)]);
    init_refused(refused_clean(&mut x, comp, t.root), &set_active_child("Engine.AnimNodeBlendBySpeed"));
}

/// D: the name kept, the sequence unset, the linkup index -1, the bone count 0; with notifies issuing, the set-anim
/// does nothing. Refusal 2 for a set sequence (notifies clear) and for a camera animation.
#[test]
fn d_keeps_its_name_and_unbinds_its_sequence() {
    let mut x = Fx::new();
    let walk = Value::Name(x.vm.space.names.intern("Walk"));
    let n = x.obj(SEQUENCE);
    let (t, comp) = with_node(&mut x, n);
    x.set(n, "AnimSeqName", walk.clone());
    x.set(n, "AnimLinkupIndex", Value::Int(4));
    x.set(n, "CachedNumDesiredBones", Value::Int(9));
    instance(&mut x.vm, comp, t.root).unwrap();
    let c = copy_of(&x, comp, n);
    assert_eq!(x.get(c, "AnimSeqName"), walk);
    assert_eq!((x.get(c, "AnimLinkupIndex"), x.get(c, "CachedNumDesiredBones")), (Value::Int(-1), Value::Int(0)));
    // Issuing notifies: the set-anim does nothing (a set sequence is kept, the linkup index too).
    let mut x = Fx::new();
    let n = x.obj(SEQUENCE);
    let seq = x.obj(NODE);
    let (t, comp) = with_node(&mut x, n);
    x.set(n, "bIsIssuingNotifies", Value::Bool(true));
    x.set(n, "AnimSeq", Value::Object(Some(seq)));
    x.set(n, "AnimLinkupIndex", Value::Int(4));
    instance(&mut x.vm, comp, t.root).unwrap();
    let c = copy_of(&x, comp, n);
    assert_eq!((x.get(c, "AnimSeq"), x.get(c, "AnimLinkupIndex")), (Value::Object(Some(seq)), Value::Int(4)));
    for (what, f) in [(SEQUENCE_CHANGE, "AnimSeq"), (CAMERA, "ActiveCameraAnimInstance")] {
        let mut x = Fx::new();
        let n = x.obj(SEQUENCE);
        let other = x.obj(NODE);
        let (t, comp) = with_node(&mut x, n);
        x.set(n, f, Value::Object(Some(other)));
        init_refused(refused_clean(&mut x, comp, t.root), what);
    }
}

/// E: the component's fitted flag set (UNVERIFIED); a component class that does not declare it is a script error.
#[test]
fn e_sets_the_component_flag_and_needs_it_declared() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let comp = x.component();
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "bUpdateComposeSkeletonPasses"), Value::Bool(true));
    let mut x = Fx::with(Shape::NoComposeFlag);
    let t = diamond(&mut x);
    let comp = x.component();
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("bUpdateComposeSkeletonPasses"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// F and G: with no mesh, F builds nothing and both set the first child's weight to 1; with a mesh, F is #529's
/// `MESH_LAYER` refusal.
#[test]
fn f_and_g_set_the_first_weight_and_f_needs_the_mesh_layer() {
    for class in ["Engine.AnimNodeBlendPerBone", "Engine.AnimNodeAdditiveBlending"] {
        let mut x = Fx::new();
        let n = x.obj(class);
        let (t, comp) = with_node(&mut x, n);
        slots_of(&mut x, n, &[(None, 0.25, false, 1.0)]);
        instance(&mut x.vm, comp, t.root).unwrap();
        assert_eq!(slot_field(&x, copy_of(&x, comp, n), 0, "Weight"), Value::Float(1.0), "{class}");
    }
    // With a mesh and no skeleton source installed, #569's layer refusal passes through unchanged (lane
    // IMPL-PER-BONE-WEIGHTS).
    let mut x = Fx::new();
    crate::world::per_bone_weights::skeleton_for_tests(None);
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_node(&mut x, n);
    let mesh = x.obj("Engine.SkeletalMesh");
    x.set(comp, "SkeletalMesh", Value::Object(Some(mesh)));
    let e = not_yet(refused_clean(&mut x, comp, t.root));
    assert_eq!(e.missing, Missing::LayerAbsent(crate::world::body_composition::MESH_LAYER));
    assert_eq!(e.record, crate::world::skeletal::RECORD, "the layer's own refusal");
}

/// H: the slot part (the children past the first initialised again, `TargetWeight`, the first weight, `SynchNode`)
/// and the special-move part (`AnimParameters`, `SMI` the owner when it implements the interface).
#[test]
fn h_writes_its_slot_and_special_move_parts() {
    let mut x = Fx::new();
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, h1) = (x.obj(NODE), x.obj(NODE));
    x.parents(h0, &[n]);
    x.parents(h1, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(h0), 0.2, false, 1.0), (Some(h1), 0.3, false, 1.0)]);
    x.set(n, "SynchNode", Value::Object(Some(h0)));
    let owner = x.obj(NODE);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    answer_owner_for_tests(Some(true));
    instance(&mut x.vm, comp, t.root).unwrap();
    answer_owner_for_tests(None);
    let c = copy_of(&x, comp, n);
    assert_eq!(slot_field(&x, c, 0, "Weight"), Value::Float(1.0 - 0.3), "1 − the others' non-additive weights");
    assert_eq!(x.get(c, "TargetWeight"), Value::array(vec![Value::Float(1.0), Value::Float(0.0)]));
    assert_eq!(x.get(c, "SynchNode"), Value::Object(None));
    assert_eq!(x.get(c, "AnimParameters").elements().map(<[Value]>::len), Some(2), "one zeroed entry per child");
    assert_eq!(x.get(c, "SMI"), Value::Object(Some(owner)));
    let c1 = copy_of(&x, comp, h1);
    assert_eq!(x.get(c1, "SkelComponent"), Value::Object(Some(comp)), "re-initialised");
}

/// H's refusals: an entry past the first naming a `CustomAnimDef` (6), and an owner not known to implement the
/// interface (7: hand-built classes are not in the code index, so the side-effect-free check cannot answer yes).
#[test]
fn h_refuses_an_entry_weight_build_and_an_unknown_owner() {
    let setup = |x: &mut Fx| {
        let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
        let (h0, h1) = (x.obj(NODE), x.obj(NODE));
        x.parents(h0, &[n]);
        x.parents(h1, &[n]);
        let (t, comp) = with_node(x, n);
        slots_of(x, n, &[(Some(h0), 1.0, false, 1.0), (Some(h1), 0.0, false, 1.0)]);
        (t, comp, n)
    };
    let mut x = Fx::new();
    let (t, comp, n) = setup(&mut x);
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "AnimParameters").unwrap();
    let PropKind::Array(inner) = x.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
    let PropKind::Struct(ty) = *inner else { panic!() };
    let def = x.obj(NODE);
    let mut entry = x.vm.space.classes.struct_defaults(ty).unwrap();
    set_field(&x.vm, &mut entry, "CustomAnimDef", Value::Object(Some(def)), "t").unwrap();
    let plain = x.vm.space.classes.struct_defaults(ty).unwrap();
    x.set(n, "AnimParameters", Value::array(vec![plain, entry]));
    init_refused(refused_clean(&mut x, comp, t.root), ENTRY_WEIGHTS);
    let mut x = Fx::new();
    let (t, comp, _) = setup(&mut x);
    let owner = x.obj(NODE);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    answer_owner_for_tests(None);
    init_refused(refused_clean(&mut x, comp, t.root), OWNER_NOT_SMI);
}

/// A root (never fix-up checked) whose exact class is not in the table refuses by name.
#[test]
fn an_unknown_node_class_refuses_by_name() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let class = x.vm.space.classes.by_path("Test.OtherTree").unwrap();
    let root = x.vm.space.alloc(class, "OtherRoot", None, Origin::Runtime).unwrap();
    for f in ["Children", "SkelControlLists", "AnimTickArray", "AnimGroups"] {
        let v = x.get(t.root, f);
        x.set(root, f, v);
    }
    x.set(root, "bParentNodeArrayBuilt", Value::Bool(true));
    x.parents(t.a, &[root]);
    let comp = x.component();
    init_refused(refused_clean(&mut x, comp, root), &unknown_class("Test.OtherTree"));
}

// ---- Review r1 of #575 ----------------------------------------------------------------------------------------------

/// An H node in C's second slot with `h1` in its own second slot; `h1`'s one parent is `outside`, an object outside the
/// tree, so `h1` is on the tick list only when `outside`'s tag is the component's next tag.
fn h_with_outside_parent(x: &mut Fx, outside_tag: i32) -> (Diamond, ObjRef, ObjRef, ObjRef) {
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, h1) = (x.obj(NODE), x.obj(NODE));
    let outside = x.obj(NODE);
    x.set(outside, "NodeTickTag", Value::Int(outside_tag));
    x.parents(h0, &[n]);
    x.parents(h1, &[outside]);
    let (t, comp) = with_node(x, n);
    slots_of(x, n, &[(Some(h0), 1.0, false, 1.0), (Some(h1), 0.0, false, 1.0)]);
    (t, comp, n, h1)
}

/// Item 1 (and 6): a child H re-initialises that is not on the tick list (its parent outside the tree, untagged) is
/// still initialised at H's step 2, and the preflight checks it: its `OnInit` refusal fires.
#[test]
fn h_initialises_and_the_preflight_checks_a_child_off_the_tick_list() {
    let mut x = Fx::new();
    let (t, comp, _, h1) = h_with_outside_parent(&mut x, 0);
    x.set(h1, "bRelevant", Value::Bool(true));
    instance(&mut x.vm, comp, t.root).unwrap();
    let listed = Fx::list(&x.get(comp, "AnimTickArray"));
    let name = x.name_of(h1);
    assert!(listed.iter().flatten().all(|&o| x.name_of(o) != name), "h1 is not on the tick list");
    // Its copy: the one whose outer is the component, named as h1.
    let copy = x
        .vm
        .space
        .heap
        .handles()
        .into_iter()
        .find(|&o| o != h1 && x.name_of(o) == name && x.outer(o) == Some(comp))
        .expect("h1's copy");
    assert_eq!(x.get(copy, "SkelComponent"), Value::Object(Some(comp)), "H's step 2 call");
    assert_eq!(x.get(copy, "bRelevant"), Value::Bool(false), "initialised by H's step 2 only");
    let mut x = Fx::new();
    let (t, comp, _, h1) = h_with_outside_parent(&mut x, 0);
    x.set(h1, "bCallScriptEventOnInit", Value::Bool(true));
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    assert_eq!(&*n.what, ON_INIT);
}

/// Item 1: a parent outside the tree whose tag is the component's next tag lists its child in both the preflight's and
/// the writes' tick list (they agree, or the writes refuse).
#[test]
fn an_outside_parent_with_the_next_tag_lists_its_child() {
    let mut x = Fx::new();
    let (t, comp, _, h1) = h_with_outside_parent(&mut x, 1);
    instance(&mut x.vm, comp, t.root).unwrap();
    let name = x.name_of(h1);
    assert!(Fx::list(&x.get(comp, "AnimTickArray")).iter().flatten().any(|&o| x.name_of(o) == name));
}

/// Item 3: an H node whose second slot leads back to itself is a script error in the preflight, write-free.
#[test]
fn a_slot_whose_children_lead_back_to_it_is_a_script_error() {
    let mut x = Fx::new();
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let h0 = x.obj(NODE);
    x.parents(h0, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(h0), 1.0, false, 1.0), (Some(n), 0.0, false, 1.0)]);
    x.parents(n, &[t.c, t.root]);
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("lead back"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// Item 4: an `AnimParameters` that is not an array of a defined struct is a script error in the preflight.
#[test]
fn h_parameters_of_another_element_are_a_script_error_before_any_write() {
    let mut x = Fx::with(Shape::IntParameters);
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, h1) = (x.obj(NODE), x.obj(NODE));
    x.parents(h0, &[n]);
    x.parents(h1, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(h0), 1.0, false, 1.0), (Some(h1), 0.0, false, 1.0)]);
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("AnimParameters"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// Item 5: an aim-offset node not derived from `Engine.AnimNodeAimOffset` fails the fix-up's checked cast in the
/// preflight, before the copy is made.
#[test]
fn an_aim_offset_without_its_engine_base_fails_the_checked_cast_before_any_write() {
    let mut x = Fx::with(Shape::AimNoBase);
    let aim = x.obj(AIM_OFFSET_CLASS);
    let (t, comp) = with_node(&mut x, aim);
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("checked cast"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// Item 6: H's owner test on a package-backed owner whose class implements `SpecialMoveInterface`, its interface
/// entries resolved: `SMI` (interface-typed) is the owner, with no test switch.
#[test]
fn h_takes_a_package_backed_owner_that_implements_the_interface() {
    use ow_object::class_body::CLASS_INTERFACE;
    use ow_package::testkit::reflect::{ClassBodySpec, Declarations, class_body};
    let dir = std::env::temp_dir().join(format!("ow-world-animcopy-smi-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _dir = DirGuard(dir.clone());
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let iface = d.real_class("SpecialMoveInterface", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    d.real_class("ToyOwner", obj, obj, 2, &[iface], None);
    let bytes = d.build();
    let defs = DefStore::from_synthetic(&dir, &[("GearboxFramework.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let iface = vm.find_class(crate::world::anim_node_init::SPECIAL_MOVE_INTERFACE).unwrap();
    let owner_class = vm.find_class("GearboxFramework.ToyOwner").unwrap();
    // The interface entries resolved before the run (as a loaded pawn class's would be).
    assert!(vm.implements(owner_class, iface));
    let owner = vm.space.alloc(owner_class, "Owner_0", None, Origin::Runtime).unwrap();
    let mut x = Fx::on(vm, Shape::Good);
    answer_owner_for_tests(None);
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, h1) = (x.obj(NODE), x.obj(NODE));
    x.parents(h0, &[n]);
    x.parents(h1, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(h0), 1.0, false, 1.0), (Some(h1), 0.0, false, 1.0)]);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(copy_of(&x, comp, n), "SMI"), Value::Interface(Some(owner)));
}

// ---- Review r2 of #575 ----------------------------------------------------------------------------------------------

/// Item 1: H's new `AnimParameters` entries are zeroed, not the element struct's defaults (here a nonzero default
/// naming a `CustomAnimDef`).
#[test]
fn h_makes_zeroed_parameter_entries_whatever_the_struct_defaults() {
    let mut x = Fx::new();
    let n = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, h1) = (x.obj(NODE), x.obj(NODE));
    x.parents(h0, &[n]);
    x.parents(h1, &[n]);
    let (t, comp) = with_node(&mut x, n);
    slots_of(&mut x, n, &[(Some(h0), 1.0, false, 1.0), (Some(h1), 0.0, false, 1.0)]);
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "AnimParameters").unwrap();
    let PropKind::Array(inner) = x.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
    let PropKind::Struct(ty) = *inner else { panic!() };
    let def = x.obj(NODE);
    let mut default = x.vm.space.classes.struct_defaults(ty).unwrap();
    set_field(&x.vm, &mut default, "CustomAnimDef", Value::Object(Some(def)), "t").unwrap();
    x.vm.space.set_struct_defaults(ty, default).unwrap();
    instance(&mut x.vm, comp, t.root).unwrap();
    let entries = x.get(copy_of(&x, comp, n), "AnimParameters");
    let entries = entries.elements().unwrap();
    assert_eq!(entries.len(), 2);
    for e in entries {
        assert_eq!(field(&x.vm, e, "CustomAnimDef", "t").unwrap(), Value::Object(None), "zeroed, not the default");
    }
}

/// Item 2: an interface-typed child link is a script error in the preflight, write-free (the copy remaps object links
/// only, so it would be planned and walked differently).
#[test]
fn an_interface_typed_child_link_is_a_script_error_before_any_write() {
    let mut x = Fx::with(Shape::InterfaceAnim);
    let t = diamond(&mut x);
    let comp = x.component();
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("not an object reference"), "{message}"),
        e => panic!("{e:?}"),
    }
}

// ---- The per-bone weight builders (NATIVE_PER_BONE_WEIGHTS.md; lane IMPL-PER-BONE-WEIGHTS) ------------------------

use crate::world::per_bone_weights::{skeleton_for_tests, too_many_bones};
use crate::world::skeletal::{ReferenceSkeleton, SkeletonBone};

/// An invented skeleton: bone `i` named `B<i>`, with these parents.
fn skeleton(parents: &[Option<usize>]) -> ReferenceSkeleton {
    ReferenceSkeleton {
        bones: parents.iter().enumerate().map(|(i, &parent)| SkeletonBone { name: format!("B{i}"), parent, bind: None }).collect(),
    }
}

/// Root 0; B1 under 0; B2 under 1; B3 under 2; B4 under 1.
fn five() -> ReferenceSkeleton {
    skeleton(&[None, Some(0), Some(1), Some(2), Some(1)])
}

fn names(x: &mut Fx, list: &[&str]) -> Value {
    Value::array(list.iter().map(|n| Value::Name(x.vm.space.names.intern(n))).collect())
}

fn floats(v: &Value) -> Vec<f32> {
    v.elements().unwrap().iter().map(|e| match e { Value::Float(f) => *f, o => panic!("{o:?}") }).collect()
}

fn bytes(v: &Value) -> Vec<u8> {
    v.elements().unwrap().iter().map(|e| match e { Value::Byte(b) => *b, o => panic!("{o:?}") }).collect()
}

/// The diamond with F node `n` in C's second slot, a component holding a mesh, and the invented skeleton.
fn with_mesh(x: &mut Fx, n: ObjRef, s: ReferenceSkeleton) -> (Diamond, ObjRef) {
    let (t, comp) = with_node(x, n);
    let mesh = x.obj("Engine.SkeletalMesh");
    x.set(comp, "SkeletalMesh", Value::Object(Some(mesh)));
    skeleton_for_tests(Some(s));
    (t, comp)
}

/// The per-bone builder: one branch bone's weight passes down its chain; the list holds the bones whose weight changes,
/// with their missing ancestors inserted ahead of them; the first child's weight is set after.
#[test]
fn the_per_bone_builder_weights_a_branch_and_lists_its_ancestors() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_mesh(&mut x, n, five());
    let branch = names(&mut x, &["b2", "NoSuchBone", "None"]);
    x.set(n, "BranchStartBoneName", branch);
    slots_of(&mut x, n, &[(None, 0.25, false, 1.0)]);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let c = copy_of(&x, comp, n);
    assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), vec![0.0, 0.0, 1.0, 1.0, 0.0], "B2 and its child B3");
    assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), vec![0, 1, 2], "B2, after its ancestors B1 and the root");
    assert_eq!(slot_field(&x, c, 0, "Weight"), Value::Float(1.0));
}

/// The root as a branch bone: every weight 1, no bone differs from its parent, so the list is empty.
#[test]
fn the_root_as_branch_gives_an_empty_list() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_mesh(&mut x, n, five());
    let branch = names(&mut x, &["B0", "B0"]);
    x.set(n, "BranchStartBoneName", branch);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let c = copy_of(&x, comp, n);
    assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), vec![1.0; 5]);
    assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), Vec::<u8>::new());
}

/// The feather builder: the clamp (negative, above 1, not a number), the first entry for a bone winning, a partial
/// weight inherited unchanged still listed; with `bForceLocalSpaceBlend` the list stays empty.
#[test]
fn the_feather_builder_clamps_lists_partial_weights_and_obeys_local_space() {
    for force in [false, true] {
        let mut x = Fx::new();
        let n = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
        let (t, comp) = with_mesh(&mut x, n, five());
        let class = x.vm.space.heap.get(n).unwrap().class;
        let p = x.vm.space.prop(class, "BoneBlendTargets").unwrap();
        let PropKind::Array(inner) = x.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
        let PropKind::Struct(ty) = *inner else { panic!() };
        let entry = |x: &mut Fx, bone: &str, w: f32| {
            let mut v = x.vm.space.classes.struct_defaults(ty).unwrap();
            let name = Value::Name(x.vm.space.names.intern(bone));
            set_field(&x.vm, &mut v, "StartBoneName", name, "t").unwrap();
            set_field(&x.vm, &mut v, "BlendWeight", Value::Float(w), "t").unwrap();
            v
        };
        let targets = vec![entry(&mut x, "B1", 0.5), entry(&mut x, "B1", 0.9), entry(&mut x, "B3", 7.0), entry(&mut x, "B4", -2.0)];
        x.set(n, "BoneBlendTargets", Value::array(targets));
        x.set(n, "bForceLocalSpaceBlend", Value::Bool(force));
        instance(&mut x.vm, comp, t.root).unwrap();
        skeleton_for_tests(None);
        let c = copy_of(&x, comp, n);
        // B1 0.5 (the first entry), B2 inherits 0.5, B3 clamped to 1, B4 clamped to 0.
        assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), vec![0.0, 0.5, 0.5, 1.0, 0.0], "force {force}");
        let want: Vec<u8> = if force { Vec::new() } else { vec![0, 1, 2, 3, 4] };
        // B1 differs from the root; B2 equals B1 at a partial weight (listed); B3 differs; B4 (0) differs from its
        // parent B1 (0.5); the parents pass puts the root first.
        assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), want, "force {force}");
    }
    assert_eq!(crate::world::per_bone_weights::clamp(f32::NAN), 0.0);
}

/// The parents pass inserts a missing ancestor chain ahead of its bone, each looked at in turn.
#[test]
fn the_parents_pass_inserts_the_missing_chain() {
    let s = five();
    let mut list = vec![3, 4];
    crate::world::per_bone_weights::parents_pass(&s, &mut list);
    assert_eq!(list, vec![0, 1, 2, 3, 4]);
}

/// A null mesh builds nothing: both arrays keep the template's values, and the first child's weight is still set.
#[test]
fn a_null_mesh_keeps_both_arrays() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_node(&mut x, n);
    x.set(n, "Child2PerBoneWeight", Value::array(vec![Value::Float(0.25)]));
    x.set(n, "LocalToCompReqBones", Value::array(vec![Value::Byte(7)]));
    instance(&mut x.vm, comp, t.root).unwrap();
    let c = copy_of(&x, comp, n);
    assert_eq!(x.get(c, "Child2PerBoneWeight"), Value::array(vec![Value::Float(0.25)]));
    assert_eq!(x.get(c, "LocalToCompReqBones"), Value::array(vec![Value::Byte(7)]));
}

/// More than 256 bones, a mesh field of another kind and a target member of another kind: each refused, write-free.
#[test]
fn the_builder_s_refusals_are_write_free() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let mut parents = vec![None];
    parents.extend((0..256).map(Some));
    let (t, comp) = with_mesh(&mut x, n, skeleton(&parents));
    let e = not_yet(refused_clean(&mut x, comp, t.root));
    skeleton_for_tests(None);
    assert_eq!((&*e.what, e.missing), (too_many_bones("Engine.AnimNodeBlendPerBone").as_str(), Missing::NotRead));
    let mut x = Fx::with(Shape::IntMesh);
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_node(&mut x, n);
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("SkeletalMesh"), "{message}"),
        e => panic!("{e:?}"),
    }
    let mut x = Fx::with(Shape::IntBlendWeight);
    let n = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
    let (t, comp) = with_mesh(&mut x, n, five());
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "BoneBlendTargets").unwrap();
    let PropKind::Array(inner) = x.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
    let PropKind::Struct(ty) = *inner else { panic!() };
    let v = x.vm.space.classes.struct_defaults(ty).unwrap();
    x.set(n, "BoneBlendTargets", Value::array(vec![v]));
    let r = refused_clean(&mut x, comp, t.root);
    skeleton_for_tests(None);
    match r {
        VmError::Script { message, .. } => assert!(message.contains("BlendWeight"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// An F node in an H node's second slot is built at H's nested initialisation and again at its own place: the writes
/// land, and both builds agree.
#[test]
fn an_f_node_under_h_is_built_at_both_initialisations() {
    let mut x = Fx::new();
    let h = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, f) = (x.obj(NODE), x.obj("Engine.AnimNodeBlendPerBone"));
    x.parents(h0, &[h]);
    x.parents(f, &[h]);
    let (t, comp) = with_mesh(&mut x, h, five());
    slots_of(&mut x, h, &[(Some(h0), 1.0, false, 1.0), (Some(f), 0.0, false, 1.0)]);
    let branch = names(&mut x, &["B4"]);
    x.set(f, "BranchStartBoneName", branch);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let c = copy_of(&x, comp, f);
    assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), vec![0.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), vec![0, 1, 4]);
}

// ---- Review r1 of #585 ----------------------------------------------------------------------------------------------

/// Item 2: the F node under H has two scheduled heads (H's nested initialisation and its own), each carrying its own
/// build planned in the preflight; the writes replay them.
#[test]
fn both_f_heads_carry_their_planned_builds() {
    let mut x = Fx::new();
    let h = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    let (h0, f) = (x.obj(NODE), x.obj("Engine.AnimNodeBlendPerBone"));
    x.parents(h0, &[h]);
    x.parents(f, &[h]);
    let (t, comp) = with_mesh(&mut x, h, five());
    slots_of(&mut x, h, &[(Some(h0), 1.0, false, 1.0), (Some(f), 0.0, false, 1.0)]);
    let branch = names(&mut x, &["B4"]);
    x.set(f, "BranchStartBoneName", branch);
    let p = plan(&x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    use crate::world::anim_node_init::Step;
    use crate::world::per_bone_weights::Build;
    let heads: Vec<Option<usize>> =
        p.steps.iter().filter_map(|s| match s { Step::Head(n, b) if *n == f => Some(*b), _ => None }).collect();
    assert_eq!(heads.len(), 2, "two heads: H's nested initialisation and the node's own");
    let want = Build::Write(vec![0.0, 0.0, 0.0, 0.0, 1.0], vec![0, 1, 4]);
    let heads: Vec<usize> = heads.into_iter().map(|b| b.expect("a planned build")).collect();
    // Review r2 of #585, item 2: each head has its own build, and each one's replay writes both arrays by itself, with
    // the skeleton source gone (the replay reads only the preflight's arrays).
    assert_ne!(heads[0], heads[1], "two builds, not one shared");
    for b in heads {
        assert_eq!(p.builds[b], want);
        x.set(f, "Child2PerBoneWeight", Value::array(Vec::new()));
        x.set(f, "LocalToCompReqBones", Value::array(Vec::new()));
        crate::world::anim_node_init::write_head(&mut x.vm, comp, f, Some(&p.builds[b])).unwrap();
        assert_eq!(floats(&x.get(f, "Child2PerBoneWeight")), vec![0.0, 0.0, 0.0, 0.0, 1.0], "head build {b}");
        assert_eq!(bytes(&x.get(f, "LocalToCompReqBones")), vec![0, 1, 4], "head build {b}");
    }
}

/// Item 1: an empty `BoneBlendTargets` whose declared struct lacks `StartBoneName`, or declares `BlendWeight` as an
/// int, is refused before any entry is read.
#[test]
fn an_empty_target_list_still_needs_its_declared_members() {
    for (shape, member) in [(Shape::NoStartBone, "StartBoneName"), (Shape::IntBlendWeight, "BlendWeight")] {
        let mut x = Fx::with(shape);
        let n = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
        let (t, comp) = with_mesh(&mut x, n, five());
        let r = refused_clean(&mut x, comp, t.root);
        skeleton_for_tests(None);
        match r {
            VmError::Script { message, .. } => assert!(message.contains(member), "{member}: {message}"),
            e => panic!("{member}: {e:?}"),
        }
    }
}

/// Item 3: a bool-declared `bForceLocalSpaceBlend` holding an int is a script error naming the class and the field.
/// Item 4: a name index outside the table is a script error naming the field. Both written past the heap's check.
#[test]
fn a_flag_of_another_kind_and_a_name_outside_the_table_are_script_errors() {
    let mut x = Fx::new();
    let n = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
    let (t, comp) = with_mesh(&mut x, n, five());
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "bForceLocalSpaceBlend").unwrap();
    let sp = &mut x.vm.space;
    *sp.heap.slot_mut(&sp.classes, n, PropAddr::scalar(p)).unwrap() = Value::Int(1);
    let r = refused_clean(&mut x, comp, t.root);
    match r {
        VmError::Script { message, .. } => {
            assert!(message.contains("bForceLocalSpaceBlend") && message.contains("WillowAnimNodeFeatherBlend"), "{message}")
        }
        e => panic!("{e:?}"),
    }
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_mesh(&mut x, n, five());
    let mut other = ow_object::names::NameTable::new();
    let mut far = ow_object::ids::NameId::NONE;
    for i in 0..(x.vm.space.names.len() + 10) {
        far = other.intern(&format!("Far{i}"));
    }
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "BranchStartBoneName").unwrap();
    let sp = &mut x.vm.space;
    *sp.heap.slot_mut(&sp.classes, n, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Name(far)]);
    let r = refused_clean(&mut x, comp, t.root);
    skeleton_for_tests(None);
    match r {
        VmError::Script { message, .. } => assert!(message.contains("BranchStartBoneName"), "{message}"),
        e => panic!("{e:?}"),
    }
}

/// Item 5, through `instance` (review r2 of #585, item 3): a 256-bone chain whose last bone is the branch passes the
/// size guard and stores every bone, ending with byte 255; and a branching skeleton whose ascending candidates [B2, B3]
/// become [B0, B2, B1, B3] as the ancestors are inserted, stored unsorted.
#[test]
fn a_full_byte_chain_and_an_unsorted_ancestor_insertion() {
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let mut parents = vec![None];
    parents.extend((0..255).map(Some));
    let (t, comp) = with_mesh(&mut x, n, skeleton(&parents));
    let branch = names(&mut x, &["B255"]);
    x.set(n, "BranchStartBoneName", branch);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let c = copy_of(&x, comp, n);
    let mut weights = vec![0.0; 255];
    weights.push(1.0);
    assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), weights);
    assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), (0..=255u8).collect::<Vec<u8>>(), "every bone, byte 255 last");
    // Root 0; B1 and B2 under 0; B3 under 1. Branches B2 and B3: candidates [2, 3]; B2's parent (the root) goes ahead
    // of it, then B3's parent B1 ahead of B3.
    let mut x = Fx::new();
    let n = x.obj("Engine.AnimNodeBlendPerBone");
    let (t, comp) = with_mesh(&mut x, n, skeleton(&[None, Some(0), Some(0), Some(1)]));
    let branch = names(&mut x, &["B2", "B3"]);
    x.set(n, "BranchStartBoneName", branch);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let c = copy_of(&x, comp, n);
    assert_eq!(floats(&x.get(c, "Child2PerBoneWeight")), vec![0.0, 0.0, 1.0, 1.0]);
    assert_eq!(bytes(&x.get(c, "LocalToCompReqBones")), vec![0, 2, 1, 3], "inserted ahead, never sorted");
}

/// Review r2 of #585, item 1: a target member declared as a static array (`BlendWeight` as `float[2]`) is refused on
/// an empty list, before any entry, write-free.
#[test]
fn a_static_array_target_member_is_refused() {
    let mut x = Fx::with(Shape::ArrayBlendWeight);
    let n = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
    let (t, comp) = with_mesh(&mut x, n, five());
    let r = refused_clean(&mut x, comp, t.root);
    skeleton_for_tests(None);
    match r {
        VmError::Script { message, .. } => assert!(message.contains("BoneBlendTargets.BlendWeight"), "{message}"),
        e => panic!("{e:?}"),
    }
}

// ---- Lane IMPL-COPY-COMPONENT-PASS: step 5, the component pass and the per-bone control arrays -------------------

use crate::world::control_arrays::{MORPH_SETS, TAG_COLLISION as CONTROL_TAG_COLLISION, TOO_MANY_LISTS};

/// The diamond with the owner's event answered, a component holding a mesh whose skeleton is `parents`, and the
/// tree's lists set to `lists` (a bone name or None, and the head control).
fn arrays_fx(x: &mut Fx, parents: &[Option<usize>], lists: &[(Option<&str>, Option<ObjRef>)]) -> (Diamond, ObjRef) {
    let t = diamond(x);
    let comp = x.component();
    let mesh = x.obj("Engine.SkeletalMesh");
    x.set(comp, "SkeletalMesh", Value::Object(Some(mesh)));
    skeleton_for_tests(Some(skeleton(parents)));
    set_lists(x, t.root, lists);
    (t, comp)
}

fn set_lists(x: &mut Fx, root: ObjRef, lists: &[(Option<&str>, Option<ObjRef>)]) {
    let lt = x.list_head;
    let mut out = Vec::new();
    for &(bone, head) in lists {
        let mut l = x.vm.space.classes.struct_defaults(lt).unwrap();
        let name = x.vm.space.names.intern(bone.unwrap_or("None"));
        set_field(&x.vm, &mut l, "BoneName", Value::Name(name), "t").unwrap();
        set_field(&x.vm, &mut l, "ControlHead", Value::Object(head), "t").unwrap();
        out.push(l);
    }
    x.set(root, "SkelControlLists", Value::array(out));
}

/// A chain of `n` new controls, linked by `NextControl`; their template objects in order.
fn chain(x: &mut Fx, n: usize) -> Vec<ObjRef> {
    let made: Vec<ObjRef> = (0..n).map(|_| x.obj("Engine.SkelControlBase")).collect();
    for w in made.windows(2) {
        x.set(w[0], "NextControl", Value::Object(Some(w[1])));
    }
    made
}

/// The template object each copy in the component's `SkelControlTickArray` was made from, by name (a copy keeps it).
fn listed(x: &Fx, comp: ObjRef, templates: &[ObjRef]) -> Vec<ObjRef> {
    let name = |o: ObjRef| x.vm.space.heap.get(o).unwrap().name;
    x.get(comp, "SkelControlTickArray")
        .elements()
        .unwrap()
        .iter()
        .map(|v| {
            let Value::Object(Some(c)) = v else { panic!("{v:?}") };
            assert_eq!(x.vm.space.heap.get(*c).unwrap().outer, Some(comp), "a copy, outered to the component");
            *templates.iter().find(|t| name(**t) == name(*c)).expect("a known control")
        })
        .collect()
}

fn five_bones() -> Vec<Option<usize>> {
    vec![None, Some(0), Some(1), Some(1), Some(3)]
}

/// Test 1: no mesh, or no lists: the three arrays emptied (stale entries removed) and `TickTag` only the tick list's.
#[test]
fn with_no_mesh_or_no_lists_the_arrays_are_emptied_and_the_tag_kept() {
    for case in ["no mesh", "no lists"] {
        let mut x = Fx::new();
        let k = chain(&mut x, 1);
        let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(k[0]))]);
        if case == "no mesh" {
            x.set(comp, "SkeletalMesh", Value::Object(None));
        } else {
            set_lists(&mut x, t.root, &[]);
        }
        x.set(comp, "SkelControlIndex", Value::array(vec![Value::Byte(7)]));
        x.set(comp, "PostPhysSkelControlIndex", Value::array(vec![Value::Byte(7)]));
        x.set(comp, "SkelControlTickArray", Fx::objects(&[k[0]]));
        instance(&mut x.vm, comp, t.root).unwrap();
        skeleton_for_tests(None);
        for name in ["SkelControlIndex", "PostPhysSkelControlIndex", "SkelControlTickArray"] {
            assert_eq!(x.get(comp, name), Value::array(Vec::new()), "{case}: {name}");
        }
        assert_eq!(x.get(comp, "TickTag"), Value::Int(1), "{case}: the tick list's step only");
    }
}

/// Tests 2, 3 and 4: two lists on bones 1 and 3, a second list on bone 1, a None name and an unknown name: the list
/// indices there and 255 elsewhere; the later list on a taken bone is neither tagged nor listed; the skipped lists
/// keep no index; `TickTag` + 1 after the tick list's; the controls listed in walk order with the new tag.
#[test]
fn lists_set_their_bones_first_list_wins_and_unknown_names_are_skipped() {
    let mut x = Fx::new();
    let (a, b, c) = (chain(&mut x, 2), chain(&mut x, 1), chain(&mut x, 1));
    let d = chain(&mut x, 1);
    let lists = [
        (None, Some(d[0])),
        (Some("b1"), Some(a[0])),
        (Some("NoSuchBone"), Some(d[0])),
        (Some("B3"), Some(b[0])),
        (Some("B1"), Some(c[0])),
    ];
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &lists);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    assert_eq!(bytes(&x.get(comp, "SkelControlIndex")), vec![255, 1, 255, 3, 255], "list indices, by list position");
    assert_eq!(x.get(comp, "PostPhysSkelControlIndex"), Value::array(Vec::new()));
    let all: Vec<ObjRef> = a.iter().chain(&b).chain(&c).chain(&d).copied().collect();
    assert_eq!(listed(&x, comp, &all), vec![a[0], a[1], b[0]], "walk order; the later bone-1 list and skipped ones not");
    assert_eq!(x.get(comp, "TickTag"), Value::Int(2), "the tick list's step, then the build's");
    for v in x.get(comp, "SkelControlTickArray").elements().unwrap() {
        let Value::Object(Some(k)) = v else { panic!() };
        assert_eq!(x.get(*k, "ControlTickTag"), Value::Int(2));
    }
}

/// Tests 5 and 6 (the note at 5196fcec): a control shared by two chains is listed once; in the later chain its tag,
/// set earlier in the same build, makes it walked but not listed again, and the walk goes on past it.
#[test]
fn a_shared_control_is_listed_once_and_walked_past_in_the_later_chain() {
    let mut x = Fx::new();
    let shared = chain(&mut x, 1)[0];
    let (a, b, after) = (chain(&mut x, 1)[0], chain(&mut x, 1)[0], chain(&mut x, 1)[0]);
    x.set(a, "NextControl", Value::Object(Some(shared)));
    x.set(b, "NextControl", Value::Object(Some(shared)));
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(a)), (Some("B2"), Some(b))]);
    // Both chains reach `after` through the shared control: the later walk goes on past it.
    x.set(shared, "NextControl", Value::Object(Some(after)));
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    assert_eq!(listed(&x, comp, &[a, shared, b, after]), vec![a, shared, after, b], "shared and after listed once");
}

/// Test 7: a post-physics control in the middle of a chain sizes `PostPhysSkelControlIndex` and ends the walk; a
/// second post-physics list keeps the earlier entry.
#[test]
fn a_post_physics_control_sets_its_bone_and_ends_the_chain() {
    let mut x = Fx::new();
    let a = chain(&mut x, 3);
    let b = chain(&mut x, 1);
    x.set(a[1], "bPostPhysicsController", Value::Bool(true));
    x.set(b[0], "bPostPhysicsController", Value::Bool(true));
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B2"), Some(a[0])), (Some("B4"), Some(b[0]))]);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    assert_eq!(bytes(&x.get(comp, "PostPhysSkelControlIndex")), vec![255, 255, 0, 255, 1]);
    assert_eq!(bytes(&x.get(comp, "SkelControlIndex")), vec![255, 255, 0, 255, 1]);
    assert_eq!(listed(&x, comp, &[a[0], a[1], a[2], b[0]]), vec![a[0], a[1], b[0]], "a[2] after the post-physics one");
}

/// Test 7, the shared case (the note at 5196fcec): a post-physics control shared with an earlier chain is not listed
/// again, the later list's bone still gets its `PostPhysSkelControlIndex` entry, and that chain's walk stops there.
#[test]
fn a_shared_post_physics_control_still_stops_the_later_chain() {
    let mut x = Fx::new();
    let p = chain(&mut x, 1)[0];
    x.set(p, "bPostPhysicsController", Value::Bool(true));
    let (head, tail) = (chain(&mut x, 1)[0], chain(&mut x, 1)[0]);
    x.set(head, "NextControl", Value::Object(Some(p)));
    x.set(p, "NextControl", Value::Object(Some(tail)));
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(p)), (Some("B3"), Some(head))]);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    assert_eq!(bytes(&x.get(comp, "PostPhysSkelControlIndex")), vec![255, 0, 255, 1, 255]);
    assert_eq!(listed(&x, comp, &[p, head, tail]), vec![p, head], "the shared one once; the tail after it never");
}

/// Tests 8 and 9: 256 lists (this lane's `NotRead`), and a chain that loops (the copy's own gather's `VmError::Script`,
/// #557's rule; the note at 5196fcec adds no refusal of its own): each before any write, counters unchanged.
#[test]
fn too_many_lists_and_a_looping_chain_are_refused_before_any_write() {
    let mut x = Fx::new();
    let k = chain(&mut x, 1);
    let lists: Vec<(Option<&str>, Option<ObjRef>)> = (0..256).map(|_| (Some("B1"), Some(k[0]))).collect();
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &lists);
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    skeleton_for_tests(None);
    assert_eq!((&*n.what, n.missing), (TOO_MANY_LISTS, Missing::NotRead));
    let mut x = Fx::new();
    let k = chain(&mut x, 2);
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(k[0]))]);
    x.set(k[1], "NextControl", Value::Object(Some(k[0])));
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("loops back on itself"), "{message}"),
        e => panic!("{e:?}"),
    }
    skeleton_for_tests(None);
}

/// Test 14: a template control whose saved `ControlTickTag` equals the component's `TickTag` + 2 is the collision
/// `NotRead`, before any write.
#[test]
fn a_template_control_with_the_next_control_tag_is_refused() {
    let mut x = Fx::new();
    let k = chain(&mut x, 2);
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(k[0]))]);
    x.set(k[1], "ControlTickTag", Value::Int(2));
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    skeleton_for_tests(None);
    assert_eq!((&*n.what, n.missing, n.record), (CONTROL_TAG_COLLISION, Missing::NotRead, crate::world::control_arrays::RECORD));
}

/// Test 15: `InitTag` is the root's `NodeInitTag` + 1, and the two per-node arrays hold the tick list's length of zeros
/// and 0.0, stale entries removed.
#[test]
fn the_init_tag_and_the_per_node_arrays_are_written() {
    let mut x = Fx::new();
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[]);
    x.set(t.root, "NodeInitTag", Value::Int(41));
    x.set(comp, "AnimTickRelevancyArray", Value::array(vec![Value::Int(9); 9]));
    x.set(comp, "AnimTickWeightsArray", Value::array(vec![Value::Float(0.5)]));
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    assert_eq!(x.get(comp, "InitTag"), Value::Int(42));
    let n = x.get(comp, "AnimTickArray").elements().unwrap().len();
    assert_eq!(n, 5, "the diamond's tick list");
    assert_eq!(x.get(comp, "AnimTickRelevancyArray"), Value::array(vec![Value::Int(0); n]));
    assert_eq!(x.get(comp, "AnimTickWeightsArray"), Value::array(vec![Value::Float(0.0); n]));
}

/// Test 10: a non-empty `MorphSets` is `NotPorted` before any write; an empty one (or one of None entries) passes.
#[test]
fn a_set_morph_set_is_not_ported_before_any_write() {
    let mut x = Fx::new();
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[]);
    let set = x.obj("Engine.SkeletalMesh");
    x.set(comp, "MorphSets", Value::array(vec![Value::Object(None), Value::Object(Some(set))]));
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    assert_eq!((&*n.what, n.missing), (MORPH_SETS, Missing::NotPorted));
    x.set(comp, "MorphSets", Value::array(vec![Value::Object(None)]));
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
}

/// Test 11: #569's refusals pass through unchanged (a mesh that is not a `SkeletalMesh`, through the layer itself with
/// no invented skeleton), before any write.
#[test]
fn the_skeletal_layer_s_refusals_pass_through() {
    let mut x = Fx::new();
    let k = chain(&mut x, 1);
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(k[0]))]);
    skeleton_for_tests(None);
    let not_a_mesh = x.obj("Engine.SkelControlBase");
    x.set(comp, "SkeletalMesh", Value::Object(Some(not_a_mesh)));
    let direct = crate::world::skeletal::reference_skeleton(&x.vm, not_a_mesh).unwrap_err();
    let e = refused_clean(&mut x, comp, t.root);
    assert_eq!(format!("{e:?}"), format!("{direct:?}"), "the layer's own error");
}

/// A `BoneName` that is not a name is a script error before any write.
#[test]
fn a_bone_name_of_another_kind_is_a_script_error_before_any_write() {
    let mut x = Fx::new();
    let k = chain(&mut x, 1);
    let (t, comp) = arrays_fx(&mut x, &five_bones(), &[(Some("B1"), Some(k[0]))]);
    let lists = x.get(t.root, "SkelControlLists");
    let mut items = lists.elements().unwrap().to_vec();
    if let Value::Struct(s) = &mut items[0] {
        for f in std::rc::Rc::make_mut(s).fields.iter_mut() {
            if matches!(f, Value::Name(_)) {
                *f = Value::Int(3);
            }
        }
    }
    let p = x.vm.space.prop(x.vm.space.heap.get(t.root).unwrap().class, "SkelControlLists").unwrap();
    let sp = &mut x.vm.space;
    *sp.heap.slot_mut(&sp.classes, t.root, PropAddr::scalar(p)).unwrap() = Value::array(items);
    match refused_clean(&mut x, comp, t.root) {
        VmError::Script { message, .. } => assert!(message.contains("BoneName"), "{message}"),
        e => panic!("{e:?}"),
    }
    skeleton_for_tests(None);
}

/// Test 12, Maya-shaped (invented bone names B0..B26): 4 lists on bones 18, 4, 8 and 24, 5 controls (a chain of two,
/// then three single ones), none post-physics: the note's section 5 arrays.
#[test]
fn a_maya_shaped_tree_gives_the_note_s_arrays() {
    let mut x = Fx::new();
    let mut parents = vec![None];
    parents.extend((0..26).map(Some));
    let grip = chain(&mut x, 2);
    let (l_foot, r_foot, r_hand) = (chain(&mut x, 1)[0], chain(&mut x, 1)[0], chain(&mut x, 1)[0]);
    let lists = [(Some("B18"), Some(grip[0])), (Some("B4"), Some(l_foot)), (Some("B8"), Some(r_foot)), (Some("B24"), Some(r_hand))];
    let (t, comp) = arrays_fx(&mut x, &parents, &lists);
    instance(&mut x.vm, comp, t.root).unwrap();
    skeleton_for_tests(None);
    let mut want = vec![255u8; 27];
    want[18] = 0;
    want[4] = 1;
    want[8] = 2;
    want[24] = 3;
    assert_eq!(bytes(&x.get(comp, "SkelControlIndex")), want);
    assert_eq!(x.get(comp, "PostPhysSkelControlIndex"), Value::array(Vec::new()));
    let all = [grip[0], grip[1], l_foot, r_foot, r_hand];
    assert_eq!(listed(&x, comp, &all), all.to_vec());
    assert_eq!(x.get(comp, "TickTag"), Value::Int(2), "two above its value before");
}

// ---- Lane FIX-BODY-COMPOSITION-LOOKUP-STALE: the body composition's lookups over the copy ---------------------------

use crate::world::anim_node_lookup::{NO_BODY_CLASS, NO_TICK_LIST, Settled, apply, lookups};

/// An invented body class with `AimOffsetNodeName` and `HeadLookAtName` (None by default).
fn body_class(x: &mut Fx, aim: &str, head: &str) -> ObjRef {
    let object = x.object;
    let path = "Test.ToyBodyClass";
    if x.vm.space.classes.by_path(path).is_none() {
        class(&mut x.vm, path, object, &[("AimOffsetNodeName", PropKind::Name), ("HeadLookAtName", PropKind::Name)]);
    }
    let b = x.obj(path);
    let (aim, head) = (x.vm.space.names.intern(aim), x.vm.space.names.intern(head));
    x.set(b, "AimOffsetNodeName", Value::Name(aim));
    x.set(b, "HeadLookAtName", Value::Name(head));
    b
}

/// The control class `path` (defined under `SkelControlBase` on first use), one control of it named `name`.
fn control_of(x: &mut Fx, path: &str, name: &str) -> ObjRef {
    if x.vm.space.classes.by_path(path).is_none() {
        let base = x.vm.space.classes.by_path("Engine.SkelControlBase").unwrap();
        class(&mut x.vm, path, base, &[]);
    }
    let c = x.obj(path);
    let n = x.vm.space.names.intern(name);
    x.set(c, "ControlName", Value::Name(n));
    c
}

fn named(x: &mut Fx, o: ObjRef, name: &str) {
    let n = x.vm.space.names.intern(name);
    x.set(o, "NodeName", Value::Name(n));
}

/// The lookups over `t`'s planned copy on `comp`, with the counters unchanged.
fn looked_up(x: &Fx, comp: ObjRef, root: ObjRef, body: Option<ObjRef>) -> Result<Settled, VmError> {
    let counts = |x: &Fx| (x.vm.space.heap.committed_writes(), x.vm.rng.draws(), x.vm.defs.load_transitions());
    let before = counts(x);
    let p = plan(&x.vm, comp, root).unwrap();
    let r = lookups(&x.vm, &p, body);
    assert_eq!(counts(x), before, "the lookups write, draw and load nothing");
    r
}

/// The note's section 1 on the planned copy: of two nodes named `AimOffset`, the first in tick order is found; the
/// plain one first gives None (not the kept class), the kept one first gives it.
#[test]
fn a_node_lookup_takes_the_first_in_tick_order_and_keeps_only_its_class() {
    let mut x = Fx::new();
    let aim = x.obj(AIM_OFFSET_CLASS);
    let (t, comp) = with_node(&mut x, aim);
    named(&mut x, aim, "AimOffset");
    named(&mut x, t.a, "AimOffset");
    let body = body_class(&mut x, "AimOffset", "None");
    let p = plan(&x.vm, comp, t.root).unwrap();
    let a_at = p.order.iter().position(|&n| n == t.a).unwrap();
    let aim_at = p.order.iter().position(|&n| n == aim).unwrap();
    assert!(a_at < aim_at, "the fixture puts A first");
    assert_eq!(looked_up(&x, comp, t.root, Some(body)).unwrap().aim_offset, None, "A first: not the kept class");
    named(&mut x, t.a, "Other");
    assert_eq!(looked_up(&x, comp, t.root, Some(body)).unwrap().aim_offset, Some(aim));
}

/// Controls in list order: `LeftHandGrip` and the head look-at of their kept classes are found, `bUsingFootIK` is set
/// by a `WillowFootIK` control of any class, and a find of another class is None (`ArmBoneBlend` on a plain node).
#[test]
fn control_lookups_and_foot_ik_follow_the_lists() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let comp = x.component();
    named(&mut x, t.b, "ArmBoneBlend");
    let grip = control_of(&mut x, "WillowGame.SkelControlLeftHandGripWeapon", "LeftHandGrip");
    let look = control_of(&mut x, "WillowGame.WillowSkelControl_LookAtActor", "HeadLook");
    let foot = control_of(&mut x, "WillowGame.WillowSkelControl_FootPlacement", "WillowFootIK");
    x.set(grip, "NextControl", Value::Object(Some(look)));
    set_lists(&mut x, t.root, &[(None, Some(grip)), (None, Some(foot))]);
    let body = body_class(&mut x, "None", "HeadLook");
    let s = looked_up(&x, comp, t.root, Some(body)).unwrap();
    assert_eq!(s.arm_bone_blend, None, "found, but not an AnimNodeBlendPerBone");
    assert_eq!(s.left_hand_ik, Some(grip));
    assert_eq!(s.head_look_at, Some(look));
    assert!(s.foot_ik);
    let body = body_class(&mut x, "None", "None");
    assert_eq!(looked_up(&x, comp, t.root, Some(body)).unwrap().head_look_at, None, "a None name: no search");
}

/// `SlotNodes`: the tick list's `AnimNodeSlot` nodes (a subclass is matched by its class chain, as `is_kind` reads
/// it), in tick order; a None `AimOffsetNodeName`
/// matches the first node with no name (the root, not the kept class).
#[test]
fn slot_nodes_are_the_tick_list_s_slots_in_tick_order() {
    let mut x = Fx::new();
    let slot = x.obj("Engine.AnimNodeSlot");
    let (t, comp) = with_node(&mut x, slot);
    slots_of(&mut x, slot, &[(None, 1.0, false, 1.0)]);
    let body = body_class(&mut x, "None", "None");
    let s = looked_up(&x, comp, t.root, Some(body)).unwrap();
    assert_eq!(s.slots, vec![slot]);
    assert_eq!(s.aim_offset, None);
}

/// The refusals kept: a planned copy with no tick list (the component already initialised) and no body class, each
/// `NotRead`, with the counters unchanged.
#[test]
fn no_tick_list_and_no_body_class_are_not_read() {
    let mut x = Fx::new();
    let t = diamond(&mut x);
    let comp = x.component();
    match looked_up(&x, comp, t.root, None) {
        Err(VmError::NotYet(n)) => assert_eq!((&*n.what, n.missing), (NO_BODY_CLASS, Missing::NotRead)),
        other => panic!("{other:?}"),
    }
    x.set(comp, "bAnimTreeInitialised", Value::Bool(true));
    let body = body_class(&mut x, "AimOffset", "None");
    match looked_up(&x, comp, t.root, Some(body)) {
        Err(VmError::NotYet(n)) => assert_eq!((&*n.what, n.missing), (NO_TICK_LIST, Missing::NotRead)),
        other => panic!("{other:?}"),
    }
}

/// The writes: after the attach's copy (`instance_with_map`), `apply` writes the copies, outered to the component,
/// into the pawn's fields, and `SlotNodes` in tick order.
#[test]
fn apply_writes_the_copies_into_the_pawn() {
    let mut x = Fx::new();
    let aim = x.obj(AIM_OFFSET_CLASS);
    let (t, comp) = with_node(&mut x, aim);
    named(&mut x, aim, "AimOffset");
    let body = body_class(&mut x, "AimOffset", "None");
    let s = looked_up(&x, comp, t.root, Some(body)).unwrap();
    let object = x.object;
    let o = PropKind::Object { class: object };
    let fields = [
        ("ArmBoneBlend", o.clone()),
        ("AimOffset", o.clone()),
        ("LeftHandIK", o.clone()),
        ("HeadLookAt", o.clone()),
        ("bUsingFootIK", PropKind::Bool),
        ("SlotNodes", arr(o.clone())),
    ];
    class(&mut x.vm, "Test.ToyPawn", object, &fields);
    let pawn = x.obj("Test.ToyPawn");
    let (_, map) = crate::world::anim_tree_copy::instance_with_map(&mut x.vm, comp, t.root).unwrap();
    apply(&mut x.vm, pawn, &s, &map).unwrap();
    let Value::Object(Some(copy)) = x.get(pawn, "AimOffset") else { panic!("AimOffset set") };
    assert_ne!(copy, aim, "the copy, not the template node");
    assert_eq!(x.vm.space.heap.get(copy).unwrap().outer, Some(comp));
    assert_eq!(copy, copy_of(&x, comp, aim));
    assert_eq!(x.get(pawn, "SlotNodes"), Value::array(Vec::new()));
    assert_eq!(x.get(pawn, "bUsingFootIK"), Value::Bool(false));
    // Review r1 of #618, item 2: an answer the map has no copy of is a script error, with nothing written.
    let stranger = x.obj(AIM_OFFSET_CLASS);
    let drifted = Settled { aim_offset: Some(stranger), ..Settled::default() };
    let before = x.vm.space.heap.committed_writes();
    match apply(&mut x.vm, pawn, &drifted, &map) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("has no copy"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.vm.space.heap.committed_writes(), before, "checked before the first write");
}

// ---- The owner's event (NATIVE_TREE_INIT_OWNER_EVENT.md section 3; lane IMPL-TREE-INIT-OWNER-EVENT) ----------------

use crate::world::tree_lookup::{FIND_ANIM_NODE, FIND_SKEL_CONTROL, find_anim_node, find_skel_control};
use ow_object::ids::NameId;
use ow_package::testkit::script::{Asm, FunctionSpec, Param};

/// Defined | Event | Public, for the toy events.
const EVENT_FLAGS: u32 = 0x2 | 0x800 | 0x20000;
/// Native | Public, for the toy unread native.
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;

/// What an owner store changes from the good shape (test 12's malformed owners).
#[derive(Clone, Copy, PartialEq)]
enum OwnerShape {
    Good,
    /// `Engine.Pawn` declares no `Mesh`.
    NoMesh,
    /// `WillowPlayerPawn.AimOffset` declared an int.
    IntAimOffset,
    /// `Actor`'s version calls an invented native `ToyUnread`, which nothing binds (test 18).
    UnreadNative,
    /// A second package, `ToyOther`, declares its own `SkeletalMeshComponent.FindAnimNode(name)` native (review r1 of
    /// #631, item 4: a same-key binding in another package).
    OtherMeshPackage,
}

/// Toy `Core`, `Engine` and `WillowGame` packages with the three read versions of `PostInitAnimTree` at their
/// installed paths (Actor's and Pawn's empty; WillowPlayerPawn's records its argument in the invented `ToySeen`, so a
/// test sees the call and its argument), an invented override (`ToyOverridePawn`), an enemy-like pawn and an
/// actor-only class with no version of their own, two invented body classes, a pawn class with a state `Toying` that
/// holds its own `PostInitAnimTree` (test 15), and a data package whose holder refers to an unloaded `ToyBody`
/// (test 16).
fn owner_store(shape: OwnerShape) -> (Vm, DirGuard) {
    let dir = std::env::temp_dir().join(format!("ow-world-animcopy-ev-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let guard = DirGuard(dir.clone());
    let _ = std::fs::remove_dir_all(&dir);
    let kinds = ["Class", "Function", "State", "ObjectProperty", "IntProperty", "NameProperty", "BoolProperty"];
    let event_calling = |t: &mut ScriptToy, owner: i32, object: i32, record: Option<i32>, call: Option<i32>| {
        let params = [Param { type_ref: object, ..Param::new("Object", "SkelComp") }];
        t.function(owner, "PostInitAnimTree", &params, None, &[], &FunctionSpec::new(EVENT_FLAGS, "PostInitAnimTree"), |ids| {
            let mut a = Asm::new();
            if let Some(field) = record {
                a.stmt().raw(&[0x0F]).instance(field).local(ids.get("SkelComp"));
            }
            if let Some(native) = call {
                a.stmt().final_function(native, |_| {});
            }
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
    };
    let event = |t: &mut ScriptToy, owner: i32, object: i32, record: Option<i32>| event_calling(t, owner, object, record, None);
    let core = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        t.class("Object", 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&kinds);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let actor = t.class("Actor", object);
        let unread = (shape == OwnerShape::UnreadNative)
            .then(|| t.function(actor, "ToyUnread", &[], None, &[], &FunctionSpec::new(NATIVE_FLAGS, "ToyUnread"), |_| None).function);
        event_calling(&mut t, actor, object, None, unread);
        let pawn = t.class("Pawn", actor);
        if shape != OwnerShape::NoMesh {
            t.prop("Object", pawn, "Mesh", 0, object);
        }
        event(&mut t, pawn, object, None);
        t.class("ToyActorOnly", actor);
        // The component class with the initialised flag and the two lookups as natives (review r1 of #631, item 6),
        // so the player event below reaches the bound natives through bytecode; `Fx::on` adds the other fields on a
        // hand-declared class below it.
        let mesh = t.class("SkeletalMeshComponent", object);
        t.prop("Bool", mesh, "bAnimTreeInitialised", 0, 0);
        for find in ["FindAnimNode", "FindSkelControl"] {
            let params = [Param::new("Name", "InName")];
            t.function(mesh, find, &params, Some(("Object", object)), &[], &FunctionSpec::new(NATIVE_FLAGS, find), |_| None);
        }
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&kinds);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
        let player = t.class("WillowPlayerPawn", pawn);
        let caches: Vec<i32> = ["ArmBoneBlend", "LeftHandIK", "BodyClass"].iter().map(|f| t.prop("Object", player, f, 0, object)).collect();
        if shape == OwnerShape::IntAimOffset {
            t.prop("Int", player, "AimOffset", 0, 0);
        } else {
            t.prop("Object", player, "AimOffset", 0, object);
        }
        let seen = t.prop("Object", player, "ToySeen", 0, object);
        // UNVERIFIED stand-in for WillowPlayerPawn's version (an invented body, not the game's script; review r2 of
        // #631, item 4), its order kept (review r1 of #631, item 6): the argument recorded, the flag the call sees
        // recorded, then the two finds through the bound natives, each stored in its cache. The casts and the
        // aim-offset find (through `BodyClass`) are not reproduced: the cast classes are hand-declared, which bytecode
        // cannot name; the aim-offset find needs a `BodyClass != None` guard, an object comparison this toolkit's
        // bytecode has no tested form of, and unguarded it would read a None context in the tests whose `BodyClass` is
        // None. The preflight's cast and aim-offset answers are tested through the Rust lookups instead.
        let flag_seen = t.prop("Bool", player, "ToyFlagSeen", 0, 0);
        let (arm_cache, left_cache) = (caches[0], caches[1]);
        let mesh = t.p.add_import("Core", "Class", engine, "SkeletalMeshComponent");
        let flag = t.p.add_import("Core", "BoolProperty", mesh, "bAnimTreeInitialised");
        let find_node = t.p.add_import("Core", "Function", mesh, "FindAnimNode");
        let find_control = t.p.add_import("Core", "Function", mesh, "FindSkelControl");
        let arm_name = t.p.fname("ArmBoneBlend", 0);
        let grip_name = t.p.fname("LeftHandGrip", 0);
        let params = [Param { type_ref: object, ..Param::new("Object", "SkelComp") }];
        t.function(player, "PostInitAnimTree", &params, None, &[], &FunctionSpec::new(EVENT_FLAGS, "PostInitAnimTree"), |ids| {
            let c = ids.get("SkelComp");
            let mut a = Asm::new();
            a.stmt().raw(&[0x0F]).instance(seen).local(c);
            a.stmt().raw(&[0x14]).instance(flag_seen).context(|a| { a.local(c); }, |a| { a.instance(flag); });
            for (cache, find, name) in [(arm_cache, find_node, &arm_name), (left_cache, find_control, &grip_name)] {
                a.stmt().raw(&[0x0F]).instance(cache).context(
                    |a| {
                        a.local(c);
                    },
                    |a| {
                        a.final_function(find, |a| {
                            a.raw(&[0x21]).raw(name);
                        });
                    },
                );
            }
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
        let over = t.class("ToyOverridePawn", player);
        event(&mut t, over, object, None);
        t.class("ToyEnemyPawn", pawn);
        let body = t.class("ToyBody", object);
        t.prop("Name", body, "AimOffsetNodeName", 0, 0);
        let bad = t.class("ToyBadBody", object);
        t.prop("Int", bad, "AimOffsetNodeName", 0, 0);
        let stated = t.class("ToyStatePawn", pawn);
        let mut state = ow_package::testkit::Export::new(t.import("State"), "Toying", ow_package::testkit::ints(&[0, 0, 0, 0]));
        state.outer = stated;
        let toying = t.p.add_export_with(state);
        event(&mut t, toying, object, None);
        let holder = t.class("ToyBodyHolder", object);
        t.prop("Object", holder, "Body", 0, object);
        t.build()
    };
    let data = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let pkg = t.p.add_import("Core", "Package", 0, "WillowGame");
        let body = t.p.add_import("Core", "Class", pkg, "ToyBody");
        let holder = t.p.add_import("Core", "Class", pkg, "ToyBodyHolder");
        let mut payload = ow_package::testkit::ints(&[0, 0]);
        payload.extend_from_slice(&t.none);
        let b = t.p.add_export(body, "ColdBody", payload);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.object_tag("Body", b));
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    let other = {
        let mut t = ScriptToy::with_imports(&kinds);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let mesh = t.class("SkeletalMeshComponent", object);
        let params = [Param::new("Name", "InName")];
        t.function(mesh, "FindAnimNode", &params, Some(("Object", object)), &[], &FunctionSpec::new(NATIVE_FLAGS, "FindAnimNode"), |_| None);
        t.build()
    };
    let mut files: Vec<(&str, &[u8])> = vec![
        ("Core.upk", core.as_slice()),
        ("Engine.upk", engine.as_slice()),
        ("WillowGame.upk", willow.as_slice()),
        ("GD_ToyBodies.upk", data.as_slice()),
    ];
    if shape == OwnerShape::OtherMeshPackage {
        files.push(("ToyOther.upk", other.as_slice()));
    }
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    (Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off), guard)
}

/// The fixture on an owner store, the two lookups bound (as the Off pack binds them) and the left-hand control class
/// defined (the three cast classes, rule 7).
fn owner_fx(shape: OwnerShape) -> (Fx, DirGuard) {
    let (mut vm, guard) = owner_store(shape);
    crate::world::tree_lookup::register(&mut vm).unwrap();
    (cast_ready(Fx::on(vm, Shape::Good)), guard)
}

fn cast_ready(mut x: Fx) -> Fx {
    let base = x.vm.space.classes.by_path("Engine.SkelControlBase").unwrap();
    class(&mut x.vm, LEFT_HAND_CLASS, base, &[]);
    x
}

/// A runtime object of package class `class`. With `load` its class code is loaded (`find_class`); without, only its
/// layout is defined, straight through the code index (as ow-vm's `find_loaded_method` test does), so its functions
/// are not loaded.
fn owner_of(x: &mut Fx, class: &str, load: bool) -> ObjRef {
    let c = if load {
        x.vm.find_class(class).unwrap()
    } else {
        let vm = &mut x.vm;
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, class).unwrap();
        code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap()
    };
    x.n += 1;
    x.vm.space.alloc(c, &format!("Owner_{}", x.n), None, Origin::Runtime).unwrap()
}

fn event_refused(e: VmError, what: &str) {
    let n = not_yet(e);
    assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, EVENT_RECORD));
}

fn script_error(e: VmError, needle: &str) {
    match e {
        VmError::Script { message, .. } => assert!(message.contains(needle), "{message}"),
        e => panic!("expected a script error naming {needle}, got {e:?}"),
    }
}

/// The owner and the path of the event `plan` picks.
fn planned(x: &Fx, comp: ObjRef, root: ObjRef) -> Option<(ObjRef, String)> {
    plan(&x.vm, comp, root).unwrap().event.map(|(o, f)| (o, x.vm.function(f).unwrap().path.as_str().to_string()))
}

/// Test 1: no owner, no event; the initialisation still sets `bAnimTreeInitialised`.
#[test]
fn with_no_owner_no_event_is_sent_and_the_component_is_initialised() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let t = diamond(&mut x);
    let comp = x.component();
    assert_eq!(planned(&x, comp, t.root), None);
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "bAnimTreeInitialised"), Value::Bool(true));
}

/// Test 2: an owner with only `Actor`'s version: the event is planned and runs, writing nothing of its own.
#[test]
fn an_actor_only_owner_gets_the_actor_event_which_writes_nothing() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "Engine.ToyActorOnly", true);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    assert_eq!(planned(&x, comp, t.root), Some((owner, ACTOR_EVENT.to_string())));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "bAnimTreeInitialised"), Value::Bool(true));
}

/// Tests 3 and 4: a `Pawn`-line owner whose `Mesh` is the component is the `CacheAnimNodes` refusal before any write;
/// whose `Mesh` is another component gets `Pawn`'s version, its branch skipped.
#[test]
fn a_pawn_s_own_mesh_is_refused_and_another_mesh_passes() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.ToyEnemyPawn", true);
    let t = diamond(&mut x);
    let comp = x.component();
    let other = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    x.set(owner, "Mesh", Value::Object(Some(comp)));
    event_refused(refused_clean(&mut x, comp, t.root), OWN_MESH);
    x.set(owner, "Mesh", Value::Object(Some(other)));
    assert_eq!(planned(&x, comp, t.root), Some((owner, PAWN_EVENT.to_string())));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(owner, "Mesh"), Value::Object(Some(other)), "Pawn's branch skipped");
}

/// A diamond with a `WillowAnimNodeAimOffset` named `AimOffset` under C and its first control named `LeftHandGrip`.
fn player_tree(x: &mut Fx) -> (Diamond, ObjRef, ObjRef) {
    let aim = x.obj(AIM_OFFSET_CLASS);
    let (t, comp) = with_node(x, aim);
    let n = x.vm.space.names.intern("AimOffset");
    x.set(aim, "NodeName", Value::Name(n));
    let g = x.vm.space.names.intern("LeftHandGrip");
    x.set(t.k1, "ControlName", Value::Name(g));
    (t, aim, comp)
}

/// Test 5 (its port side): a `WillowPlayerPawn` owner gets its version, called with the component; the lookups it
/// calls answer on the instance: no `ArmBoneBlend` node (None), the copy's aim-offset node by `AimOffsetNodeName`, the
/// copy's `LeftHandGrip` control. Tests 6 to 8 (each cache only while None, a result of another class stored as None)
/// are the event's own script, which the toy event does not reproduce, so they are not covered here.
#[test]
fn a_player_owner_gets_its_event_with_the_component_and_the_lookups_answer_on_the_instance() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let body = owner_of(&mut x, "WillowGame.ToyBody", true);
    let (t, aim, comp) = player_tree(&mut x);
    let name = x.vm.space.names.intern("AimOffset");
    x.set(body, "AimOffsetNodeName", Value::Name(name));
    x.set(owner, "BodyClass", Value::Object(Some(body)));
    x.set(comp, "Owner", Value::Object(Some(owner)));
    assert_eq!(planned(&x, comp, t.root), Some((owner, PLAYER_EVENT.to_string())));
    let draws = x.vm.rng.draws();
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.vm.rng.draws(), draws, "no random number");
    assert_eq!(x.get(owner, "ToySeen"), Value::Object(Some(comp)), "called with the component");
    assert_eq!(x.get(owner, "ToyFlagSeen"), Value::Bool(false), "sent before bAnimTreeInitialised is set");
    assert_eq!(x.get(comp, "bAnimTreeInitialised"), Value::Bool(true));
    // Through bytecode dispatch (the stand-in event above): `ToySeen`, `ToyFlagSeen` and the two caches it filled
    // through the bound natives (review r1 of #631, item 6). The `find_anim_node` / `find_skel_control` calls after
    // them call the Rust lookups directly; the aim-offset answer is checked only that way (review r2, item 4).
    assert_eq!(x.get(owner, "ArmBoneBlend"), Value::Object(None), "no ArmBoneBlend node");
    let left = object_of(&x.get(owner, "LeftHandIK")).expect("the grip control, through FindSkelControl");
    assert_eq!((x.name_of(left), x.outer(left)), (x.name_of(t.k1), Some(comp)), "the copy, not the template");
    let arm = x.vm.space.names.intern("ArmBoneBlend");
    assert_eq!(find_anim_node(&x.vm, comp, arm).unwrap(), None);
    assert_eq!(find_anim_node(&x.vm, comp, name).unwrap(), Some(copy_of(&x, comp, aim)));
    let grip = x.vm.space.names.intern("LeftHandGrip");
    let k = find_skel_control(&x.vm, comp, grip).unwrap().expect("the grip control");
    assert_eq!((x.name_of(k), x.outer(k)), (x.name_of(t.k1), Some(comp)), "the copy, not the template");
}

/// Test 7 (its preflight side), and test 5's body class checks: with `BodyClass` None the body class is not read; a
/// set one must declare `AimOffsetNodeName` as a name.
#[test]
fn the_body_class_is_checked_only_when_set() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let bad = owner_of(&mut x, "WillowGame.ToyBadBody", true);
    let (t, _aim, comp) = player_tree(&mut x);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    assert_eq!(planned(&x, comp, t.root).map(|(_, p)| p), Some(PLAYER_EVENT.to_string()), "BodyClass None");
    x.set(owner, "BodyClass", Value::Object(Some(bad)));
    script_error(refused_clean(&mut x, comp, t.root), "AimOffsetNodeName");
}

/// Test 17: each of the two natives unbound is its own `NotPorted`, before any write.
#[test]
fn each_unbound_lookup_is_its_own_not_ported_before_any_write() {
    for bind_node in [false, true] {
        let (mut vm, _dir) = owner_store(OwnerShape::Good);
        if bind_node {
            vm.register_native(FIND_ANIM_NODE, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Object(None)))).unwrap();
        }
        let mut x = cast_ready(Fx::on(vm, Shape::Good));
        let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
        let (t, _aim, comp) = player_tree(&mut x);
        x.set(comp, "Owner", Value::Object(Some(owner)));
        let n = not_yet(refused_clean(&mut x, comp, t.root));
        let want = if bind_node { SKEL_CONTROL_UNBOUND } else { ANIM_NODE_UNBOUND };
        assert_eq!((&*n.what, n.missing, n.record), (want, Missing::NotPorted, EVENT_RECORD));
    }
}

/// Rule 7's cast classes: one not defined at set-up is refused before any write (the cast would define it).
#[test]
fn an_undefined_cast_class_is_refused_before_any_write() {
    let (mut vm, _dir) = owner_store(OwnerShape::Good);
    crate::world::tree_lookup::register(&mut vm).unwrap();
    let mut x = Fx::on(vm, Shape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let (t, _aim, comp) = player_tree(&mut x);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    event_refused(refused_clean(&mut x, comp, t.root), &cast_undefined(LEFT_HAND_CLASS));
}

/// Test 8 (its preflight side): a node named `ArmBoneBlend` that is not an `AnimNodeBlendPerBone` passes the
/// preflight, the cast's answer (no) computed there; the event runs.
#[test]
fn a_find_of_another_class_is_answered_in_the_preflight() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let (t, _aim, comp) = player_tree(&mut x);
    let arm = x.vm.space.names.intern("ArmBoneBlend");
    x.set(t.b, "NodeName", Value::Name(arm));
    x.set(comp, "Owner", Value::Object(Some(owner)));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(owner, "ToySeen"), Value::Object(Some(comp)));
    assert_eq!(find_anim_node(&x.vm, comp, arm).unwrap(), Some(copy_of(&x, comp, t.b)), "found; the cast is the script's");
}

/// Test 9: an override on an invented subclass, refused by path before any write.
#[test]
fn an_override_is_refused_by_path_before_any_write() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.ToyOverridePawn", true);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    event_refused(refused_clean(&mut x, comp, t.root), &event_override("WillowGame.ToyOverridePawn.PostInitAnimTree"));
}

/// Test 10: an owner whose class code is not loaded, with nothing loaded.
#[test]
fn an_owner_whose_code_is_not_loaded_is_refused_with_nothing_loaded() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", false);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    event_refused(refused_clean(&mut x, comp, t.root), EVENT_NOT_LOADED);
    let class = x.vm.space.heap.get(owner).unwrap().class;
    assert_eq!(x.vm.find_loaded_method(class, EVENT, None), None, "still not loaded");
}

/// Test 11: an unloaded owner handle is #543's refusal.
#[test]
fn an_unloaded_owner_is_the_lookup_refusal() {
    let (vm, _dir, handle, _) = handles_store();
    let mut x = Fx::on(vm, Shape::Good);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(handle)));
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    assert_eq!((&*n.what, n.missing), (ANIM_NODES, Missing::LayerAbsent(ANIM_LAYER)));
}

/// Test 12: an owner with no `Mesh`, or a cache field of another kind, is a script error before any write.
#[test]
fn a_malformed_owner_is_a_script_error_before_any_write() {
    for (shape, class, needle) in [
        (OwnerShape::NoMesh, "WillowGame.ToyEnemyPawn", "Mesh"),
        (OwnerShape::IntAimOffset, "WillowGame.WillowPlayerPawn", "AimOffset"),
    ] {
        let (mut x, _dir) = owner_fx(shape);
        let owner = owner_of(&mut x, class, true);
        let t = diamond(&mut x);
        let comp = x.component();
        x.set(comp, "Owner", Value::Object(Some(owner)));
        script_error(refused_clean(&mut x, comp, t.root), needle);
    }
}

/// Test 13: the attach check plans the event for the passed owner (not the template's own `Owner`), with the same
/// refusals and nothing written; the own-`Mesh` refusal is the instance path's only.
#[test]
fn the_attach_check_plans_the_passed_owner_s_event() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let over = owner_of(&mut x, "WillowGame.ToyOverridePawn", true);
    let enemy = owner_of(&mut x, "WillowGame.ToyEnemyPawn", true);
    let cold = owner_of(&mut x, "Engine.ToyActorOnly", false);
    let t = diamond(&mut x);
    let template = x.component();
    x.set(template, "AnimTreeTemplate", Value::Object(Some(t.root)));
    x.set(template, "Owner", Value::Object(Some(over)));
    x.set(enemy, "Mesh", Value::Object(Some(template)));
    let counts = |x: &Fx| {
        (
            x.vm.space.heap.committed_writes(),
            x.vm.space.heap.handles().len(),
            x.vm.defs.load_transitions(),
            x.vm.space.names.len(),
            x.vm.rng.draws(),
        )
    };
    let before = counts(&x);
    attach_check(&x.vm, template, None).unwrap();
    attach_check(&x.vm, template, Some(enemy)).unwrap();
    let e = attach_check(&x.vm, template, Some(over)).unwrap_err();
    event_refused(e, &event_override("WillowGame.ToyOverridePawn.PostInitAnimTree"));
    event_refused(attach_check(&x.vm, template, Some(cold)).unwrap_err(), EVENT_NOT_LOADED);
    assert_eq!(counts(&x), before, "nothing written, made, loaded, interned or drawn");
}

/// Test 19 and the binding's other rules: no tree is None; the gather path (a tree that is not the component's own, or
/// an empty tick list) is `NotRead`; names match whole (`Name_1` is not `Name_2`); a None name finds no control; a
/// looping chain is a script error.
#[test]
fn the_lookups_follow_the_scoped_binding() {
    use crate::world::tree_lookup::{GATHER, RECORD as LOOKUP_RECORD};
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let (t, aim, comp) = player_tree(&mut x);
    let one = x.vm.space.names.intern("Name_1");
    let two = x.vm.space.names.intern("Name_2");
    x.set(aim, "NodeName", Value::Name(one));
    assert_eq!(find_anim_node(&x.vm, comp, one).unwrap(), None, "no tree");
    assert_eq!(find_skel_control(&x.vm, comp, one).unwrap(), None, "no tree");
    let gather = |e: VmError| {
        let n = not_yet(e);
        assert_eq!((&*n.what, n.missing, n.record), (GATHER, Missing::NotRead, LOOKUP_RECORD));
    };
    x.set(comp, "Animations", Value::Object(Some(t.root)));
    gather(find_anim_node(&x.vm, comp, one).unwrap_err());
    x.set(comp, "Animations", Value::Object(None));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(find_anim_node(&x.vm, comp, one).unwrap(), Some(copy_of(&x, comp, aim)));
    assert_eq!(find_anim_node(&x.vm, comp, two).unwrap(), None, "a whole-name match");
    let ticks = x.get(comp, "AnimTickArray");
    x.set(comp, "AnimTickArray", Value::array(Vec::new()));
    gather(find_anim_node(&x.vm, comp, one).unwrap_err());
    x.set(comp, "AnimTickArray", ticks);
    assert_eq!(find_skel_control(&x.vm, comp, NameId::NONE).unwrap(), None);
    let root = object_of(&x.get(comp, "Animations")).unwrap();
    let k2 = Fx::list(&x.get(comp, "SkelControlTickArray")).into_iter().flatten().find(|&k| x.name_of(k) == x.name_of(t.k2));
    let lists = x.get(root, "SkelControlLists");
    let head = object_of(&field(&x.vm, &lists.elements().unwrap()[0], "ControlHead", "t").unwrap()).unwrap();
    x.set(k2.unwrap_or(head), "NextControl", Value::Object(Some(head)));
    script_error(find_skel_control(&x.vm, comp, two).unwrap_err(), "loops back");
}

/// Test 15: an owner in a state is refused before any write, also when that state holds its own `PostInitAnimTree`
/// (rule 3 before rule 4).
#[test]
fn an_owner_in_a_state_is_refused_before_any_write() {
    for (class, state) in [("WillowGame.ToyEnemyPawn", "Anything"), ("WillowGame.ToyStatePawn", "Toying")] {
        let (mut x, _dir) = owner_fx(OwnerShape::Good);
        let owner = owner_of(&mut x, class, true);
        let s = x.vm.space.names.intern(state);
        let c = x.vm.space.heap.get(owner).unwrap().class;
        if state == "Toying" {
            let f = x.vm.find_loaded_method(c, EVENT, Some(s)).unwrap().expect("the state's own function");
            assert!(x.vm.function(f).unwrap().path.as_str().contains("Toying"), "reached by the state");
        }
        x.vm.space.heap.get_mut(owner).unwrap().state = Some(s);
        let t = diamond(&mut x);
        let comp = x.component();
        x.set(comp, "Owner", Value::Object(Some(owner)));
        event_refused(refused_clean(&mut x, comp, t.root), IN_STATE);
    }
}

/// Test 16: an unloaded `BodyClass` is #543's refusal, before any write.
#[test]
fn an_unloaded_body_class_is_the_lookup_refusal() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let path = ObjectPath::parse("GD_ToyBodies.Holder").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let cold = object_of(&x.get(holder, "Body")).unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, cold));
    let (t, _aim, comp) = player_tree(&mut x);
    x.set(owner, "BodyClass", Value::Object(Some(cold)));
    x.set(comp, "Owner", Value::Object(Some(owner)));
    let n = not_yet(refused_clean(&mut x, comp, t.root));
    assert_eq!((&*n.what, n.missing), (ANIM_NODES, Missing::LayerAbsent(ANIM_LAYER)));
}

/// Test 18: a `NotYet` the event's run raises after the copy's writes (an invented body reaching an invented native
/// that answers `NotYet`, standing for an unread one) is marked "stopped after partial writes: " once, its kind and
/// record kept. (An unbound native inside the run is `VmError::UnboundNative`, which passes unchanged.)
#[test]
fn a_not_yet_in_the_event_s_run_is_marked_after_partial_writes() {
    let (mut x, _dir) = owner_fx(OwnerShape::UnreadNative);
    let unread = |_c: &mut ow_vm::api::NativeCall<'_>| -> Result<Value, VmError> {
        Err(ow_core::NotYet::not_read("Actor.ToyUnread (invented)", "a toy record").into())
    };
    x.vm.register_native("Actor.ToyUnread()", Rc::new(unread)).unwrap();
    let owner = owner_of(&mut x, "Engine.ToyActorOnly", true);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    let (_, f) = plan(&x.vm, comp, t.root).unwrap().event.expect("planned");
    let writes = x.vm.space.heap.committed_writes();
    let n = not_yet(instance(&mut x.vm, comp, t.root).unwrap_err());
    assert!(x.vm.space.heap.committed_writes() > writes, "after the copy's writes");
    let prefix = "stopped after partial writes: ";
    assert!(n.what.starts_with(prefix) && n.what.matches(prefix).count() == 1, "{}", n.what);
    assert_eq!(&n.what[prefix.len()..], "Actor.ToyUnread (invented)");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "a toy record"), "the kind and record kept");
    assert!(x.vm.function(f).is_some());
}


/// Test 14: Reference binds neither lookup; Off binds both. A key bound first keeps its binding (a duplicate
/// registration would be an error).
#[test]
fn reference_binds_no_lookup_and_off_keeps_an_existing_binding() {
    for (mode, bound) in [(ParityMode::Reference, false), (ParityMode::Off, true)] {
        let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
        ow_vm::api::NativePack::register(&crate::world::Pack, &mut vm).unwrap();
        assert_eq!((vm.has_native(FIND_ANIM_NODE), vm.has_native(FIND_SKEL_CONTROL)), (bound, bound), "{mode:?}");
    }
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    vm.register_native(FIND_ANIM_NODE, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
    crate::world::tree_lookup::register(&mut vm).unwrap();
    assert!(vm.has_native(FIND_SKEL_CONTROL));
}

/// Review r1 of #631, item 1: a component with a `ParentAnimComponent` gets the copy and no initialisation, so no
/// owner event: on the instance (an owner whose event would be refused runs nothing) and on the attach check.
#[test]
fn a_component_with_a_parent_animation_component_is_not_initialised() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let over = owner_of(&mut x, "WillowGame.ToyOverridePawn", true);
    let player = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let t = diamond(&mut x);
    let parent = x.component();
    let comp = x.component();
    x.set(comp, "ParentAnimComponent", Value::Object(Some(parent)));
    x.set(comp, "Owner", Value::Object(Some(player)));
    let p = plan(&x.vm, comp, t.root).unwrap();
    assert!(!p.initialise && p.event.is_none() && p.steps.is_empty() && p.order.is_empty());
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(comp, "Animations"), Value::Object(Some(root)), "the copy is made");
    assert_eq!(x.get(comp, "TickTag"), Value::Int(0));
    assert_eq!(x.get(comp, "bAnimTreeInitialised"), Value::Bool(false));
    assert_eq!(x.get(player, "ToySeen"), Value::Object(None), "no event");
    let template = x.component();
    x.set(template, "AnimTreeTemplate", Value::Object(Some(t.root)));
    let e = attach_check(&x.vm, template, Some(over)).unwrap_err();
    event_refused(e, &event_override("WillowGame.ToyOverridePawn.PostInitAnimTree"));
    x.set(template, "ParentAnimComponent", Value::Object(Some(parent)));
    attach_check(&x.vm, template, Some(over)).unwrap();
}

/// Review r1 of #631, item 2: neither lookup answers from a tree that is not the receiving component's own (a shared
/// template, or another component's copy), nor from a tick list holding another tree's node.
#[test]
fn the_lookups_refuse_a_foreign_tree() {
    use crate::world::tree_lookup::{FOREIGN, GATHER, RECORD as LOOKUP_RECORD};
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let (t, _aim, comp) = player_tree(&mut x);
    let grip = x.vm.space.names.intern("LeftHandGrip");
    let any = x.vm.space.names.intern("AimOffset");
    let refused = |e: VmError, what: &str| {
        let n = not_yet(e);
        assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, LOOKUP_RECORD));
    };
    // The shared template as the component's tree.
    let other = x.component();
    x.set(other, "Animations", Value::Object(Some(t.root)));
    x.set(other, "AnimTickArray", Fx::objects(&[t.root, t.a]));
    refused(find_anim_node(&x.vm, other, any).unwrap_err(), GATHER);
    refused(find_skel_control(&x.vm, other, grip).unwrap_err(), FOREIGN);
    // Another component's copy.
    let root = instance(&mut x.vm, comp, t.root).unwrap();
    assert!(find_skel_control(&x.vm, comp, grip).unwrap().is_some(), "its own tree answers");
    x.set(other, "Animations", Value::Object(Some(root)));
    refused(find_skel_control(&x.vm, other, grip).unwrap_err(), FOREIGN);
    // The own tree with a template node in its tick list.
    x.set(comp, "AnimTickArray", Fx::objects(&[root, t.a]));
    refused(find_anim_node(&x.vm, comp, any).unwrap_err(), GATHER);
    // The documented None results stay: a None name, no tree, a root that is not an AnimTree.
    assert_eq!(find_skel_control(&x.vm, other, NameId::NONE).unwrap(), None);
    x.set(other, "Animations", Value::Object(None));
    assert_eq!(find_skel_control(&x.vm, other, grip).unwrap(), None);
    assert_eq!(find_anim_node(&x.vm, other, any).unwrap(), None);
    x.set(other, "Animations", Value::Object(Some(t.a)));
    assert_eq!(find_skel_control(&x.vm, other, grip).unwrap(), None);
}

struct ToyService;

/// Review r1 of #631, item 3: a service borrow held over the instance is the event call's refusal in the preflight
/// (`Vm::check_call`), before any write, rather than an unlabelled error after the copy.
#[test]
fn a_held_service_borrow_is_refused_in_the_preflight() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "Engine.ToyActorOnly", true);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    let cell = x.vm.add_service(ToyService).unwrap();
    let held = cell.borrow_mut();
    match refused_clean(&mut x, comp, t.root) {
        VmError::ServiceBorrowed(name) => assert!(name.contains("ToyService"), "{name}"),
        e => panic!("expected ServiceBorrowed, got {e:?}"),
    }
    drop(held);
    instance(&mut x.vm, comp, t.root).unwrap();
}

/// Review r1 of #631, item 4: another package's binding of the same key does not stand for Engine's: the Off binding
/// still binds Engine's two natives, and the player event reaches them.
#[test]
fn a_same_key_binding_in_another_package_does_not_bind_engine_s() {
    use crate::world::tree_lookup::PACKAGE;
    let (mut vm, _dir) = owner_store(OwnerShape::OtherMeshPackage);
    let f = vm.find_function("ToyOther.SkeletalMeshComponent.FindAnimNode").unwrap();
    vm.register_native_for(f, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
    assert!(vm.has_native(FIND_ANIM_NODE) && !vm.is_bound_in(PACKAGE, FIND_ANIM_NODE));
    crate::world::tree_lookup::register(&mut vm).unwrap();
    assert!(vm.is_bound_in(PACKAGE, FIND_ANIM_NODE) && vm.is_bound_in(PACKAGE, FIND_SKEL_CONTROL));
    let mut x = cast_ready(Fx::on(vm, Shape::Good));
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let (t, _aim, comp) = player_tree(&mut x);
    x.set(comp, "Owner", Value::Object(Some(owner)));
    instance(&mut x.vm, comp, t.root).unwrap();
    assert_eq!(x.get(owner, "ToySeen"), Value::Object(Some(comp)));
    assert!(object_of(&x.get(owner, "LeftHandIK")).is_some(), "Engine's FindSkelControl answered");
}

/// Review r1 of #631, item 5: with `LeftHandIK` None and the `LeftHandGrip` literal not interned yet, the run's
/// control search still reads every control's `ControlName`, so a control that declares none is a script error in
/// the preflight, with nothing written or interned.
#[test]
fn an_uninterned_grip_literal_still_checks_the_controls() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
    let t = diamond(&mut x);
    assert!(x.vm.space.names.find("LeftHandGrip").is_none(), "the literal is not interned");
    let object = x.object;
    let o = PropKind::Object { class: object };
    class(&mut x.vm, "Test.ToyNamelessControl", object, &[
        ("NextControl", o),
        ("ControlTickTag", PropKind::Int),
        ("bPostPhysicsController", PropKind::Bool),
    ]);
    let bare = x.obj("Test.ToyNamelessControl");
    x.set(t.k2, "NextControl", Value::Object(Some(bare)));
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    script_error(refused_clean(&mut x, comp, t.root), "ControlName");
}

/// A handler of either key bound before the port's, so kept by the binding.
fn foreign_bound(key: &str) -> (Fx, DirGuard) {
    let (mut vm, guard) = owner_store(OwnerShape::Good);
    vm.register_native(key, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Object(None)))).unwrap();
    crate::world::tree_lookup::register(&mut vm).unwrap();
    (cast_ready(Fx::on(vm, Shape::Good)), guard)
}

/// Review r2 of #631, item 1: a handler bound before the port's is kept, and the player event, whose answers the
/// preflight predicts from the port's own lookups, is refused before any write; an Actor-only owner, which calls no
/// lookup, still runs.
#[test]
fn a_foreign_handler_bound_first_is_kept_and_the_player_event_refused_before_any_write() {
    use crate::world::tree_lookup::{PACKAGE, bound_to_ours};
    for (key, want) in [(FIND_ANIM_NODE, ANIM_NODE_OTHER), (FIND_SKEL_CONTROL, SKEL_CONTROL_OTHER)] {
        let (mut x, _dir) = foreign_bound(key);
        assert!(x.vm.is_bound_in(PACKAGE, key) && !bound_to_ours(&x.vm, key).unwrap(), "the earlier binding kept");
        let owner = owner_of(&mut x, "WillowGame.WillowPlayerPawn", true);
        let (t, _aim, comp) = player_tree(&mut x);
        x.set(comp, "Owner", Value::Object(Some(owner)));
        let n = not_yet(refused_clean(&mut x, comp, t.root));
        assert_eq!((&*n.what, n.missing, n.record), (want, Missing::NotPorted, EVENT_RECORD), "{key}");
        assert_eq!(x.get(owner, "ToySeen"), Value::Object(None), "no event");
        let actor = owner_of(&mut x, "Engine.ToyActorOnly", true);
        x.set(comp, "Owner", Value::Object(Some(actor)));
        instance(&mut x.vm, comp, t.root).unwrap();
    }
}

/// Review r2 of #631, item 2: the call-entry depth refusal is met in the preflight (`Vm::check_call`), before the copy
/// writes, rather than after it by the call. The frame-slot bound shares the same check; its bound is too large for a
/// synthetic frame to reach here, so only the depth is exercised.
#[test]
fn a_call_depth_refusal_is_met_in_the_preflight() {
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let owner = owner_of(&mut x, "Engine.ToyActorOnly", true);
    let t = diamond(&mut x);
    let comp = x.component();
    x.set(comp, "Owner", Value::Object(Some(owner)));
    let depth = x.vm.limits.max_call_depth;
    x.vm.limits.max_call_depth = 0;
    script_error(refused_clean(&mut x, comp, t.root), "call depth limit");
    x.vm.limits.max_call_depth = depth;
    instance(&mut x.vm, comp, t.root).unwrap();
}

/// The control list head of `root`'s first list.
fn first_head(x: &Fx, root: ObjRef) -> ObjRef {
    let lists = x.get(root, "SkelControlLists");
    object_of(&field(&x.vm, &lists.elements().unwrap()[0], "ControlHead", "t").unwrap()).unwrap()
}

/// Review r2 of #631, item 3: a node or control of an earlier copy on the same component shares the heap outer but
/// is not the current copy's: `FindAnimNode` does not return a stale tick-list node, and `FindSkelControl` does not
/// reach a stale control through `NextControl`, nor a template control through `ControlHead`.
#[test]
fn the_lookups_refuse_a_stale_copy_s_node_or_control() {
    use crate::world::tree_lookup::{FOREIGN, GATHER, RECORD as LOOKUP_RECORD};
    let (mut x, _dir) = owner_fx(OwnerShape::Good);
    let (t, aim, comp) = player_tree(&mut x);
    let name = x.vm.space.names.intern("AimOffset");
    let grip = x.vm.space.names.intern("LeftHandGrip");
    let refused = |e: VmError, what: &str| {
        let n = not_yet(e);
        assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, LOOKUP_RECORD));
    };
    let first = instance(&mut x.vm, comp, t.root).unwrap();
    let stale_aim = copy_of(&x, comp, aim);
    let stale_grip = first_head(&x, first);
    assert_eq!(find_skel_control(&x.vm, comp, grip).unwrap(), Some(stale_grip), "the first copy answers");
    // A second copy on the same, now initialised, component.
    let second = instance(&mut x.vm, comp, t.root).unwrap();
    assert_ne!(second, first);
    assert_eq!(x.outer(stale_aim), Some(comp), "the stale node shares the outer");
    x.set(comp, "AnimTickArray", Fx::objects(&[second, stale_aim]));
    refused(find_anim_node(&x.vm, comp, name).unwrap_err(), GATHER);
    x.set(comp, "AnimTickArray", Fx::objects(&[second]));
    assert_eq!(find_anim_node(&x.vm, comp, name).unwrap(), None, "the current copy alone");
    // A stale control through NextControl: the current head renamed, its next the first copy's grip control.
    let head = first_head(&x, second);
    assert_ne!(head, stale_grip);
    assert_eq!(find_skel_control(&x.vm, comp, grip).unwrap(), Some(head), "the current copy answers");
    let other = x.vm.space.names.intern("ToyOtherControl");
    x.set(head, "ControlName", Value::Name(other));
    x.set(head, "NextControl", Value::Object(Some(stale_grip)));
    refused(find_skel_control(&x.vm, comp, grip).unwrap_err(), FOREIGN);
    // A template control through ControlHead.
    let lt = x.list_head;
    let mut l = x.vm.space.classes.struct_defaults(lt).unwrap();
    set_field(&x.vm, &mut l, "ControlHead", Value::Object(Some(t.k1)), "t").unwrap();
    x.set(second, "SkelControlLists", Value::array(vec![l]));
    refused(find_skel_control(&x.vm, comp, grip).unwrap_err(), FOREIGN);
}
