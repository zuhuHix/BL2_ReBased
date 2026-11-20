//! Owner: lane IMPL-GESTALT-DATA-LAYER. AI-assisted (Claude Code, Opus 5.5). The gestalt component data layer in Off:
//! the fill `BuildCompositeMesh` makes on its gestalt path, and the whole-slot copy of `InitGestaltMeshDataFromOther`.
//! Coded from NATIVE_GESTALT_DATA_LAYER.md alone (#603, at 9e5113c0, with the hub's ruling on #603). This lane read
//! neither native (clean room). Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! **The layer** is the component's `GestaltData` slot in the heap: one `GestaltDataContainer` value (the note's
//! section 6). Both natives write it as one whole-slot heap write, after a preflight that writes, loads and draws
//! nothing. So neither ever refuses after a write of its own, and none of its refusals carries the "stopped after
//! partial writes: " prefix; the caller's earlier writes can (#518's counters).
//!
//! **The fill's preflight** ([`plan_fill`]), after #588's checks 1 to 5 (`crate::gestalt::build_composite_mesh`), in
//! the note's order; the first failure is the answer:
//! 1. the component's `GestaltData` declares the seven members of the note's section 1 at their declared types, nested
//!    structs included ([`CONTAINER`]), else a data error naming the member;
//! 2. F1, the slot held whole ([`held_whole`]): F1(i) a store definition whose slots were never decoded
//!    ([`BUILD_NOT_DECODED`]), F1(ii) a slotless stand-in that never loads (#596's [`crate::gestalt::STAND_IN`], the
//!    hub's ruling on #603), F1(iii) a slot that does not match its declaration (a data error, [`BUILD_MISMATCH`]);
//! 3. the gestalt definition's members ([`DEFINITION`]), else a data error naming the field; its slots must then hold
//!    complete values, else a data error;
//! 4. F2: the definition's three arrays decoded whole in the package walk ([`arrays_decoded`],
//!    [`ARRAYS_NOT_DECODED`]);
//! 5. the new value, worked out on a copy of the slot, checked against the declaration with the heap's own rules
//!    (`Heap::check_value`), so the write cannot fail.
//!
//! **The copy** ([`plan_copy`]): F1 on the receiver, then on the argument (texts [`NOT_DECODED`](crate::gestalt::NOT_DECODED),
//! [`INIT_STAND_IN`], [`INIT_MISMATCH`]); the argument's value checked against the receiver's declaration; one write.
//! The self-copy answers before this, with nothing written (`crate::gestalt::init_gestalt_mesh_data`).
//!
//! **Floats** (the note's section 2): every sum, difference and product is formed in f32, in the note's operation
//! order, and each root goes through `ow_core::num::sqrt_f64` and back to f32 (rule N3, as `ow-world` `placement.rs`
//! does). A double root of an f32, rounded to f32, is the correctly rounded single root. Bit parity with the game is
//! **UNVERIFIED**. The start constant is the zero vector, a port input (UNVERIFIED: the engine sets it at start-up).
//!
//! **F2's walk is a port answer for objects the store did not make.** A definition with no export origin (one a test
//! or the VM built in the heap) has no tags to walk, so its heap values are taken as they stand.

