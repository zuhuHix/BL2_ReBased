//! Tests of the gestalt component data layer in Off (lane IMPL-GESTALT-DATA-LAYER) on #596's invented `Engine.upk`,
//! extended with the container's seven members and three serialized gestalt definitions (AGENTS.md rule 9). The
//! numbered tests follow NATIVE_GESTALT_DATA_LAYER.md section 6's list. Every value is invented. AI-assisted (Claude
//! Code, Opus 5.5).

use ow_core::Missing;
use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;

use super::*;
use crate::gestalt::tests::{
    FULL_BOUNDS, FULL_PARTS, FULL_SOCKETS, Fx, Mis, fx, fx_mis, not_read, not_yet, quiet, script, some,
};
use crate::gestalt::{STAND_IN, build_composite_mesh, init_gestalt_mesh_data};

fn f(x: &Fx, v: &Value, name: &str) -> Value {
    ow_attr::eval::field(&x.vm.space, v, name).unwrap().clone()
}

fn arr(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.as_ref().clone(),
        other => panic!("{other:?}"),
    }
}

fn slot(x: &Fx, c: ObjRef) -> Value {
    crate::equip::read_prop(&x.vm, c, "GestaltData").unwrap()
}

/// Per LOD, the fragments' (material, first index, primitives).
fn fragments(x: &Fx, v: &Value) -> Vec<Vec<(i32, i32, i32)>> {
    let int = |v: Value| match v {
        Value::Int(i) => i,
        other => panic!("{other:?}"),
    };
    arr(&f(x, v, "LODModelData"))
        .iter()
        .map(|lod| {
            arr(&f(x, lod, "GestaltFragments"))
                .iter()
                .map(|fr| (int(f(x, fr, "MaterialIndex")), int(f(x, fr, "FirstIndex")), int(f(x, fr, "NumPrimitives"))))
                .collect()
        })
        .collect()
}

fn sockets(x: &Fx, v: &Value) -> Vec<(NameId, NameId)> {
    let name = |v: Value| match v {
        Value::Name(n) => n,
        other => panic!("{other:?}"),
    };
    arr(&f(x, v, "SocketMappings"))
        .iter()
        .map(|m| (name(f(x, m, "OriginalSocketName")), name(f(x, m, "MangledSocketName"))))
        .collect()
}

fn bounds(x: &Fx, v: &Value) -> Bounds {
    read_bounds(&x.vm, &f(x, v, "ReferencePoseBounds"), "test").unwrap()
}

fn full_bounds(i: usize) -> Bounds {
    let (_, origin, extent, radius) = FULL_BOUNDS[i];
    Bounds { origin, extent, radius }
}

/// `ToyGestaltFull` with the shared mesh set, and component 1.
fn full(x: &mut Fx) -> (ObjRef, ObjRef, ObjRef) {
    let (g, shared, c) = (x.o("ToyGestaltFull"), x.o("ToyShared"), x.o("ToyComp1"));
    x.set(g, "GestaltSkeletalMesh", some(shared));
    (g, shared, c)
}

fn names(x: &mut Fx, list: &[&str]) -> Vec<NameId> {
    list.iter().map(|n| x.name(n)).collect()
}

fn fill(x: &mut Fx, c: ObjRef, g: ObjRef, list: &[&str]) {
    let n = names(x, list);
    plan_fill(&x.vm, c, g, &n).unwrap().apply(&mut x.vm).unwrap();
}

/// `GestaltData`'s `GestaltAccessoryList` given one invented entry and its matrices object set: the state the fill
/// must keep.
fn give_accessories(x: &mut Fx, c: ObjRef) -> (Value, Value) {
    let ty = x.vm.space.classes.struct_by_path("Engine.SkeletalMeshComponent.ToyAccessory").unwrap();
    let mut entry = x.vm.space.classes.struct_defaults(ty).unwrap();
    put(&x.vm, &mut entry, "TransformIndex", Value::Int(7), "test").unwrap();
    let list = Value::array(vec![entry]);
    let matrices = some(x.o("ToyMeshObj"));
    let mut v = slot(x, c);
    put(&x.vm, &mut v, "GestaltAccessoryList", list.clone(), "test").unwrap();
    put(&x.vm, &mut v, "GestaltPartMatricesContainer", matrices.clone(), "test").unwrap();
    x.set(c, "GestaltData", v);
    (list, matrices)
}

