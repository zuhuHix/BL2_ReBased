//! Owner: lane W3-DECODE. Tests of `decode.rs` (and the fixture `cdo.rs`'s tests share) on invented packages
//! (AGENTS.md rule 9), built with `ow_package::testkit::reflect`. The struct-default cases re-express case 4 of
//! `tests/behavior_test.py` (an omitted field takes the struct's default tag; a default stream not consumed exactly
//! is ignored); the rest pin the C++ `applyTaggedDefaults` / `TagReader` rules listed in the module documentation.

use std::path::PathBuf;

use super::*;
use crate::ids::ObjRef;
use ow_package::props::{PropValue, read_tagged};
use ow_package::testkit::reflect::{Declarations, PropSpec};
use ow_package::testkit::{Export, ints};

/// A temporary folder of invented packages, removed on drop.
pub(crate) struct Tree(pub(crate) PathBuf);

impl Tree {
    pub(crate) fn new(tag: &str) -> Tree {
        let dir =
            std::env::temp_dir().join(format!("ow-object-w3decode-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tree(dir)
    }
    pub(crate) fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).unwrap();
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A loaded world: the store over the tree, the reflection index, the object space and the decode state.
pub(crate) struct World {
    pub(crate) _tree: Tree,
    pub(crate) store: PackageStore,
    pub(crate) code: CodeIndex,
    pub(crate) space: ObjectSpace,
    pub(crate) state: DecodeState,
    pub(crate) core: PackageId,
}

impl World {
    pub(crate) fn open(tree: Tree) -> World {
        let mut store = PackageStore::open(tree.0.clone()).unwrap();
        let mut code = CodeIndex::new();
        let core = code.package(&mut store, "Core").unwrap();
        World {
            _tree: tree,
            store,
            code,
            space: ObjectSpace::new(),
            state: DecodeState::new(),
            core,
        }
    }

    pub(crate) fn cx(&mut self, mode: ParityMode) -> ValueCx<'_> {
        ValueCx::new(
            &mut self.store,
            &mut self.code,
            &mut self.space,
            &mut self.state,
            mode,
        )
    }

    pub(crate) fn class(&mut self, name: &str) -> ClassKey {
        self.code
            .find_class(&mut self.store, &format!("Core.{name}"))
            .unwrap()
    }

    pub(crate) fn export(&mut self, path: &str) -> i32 {
        let index = self.code.find_export(&self.store, self.core, path);
        assert_ne!(index, 0, "no export {path}");
        index
    }
}

/// Property `name` of object `o` (a static array as an array of its elements).
pub(crate) fn get(space: &ObjectSpace, o: ObjRef, name: &str) -> Value {
    let class = space.heap.get(o).unwrap().class;
    let prop = space
        .prop(class, name)
        .unwrap_or_else(|| panic!("no property {name}"));
    let decl = space.classes.decl(prop).unwrap();
    if decl.array_dim > 1 {
        let n = decl.array_dim as u32;
        Value::array(
            (0..n)
                .map(|i| space.read(o, PropAddr::element(prop, i)).unwrap().clone())
                .collect(),
        )
    } else {
        space.read(o, PropAddr::scalar(prop)).unwrap().clone()
    }
}

/// Field `name` of struct value `v`.
pub(crate) fn field(space: &ObjectSpace, v: &Value, name: &str) -> Value {
    let sv = v.as_struct().expect("a struct value");
    let layout = space.classes.struct_layout(sv.ty).unwrap();
    let (entry, _) = layout.field(space.names.find(name).unwrap()).unwrap();
    sv.fields[entry as usize].clone()
}

fn ints_of(v: &Value) -> Vec<i32> {
    v.elements()
        .unwrap()
        .iter()
        .map(|e| match e {
            Value::Int(i) => *i,
            other => panic!("not an int: {other:?}"),
        })
        .collect()
}

pub(crate) fn decls() -> Declarations {
    Declarations::with_imports(&[
        "Class",
        "ScriptStruct",
        "Enum",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "NameProperty",
        "StrProperty",
        "ByteProperty",
        "StructProperty",
        "ArrayProperty",
        "ObjectProperty",
        "ClassProperty",
        "InterfaceProperty",
        "DelegateProperty",
    ])
}

/// A tag stream: the 4-byte prefix word, the tags, `None`.
pub(crate) fn stream(d: &mut Declarations, prefix: usize, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; prefix];
    for t in tags {
        out.extend_from_slice(t);
    }
    out.extend_from_slice(&d.none);
    out
}

/// Struct tags (a nested stream ends with `None`).
fn body(d: &mut Declarations, tags: &[Vec<u8>]) -> Vec<u8> {
    stream(d, 0, tags)
}