use ow_core::NotYet;
use ow_object::decode::ReflectionTyper;
use ow_object::heap::Origin;
use ow_object::ids::{NameId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::props::{PropValue, read_tagged};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::equip::bad;
use crate::gestalt::{INIT_RECORD, NOT_DECODED, STAND_IN};

/// The record the layer's own refusals follow.
pub const DATA_RECORD: &str = "docs/verification/NATIVE_GESTALT_DATA_LAYER.md";

/// F1(i) on the fill's component (#596's reading, in the fill's words).
pub const BUILD_NOT_DECODED: &str =
    "BuildCompositeMesh: a component whose properties the store has not decoded (a decode is a write)";
/// F1(ii) on the copy (the hub's ruling on #603).
pub const INIT_STAND_IN: &str =
    "InitGestaltMeshDataFromOther: a component that is a stand-in with no properties (an object the store cannot load)";
/// F1(iii), data errors.
pub const BUILD_MISMATCH: &str = "BuildCompositeMesh: the component's GestaltData does not match its declaration";
pub const INIT_MISMATCH: &str = "InitGestaltMeshDataFromOther: a GestaltData slot that does not match its declaration";
/// F2.
pub const ARRAYS_NOT_DECODED: &str =
    "BuildCompositeMesh: the gestalt definition's arrays did not decode whole (a decode is a write)";

const BUILD: &str = "WillowWeapon.BuildCompositeMesh";
const INIT: &str = "SkeletalMeshComponent.InitGestaltMeshDataFromOther";

/// A declared shape: what a member's declaration must be.
#[derive(Debug)]
pub enum Shape {
    Name,
    Int,
    Float,
    Object,
    /// A struct with at least these members, each of its shape (other members are allowed).
    Struct(&'static [(&'static str, Shape)]),
    /// A struct of any members.
    AnyStruct,
    Array(&'static Shape),
}

const VECTOR: Shape = Shape::Struct(&[("X", Shape::Float), ("Y", Shape::Float), ("Z", Shape::Float)]);
const BOUNDS: Shape = Shape::Struct(&[("Origin", VECTOR), ("BoxExtent", VECTOR), ("SphereRadius", Shape::Float)]);
const FRAGMENT: Shape =
    Shape::Struct(&[("MaterialIndex", Shape::Int), ("FirstIndex", Shape::Int), ("NumPrimitives", Shape::Int)]);
const LOD_MODEL: Shape = Shape::Struct(&[("GestaltFragments", Shape::Array(&FRAGMENT))]);
const SOCKET_REMAP: Shape = Shape::Struct(&[("OriginalSocketName", Shape::Name), ("MangledSocketName", Shape::Name)]);

/// The container's seven members (the note's section 1, declared order).
pub const CONTAINER: Shape = Shape::Struct(&[
    ("PartMeshNames", Shape::Array(&Shape::Name)),
    ("SkeletalMesh", Shape::Object),
    ("LODModelData", Shape::Array(&LOD_MODEL)),
    ("SocketMappings", Shape::Array(&SOCKET_REMAP)),
    ("GestaltAccessoryList", Shape::Array(&Shape::AnyStruct)),
    ("GestaltPartMatricesContainer", Shape::Object),
    ("ReferencePoseBounds", BOUNDS),
]);

const PART: Shape = Shape::Struct(&[
    ("SkeletalMeshFragmentName", Shape::Name),
    ("MaterialIndex", Shape::Int),
    ("FirstIndex", Shape::Int),
    ("NumPrimitives", Shape::Int),
]);
const INFO: Shape = Shape::Struct(&[("Parts", Shape::Array(&PART))]);
const SOCKET: Shape = Shape::Struct(&[
    ("SkeletalMeshFragmentName", Shape::Name),
    ("OriginalSocketName", Shape::Name),
    ("MangledSocketName", Shape::Name),
]);
const PART_BOUNDS: Shape = Shape::Struct(&[("SkeletalMeshFragmentName", Shape::Name), ("ReferencePoseBounds", BOUNDS)]);

/// The gestalt definition's members the fill reads (the note's section 6, check 2).
pub const DEFINITION: [(&str, Shape); 4] = [
    ("GestaltSkeletalMesh", Shape::Object),
    ("GestaltInfos", Shape::Array(&INFO)),
    ("GestaltSocketMappings", Shape::Array(&SOCKET)),
    ("GestaltPartBounds", Shape::Array(&PART_BOUNDS)),
];

/// The three arrays F2 walks.
const WALKED: [&str; 3] = ["GestaltInfos", "GestaltSocketMappings", "GestaltPartBounds"];