/// Edits `g`'s array `name` with `edit` on a copy, then writes it back.
fn edit_array(x: &mut Fx, g: ObjRef, name: &str, edit: impl FnOnce(&Fx, &mut Vec<Value>)) {
    let mut items = arr(&crate::equip::read_prop(&x.vm, g, name).unwrap());
    edit(x, &mut items);
    x.set(g, name, Value::array(items));
}

/// Test 1, the fill's order: N with a name twice; `PartMeshNames` equal to N; per LOD the fragments in the parts'
/// order, one per part; the socket mappings grouped in N's order, the duplicate's twice; the bounds copied then
/// unioned in N's order; the accessory list and the matrices object kept. The definition is the serialized one, so
/// the store's decode and F2's walk both pass on it.
#[test]
fn the_fill_follows_the_notes_order() {
    let mut x = fx();
    let (g, shared, c) = full(&mut x);
    let (list, matrices) = give_accessories(&mut x, c);
    fill(&mut x, c, g, &["Toy_B", "Toy_A", "Toy_B"]);
    let v = slot(&x, c);
    let n = names(&mut x, &["Toy_B", "Toy_A", "Toy_B"]);
    assert_eq!(f(&x, &v, "PartMeshNames"), Value::array(n.iter().map(|n| Value::Name(*n)).collect()));
    assert_eq!(f(&x, &v, "SkeletalMesh"), some(shared));
    let lod = |i: usize, k: usize| (FULL_PARTS[i][k].1, FULL_PARTS[i][k].2, FULL_PARTS[i][k].3);
    assert_eq!(fragments(&x, &v), vec![vec![lod(0, 0), lod(0, 1)], vec![lod(1, 0)]]);
    let sock = |x: &mut Fx, i: usize| (x.name(FULL_SOCKETS[i].1), x.name(FULL_SOCKETS[i].2));
    let want = vec![sock(&mut x, 0), sock(&mut x, 1), sock(&mut x, 2), sock(&mut x, 0)];
    assert_eq!(sockets(&x, &v), want);
    let (a, b) = (full_bounds(0), full_bounds(1));
    assert_eq!(bounds(&x, &v), union(union(b, a), b), "B copied whole, then A, then B again unioned");
    assert_eq!(f(&x, &v, "GestaltAccessoryList"), list);
    assert_eq!(f(&x, &v, "GestaltPartMatricesContainer"), matrices);
}

/// Test 2, the union in the note's operation order, exact in f32: the distance sum smaller than the half diagonal,
/// the half diagonal smaller, and a negative extent (the per-axis normalised box).
#[test]
fn the_union_is_exact_in_single_precision() {
    let b = |o: [f32; 3], e: [f32; 3], r: f32| Bounds { origin: o, extent: e, radius: r };
    // x spans -1..4: half 2.5, centre 1.5; y and z -1..1. Both distance sums are 1.5 + 1 = 2.5 < sqrt(8.25).
    let u = union(b([0.0; 3], [1.0; 3], 1.0), b([3.0, 0.0, 0.0], [1.0; 3], 1.0));
    assert_eq!(u, b([1.5, 0.0, 0.0], [2.5, 1.0, 1.0], 2.5));
    // The same box twice with radius 10: the half diagonal sqrt(3), rounded to f32 (through ow_core::num, rule N3).
    let u = union(b([0.0; 3], [1.0; 3], 10.0), b([0.0; 3], [1.0; 3], 10.0));
    assert_eq!(u, b([0.0; 3], [1.0; 3], (ow_core::num::sqrt_f64(3.0) as f32)));
    assert_eq!(u.radius.to_bits(), (ow_core::num::sqrt_f64(3.0) as f32).to_bits());
    // A negative extent: min and max of both ends per axis, so the box is -1..1, not the stock sum's.
    let u = union(b([0.0; 3], [-1.0; 3], 1.0), b([0.0; 3], [0.5; 3], 0.5));
    assert_eq!(u, b([0.0; 3], [1.0; 3], 1.0));
}

