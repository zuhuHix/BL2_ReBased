//! 2.A to 2.C of NATIVE_LINK1_TAIL_SETTERS.md on invented classes (AGENTS.md rule 9: invented objects and values,
//! the class and field names the packages'). Every refusal leaves #518's counters, `class_code_loads` and the name
//! table unchanged; lazy objects are slotless stand-ins.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    find: FuncId,
    blend_fn: FuncId,
    active_child: FuncId,
    component: ClassId,
    node: ClassId,
    blend: ClassId,
    feather: ClassId,
    per_bone: ClassId,
    list: ClassId,
    child: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn native(vm: &mut Vm, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>) -> FuncId {
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner).with_params(params);
    vm.declare_function(d).unwrap()
}

fn fx() -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let o = PropKind::Object { class: object };
    let props = vec![
        decl(&mut vm, "AnimTickArray", PropKind::Array(Box::new(o.clone()))),
        decl(&mut vm, "Animations", o.clone()),
        decl(&mut vm, "bRecentlyRendered", PropKind::Bool),
    ];
    let component = vm.space.classes.define("Engine.SkeletalMeshComponent", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "SkelComponent", o.clone()), decl(&mut vm, "NodeName", PropKind::Name)];
    let node = vm.space.classes.define("Engine.AnimNode", Some(object), props).unwrap();
    let weight = vec![decl(&mut vm, "Anim", o.clone()), decl(&mut vm, "Weight", PropKind::Float)];
    let child = vm.space.classes.define_struct("Engine.AnimNodeBlendBase.AnimBlendChild", None, weight).unwrap();
    let props = vec![decl(&mut vm, "Children", PropKind::Array(Box::new(PropKind::Struct(child))))];
    let base = vm.space.classes.define("Engine.AnimNodeBlendBase", Some(node), props).unwrap();
    let props = vec![
        decl(&mut vm, "Child2Weight", PropKind::Float),
        decl(&mut vm, "Child2WeightTarget", PropKind::Float),
        decl(&mut vm, "BlendTimeToGo", PropKind::Float),
        decl(&mut vm, "bSkipBlendWhenNotRendered", PropKind::Bool),
    ];
    let blend = vm.space.classes.define("Engine.AnimNodeBlend", Some(base), props).unwrap();
    let feather = vm.space.classes.define("WillowGame.WillowAnimNodeFeatherBlend", Some(blend), vec![]).unwrap();
    let per_bone = vm.space.classes.define("Engine.AnimNodeBlendPerBone", Some(blend), vec![]).unwrap();
    let list = vm.space.classes.define("Engine.AnimNodeBlendList", Some(base), vec![]).unwrap();
    let nodes = decl(&mut vm, "Nodes", PropKind::Array(Box::new(o))).with_flags(PropertyDecl::PARAM | PropertyDecl::OUT);
    let name = decl(&mut vm, "NodeName", PropKind::Name).with_flags(PropertyDecl::PARAM);
    let find = native(&mut vm, "Engine.AnimNode.FindAllAnimNodes", FIND_ALL_ANIM_NODES, node, vec![nodes, name]);
    let a = decl(&mut vm, "BlendTarget", PropKind::Float).with_flags(PropertyDecl::PARAM);
    let b = decl(&mut vm, "BlendTime", PropKind::Float).with_flags(PropertyDecl::PARAM);
    let blend_fn = native(&mut vm, "Engine.AnimNodeBlend.SetBlendTarget", SET_BLEND_TARGET, blend, vec![a, b]);
    let a = decl(&mut vm, "ChildIndex", PropKind::Int).with_flags(PropertyDecl::PARAM);
    let b = decl(&mut vm, "BlendTime", PropKind::Float).with_flags(PropertyDecl::PARAM);
    let active_child = native(&mut vm, "Engine.AnimNodeBlendList.SetActiveChild", SET_ACTIVE_CHILD_KEY, list, vec![a, b]);
    register(&mut vm).unwrap();
    Fx { vm, find, blend_fn, active_child, component, node, blend, feather, per_bone, list, child }
}

impl Fx {
    fn make(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
    }

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