/// Whether declaration `kind` (dimension 1) has `shape`; else the path of the first member that does not.
fn conforms(vm: &Vm, kind: &PropKind, shape: &Shape, path: &str) -> Result<(), String> {
    let ok = match (shape, kind) {
        (Shape::Name, PropKind::Name)
        | (Shape::Int, PropKind::Int)
        | (Shape::Float, PropKind::Float)
        | (Shape::Object, PropKind::Object { .. })
        | (Shape::AnyStruct, PropKind::Struct(_)) => true,
        (Shape::Array(inner), PropKind::Array(k)) => return conforms(vm, k, inner, path),
        (Shape::Struct(members), PropKind::Struct(s)) => {
            let layout = vm.space.classes.struct_layout(*s).ok_or_else(|| path.to_string())?;
            for (name, sub) in members.iter() {
                let at = format!("{path}.{name}");
                let decl = vm.space.names.find(name).and_then(|n| layout.field(n)).map(|(_, d)| d).ok_or_else(|| at.clone())?;
                if decl.array_dim != 1 {
                    return Err(at);
                }
                conforms(vm, &decl.kind, sub, &at)?;
            }
            true
        }
        _ => false,
    };
    if ok { Ok(()) } else { Err(path.to_string()) }
}

/// `o`'s declared property `name` with `shape`: its id and declared kind, else a data error naming it.
fn shaped(vm: &Vm, o: ObjRef, name: &str, shape: &Shape, what: &str) -> Result<(ow_object::ids::PropId, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let class_path = ow_attr::eval::class_path(&vm.space, o);
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{class_path} declares no {name}")))?;
    let decl = vm.space.classes.decl(p).ok_or_else(|| bad(what, format!("{name} has no declaration")))?;
    if decl.array_dim != 1 {
        return Err(bad(what, format!("{class_path}.{name} is a static array")));
    }
    conforms(vm, &decl.kind, shape, name)
        .map_err(|at| bad(what, format!("{class_path}.{at} is not declared as the gestalt data layer reads it")))?;
    Ok((p, decl.kind.clone()))
}

/// Which native a check speaks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Native {
    Build,
    Init,
}

/// F1 (the note's section 6): `o`'s `GestaltData` slot is held whole, read as it stands, with no decode and no load.
pub fn held_whole(vm: &Vm, o: ObjRef, native: Native) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = u32::try_from(x.slots.len()).ok() != vm.space.classes.get_layout(x.class).map(|l| l.slot_count());
    if slotless {
        // F1(i): a store definition holds data it has not decoded. F1(ii): any other slotless object is a stand-in.
        let loaded = vm.defs.is_loaded(&vm.space, o);
        let e = match (native, loaded) {
            (Native::Build, true) => NotYet::not_read(BUILD_NOT_DECODED, crate::gestalt::BUILD_RECORD),
            (Native::Init, true) => NotYet::not_read(NOT_DECODED, INIT_RECORD),
            (Native::Build, false) => NotYet::not_read(STAND_IN, DATA_RECORD),
            (Native::Init, false) => NotYet::not_read(INIT_STAND_IN, DATA_RECORD),
        };
        return Err(e.into());
    }
    let (what, text) = match native {
        Native::Build => (BUILD, BUILD_MISMATCH),
        Native::Init => (INIT, INIT_MISMATCH),
    };
    let p = vm.space.prop(x.class, "GestaltData").ok_or_else(|| bad(what, text))?;
    let decl = vm.space.classes.decl(p).ok_or_else(|| bad(what, text))?;
    if crate::gestalt::complete(vm, vm.space.read(o, PropAddr::scalar(p))?, &decl.kind, decl.array_dim) {
        Ok(())
    } else {
        Err(bad(what, text))
    }
}