/// Test 3, `GestaltSkeletalMesh` None, by calling the fill routine directly: names and mesh written, both arrays
/// emptied, the bounds and the other two members kept.
#[test]
fn no_shared_mesh_stops_after_the_arrays() {
    let mut x = fx();
    let (g, _, c) = full(&mut x);
    let (list, matrices) = give_accessories(&mut x, c);
    fill(&mut x, c, g, &["Toy_A"]);
    let before = slot(&x, c);
    x.set(g, "GestaltSkeletalMesh", Value::Object(None));
    let n = names(&mut x, &["Toy_B"]);
    let v = fill_value(&x.vm, &before, g, &n).unwrap();
    assert_eq!(f(&x, &v, "PartMeshNames"), Value::array(vec![Value::Name(n[0])]));
    assert_eq!(f(&x, &v, "SkeletalMesh"), Value::Object(None));
    assert!(arr(&f(&x, &v, "LODModelData")).is_empty() && arr(&f(&x, &v, "SocketMappings")).is_empty());
    assert_eq!(bounds(&x, &v), full_bounds(0), "the bounds kept");
    assert_eq!((f(&x, &v, "GestaltAccessoryList"), f(&x, &v, "GestaltPartMatricesContainer")), (list, matrices));
}

/// Test 4, no match at all: one empty fragment list per gestalt info, no mappings, the start constant with radius 0.
#[test]
fn no_match_gives_empty_lods_and_the_start_bounds() {
    let mut x = fx();
    let (g, _, c) = full(&mut x);
    fill(&mut x, c, g, &["Toy_Q"]);
    let v = slot(&x, c);
    assert_eq!(fragments(&x, &v), vec![Vec::new(), Vec::new()]);
    assert!(sockets(&x, &v).is_empty());
    assert_eq!(bounds(&x, &v), Bounds { origin: [0.0; 3], extent: [0.0; 3], radius: 0.0 });
}

/// Test 5, the refusals, each before any write with #518's three counters unchanged: F1(i), F1(ii), F1(iii); F2 on
/// a definition whose part holds a tag the walk leaves raw (it must refuse, not fill); a wrong member declaration;
/// F1 and F2 both failing, F1 first.
#[test]
fn the_refusals_come_before_any_write() {
    let mut x = fx();
    let ty = x.o("ToyType");
    let shared = x.o("ToyShared");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    let point = |x: &mut Fx, g: &str| {
        let g = x.o(g);
        x.set(g, "GestaltSkeletalMesh", some(shared));
        x.set(ty, "GestaltMesh", some(g));
    };
    let call = |x: &mut Fx, c: ObjRef| {
        let data = data.clone();
        let before = x.snapshot();
        let e = quiet(&mut x.vm, |vm| build_composite_mesh(vm, Some(c), &data));
        assert_eq!(x.snapshot(), before, "nothing written: {e}");
        e
    };
    point(&mut x, "ToyGestaltFull");
    // F1(iii): a malformed slot.
    let c1 = x.o("ToyComp1");
    let class = x.vm.space.heap.get(c1).unwrap().class;
    let p = x.vm.space.prop(class, "GestaltData").unwrap();
    let good = slot(&x, c1);
    let mut bad = good.clone();
    let at = crate::gestalt::tests::part_mesh_names(&x, &bad);
    bad.struct_mut().unwrap().fields[at] = Value::array(vec![Value::Int(3)]);
    *x.vm.space.slot_mut(c1, PropAddr::scalar(p)).unwrap() = bad;
    assert_eq!(script(call(&mut x, c1)), BUILD_MISMATCH);
    *x.vm.space.slot_mut(c1, PropAddr::scalar(p)).unwrap() = good;
    // F1(ii): a stand-in component (the hub's ruling on #603), with this note's record.
    let ghost = x.vm.space.alloc_stand_in(class, "ToyGhostComp", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    let n = not_yet(call(&mut x, ghost));
    assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, STAND_IN, DATA_RECORD));
    // F2: the raw tag inside the first part.
    point(&mut x, "ToyGestaltRaw");
    assert_eq!(not_read(call(&mut x, c1)), ARRAYS_NOT_DECODED);
    // F1(i) and F2 both failing: F1 first.
    let c2 = x.o("ToyComp2");
    assert!(x.vm.defs.is_loaded(&x.vm.space, c2));
    x.vm.space.heap.get_mut(c2).unwrap().slots.clear();
    assert_eq!(not_read(call(&mut x, c2)), BUILD_NOT_DECODED, "F1(i) before F2");
    // A definition class without the arrays: a data error naming the first missing member.
    point(&mut x, "ToyGestaltBare");
    assert!(script(call(&mut x, c1)).contains("GestaltInfos"));
}

