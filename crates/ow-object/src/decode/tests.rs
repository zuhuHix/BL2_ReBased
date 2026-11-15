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
/// struct Vector { float X, Y, Z }    struct Plane extends Vector { float W }        (native layouts)
/// struct Scaled { int Base; int Scale }     defaults: Scale = 1 (or a stream that does not end exactly)
/// struct Listed { array<int> L }            defaults: L = [1, 2]
/// struct CycA { array<CycB> Bs }  defaults: Bs = [{}];   struct CycB { array<CycA> As }  defaults: As = [{}]
/// class Object
/// class Base extends Object { int A; float B; bool C; name N; string S; EKind Kind; ESize Size; int Count[3];
///     Scaled Holder, Other; Listed Lists; Plane P; CycA Loop; array<int> Ints; array<Scaled> Rows;
///     Object Owner, Asset; class<Object> Kindred; interface Iface; delegate OnDone }
/// class Leaf extends Base { int D }
/// Default__Base: A = 3, B = 1.5
/// Default__Leaf: A = 7, B (an int tag of 8 bytes: skipped), N = Foo, S = bar, C, Holder { Base = 2 }, Count[0] = 1,
///     Count[2] = 3, Count[5] = 9 (past the array: ignored), Kind = K_C, Size = EPSM_2, P (W 4, X 1, Y 2, Z 3),
///     Ints = [5, 6], Rows = [{ Base = 1 }, { Scale = 4 }], Owner = Thing, Asset = Mesh, Kindred = Leaf (as an
///     ObjectProperty tag), Iface = Thing, OnDone (a delegate tag: skipped), Editor (undeclared int: dropped),
///     Menu (undeclared array with an element: skipped), D = 1
/// Thing (a Leaf): D = 2      Comp (a Leaf, 8-byte prefix): D = 3      Tiny (a Leaf, 4 bytes)
/// Mesh: an object of class Engine.StaticMesh, a class no package exports
/// ```
pub(crate) fn fixture(broken_defaults: bool) -> Vec<u8> {
    let mut d = decls();
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let kind = d.enum_(object, "EKind", &["K_A", "K_B", "K_C"]);
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
    d.prop("Struct", base, "Loop", cyc_a);
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

#[test]
fn struct_defaults_that_need_themselves_stop_with_a_typed_error() {
    // CycA's defaults hold a CycB, whose defaults hold a CycA: the inner one cannot wait on the outer, so its array is
    // skipped and the outer completes. No loop, no overflow.
    let mut w = world("cycle", false);
    let o = leaf_default(&mut w, ParityMode::Off);
    let s = &w.space;
    let looped = get(s, o, "Loop");
    let bs = field(s, &looped, "Bs");
    let bs = bs.elements().unwrap();
    assert_eq!(bs.len(), 1);
    assert_eq!(field(s, &bs[0], "As").elements().unwrap().len(), 0);
    let cyc_a = s.classes.struct_by_path("Core.Object.CycA").unwrap();
    let mut cx = w.cx(ParityMode::Off);
    assert_eq!(cx.ensure_struct_defaults(cyc_a), Ok(()));
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
    let err = w.cx(ParityMode::Off).enum_value(&decl, "None").unwrap_err();
    assert_eq!(err.to_string(), "Kind: the enum has no value None");
    assert_eq!(w.cx(ParityMode::Reference).enum_value(&decl, "None"), Ok(0));
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
fn a_class_no_package_exports_gets_an_engine_native_stand_in_class() {
    let mut w = world("native-class", false);
    let o = leaf_default(&mut w, ParityMode::Off);
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
    let typer = ReflectionTyper::new(&w.store, &w.code, &w.space, core);
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