/// Whether a walked value holds no undecoded part: no `Raw` and no delegate anywhere in its tree.
fn whole(v: &PropValue) -> bool {
    match v {
        PropValue::Raw(_) | PropValue::Delegate { .. } => false,
        PropValue::Struct { fields, .. } => fields.iter().all(|f| whole(&f.value)),
        PropValue::Array(items) => items.iter().all(whole),
        _ => true,
    }
}

/// F2 (the note's section 6, check 2): each of the definition's three arrays decoded from its tag with no `Raw` (or
/// delegate) anywhere in its tree, by the package walker the tools use, typed from reflection as the store types it
/// (`ow_object::decode::ReflectionTyper`). Reading the package's tags is write-free. A walk that fails is not whole.
/// A definition the store did not make from an export has no tags: its heap values stand (module documentation).
pub fn arrays_decoded(vm: &Vm, definition: ObjRef) -> Result<bool, VmError> {
    let Origin::Export { package, index } = vm.space.heap.get(definition)?.origin else {
        return Ok(true);
    };
    let store = vm.defs.packages();
    let typer = ReflectionTyper::new(store, vm.defs.code(), &vm.space, package, vm.defs.mode());
    // A gestalt definition is an ordinary object, so its tags start after the ordinary prefix: the store chooses the
    // same `PropertyPrefix` for it (`ow-defs/src/store.rs`, `derive_prefix`).
    let start = ow_object::prefix::PropertyPrefix::Ordinary.bytes();
    let Ok(walked) = read_tagged(store.get(package), index, start, &typer) else {
        return Ok(false);
    };
    Ok(walked.entries.iter().filter(|e| WALKED.contains(&e.name.as_str())).all(|e| whole(&e.value)))
}

/// Field `name` of struct value `v` (declared, else a data error).
fn get<'a>(vm: &Vm, v: &'a Value, name: &str, what: &str) -> Result<&'a Value, VmError> {
    ow_attr::eval::field(&vm.space, v, name).map_err(|e| bad(what, format!("{name}: {e}")))
}

/// Sets field `name` of struct value `v`.
fn put(vm: &Vm, v: &mut Value, name: &str, x: Value, what: &str) -> Result<(), VmError> {
    let id = vm.space.names.find(name).ok_or_else(|| bad(what, format!("no name {name}")))?;
    let sv = v.struct_mut().ok_or_else(|| bad(what, format!("{name}: not a struct")))?;
    let (at, _) = vm
        .space
        .classes
        .struct_layout(sv.ty)
        .and_then(|l| l.field(id))
        .ok_or_else(|| bad(what, format!("no member {name}")))?;
    let slot = sv.fields.get_mut(at as usize).ok_or_else(|| bad(what, format!("{name}: missing")))?;
    *slot = x;
    Ok(())
}

/// The struct id member `name` of struct `s` is declared with (directly, or as an array's element).
fn member_struct(vm: &Vm, s: StructId, name: &str, what: &str) -> Result<StructId, VmError> {
    let id = vm.space.names.find(name).ok_or_else(|| bad(what, format!("no name {name}")))?;
    let decl = vm.space.classes.struct_layout(s).and_then(|l| l.field(id)).map(|(_, d)| d);
    match decl.map(|d| &d.kind) {
        Some(PropKind::Struct(t)) => Ok(*t),
        Some(PropKind::Array(k)) => match k.as_ref() {
            PropKind::Struct(t) => Ok(*t),
            _ => Err(bad(what, format!("{name} is not an array of structs"))),
        },
        _ => Err(bad(what, format!("{name} is not a struct member"))),
    }
}

fn items<'a>(v: &'a Value, name: &str, what: &str) -> Result<&'a [Value], VmError> {
    match v {
        Value::Array(a) => Ok(a.as_slice()),
        other => Err(bad(what, format!("{name} holds {}, not an array", other.kind_name()))),
    }
}