/// Test 6, a refill: a second fill replaces the LOD and socket data, and the bounds restart from the start constant.
#[test]
fn a_refill_replaces_the_data_and_restarts_the_bounds() {
    let mut x = fx();
    let (g, _, c) = full(&mut x);
    fill(&mut x, c, g, &["Toy_A"]);
    fill(&mut x, c, g, &["Toy_B"]);
    let v = slot(&x, c);
    assert_eq!(fragments(&x, &v), vec![vec![(2, 30, 5)], vec![(4, 0, 3)]]);
    let want = vec![(x.name("SockB1"), x.name("MangB1"))];
    assert_eq!(sockets(&x, &v), want);
    assert_eq!(bounds(&x, &v), full_bounds(1), "B copied whole, not unioned with A's");
}

/// Test 7, a first matching bounds entry with radius 0: the next match is copied whole, not unioned.
#[test]
fn a_zero_radius_lets_the_next_match_be_copied() {
    let mut x = fx();
    let (g, _, c) = full(&mut x);
    edit_array(&mut x, g, "GestaltPartBounds", |x, items| {
        let mut rb = f(x, &items[0], "ReferencePoseBounds");
        put(&x.vm, &mut rb, "SphereRadius", Value::Float(0.0), "test").unwrap();
        put(&x.vm, &mut items[0], "ReferencePoseBounds", rb, "test").unwrap();
    });
    fill(&mut x, c, g, &["Toy_A", "Toy_B"]);
    assert_eq!(bounds(&x, &slot(&x, c)), full_bounds(1));
}

/// Test 8, name numbers: a list name `Toy_A_1` does not match a part `Toy_A`, and a part `Toy_C_1` does not match a
/// list name `Toy_C`.
#[test]
fn names_match_with_their_numbers() {
    let mut x = fx();
    let (g, _, c) = full(&mut x);
    fill(&mut x, c, g, &["Toy_A_1"]);
    assert_eq!(fragments(&x, &slot(&x, c)), vec![Vec::new(), Vec::new()]);
    let c_1 = x.name("Toy_C_1");
    edit_array(&mut x, g, "GestaltInfos", |x, items| {
        let mut parts = arr(&f(x, &items[0], "Parts"));
        put(&x.vm, &mut parts[2], "SkeletalMeshFragmentName", Value::Name(c_1), "test").unwrap();
        put(&x.vm, &mut items[0], "Parts", Value::array(parts), "test").unwrap();
    });
    fill(&mut x, c, g, &["Toy_C"]);
    assert_eq!(fragments(&x, &slot(&x, c)), vec![Vec::new(), Vec::new()]);
}

/// Test 9, the copy: after a fill, an equal value (the references the same objects); a copy from a stand-in is
/// F1(ii); a self-copy with the layer present writes nothing.
#[test]
fn the_copy_hands_the_value_on_whole() {
    let mut x = fx();
    let (g, _, c1) = full(&mut x);
    fill(&mut x, c1, g, &["Toy_A", "Toy_B"]);
    let c2 = x.o("ToyComp2");
    init_gestalt_mesh_data(&mut x.vm, c2, Some(c1)).unwrap();
    assert_eq!(slot(&x, c2), slot(&x, c1));
    let class = x.vm.space.heap.get(c1).unwrap().class;
    let ghost = x.vm.space.alloc_stand_in(class, "ToyGhostComp", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    let e = quiet(&mut x.vm, |vm| init_gestalt_mesh_data(vm, c2, Some(ghost)));
    let n = not_yet(e);
    assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, INIT_STAND_IN, DATA_RECORD));
    let before = x.snapshot();
    let mark = crate::equip::card::load_mark(&x.vm);
    init_gestalt_mesh_data(&mut x.vm, c1, Some(c1)).unwrap();
    assert_eq!((x.snapshot(), crate::equip::card::load_mark(&x.vm)), (before, mark));
}