/// The fixture `Core.upk`, every name invented:
///
/// ```text
/// enum EKind { K_A, K_B, K_C }    enum ESize { EPSM_1, EPSM_2, EPSM_4 } (numbered names: base EPSM + number)
/// enum EState { ready, set }
/// struct Vector { float X, Y, Z }    struct Plane extends Vector { float W }        (native layouts)
/// struct Scaled { int Base; int Scale }     defaults: Scale = 1 (or a stream that does not end exactly)
/// struct Listed { array<int> L }            defaults: L = [1, 2]
/// struct CycA { array<CycB> Bs }  defaults: Bs = [{}];   struct CycB { array<CycA> As }  defaults: As = [{}]
/// class Object
/// class Base extends Object { int A; float B; bool C; name N; string S; EKind Kind; ESize Size; EState State;
///     int Count[3]; Scaled Holder, Other; Listed Lists; Plane P; array<int> Ints; array<Scaled> Rows;
///     array<EKind> Kinds; array<bool> Flags; Object Owner, Asset; class<Object> Kindred; interface Iface;
///     delegate OnDone }
/// class Leaf extends Base { int D }         class Looper extends Object { CycA Loop }
/// Default__Base: A = 3, B = 1.5
/// Default__Leaf: A = 7, B (an int tag of 8 bytes: skipped), N = Foo, S = bar, C, Holder { Base = 2 }, Count[0] = 1,
///     Count[2] = 3, Count[5] = 9 (past the array: ignored), Kind = K_C, Size = EPSM_2, P (W 4, X 1, Y 2, Z 3),
///     Ints = [5, 6], Rows = [{ Base = 1 }, { Scale = 4 }], Owner = Thing, Asset = Mesh, Kindred = Leaf (as an
///     ObjectProperty tag), Iface = Thing, OnDone (a delegate tag: skipped), Editor (undeclared int: dropped),
///     Menu (undeclared array with an element: skipped), Kinds = [K_B, K_A] (8-byte names), Flags = [1, 0, 1]
///     (1-byte bools), State = set, D = 1
/// Thing (a Leaf): D = 2      Comp (a Leaf, 8-byte prefix): D = 3      Tiny (a Leaf, 4 bytes)
/// Broken (a Leaf): D = 4, then a tag of negative size
/// Mesh: an object of class Engine.StaticMesh, a class no package exports
/// ```
pub(crate) fn fixture(broken_defaults: bool) -> Vec<u8> {
    let mut d = decls();
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let kind = d.enum_(object, "EKind", &["K_A", "K_B", "K_C"]);
    let state = d.enum_(object, "EState", &["ready", "set"]);
    // An enum whose value names carry FName numbers (the package stores EPSM with numbers 2, 3 and 5).
    let size = {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&d.none);
        payload.extend(ints(&[0, 3]));
        for number in [2, 3, 5] {
            payload.extend_from_slice(&d.p.fname("EPSM", number));
        }
        let mut export = Export::new(d.import("Enum"), "ESize", payload);
        export.outer = object;
        d.p.add_export_with(export)
    };
    let vector = d.struct_in(object, "Vector", 0, None);
    for f in ["Z", "Y", "X"] {
        d.prop("Float", vector, f, 0);
    }
    let plane = d.struct_in(object, "Plane", vector, None);
    d.prop("Float", plane, "W", 0);
    let scaled_defaults = {
        let scale = d.p.int_tag("Scale", 1);
        let mut tags = body(&mut d, &[scale]);
        if broken_defaults {
            tags.extend_from_slice(&[0, 0, 0, 0]);
        }
        tags
    };
    let scaled = d.struct_in(object, "Scaled", 0, Some(&scaled_defaults));
    d.prop("Int", scaled, "Scale", 0);
    d.prop("Int", scaled, "Base", 0);
    let listed_defaults = {
        let list = d.p.array_tag("L", 2, &ints(&[1, 2]));
        body(&mut d, &[list])
    };
    let listed = d.struct_in(object, "Listed", 0, Some(&listed_defaults));
    d.array_of(listed, "L", "Int", 0);
    let empty_element = body(&mut d, &[]);
    let cyc_a_defaults = {
        let bs = d.p.array_tag("Bs", 1, &empty_element);
        body(&mut d, &[bs])
    };
    let cyc_b_defaults = {
        let r#as = d.p.array_tag("As", 1, &empty_element);
        body(&mut d, &[r#as])
    };
    let cyc_a = d.struct_in(object, "CycA", 0, Some(&cyc_a_defaults));
    let cyc_b = d.struct_in(object, "CycB", 0, Some(&cyc_b_defaults));
    d.array_of(cyc_a, "Bs", "Struct", cyc_b);
    d.array_of(cyc_b, "As", "Struct", cyc_a);

    let base = d.cls("Base", object);
    let class_import = d.import("Class");
    d.prop("Delegate", base, "OnDone", 0);
    d.prop("Interface", base, "Iface", object);
    d.prop_with(&PropSpec {
        tail: Some(ints(&[class_import, object])),
        ..PropSpec::new("Class", base, "Kindred")
    });
    d.prop("Object", base, "Asset", object);
    d.prop("Object", base, "Owner", object);
    d.array_of(base, "Rows", "Struct", scaled);
    d.array_of(base, "Ints", "Int", 0);
    d.array_of(base, "Kinds", "Byte", kind);
    d.array_of(base, "Flags", "Bool", 0);
    d.prop("Byte", base, "State", state);
    d.prop("Struct", base, "P", plane);
    d.prop("Struct", base, "Lists", listed);
    d.prop("Struct", base, "Other", scaled);
    d.prop("Struct", base, "Holder", scaled);
    d.prop_with(&PropSpec {
        dim: 3,
        ..PropSpec::new("Int", base, "Count")
    });
    d.prop("Byte", base, "Size", size);
    d.prop("Byte", base, "Kind", kind);
    d.prop("Str", base, "S", 0);
    d.prop("Name", base, "N", 0);
    d.prop("Bool", base, "C", 0);
    d.prop("Float", base, "B", 0);
    d.prop("Int", base, "A", 0);
    let leaf = d.cls("Leaf", base);
    d.prop("Int", leaf, "D", 0);
    let looper = d.cls("Looper", object);
    d.prop("Struct", looper, "Loop", cyc_a);

    let engine = d.p.add_import("Core", "Package", 0, "Engine");
    let static_mesh = d.p.add_import("Core", "Class", engine, "StaticMesh");
    let mesh = d.p.add_export(static_mesh, "Mesh", ints(&[0, 0]));

    let tags = vec![d.p.int_tag("A", 3), d.p.float_tag("B", 1.5)];
    let payload = stream(&mut d, 4, &tags);
    d.p.add_export(base, "Default__Base", payload);

    let thing = d.p.next_export() + 1; // after Default__Leaf
    let holder = {
        let b = d.p.int_tag("Base", 2);
        body(&mut d, &[b])
    };
    let rows = {
        let r0 = d.p.int_tag("Base", 1);
        let r1 = d.p.int_tag("Scale", 4);
        let mut b = body(&mut d, &[r0]);
        b.extend(body(&mut d, &[r1]));
        b
    };
    let mut plane_bytes = Vec::new();
    for v in [4.0f32, 1.0, 2.0, 3.0] {
        plane_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let numbered = {
        let value = d.p.fname("EPSM", 3);
        let detail = d.p.fname("ESize", 0);
        d.p.tag_detail("Size", "ByteProperty", &detail, &value)
    };
    let kinds = {
        let mut names = d.p.fname("K_B", 0).to_vec();
        names.extend_from_slice(&d.p.fname("K_A", 0));
        d.p.array_tag("Kinds", 2, &names)
    };
    let tags = vec![
        d.p.int_tag("A", 7),
        d.p.tag_full("B", 0, "IntProperty", 8, &[], &[0; 8]),
        d.p.name_tag("N", "Foo"),
        d.p.str_tag("S", "bar"),
        d.p.bool_tag("C", true),
        d.p.struct_tag("Holder", "Scaled", &holder),
        d.p.tag_full("Count", 0, "IntProperty", 4, &[], &1i32.to_le_bytes()),
        d.p.tag_full("Count", 2, "IntProperty", 4, &[], &3i32.to_le_bytes()),
        d.p.tag_full("Count", 5, "IntProperty", 4, &[], &9i32.to_le_bytes()),
        d.p.enum_tag("Kind", "EKind", "K_C"),
        numbered,
        d.p.struct_tag("P", "Plane", &plane_bytes),
        d.p.array_tag("Ints", 2, &ints(&[5, 6])),
        d.p.array_tag("Rows", 2, &rows),
        d.p.object_tag("Owner", thing),
        d.p.object_tag("Asset", mesh),
        d.p.object_tag("Kindred", leaf),
        d.p.tag("Iface", "InterfaceProperty", &thing.to_le_bytes()),
        d.p.tag("OnDone", "DelegateProperty", &[0; 12]),
        d.p.int_tag("Editor", 5),
        d.p.array_tag("Menu", 1, &[0; 8]),
        kinds,
        d.p.array_tag("Flags", 3, &[1, 0, 1]),
        d.p.enum_tag("State", "EState", "set"),
        d.p.int_tag("D", 1),
    ];
    let payload = stream(&mut d, 4, &tags);
    d.p.add_export(leaf, "Default__Leaf", payload);
    let tags = vec![d.p.int_tag("D", 2)];
    let payload = stream(&mut d, 4, &tags);
    assert_eq!(d.p.add_export(leaf, "Thing", payload), thing);
    let tags = vec![d.p.int_tag("D", 3)];
    let payload = stream(&mut d, 8, &tags);
    d.p.add_export(leaf, "Comp", payload);
    d.p.add_export(leaf, "Tiny", ints(&[0]));
    let tags = vec![
        d.p.int_tag("D", 4),
        d.p.tag_full("A", 0, "IntProperty", -1, &[], &[]),
    ];
    let payload = stream(&mut d, 4, &tags);
    d.p.add_export(leaf, "Broken", payload);
    d.build()
}

pub(crate) fn world(tag: &str, broken_defaults: bool) -> World {
    let tree = Tree::new(tag);
    tree.write("Core.upk", &fixture(broken_defaults));
    World::open(tree)
}

fn leaf_default(w: &mut World, mode: ParityMode) -> ObjRef {
    let leaf = w.class("Leaf");
    crate::cdo::class_default(&mut w.cx(mode), leaf).unwrap()
}

#[test]
fn values_take_the_tag_types_the_cxx_vm_reads() {
    let mut w = world("scalars", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let s = &w.space;
    assert_eq!(get(s, o, "A"), Value::Int(7));
    assert_eq!(get(s, o, "C"), Value::Bool(true));
    assert_eq!(get(s, o, "N"), Value::Name(s.names.find("Foo").unwrap()));
    assert_eq!(get(s, o, "S"), Value::string("bar"));
    assert_eq!(get(s, o, "D"), Value::Int(1));
    assert_eq!(get(s, o, "Kind"), Value::Byte(2));
    assert_eq!(ints_of(&get(s, o, "Ints")), [5, 6]);
}

#[test]
fn a_value_that_fails_is_skipped_and_logged_the_rest_applies() {
    let mut w = world("skips", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    // B's int tag is 8 bytes long: skipped, so B keeps Default__Base's 1.5.
    assert_eq!(get(&w.space, o, "B"), Value::Float(1.5));
    assert_eq!(
        w.state.log,
        [
            "defaults: skipped Leaf.B (IntProperty)",
            // Off mode: Mesh's class is an engine-native class, a NotYet.
            "defaults: skipped Leaf.Asset (ObjectProperty)",
            "defaults: skipped Leaf.OnDone (DelegateProperty)",
            "defaults: skipped Leaf.Menu (ArrayProperty)",
        ]
    );
    // An undeclared tag the C++ bag would hold is dropped without a log line, and recorded.
    assert_eq!(w.state.dropped, ["Leaf.Editor (IntProperty)"]);
}

#[test]
fn static_arrays_take_one_tag_per_element_and_ignore_an_index_past_them() {
    let mut w = world("static", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    assert_eq!(ints_of(&get(&w.space, o, "Count")), [1, 0, 3]);
}

#[test]
fn an_omitted_struct_field_takes_the_struct_default() {
    // behavior_test.py case 4: Holder sets Base only, so Scale comes from Scaled's default tags; Other is absent.
    let mut w = world("struct-defaults", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let s = &w.space;
    let holder = get(s, o, "Holder");
    assert_eq!(
        (field(s, &holder, "Base"), field(s, &holder, "Scale")),
        (Value::Int(2), Value::Int(1))
    );
    let other = get(s, o, "Other");
    assert_eq!(
        (field(s, &other, "Base"), field(s, &other, "Scale")),
        (Value::Int(0), Value::Int(1))
    );
    // Array elements start from the struct's defaults too.
    let rows = get(s, o, "Rows");
    let rows = rows.elements().unwrap();
    assert_eq!(
        (field(s, &rows[0], "Base"), field(s, &rows[0], "Scale")),
        (Value::Int(1), Value::Int(1))
    );
    assert_eq!(
        (field(s, &rows[1], "Base"), field(s, &rows[1], "Scale")),
        (Value::Int(0), Value::Int(4))
    );
}

#[test]
fn a_default_stream_not_consumed_exactly_is_ignored() {
    // behavior_test.py case 4c.
    let mut w = world("broken-defaults", true);
    let o = leaf_default(&mut w, ParityMode::Off);
    let s = &w.space;
    let holder = get(s, o, "Holder");
    assert_eq!(
        (field(s, &holder, "Base"), field(s, &holder, "Scale")),
        (Value::Int(2), Value::Int(0))
    );
    let other = get(s, o, "Other");
    assert_eq!(
        (field(s, &other, "Base"), field(s, &other, "Scale")),
        (Value::Int(0), Value::Int(0))
    );
}

#[test]
fn arrays_in_struct_defaults_decode_only_off_the_reference_mode() {
    let mut w = world("default-arrays-off", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let lists = get(&w.space, o, "Lists");
    assert_eq!(ints_of(&field(&w.space, &lists, "L")), [1, 2]);

    // The C++ reads struct defaults with no array typing: the array stays empty.
    let mut w = world("default-arrays-ref", false);
    let o = leaf_default(&mut w, ParityMode::Reference);
    let lists = get(&w.space, o, "Lists");
    assert_eq!(ints_of(&field(&w.space, &lists, "L")), Vec::<i32>::new());
}

/// CycA's defaults hold a CycB, whose defaults hold a CycA. Reading either first refuses both, and neither publishes
/// a default; a class holding one by value cannot build its default object.
fn cycle_outcome(first: &str) -> [Result<(), DecodeError>; 2] {
    let mut w = world(&format!("cycle-{first}"), false);
    // Lay the structs out first (through the class that holds one).
    let looper = w.class("Looper");
    w.cx(ParityMode::Off).layout(looper).unwrap();
    let a = w.space.classes.struct_by_path("Core.Object.CycA").unwrap();
    let b = w.space.classes.struct_by_path("Core.Object.CycB").unwrap();
    let order = if first == "a" { [a, b] } else { [b, a] };
    let mut cx = w.cx(ParityMode::Off);
    let first_outcome = cx.ensure_struct_defaults(order[0]);
    let second_outcome = cx.ensure_struct_defaults(order[1]);
    assert!(
        matches!(
            crate::cdo::class_default(&mut cx, looper),
            Err(DecodeError::StructDefaultsCycle(_))
        ),
        "the default object of a class holding CycA is refused"
    );
    for s in [a, b] {
        let layout = w.space.classes.struct_layout(s).unwrap();
        assert!(layout.own_defaults().is_empty(), "nothing published");
    }
    if first == "a" {
        [first_outcome, second_outcome]
    } else {
        [second_outcome, first_outcome]
    }
}

#[test]
fn struct_defaults_that_need_themselves_are_refused_in_either_order() {
    let cycle = |path: &str| Err(DecodeError::StructDefaultsCycle(path.to_string()));
    let expected = [cycle("Core.Object.CycA"), cycle("Core.Object.CycB")];
    assert_eq!(cycle_outcome("a"), expected);
    assert_eq!(cycle_outcome("b"), expected);
}

#[test]
fn native_structs_are_read_untagged_in_the_cxx_order_and_placed_by_name() {
    let mut w = world("native", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let s = &w.space;
    let p = get(s, o, "P");
    let xyzw: Vec<Value> = ["X", "Y", "Z", "W"]
        .iter()
        .map(|f| field(s, &p, f))
        .collect();
    assert_eq!(xyzw, [1.0f32, 2.0, 3.0, 4.0].map(Value::Float));
    assert_eq!(native_layout("Plane").unwrap().0, ["W", "X", "Y", "Z"]);
    assert_eq!(
        native_layout("Color").unwrap(),
        (&["B", "G", "R", "A"][..], NativeField::Byte)
    );
    assert_eq!(
        native_layout("plane"),
        None,
        "matched by exact name, as the C++ map"
    );
}

#[test]
fn numbered_enum_values_take_their_position_except_in_the_reference_mode() {
    let mut w = world("enum-off", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    assert_eq!(get(&w.space, o, "Size"), Value::Byte(1));
    // The C++ compares the entries without their numbers, so EPSM_2 matches nothing there: 0.
    let mut w = world("enum-ref", false);
    let o = leaf_default(&mut w, ParityMode::Reference);
    assert_eq!(get(&w.space, o, "Size"), Value::Byte(0));
}

#[test]
fn an_enum_value_the_enum_lacks_is_refused_except_in_the_reference_mode() {
    let mut w = world("enum-unknown", false);
    let decl = {
        let base = w.class("Base");
        let base = w.cx(ParityMode::Off).layout(base).unwrap();
        let prop = w.space.prop(base, "Kind").unwrap();
        w.space.classes.decl(prop).unwrap().clone()
    };
    let e = enum_of(&decl.kind);
    let err = w
        .cx(ParityMode::Off)
        .enum_value(e, decl.name, "None")
        .unwrap_err();
    assert_eq!(err.to_string(), "Kind: the enum has no value None");
    assert_eq!(
        w.cx(ParityMode::Reference).enum_value(e, decl.name, "None"),
        Ok(0)
    );
}

#[test]
fn the_reference_enum_comparison_uses_the_package_spelling_in_either_load_order() {
    // EState is { ready, set } in the package; a tag says `set`. Interning `SET` first makes the name table keep that
    // spelling, which must not change the C++ comparison (the package's own text, case-sensitive).
    for spelled_first in [false, true] {
        let mut w = world(&format!("enum-spelling-{spelled_first}"), false);
        if spelled_first {
            w.space.names.intern("SET");
        }
        let o = leaf_default(&mut w, ParityMode::Reference);
        assert_eq!(
            get(&w.space, o, "State"),
            Value::Byte(1),
            "SET interned first: {spelled_first}"
        );
        let mut w = world(&format!("enum-spelling-off-{spelled_first}"), false);
        if spelled_first {
            w.space.names.intern("SET");
        }
        let o = leaf_default(&mut w, ParityMode::Off);
        assert_eq!(get(&w.space, o, "State"), Value::Byte(1));
    }
}

#[test]
fn array_element_widths_follow_the_parity_mode() {
    // Off: the measured widths, an enum byte as an 8-byte name and a bool as one byte.
    let mut w = world("widths-off", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    assert_eq!(
        get(&w.space, o, "Kinds"),
        Value::array(vec![Value::Byte(1), Value::Byte(0)])
    );
    assert_eq!(
        get(&w.space, o, "Flags"),
        Value::array([true, false, true].map(Value::Bool).to_vec())
    );
    // Reference: the C++ widths (one byte, four bytes), which do not meet these tags' sizes: both skipped.
    let mut w = world("widths-ref", false);
    let o = leaf_default(&mut w, ParityMode::Reference);
    assert_eq!(get(&w.space, o, "Kinds"), Value::array(Vec::new()));
    assert_eq!(get(&w.space, o, "Flags"), Value::array(Vec::new()));
    assert!(
        w.state
            .log
            .contains(&"defaults: skipped Leaf.Kinds (ArrayProperty)".to_string())
    );
    assert!(
        w.state
            .log
            .contains(&"defaults: skipped Leaf.Flags (ArrayProperty)".to_string())
    );
}

#[test]
fn references_get_one_stand_in_each_and_class_tags_type_by_declaration() {
    let mut w = world("refs", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let thing = w.export("Thing");
    let s = &w.space;
    let Value::Object(Some(owner)) = get(s, o, "Owner") else {
        panic!("Owner")
    };
    assert_eq!(
        s.heap.get(owner).unwrap().origin,
        Origin::Resource {
            package: w.core,
            index: thing
        }
    );
    // The interface reference names the same object, through the same stand-in.
    assert_eq!(get(s, o, "Iface"), Value::Interface(Some(owner)));
    // A class property tagged ObjectProperty holds the class.
    let leaf = s.classes.by_path("Core.Leaf").unwrap();
    assert_eq!(get(s, o, "Kindred"), Value::Class(Some(leaf)));
}

#[test]
fn a_class_no_package_exports_gets_a_stand_in_class_in_the_reference_mode_only() {
    let mut w = world("native-class", false);
    let o = leaf_default(&mut w, ParityMode::Reference);
    let s = &w.space;
    let Value::Object(Some(mesh)) = get(s, o, "Asset") else {
        panic!("Asset")
    };
    let class = s.heap.get(mesh).unwrap().class;
    let layout = s.classes.layout(class);
    assert_eq!(layout.path().as_str(), "Engine.StaticMesh");
    assert_eq!(layout.super_class(), s.classes.by_path("Core.Object"));
    assert_eq!(
        layout.own_decls().count(),
        0,
        "no native property is invented"
    );
    assert_eq!(w.state.native_classes, ["Engine.StaticMesh"]);
    assert!(w.state.is_native_stand_in(class));
    assert!(
        !w.state
            .is_native_stand_in(s.classes.by_path("Core.Leaf").unwrap())
    );

    // Off: the class is not read, a NotYet, and the property is skipped.
    let mut w = world("native-class-off", false);
    let mesh = w.export("Mesh");
    let core = w.core;
    let o = leaf_default(&mut w, ParityMode::Off);
    assert_eq!(get(&w.space, o, "Asset"), Value::Object(None));
    let err = w.cx(ParityMode::Off).resource(core, mesh).unwrap_err();
    assert!(matches!(err, DecodeError::NotYet(_)), "{err:?}");
    assert!(w.state.native_classes.is_empty());
}

#[test]
fn stand_ins_hold_no_slots_and_have_a_budget() {
    let mut w = world("stand-ins", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let Value::Object(Some(owner)) = get(&w.space, o, "Owner") else {
        panic!("Owner")
    };
    assert!(w.space.heap.get(owner).unwrap().slots.is_empty());
    // Past the budget, a typed error: the property is skipped, the rest applies.
    let mut w = world("stand-in-budget", false);
    w.state.stand_in_budget = 0;
    let o = leaf_default(&mut w, ParityMode::Off);
    assert_eq!(get(&w.space, o, "Owner"), Value::Object(None));
    assert_eq!(get(&w.space, o, "D"), Value::Int(1));
    assert!(
        w.state
            .log
            .contains(&"defaults: skipped Leaf.Owner (ObjectProperty)".to_string())
    );
    let core = w.core;
    let thing = w.export("Thing");
    assert_eq!(
        w.cx(ParityMode::Off).resource(core, thing),
        Err(DecodeError::StandInBudget(0))
    );
}

/// A resolver that answers every reference with one object of its own, and counts the calls.
struct One {
    object: ObjRef,
    calls: std::cell::Cell<u32>,
}

impl ReferenceResolver for One {
    fn resolve(
        &self,
        _cx: &mut ValueCx<'_>,
        _package: PackageId,
        _reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        self.calls.set(self.calls.get() + 1);
        Ok(Some(self.object))
    }
}

#[test]
fn a_resolver_decides_what_references_point_at() {
    let mut w = world("resolver", false);
    let thing = w.class("Leaf");
    let leaf_id = w.cx(ParityMode::Off).layout(thing).unwrap();
    let own = w
        .space
        .alloc(leaf_id, "Mine", None, Origin::Runtime)
        .unwrap();
    let one = One {
        object: own,
        calls: std::cell::Cell::new(0),
    };
    let o = {
        let leaf = w.class("Leaf");
        let mut cx = w.cx(ParityMode::Reference).with_resolver(&one);
        crate::cdo::class_default(&mut cx, leaf).unwrap()
    };
    assert_eq!(get(&w.space, o, "Owner"), Value::Object(Some(own)));
    assert_eq!(get(&w.space, o, "Iface"), Value::Interface(Some(own)));
    // Owner, Asset and Iface each ask once (Asset gets Mine too: a Leaf is an Object).
    assert_eq!(one.calls.get(), 3);
    assert!(w.state.native_classes.is_empty(), "no stand-in was made");
}

#[test]
fn errors_print_the_cxx_texts() {
    for (e, text) in [
        (
            DecodeError::UnsupportedRequest,
            "unsupported exported object request",
        ),
        (DecodeError::NullClass, "cannot instantiate a null class"),
        (
            DecodeError::PrefixOutsideExport,
            "object property prefix outside export",
        ),
        (
            DecodeError::NegativeSizeOrIndex,
            "negative property size or index",
        ),
        (DecodeError::InvalidArrayLength, "invalid array length"),
        (
            DecodeError::ElementTypeUnknown,
            "array element type unknown",
        ),
        (DecodeError::StructTooDeep, "struct nesting too deep"),
        (DecodeError::Read(ReadError::Truncated), "truncated package"),
    ] {
        assert_eq!(e.to_string(), text);
    }
}

#[test]
fn walker_trees_type_to_the_same_values() {
    // A walker tree of each kind, typed by the declaration, equals what the VM-side reader stored.
    let mut w = world("to-value", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let leaf = w.space.classes.by_path("Core.Leaf").unwrap();
    for (name, tree) in [
        ("A", PropValue::Int(7)),
        ("Kind", PropValue::Enum("K_C".into())),
        (
            "Ints",
            PropValue::Array(vec![PropValue::Int(5), PropValue::Int(6)]),
        ),
        ("Owner", PropValue::Object(w.export("Thing"))),
        ("Kindred", PropValue::Object(w.export("Leaf"))),
    ] {
        let prop = w.space.prop(leaf, name).unwrap();
        let decl = w.space.classes.decl(prop).unwrap().clone();
        let core = w.core;
        let v = to_value(&tree, &decl, core, &mut w.cx(ParityMode::Off)).unwrap();
        assert_eq!(v, get(&w.space, o, name), "{name}");
    }
    let prop = w.space.prop(leaf, "A").unwrap();
    let decl = w.space.classes.decl(prop).unwrap().clone();
    let core = w.core;
    let err = to_value(
        &PropValue::Raw(vec![1]),
        &decl,
        core,
        &mut w.cx(ParityMode::Off),
    )
    .unwrap_err();
    assert!(matches!(err, DecodeError::KindMismatch { .. }));
}

/// `Core.upk` for the typer: two classes each holding `array<Row> Rows`, where `Holder.Row` holds `array<int> Items`
/// and `Keeper.Row` holds `array<name> Items`; one export of each.
fn typer_fixture() -> Vec<u8> {
    let mut d = decls();
    let object = d.cls("Object", 0);
    let holder = d.cls("Holder", object);
    let holder_row = d.struct_in(holder, "Row", 0, None);
    d.array_of(holder_row, "Items", "Int", 0);
    d.array_of(holder, "Rows", "Struct", holder_row);
    let keeper = d.cls("Keeper", object);
    let keeper_row = d.struct_in(keeper, "Row", 0, None);
    d.array_of(keeper_row, "Items", "Name", 0);
    d.array_of(keeper, "Rows", "Struct", keeper_row);
    let items = d.p.array_tag("Items", 2, &ints(&[1, 2]));
    let row = body(&mut d, &[items]);
    let rows = d.p.array_tag("Rows", 1, &row);
    let payload = stream(&mut d, 4, &[rows]);
    d.p.add_export(holder, "H", payload);
    let x = d.p.fname("X", 0);
    let items = d.p.array_tag("Items", 1, &x);
    let row = body(&mut d, &[items]);
    let rows = d.p.array_tag("Rows", 1, &row);
    let unknown = d.p.array_tag("Unknown", 1, &ints(&[3]));
    let payload = stream(&mut d, 4, &[rows, unknown]);
    d.p.add_export(keeper, "K", payload);
    d.build()
}

#[test]
fn the_reflection_typer_follows_the_root_and_the_path() {
    let tree = Tree::new("typer");
    tree.write("Core.upk", &typer_fixture());
    let mut w = World::open(tree);
    let core = w.core;
    prepare_typer(&mut w.cx(ParityMode::Off), core);
    let h = w.export("H");
    let k = w.export("K");
    let pkg = w.store.shared(core);
    let typer = ReflectionTyper::new(&w.store, &w.code, &w.space, core, ParityMode::Off);
    let items = |export: i32| {
        let walked = read_tagged(&pkg, export, 4, &typer).unwrap();
        let PropValue::Array(rows) = &walked.entries[0].value else {
            panic!("Rows")
        };
        let PropValue::Struct { fields, .. } = &rows[0] else {
            panic!("Row")
        };
        (
            fields[0].value.clone(),
            walked.entries.get(1).map(|e| e.value.clone()),
        )
    };
    // Two structs named Row, one Items each: told apart by the root class.
    assert_eq!(
        items(h).0,
        PropValue::Array(vec![PropValue::Int(1), PropValue::Int(2)])
    );
    assert_eq!(
        items(k).0,
        PropValue::Array(vec![PropValue::Name("X".into())])
    );
    // A property the class does not declare stays raw.
    assert_eq!(items(k).1, Some(PropValue::Raw(ints(&[3]))));
}

#[test]
fn to_value_skips_what_tagged_skips_and_shares_the_nesting_bound() {
    let mut w = world("to-value-rules", false);
    let leaf = w.class("Leaf");
    let leaf = w.cx(ParityMode::Off).layout(leaf).unwrap();
    let prop = w.space.prop(leaf, "Holder").unwrap();
    let decl = w.space.classes.decl(prop).unwrap().clone();
    let core = w.core;
    let tree = PropValue::Struct {
        ty: "Scaled".into(),
        fields: vec![
            ow_package::props::PropEntry::new("Base", 0, PropValue::Int(2), None),
            ow_package::props::PropEntry::new("Nowhere", 0, PropValue::Int(9), None),
            ow_package::props::PropEntry::new("Scale", 0, PropValue::Str("no".into()), None),
        ],
        start: 0,
    };
    let v = to_value(&tree, &decl, core, &mut w.cx(ParityMode::Off)).unwrap();
    // The undeclared field and the field of the wrong kind are skipped; Scale keeps its default.
    assert_eq!(
        (field(&w.space, &v, "Base"), field(&w.space, &v, "Scale")),
        (Value::Int(2), Value::Int(1))
    );

    // A struct reached at the VM reader's deepest stream is refused, as that reader refuses it.
    let mut cx = w.cx(ParityMode::Off);
    let deep = Stream {
        top: false,
        defaults: false,
        depth: MAX_STRUCT_DEPTH,
    };
    assert_eq!(
        tree_value(&tree, &decl, core, &mut cx, deep),
        Err(DecodeError::StructTooDeep)
    );
    let shallow = Stream { depth: 0, ..deep };
    assert!(tree_value(&tree, &decl, core, &mut cx, shallow).is_ok());
}

#[test]
fn the_reflection_typer_takes_the_element_widths_of_its_mode() {
    let mut w = world("typer-modes", false);
    let core = w.core;
    prepare_typer(&mut w.cx(ParityMode::Off), core);
    let leaf_ref = w.export("Leaf");
    let root = ow_package::props::StreamRoot::new("Leaf", Some(leaf_ref));
    let scope = ArrayScope::new("Leaf", Some(leaf_ref), &[], &root);
    for (mode, kinds) in [
        (ParityMode::Reference, Some(ElementType::Byte)),
        (ParityMode::Off, None),
    ] {
        let typer = ReflectionTyper::new(&w.store, &w.code, &w.space, core, mode);
        assert_eq!(typer.element(&scope, "Kinds"), kinds, "{mode:?}");
        assert_eq!(typer.element(&scope, "Flags"), None);
        assert_eq!(typer.element(&scope, "Ints"), Some(ElementType::Int));
    }
}