fn name_of(v: &Value, name: &str, what: &str) -> Result<NameId, VmError> {
    match v {
        Value::Name(n) => Ok(*n),
        other => Err(bad(what, format!("{name} holds {}, not a name", other.kind_name()))),
    }
}

fn float_of(v: &Value, name: &str, what: &str) -> Result<f32, VmError> {
    match v {
        Value::Float(f) => Ok(*f),
        other => Err(bad(what, format!("{name} holds {}, not a float", other.kind_name()))),
    }
}

/// A `BoxSphereBounds` as numbers: origin, extent (x, y, z) and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub origin: [f32; 3],
    pub extent: [f32; 3],
    pub radius: f32,
}

const AXES: [&str; 3] = ["X", "Y", "Z"];

fn vector(vm: &Vm, v: &Value, what: &str) -> Result<[f32; 3], VmError> {
    let mut out = [0.0; 3];
    for (o, a) in out.iter_mut().zip(AXES) {
        *o = float_of(get(vm, v, a, what)?, a, what)?;
    }
    Ok(out)
}

fn read_bounds(vm: &Vm, v: &Value, what: &str) -> Result<Bounds, VmError> {
    Ok(Bounds {
        origin: vector(vm, get(vm, v, "Origin", what)?, what)?,
        extent: vector(vm, get(vm, v, "BoxExtent", what)?, what)?,
        radius: float_of(get(vm, v, "SphereRadius", what)?, "SphereRadius", what)?,
    })
}

fn write_bounds(vm: &Vm, v: &mut Value, b: Bounds, what: &str) -> Result<(), VmError> {
    for (name, xyz) in [("Origin", b.origin), ("BoxExtent", b.extent)] {
        let mut vec = get(vm, v, name, what)?.clone();
        for (a, x) in AXES.iter().zip(xyz) {
            put(vm, &mut vec, a, Value::Float(x), what)?;
        }
        put(vm, v, name, vec, what)?;
    }
    put(vm, v, "SphereRadius", Value::Float(b.radius), what)
}

/// A single-precision square root through `ow_core::num` (rule N3).
fn sqrt32(x: f32) -> f32 {
    ow_core::num::sqrt_f64(f64::from(x)) as f32
}