/// The fill through `BuildCompositeMesh`: one heap write (the whole slot), no load, no draw; the shared mesh answers.
#[test]
fn the_native_writes_the_slot_once() {
    let mut x = fx();
    let (g, shared, c) = full(&mut x);
    let (ty, a) = (x.o("ToyType"), x.o("ToyPartA"));
    x.set(ty, "GestaltMesh", some(g));
    let n = x.name("Toy_A");
    x.set(a, "bIsGestaltMode", Value::Bool(true));
    x.set(a, "GestaltModeSkeletalMeshName", Value::Name(n));
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("BodyPartDefinition", some(a))]);
    let (w, draws, loads) = crate::equip::card::load_mark(&x.vm);
    assert_eq!(build_composite_mesh(&mut x.vm, Some(c), &data).unwrap(), Some(shared));
    assert_eq!(crate::equip::card::load_mark(&x.vm), (w + 1, draws, loads), "one write, no draw, no load");
    assert_eq!(fragments(&x, &slot(&x, c)), vec![vec![(1, 0, 10)], Vec::new()]);
}

/// Points `ToyType` at definition `g` (its shared mesh `mesh`) and calls `BuildCompositeMesh` on `c`, asserting the
/// heap unchanged and #518's counters still (`quiet`): the refusal.
fn refuse(x: &mut Fx, g: &str, mesh: ObjRef, c: ObjRef) -> String {
    let ty = x.o("ToyType");
    let g = x.o(g);
    x.set(g, "GestaltSkeletalMesh", some(mesh));
    x.set(ty, "GestaltMesh", some(g));
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    let before = x.snapshot();
    let e = quiet(&mut x.vm, |vm| build_composite_mesh(vm, Some(c), &data));
    assert_eq!(x.snapshot(), before, "nothing written: {e}");
    script(e)
}

/// Review r1 of #613, item 1: a container member of the wrong kind is a data error naming it, and the component's
/// declaration is checked before F1 (a slotless store component still gets the declaration's error, not F1(i)).
#[test]
fn a_mis_declared_container_member_is_a_data_error_before_f1() {
    let mut x = fx_mis(Mis::BoundsInt);
    let (shared, c1, c2) = (x.o("ToyShared"), x.o("ToyComp1"), x.o("ToyComp2"));
    let text = refuse(&mut x, "ToyGestalt", shared, c1);
    assert!(text.contains("GestaltData.ReferencePoseBounds") && text.contains("not declared as"), "{text}");
    x.vm.space.heap.get_mut(c2).unwrap().slots.clear();
    assert!(x.vm.defs.is_loaded(&x.vm.space, c2), "a slotless store component: F1(i) if F1 came first");
    let text = refuse(&mut x, "ToyGestalt", shared, c2);
    assert!(text.contains("GestaltData.ReferencePoseBounds"), "the declaration before F1: {text}");
}

/// Review r1 of #613, item 1: a definition array with the wrong element type is a data error naming it.
#[test]
fn a_mis_declared_definition_array_is_a_data_error() {
    let mut x = fx_mis(Mis::InfosInt);
    let (shared, c) = (x.o("ToyShared"), x.o("ToyComp1"));
    let text = refuse(&mut x, "ToyGestalt", shared, c);
    assert!(text.contains("ToyGestaltDefinition.GestaltInfos") && text.contains("not declared as"), "{text}");
}

/// Review r1 of #613, item 1: a definition slot that is not complete (placed past the heap's write checks, a fixture
/// of a malformed decode) is a data error naming it.
#[test]
fn an_incomplete_definition_slot_is_a_data_error() {
    let mut x = fx();
    let (shared, c) = (x.o("ToyShared"), x.o("ToyComp1"));
    let g = x.o("ToyGestaltFull");
    let class = x.vm.space.heap.get(g).unwrap().class;
    let p = x.vm.space.prop(class, "GestaltInfos").unwrap();
    *x.vm.space.slot_mut(g, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Int(3)]);
    let text = refuse(&mut x, "ToyGestaltFull", shared, c);
    assert_eq!(text, "the gestalt definition's GestaltInfos does not match its declaration");
}

/// Review r1 of #613, item 1: a planned value the heap's own write rules refuse is a data error, before the write.
/// The container's `SkeletalMesh` is declared as the mesh class; a shared mesh of another class cannot be written.
#[test]
fn a_planned_value_the_heap_refuses_is_a_data_error() {
    let mut x = fx();
    let (part, c) = (x.o("ToyPartA"), x.o("ToyComp1"));
    let text = refuse(&mut x, "ToyGestaltFull", part, c);
    assert!(text.starts_with("the GestaltData write would be refused: "), "{text}");
}