    fn name(&mut self, s: &str) -> Value {
        Value::Name(self.vm.space.names.intern(s))
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    fn refusal(&mut self, f: FuncId, this: ObjRef, args: Vec<Value>) -> VmError {
        let before = self.marks();
        let e = self.vm.call(f, Some(this), args).unwrap_err();
        assert_eq!(self.marks(), before, "a refusal writes nothing");
        e
    }

    /// A tree root with tick-list nodes named `names`, its live component pointing back at it.
    fn tree(&mut self, names: &[&str]) -> (ObjRef, ObjRef, Vec<ObjRef>) {
        let (node_class, comp_class) = (self.node, self.component);
        let root = self.make(node_class, "Root");
        let comp = self.make(comp_class, "Mesh");
        let mut nodes = vec![root];
        for (i, n) in names.iter().enumerate() {
            let x = self.make(node_class, &format!("Node_{i}"));
            let v = self.name(n);
            self.set(x, "NodeName", v);
            nodes.push(x);
        }
        self.set(root, "SkelComponent", Value::Object(Some(comp)));
        self.set(comp, "Animations", Value::Object(Some(root)));
        self.set(comp, "AnimTickArray", Value::array(nodes.iter().map(|&n| Value::Object(Some(n))).collect()));
        (root, comp, nodes)
    }

    fn find(&mut self, root: ObjRef, prefilled: Vec<Value>, name: &str) -> Result<Value, VmError> {
        let n = self.name(name);
        let f = self.find;
        Ok(self.vm.call(f, Some(root), vec![Value::array(prefilled), n])?.outs[0].clone())
    }

    /// A blend node of `class` with two children (weights 0.5 / 0.5).
    fn blend_node(&mut self, class: ClassId, name: &str) -> ObjRef {
        let n = self.make(class, name);
        let kids = (0..2).map(|_| Value::structure(self.child, vec![Value::Object(None), Value::Float(0.5)])).collect();
        self.set(n, "Children", Value::array(kids));
        n
    }

    fn weights(&self, n: ObjRef) -> Vec<f32> {
        self.get(n, "Children")
            .elements()
            .unwrap()
            .iter()
            .map(|c| match &c.as_struct().unwrap().fields[1] {
                Value::Float(w) => *w,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    fn float(&self, n: ObjRef, name: &str) -> f32 {
        match self.get(n, name) {
            Value::Float(f) => f,
            other => panic!("{other:?}"),
        }
    }

    fn blend(&mut self, n: ObjRef, target: f32, time: f32) -> Result<(), VmError> {
        let f = self.blend_fn;
        self.vm.call(f, Some(n), vec![Value::Float(target), Value::Float(time)]).map(|_| ())
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// 2.A tests 1 to 3: the tick list's matches in list order; no match leaves `Nodes` unchanged; a pre-filled `Nodes`
/// gets the matches appended.
#[test]
fn find_all_anim_nodes_reads_the_live_tick_list() {
    let mut x = fx();
    let (root, _, nodes) = x.tree(&["X", "Y", "X"]);
    let got = x.find(root, vec![], "X").unwrap();
    assert_eq!(got, Value::array(vec![Value::Object(Some(nodes[1])), Value::Object(Some(nodes[3]))]));
    x.name("Z");
    let before = x.marks();
    assert_eq!(x.find(root, vec![], "Z").unwrap(), Value::array(vec![]), "unchanged");
    assert_eq!(x.marks(), before);
    let pre = vec![Value::Object(Some(root))];
    let got = x.find(root, pre, "Y").unwrap();
    assert_eq!(got, Value::array(vec![Value::Object(Some(root)), Value::Object(Some(nodes[2]))]), "appended");
}

/// 2.A test 4: an empty tick list, another tree, or no `SkelComponent`: the `NO_TICK_LIST` `NotRead`; a lazy
/// component: the not-loaded `NotRead`. `Nodes` unchanged.
#[test]
fn find_all_anim_nodes_without_a_live_list_is_not_read() {
    let mut x = fx();
    let (root, comp, _) = x.tree(&["X"]);
    let f = x.find;
    let args = |x: &mut Fx| vec![Value::array(vec![]), x.name("X")];
    x.set(comp, "AnimTickArray", Value::array(vec![]));
    let a = args(&mut x);
    assert_eq!(not_read(x.refusal(f, root, a)), NO_TICK_LIST, "empty");
    let (root, comp, _) = x.tree(&["X"]);
    let other = x.make(x.node, "OtherRoot");
    x.set(comp, "Animations", Value::Object(Some(other)));
    let a = args(&mut x);
    assert_eq!(not_read(x.refusal(f, root, a)), NO_TICK_LIST, "another tree");
    x.set(root, "SkelComponent", Value::Object(None));
    let a = args(&mut x);
    assert_eq!(not_read(x.refusal(f, root, a)), NO_TICK_LIST, "no component");
    let class = x.component;
    let lazy = x.vm.space.alloc_stand_in(class, "LazyMesh", Origin::Default { class }).unwrap();
    x.set(root, "SkelComponent", Value::Object(Some(lazy)));
    let a = args(&mut x);
    assert_eq!(not_read(x.refusal(f, root, a)), FIND_NOT_LOADED, "lazy");
}

/// 2.B tests 1, 2 and 5: the immediate blend's five writes (0.25 gives 0.75 / 0.25), the clamp, and the feather step.
#[test]
fn set_blend_target_immediate_clamp_and_feather() {
    let mut x = fx();
    let n = x.blend_node(x.blend, "Blend_0");
    x.blend(n, 0.25, 0.0).unwrap();
    assert_eq!((x.float(n, "Child2WeightTarget"), x.float(n, "Child2Weight"), x.float(n, "BlendTimeToGo")), (0.25, 0.25, 0.0));
    assert_eq!(x.weights(n), [0.75, 0.25]);
    x.blend(n, 2.0, 0.0).unwrap();
    assert_eq!((x.float(n, "Child2WeightTarget"), x.weights(n)), (1.0, vec![0.0, 1.0]), "clamped");
    // Maya's shape: a feather blend with (1.0, 0.0).
    let f = x.blend_node(x.feather, "UBMoveBlend");
    x.blend(f, 1.0, 0.0).unwrap();
    assert_eq!((x.float(f, "Child2WeightTarget"), x.float(f, "Child2Weight"), x.float(f, "BlendTimeToGo")), (1.0, 1.0, 0.0));
    assert_eq!(x.weights(f), [1.0, 1.0], "the feather step sets child 0 to 1 last");
    let f = x.blend_node(x.feather, "UBMoveBlend_1");
    x.blend(f, 0.25, 0.0).unwrap();
    assert_eq!(x.weights(f), [1.0, 0.25]);
}

/// 2.B tests 3 and 4: a timed blend writes the target and `BlendTimeToGo` only; with `bSkipBlendWhenNotRendered` set
/// and `bRecentlyRendered` clear, the engine flag's `NotRead`; with it set, the timed blend; no component there is
/// `NotRead`.
#[test]
fn set_blend_target_timed_and_the_not_rendered_test() {
    let mut x = fx();
    let n = x.blend_node(x.blend, "Blend_1");
    x.blend(n, 0.5, 0.3).unwrap();
    assert_eq!((x.float(n, "Child2WeightTarget"), x.float(n, "BlendTimeToGo"), x.float(n, "Child2Weight")), (0.5, 0.3, 0.0));
    assert_eq!(x.weights(n), [0.5, 0.5], "children untouched");
    let n = x.blend_node(x.blend, "Blend_2");
    x.set(n, "bSkipBlendWhenNotRendered", Value::Bool(true));
    let f = x.blend_fn;
    assert_eq!(not_read(x.refusal(f, n, vec![Value::Float(0.5), Value::Float(0.3)])), NOT_RENDERED_COMPONENT);
    let comp = x.make(x.component, "Mesh_1");
    x.set(n, "SkelComponent", Value::Object(Some(comp)));
    assert_eq!(not_read(x.refusal(f, n, vec![Value::Float(0.5), Value::Float(0.3)])), NOT_RENDERED_FLAG);
    x.set(comp, "bRecentlyRendered", Value::Bool(true));
    x.blend(n, 0.5, 0.3).unwrap();
    assert_eq!(x.float(n, "BlendTimeToGo"), 0.3);
    // An immediate blend does not read the not-rendered test at all.
    x.set(comp, "bRecentlyRendered", Value::Bool(false));
    x.blend(n, 0.5, 0.0).unwrap();
    assert_eq!(x.weights(n), [0.5, 0.5]);
}

/// 2.B tests 6 and 7: one child on the immediate path is `VmError::Script`; an unread subclass is its `NotRead`.
#[test]
fn set_blend_target_one_child_and_an_unread_subclass() {
    let mut x = fx();
    let n = x.make(x.blend, "OneChild");
    let one = Value::array(vec![Value::structure(x.child, vec![Value::Object(None), Value::Float(1.0)])]);
    x.set(n, "Children", one);
    let f = x.blend_fn;
    match x.refusal(f, n, vec![Value::Float(0.5), Value::Float(0.0)]) {
        VmError::Script { message, .. } => assert!(message.contains("two children"), "{message}"),
        other => panic!("{other:?}"),
    }
    let p = x.blend_node(x.per_bone, "PerBone");
    assert_eq!(not_read(x.refusal(f, p, vec![Value::Float(0.5), Value::Float(0.0)])), unread_body("Engine.AnimNodeBlendPerBone"));
}

/// 2.C: `SetActiveChild` is `NotRead`, nothing written.
#[test]
fn set_active_child_is_not_read() {
    let mut x = fx();
    let n = x.make(x.list, "List");
    let f = x.active_child;
    assert_eq!(not_read(x.refusal(f, n, vec![Value::Int(1), Value::Float(0.0)])), SET_ACTIVE_CHILD);
}

/// Review of #739, item 1: a plain `AnimNodeBlend` on the timed path with a malformed `Children` (an element that is
/// not the child struct, or a child struct whose `Weight` is no float) is `VmError::Script`, nothing written.
#[test]
fn a_malformed_children_is_a_script_error_on_every_path() {
    let mut x = fx();
    let n = x.blend_node(x.blend, "Malformed");
    let class = x.vm.space.heap.get(n).unwrap().class;
    let p = x.vm.space.prop(class, "Children").unwrap();
    *x.vm.space.slot_mut(n, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Int(1), Value::Int(2)]);
    let f = x.blend_fn;
    for (target, time) in [(0.5, 0.3), (0.5, 0.0)] {
        match x.refusal(f, n, vec![Value::Float(target), Value::Float(time)]) {
            VmError::Script { message, .. } => assert!(message.contains("Children[0]"), "{message}"),
            other => panic!("{other:?}"),
        }
    }
    let bad_weight = Value::structure(x.child, vec![Value::Object(None), Value::Int(1)]);
    *x.vm.space.slot_mut(n, PropAddr::scalar(p)).unwrap() = Value::array(vec![bad_weight.clone(), bad_weight]);
    match x.refusal(f, n, vec![Value::Float(0.5), Value::Float(0.3)]) {
        VmError::Script { message, .. } => assert!(message.contains("float Weight"), "{message}"),
        other => panic!("{other:?}"),
    }
}

/// Review of #739, item 2: a NaN or infinite `BlendTime` or `BlendTarget` is `VmError::Script` (the port's choice,
/// UNVERIFIED), nothing written.
#[test]
fn a_non_finite_argument_is_refused() {
    let mut x = fx();
    let n = x.blend_node(x.blend, "Finite");
    let f = x.blend_fn;
    for (target, time) in [(0.5, f32::NAN), (f32::NAN, 0.0), (0.5, f32::INFINITY), (f32::NEG_INFINITY, 0.0)] {
        match x.refusal(f, n, vec![Value::Float(target), Value::Float(time)]) {
            VmError::Script { message, .. } => assert!(message.contains("not finite"), "{message}"),
            other => panic!("{other:?}"),
        }
    }
}

/// Review of #739, item 3: a `Nodes` out argument holding no array is a data error, not an empty list.
#[test]
fn a_nodes_argument_that_is_no_array_is_a_script_error() {
    let mut x = fx();
    let (root, _, _) = x.tree(&["X"]);
    let name = x.name("X");
    let f = x.find;
    let before = x.marks();
    match x.vm.call(f, Some(root), vec![Value::Int(3), name]) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("Nodes holds"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before);
}