/// The engine's single-precision minimum and maximum, as compare-and-select (UNVERIFIED for NaN; the data is finite).
fn min32(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
fn max32(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// The note's section 2 union of `cur` (O, E, R) with `entry` (P, F, S), in its operation order, every step in f32.
pub fn union(cur: Bounds, entry: Bounds) -> Bounds {
    let (o, e, p, f) = (cur.origin, cur.extent, entry.origin, entry.extent);
    let mut h = [0.0f32; 3];
    let mut c = [0.0f32; 3];
    for i in 0..3 {
        let (a, b) = (o[i] - e[i], o[i] + e[i]);
        let (mut lo, mut hi) = (min32(a, b), max32(a, b));
        for x in [p[i] - f[i], p[i] + f[i]] {
            lo = min32(lo, x);
            hi = max32(hi, x);
        }
        h[i] = (hi - lo) * 0.5;
        c[i] = h[i] + lo;
    }
    // The squares summed y, x, then z, as the note orders them.
    let sum = |d: [f32; 3]| (d[1] * d[1] + d[0] * d[0]) + d[2] * d[2];
    let diagonal = sqrt32(sum(h));
    let a = sqrt32(sum([o[0] - c[0], o[1] - c[1], o[2] - c[2]])) + cur.radius;
    let b = sqrt32(sum([p[0] - c[0], p[1] - c[1], p[2] - c[2]])) + entry.radius;
    let m = if b <= a { a } else { b };
    Bounds { origin: c, extent: h, radius: if m < diagonal { m } else { diagonal } }
}

/// The fill routine (the note's section 2) on `current`, the component's `GestaltData` value, for definition `g` and
/// name list `names`: the new value. Reads only; the caller has run the preflight's declaration checks.
pub fn fill_value(vm: &Vm, current: &Value, g: ObjRef, names: &[NameId]) -> Result<Value, VmError> {
    let what = BUILD;
    let read = |n: &str| -> Result<Value, VmError> {
        let class = vm.space.heap.get(g)?.class;
        let p = vm.space.prop(class, n).ok_or_else(|| bad(what, format!("no {n}")))?;
        Ok(vm.space.read(g, PropAddr::scalar(p))?.clone())
    };
    let mut v = current.clone();
    let container = v.as_struct().map(|s| s.ty).ok_or_else(|| bad(what, "GestaltData is not a struct"))?;
    let mesh = read("GestaltSkeletalMesh")?;
    // Steps 1 to 4.
    put(vm, &mut v, "PartMeshNames", Value::array(names.iter().map(|n| Value::Name(*n)).collect()), what)?;
    put(vm, &mut v, "SkeletalMesh", mesh.clone(), what)?;
    put(vm, &mut v, "LODModelData", Value::array(Vec::new()), what)?;
    put(vm, &mut v, "SocketMappings", Value::array(Vec::new()), what)?;
    // Step 5: no shared mesh, stop.
    if matches!(mesh, Value::Object(None)) {
        return Ok(v);
    }
    let infos = read("GestaltInfos")?;
    let sockets = read("GestaltSocketMappings")?;
    let part_bounds = read("GestaltPartBounds")?;
    // Steps 6 and 8: one LOD model per gestalt info, with its matching parts' fragments.
    let lod_ty = member_struct(vm, container, "LODModelData", what)?;
    let fragment_ty = member_struct(vm, lod_ty, "GestaltFragments", what)?;
    let mut lods = Vec::new();
    for info in items(&infos, "GestaltInfos", what)? {
        let mut fragments = Vec::new();
        for part in items(get(vm, info, "Parts", what)?, "Parts", what)? {
            let n = name_of(get(vm, part, "SkeletalMeshFragmentName", what)?, "SkeletalMeshFragmentName", what)?;
            if !names.contains(&n) {
                continue;
            }
            let mut fr = vm.space.classes.struct_defaults(fragment_ty).map_err(|e| bad(what, e.to_string()))?;
            for m in ["MaterialIndex", "FirstIndex", "NumPrimitives"] {
                put(vm, &mut fr, m, get(vm, part, m, what)?.clone(), what)?;
            }
            fragments.push(fr);
        }
        let mut lod = vm.space.classes.struct_defaults(lod_ty).map_err(|e| bad(what, e.to_string()))?;
        put(vm, &mut lod, "GestaltFragments", Value::array(fragments), what)?;
        lods.push(lod);
    }
    put(vm, &mut v, "LODModelData", Value::array(lods), what)?;
    // Step 7: the start constant (the zero vector by fit, UNVERIFIED) and radius 0.
    let mut bounds = Bounds { origin: [0.0; 3], extent: [0.0; 3], radius: 0.0 };
    // Step 9: per name in list order, the sockets then the bounds.
    let remap_ty = member_struct(vm, container, "SocketMappings", what)?;
    let mut mappings = Vec::new();
    for &n in names {
        for s in items(&sockets, "GestaltSocketMappings", what)? {
            if name_of(get(vm, s, "SkeletalMeshFragmentName", what)?, "SkeletalMeshFragmentName", what)? != n {
                continue;
            }
            let mut m = vm.space.classes.struct_defaults(remap_ty).map_err(|e| bad(what, e.to_string()))?;
            for f in ["OriginalSocketName", "MangledSocketName"] {
                put(vm, &mut m, f, get(vm, s, f, what)?.clone(), what)?;
            }
            mappings.push(m);
        }
        for b in items(&part_bounds, "GestaltPartBounds", what)? {
            if name_of(get(vm, b, "SkeletalMeshFragmentName", what)?, "SkeletalMeshFragmentName", what)? != n {
                continue;
            }
            let entry = read_bounds(vm, get(vm, b, "ReferencePoseBounds", what)?, what)?;
            bounds = if bounds.radius > 0.0 { union(bounds, entry) } else { entry };
        }
    }
    put(vm, &mut v, "SocketMappings", Value::array(mappings), what)?;
    let mut rb = get(vm, &v, "ReferencePoseBounds", what)?.clone();
    write_bounds(vm, &mut rb, bounds, what)?;
    put(vm, &mut v, "ReferencePoseBounds", rb, what)?;
    Ok(v)
}

/// A planned whole-slot write: the object, its `GestaltData` property and the new value.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotWrite {
    pub object: ObjRef,
    pub prop: ow_object::ids::PropId,
    pub value: Value,
}

impl SlotWrite {
    /// The single heap write.
    pub fn apply(self, vm: &mut Vm) -> Result<(), VmError> {
        vm.space.write(self.object, PropAddr::scalar(self.prop), self.value)?;
        Ok(())
    }
}

/// `value` checked against `o`'s `GestaltData` declaration with the heap's own write rules, as the planned write.
fn checked(vm: &Vm, o: ObjRef, p: ow_object::ids::PropId, kind: &PropKind, value: Value, what: &str) -> Result<SlotWrite, VmError> {
    vm.space
        .heap
        .check_value(&vm.space.classes, kind, &value)
        .map_err(|e| bad(what, format!("the GestaltData write would be refused: {e}")))?;
    Ok(SlotWrite { object: o, prop: p, value })
}

/// The fill's preflight (module documentation, steps 1 to 5) for component `c`, definition `g` and names `names`:
/// the planned write. Writes, loads and draws nothing.
pub fn plan_fill(vm: &Vm, c: ObjRef, g: ObjRef, names: &[NameId]) -> Result<SlotWrite, VmError> {
    let (p, kind) = shaped(vm, c, "GestaltData", &CONTAINER, BUILD)?;
    held_whole(vm, c, Native::Build)?;
    for (name, shape) in &DEFINITION {
        let (dp, dk) = shaped(vm, g, name, shape, BUILD)?;
        if !crate::gestalt::complete(vm, vm.space.read(g, PropAddr::scalar(dp))?, &dk, 1) {
            return Err(bad(BUILD, format!("the gestalt definition's {name} does not match its declaration")));
        }
    }
    if !arrays_decoded(vm, g)? {
        return Err(NotYet::not_read(ARRAYS_NOT_DECODED, DATA_RECORD).into());
    }
    let current = vm.space.read(c, PropAddr::scalar(p))?.clone();
    let value = fill_value(vm, &current, g, names)?;
    checked(vm, c, p, &kind, value, BUILD)
}

/// The copy's check 4 and write plan (module documentation): F1 on `this` then on `from`, then `from`'s value checked
/// against `this`'s declaration.
pub fn plan_copy(vm: &Vm, this: ObjRef, from: ObjRef) -> Result<SlotWrite, VmError> {
    held_whole(vm, this, Native::Init)?;
    held_whole(vm, from, Native::Init)?;
    let class = vm.space.heap.get(this)?.class;
    let p = vm.space.prop(class, "GestaltData").ok_or_else(|| bad(INIT, INIT_MISMATCH))?;
    let kind = vm.space.classes.decl(p).ok_or_else(|| bad(INIT, INIT_MISMATCH))?.kind.clone();
    let from_class = vm.space.heap.get(from)?.class;
    let fp = vm.space.prop(from_class, "GestaltData").ok_or_else(|| bad(INIT, INIT_MISMATCH))?;
    let value = vm.space.read(from, PropAddr::scalar(fp))?.clone();
    checked(vm, this, p, &kind, value, INIT)
}

#[cfg(test)]
#[path = "gestalt_data_tests.rs"]
mod tests;
