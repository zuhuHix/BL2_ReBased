//! Owner: lane W2-REFLECT. Tests of `reflect.rs` on invented packages (AGENTS.md rule 9), built with
//! `ow_package::testkit::reflect`. The `--class-check` and interface cases re-express `tests/vm_test.py`'s
//! interface fixture (`make(interfaces=True)`, its class part) and its `mutate` cases.

use super::*;
use crate::class_body::CLASS_INTERFACE;
use ow_package::testkit::reflect::{ClassBodySpec, Declarations, PropSpec, class_body};
use ow_package::testkit::{Export, ints};
use std::path::PathBuf;

/// A temporary folder of invented packages, removed on drop.
struct Tree(PathBuf);

impl Tree {
    fn new(tag: &str) -> Tree {
        let dir =
            std::env::temp_dir().join(format!("ow-object-w2reflect-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tree(dir)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
    fn store(&self) -> PackageStore {
        PackageStore::open(self.0.clone()).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The interface fixture of `tests/vm_test.py`: Object with a decodable body; IThing and IThingChild interfaces (no
/// functions); Impl names IThing with a `VfTable_IThing` child, Derived inherits it, ChildImpl names IThingChild only,
/// Plain names nothing; Util has class reference 0 and the 16-byte stand-in body. Returns Impl's reference too.
fn interface_fixture() -> (Declarations, i32) {
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let thing = d.real_class("IThing", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let thing_child = d.real_class("IThingChild", thing, obj, 2 | CLASS_INTERFACE, &[], None);
    let implementer = d.real_class("Impl", obj, obj, 2, &[thing], Some("VfTable_IThing"));
    d.real_class("Derived", implementer, obj, 2, &[], None);
    d.real_class(
        "ChildImpl",
        obj,
        obj,
        2,
        &[thing_child],
        Some("VfTable_IThingChild"),
    );
    d.real_class("Plain", obj, obj, 2, &[], None);
    let mut util = Export::new(0, "Util", ints(&[0, 0, 0, 0]));
    util.super_ = obj;
    d.p.add_export_with(util);
    (d, implementer)
}

fn check(tree: &Tree, name: &str, bytes: &[u8]) -> ClassCheck {
    let path = tree.write(name, bytes);
    let mut store = tree.store();
    let id = store.load_path(&path).unwrap();
    CodeIndex::new().check_class_bodies(&mut store, id).unwrap()
}

fn load(tree: &Tree, name: &str) -> (PackageStore, PackageId) {
    let mut store = tree.store();
    let id = store.load_path(&tree.0.join(name)).unwrap();
    (store, id)
}

/// The `--class-check` case of `vm_test.py`: the oracle reports exactly the stand-in body as failing.
#[test]
fn class_check_counts_the_vm_test_fixture() {
    let tree = Tree::new("check");
    let c = check(&tree, "Core.upk", &interface_fixture().0.build());
    assert_eq!(
        (
            c.classes,
            c.exact,
            c.failures.len(),
            c.default_objects,
            c.interface_classes
        ),
        (8, 7, 1, 7, 2)
    );
    assert_eq!(
        (
            c.classes_with_interfaces,
            c.interface_entries,
            c.entries_to_interface_classes
        ),
        (2, 2, 2)
    );
    assert_eq!((c.table_struct_properties, c.table_null), (2, 0));
    assert_eq!(
        c.failures[0],
        "Util: class body super reference differs from the export table"
    );
}

/// `vm_test.py`'s `mutate` cases: malformed bodies are refused, never skipped, and the first word is not checked.
#[test]
fn class_check_refuses_the_vm_test_mutations() {
    let tree = Tree::new("mutate");
    let failed = |change: fn(&mut Vec<u8>)| {
        let (mut d, implementer) = interface_fixture();
        let pkg =
            Package::from_decoded(PathBuf::new(), "Broken".into(), d.clone().build()).unwrap();
        let e = &pkg.exports[implementer as usize - 1];
        let mut body = pkg.data[e.offset as usize..(e.offset + e.size) as usize].to_vec();
        change(&mut body);
        d.p.set_payload(implementer, body);
        check(&tree, "Broken.upk", &d.build()).failures.len()
    };
    assert_eq!(failed(|b| b.push(0)), 2, "trailing byte");
    assert_eq!(
        failed(|b| b[8..12].copy_from_slice(&3i32.to_le_bytes())),
        2,
        "super reference"
    );
    assert_eq!(
        failed(|b| b[0..4].copy_from_slice(&99_999i32.to_le_bytes())),
        1,
        "first word"
    );
    assert_eq!(
        failed(|b| b[48..52].copy_from_slice(&(-1i32).to_le_bytes())),
        2,
        "function map count"
    );
}

/// The interface answers `vm_test.py` asks through `QueryInterface`: own table, super class table, a derived
/// interface implies its base, a base implementer is not the derived one.
#[test]
fn implements_follows_the_interface_tables() {
    let tree = Tree::new("implements");
    tree.write("Core.upk", &interface_fixture().0.build());
    let mut store = tree.store();
    let mut index = CodeIndex::new();
    let mut key = |path: &str| index.find_class(&mut store, path).unwrap();
    let thing = key("Core.IThing");
    let child = key("Core.IThingChild");
    let (implementer, derived, child_impl, plain) = (
        key("Core.Impl"),
        key("Core.Derived"),
        key("Core.ChildImpl"),
        key("Core.Plain"),
    );
    assert!(index.implements(&mut store, implementer, thing));
    assert!(index.implements(&mut store, derived, thing));
    assert!(index.implements(&mut store, child_impl, thing));
    assert!(index.implements(&mut store, child_impl, child));
    assert!(!index.implements(&mut store, plain, thing));
    assert!(!index.implements(&mut store, implementer, child));
    assert!(!index.implements(&mut store, derived, child));
    assert!(
        !index.implements(&mut store, implementer, plain),
        "not an interface class"
    );
    assert!(index.class(thing).is_interface());
    assert_eq!(
        index.find_class(&mut store, "Core.Nothing"),
        Err(ReflectError::ClassNotFound("Core.Nothing".into()))
    );
    assert_eq!(
        index.find_class(&mut store, "NoDot"),
        Err(ReflectError::ClassPath("NoDot".into()))
    );
}

/// `--interface-compare` over a store whose eight other code packages are empty; a missing one fails the command.
#[test]
fn compare_interfaces_counts_every_pair() {
    let tree = Tree::new("compare");
    tree.write("Core.upk", &interface_fixture().0.build());
    for name in &CODE_PACKAGES[1..] {
        tree.write(&format!("{name}.upk"), &Declarations::new(&[]).build());
    }
    let mut store = tree.store();
    let c = CodeIndex::new().compare_interfaces(&mut store).unwrap();
    assert_eq!((c.classes, c.interfaces), (5, 2));
    assert_eq!((c.both_yes, c.table_only_yes, c.coded_only_yes), (0, 4, 0));
    assert_eq!(c.functions_only, [0, 4, 0]);
    let keys: Vec<&str> = c.slice.keys().map(String::as_str).collect();
    assert_eq!(keys, SLICE_INTERFACES);
    assert!(c.slice.values().all(|v| *v == [0; 4]));

    let alone = Tree::new("compare-missing");
    alone.write("Core.upk", &interface_fixture().0.build());
    assert_eq!(
        CodeIndex::new()
            .compare_interfaces(&mut alone.store())
            .unwrap_err(),
        ReflectError::Store(StoreError::PackageNotFound("Engine".into()))
    );
}

/// Without decodable bodies the structural stand-in answers: an `I<Upper>` name, at least one function, every
/// function defined along the class chain (case folded).
#[test]
fn the_structural_fallback_needs_every_function_and_the_name() {
    let tree = Tree::new("structural");
    let mut d = Declarations::with_imports(&["Class", "Function"]);
    let function = d.import("Function");
    let in_class = |d: &mut Declarations, outer: i32, name: &str| {
        let mut e = Export::new(function, name, Vec::new());
        e.outer = outer;
        d.p.add_export_with(e)
    };
    let iface = d.cls("IFoo", 0);
    in_class(&mut d, iface, "Bar");
    let plain_name = d.cls("Foo", 0);
    in_class(&mut d, plain_name, "Bar");
    let base = d.cls("Base", 0);
    in_class(&mut d, base, "BAR");
    let derived = d.cls("Derived", base);
    let other = d.cls("Other", 0);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut index = CodeIndex::new();
    let mut key = |r: i32| index.class_at(&mut store, id, r).unwrap().unwrap();
    let (iface, plain_name, derived, other) =
        (key(iface), key(plain_name), key(derived), key(other));
    assert!(
        index.implements(&mut store, derived, iface),
        "inherited, case folded"
    );
    assert!(!index.implements(&mut store, other, iface));
    assert!(
        !index.implements(&mut store, derived, plain_name),
        "not an I<Upper> name"
    );
    assert!(index.functions_only(derived, plain_name));
    assert!(index.find_method(derived, "bar").is_some());
    assert!(index.find_method(other, "bar").is_none());
}

/// Every kind the loader types, the static array dimension, the declaration order, the enum values, a struct
/// holding an array of itself, the attribute companions with the byte-attribute shift, a class only declared.
#[test]
fn layouts_type_every_declaration() {
    let tree = Tree::new("layouts");
    let mut d = Declarations::with_imports(&[
        "Class",
        "ScriptStruct",
        "Enum",
        "IntProperty",
        "FloatProperty",
        "ByteProperty",
        "StructProperty",
        "ArrayProperty",
        "ObjectProperty",
        "ClassProperty",
        "FloatAttributeProperty",
        "ByteAttributeProperty",
    ]);
    let class_import = d.import("Class");
    let engine = d.p.add_import("Core", "Package", 0, "Engine");
    let actor = d.p.add_import("Core", "Class", engine, "Actor");
    let object = d.cls("Object", 0);
    let kinds = d.enum_(object, "EKind", &["K_A", "K_B", "K_C"]);
    let vec = d.struct_in(object, "Vec", 0, None);
    d.prop("Float", vec, "X", 0);
    d.prop("Float", vec, "Y", 0);
    let node = d.struct_in(object, "Node", 0, None);
    d.array_of(node, "Children", "Struct", node);
    d.prop("Struct", node, "Pos", vec);
    let thing = d.cls("Thing", object);
    d.prop_with(&PropSpec {
        dim: 3,
        ..PropSpec::new("Int", thing, "Count")
    });
    d.prop("Byte", thing, "Kind", kinds);
    d.prop("Struct", thing, "Where", node);
    d.array_of(thing, "Path", "Struct", vec);
    d.prop("Object", thing, "Owner", object);
    d.prop("Object", thing, "Target", actor);
    d.prop_with(&PropSpec {
        tail: Some(ints(&[class_import, object])),
        ..PropSpec::new("Class", thing, "Kindred")
    });
    // Speed names (stack, base); Level, a byte attribute, names (enum, stack, base).
    let speed = d.p.next_export();
    d.prop_with(&PropSpec {
        flags: PropertyDecl::STACK_ATTRIBUTE,
        tail: Some(ints(&[speed + 2, speed + 1])),
        ..PropSpec::new("FloatAttribute", thing, "Speed")
    });
    d.prop_with(&PropSpec {
        flags: PropertyDecl::PLAIN_ATTRIBUTE,
        ..PropSpec::new("FloatAttribute", thing, "SpeedBase")
    });
    d.array_of(thing, "SpeedStack", "Object", object);
    let level = d.p.next_export();
    d.prop_with(&PropSpec {
        flags: PropertyDecl::STACK_ATTRIBUTE,
        tail: Some(ints(&[kinds, level + 2, level + 1])),
        ..PropSpec::new("ByteAttribute", thing, "Level")
    });
    d.prop_with(&PropSpec {
        flags: PropertyDecl::PLAIN_ATTRIBUTE,
        tail: Some(ints(&[kinds])),
        ..PropSpec::new("ByteAttribute", thing, "LevelBase")
    });
    d.array_of(thing, "LevelStack", "Object", object);
    tree.write("Core.upk", &d.build());

    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (2, 2),
        "{:?}",
        summary.failures
    );

    let object_id = classes.by_path("Core.Object").unwrap();
    let thing_id = classes.by_path("Core.Thing").unwrap();
    assert_eq!(classes.layout(thing_id).super_class(), Some(object_id));
    let decl = |names: &mut NameTable, n: &str| {
        let p = classes
            .prop(thing_id, names.intern(n))
            .unwrap_or_else(|| panic!("no {n}"));
        classes.decl(p).unwrap().clone()
    };
    assert_eq!(decl(&mut names, "Count").array_dim, 3);
    let e = classes.enum_by_path("Core.Object.EKind").unwrap();
    assert_eq!(
        decl(&mut names, "Kind").kind,
        PropKind::Byte { enum_: Some(e) }
    );
    let values: Vec<String> = classes
        .enum_layout(e)
        .unwrap()
        .values()
        .iter()
        .map(|&v| names.text(v).into_owned())
        .collect();
    assert_eq!(values, ["K_A", "K_B", "K_C"]);
    let vec_id = classes.struct_by_path("Core.Object.Vec").unwrap();
    let node_id = classes.struct_by_path("Core.Object.Node").unwrap();
    assert_eq!(decl(&mut names, "Where").kind, PropKind::Struct(node_id));
    assert_eq!(
        decl(&mut names, "Path").kind,
        PropKind::Array(Box::new(PropKind::Struct(vec_id)))
    );
    // Fields in descending export order, as the C++ `childProperties`: Y was exported after X.
    let fields: Vec<String> = classes
        .struct_layout(vec_id)
        .unwrap()
        .fields()
        .map(|(_, f)| names.text(f.name).into_owned())
        .collect();
    assert_eq!(fields, ["Y", "X"]);
    let node_fields: Vec<PropKind> = classes
        .struct_layout(node_id)
        .unwrap()
        .fields()
        .map(|(_, f)| f.kind.clone())
        .collect();
    assert_eq!(
        node_fields,
        [
            PropKind::Struct(vec_id),
            PropKind::Array(Box::new(PropKind::Struct(node_id)))
        ]
    );
    assert_eq!(
        decl(&mut names, "Owner").kind,
        PropKind::Object { class: object_id }
    );
    assert_eq!(
        decl(&mut names, "Kindred").kind,
        PropKind::Class { meta: object_id }
    );
    let actor_id = classes
        .by_path("Engine.Actor")
        .expect("declared from the import path");
    assert!(!classes.layout(actor_id).is_defined());
    assert_eq!(
        decl(&mut names, "Target").kind,
        PropKind::Object { class: actor_id }
    );

    let companions = decl(&mut names, "Speed").attr.expect("linked");
    assert_eq!(
        companions.base,
        classes.prop(thing_id, names.intern("SpeedBase")).unwrap()
    );
    assert_eq!(
        companions.stack,
        classes.prop(thing_id, names.intern("SpeedStack")).unwrap()
    );
    assert_eq!(decl(&mut names, "SpeedBase").attr, Some(companions));
    let level_decl = decl(&mut names, "Level");
    assert_eq!(level_decl.kind, PropKind::ByteAttribute { enum_: Some(e) });
    let companions = level_decl
        .attr
        .expect("linked through the shifted references");
    assert_eq!(
        companions.base,
        classes.prop(thing_id, names.intern("LevelBase")).unwrap()
    );
    assert_eq!(
        companions.stack,
        classes.prop(thing_id, names.intern("LevelStack")).unwrap()
    );
    assert_eq!(level_decl.source, Some((id, level)));
    assert_eq!(
        index.key_of(thing_id).map(|k| index.class(k).path.clone()),
        Some("Core.Thing".into())
    );
}

/// A super class in another package, laid out first through its import; reference resolution across packages.
#[test]
fn a_super_class_in_another_package_is_laid_out_first() {
    let tree = Tree::new("cross");
    let mut engine = Declarations::new(&["Int"]);
    let actor = engine.cls("Actor", 0);
    engine.prop("Int", actor, "Health", 0);
    tree.write("Engine.upk", &engine.build());
    let mut game = Declarations::new(&["Int"]);
    let package = game.p.add_import("Core", "Package", 0, "Engine");
    let actor_import = game.p.add_import("Core", "Class", package, "Actor");
    let nowhere = game.p.add_import("Core", "Package", 0, "Nowhere");
    let lost = game.p.add_import("Core", "Class", nowhere, "Lost");
    let pawn = game.cls("Pawn", actor_import);
    game.prop("Int", pawn, "Shield", 0);
    tree.write("Game.upk", &game.build());

    let (mut store, id) = load(&tree, "Game.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (1, 1),
        "{:?}",
        summary.failures
    );
    let actor_id = classes.by_path("Engine.Actor").unwrap();
    let pawn_id = classes.by_path("Game.Pawn").unwrap();
    assert_eq!(classes.layout(pawn_id).super_class(), Some(actor_id));
    assert_eq!(classes.layout(pawn_id).first_slot(), 1);
    let shield = classes.prop(pawn_id, names.intern("Shield")).unwrap();
    assert_eq!((shield.slot, shield.declared_in), (1, pawn_id));
    assert!(classes.prop(pawn_id, names.intern("Health")).is_some());

    let engine_id = index.package(&mut store, "Engine").unwrap();
    assert_eq!(
        index.resolve_ref(&mut store, id, actor_import),
        Some((engine_id, actor))
    );
    assert_eq!(
        index.resolve_ref(&mut store, id, lost),
        None,
        "no such package"
    );
    assert_eq!(
        index.resolve_ref(&mut store, id, 99),
        None,
        "past the export table"
    );
    assert_eq!(index.resolve_ref(&mut store, id, 0), None);
    assert_eq!(index.class_at(&mut store, id, lost), Ok(None));
}

/// What is not laid out, and why: an unknown property class (and every subclass through its super), a struct holding
/// itself by value, a reference to nothing.
#[test]
fn layouts_refuse_with_typed_errors() {
    let tree = Tree::new("refuse");
    let mut d = Declarations::with_imports(&[
        "Class",
        "ScriptStruct",
        "WeirdProperty",
        "StructProperty",
        "ObjectProperty",
    ]);
    let weird = d.cls("Weird", 0);
    d.prop("Weird", weird, "Odd", 0);
    let below = d.cls("Below", weird);
    let looped = d.cls("Looped", 0);
    let cycle = d.struct_in(looped, "Cycle", 0, None);
    d.prop("Struct", cycle, "Again", cycle);
    d.prop("Struct", looped, "Here", cycle);
    let dangling = d.cls("Dangling", 0);
    d.prop("Object", dangling, "Nowhere", 999);
    tree.write("Core.upk", &d.build());

    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!((summary.classes, summary.defined), (4, 0));
    let mut cause = |r: i32| {
        let key = index.class_at(&mut store, id, r).unwrap().unwrap();
        let mut e = index.layout_of(key).unwrap().clone().unwrap_err();
        while let ReflectError::InClass { error, .. } | ReflectError::InStruct { error, .. } = e {
            e = *error;
        }
        e
    };
    let unknown = ReflectError::UnknownPropertyClass("WeirdProperty".into());
    assert_eq!(cause(weird), unknown);
    assert_eq!(cause(below), unknown);
    assert_eq!(
        cause(looped),
        ReflectError::StructCycle("Core.Looped.Cycle".into())
    );
    assert!(matches!(
        cause(dangling),
        ReflectError::Unresolved { reference: 999, .. }
    ));
    let below_key = index.class_at(&mut store, id, below).unwrap().unwrap();
    let below_error = index.layout_of(below_key).unwrap().clone().unwrap_err();
    assert_eq!(below_error.to_string(), "class Core.Below");
    assert!(
        matches!(below_error, ReflectError::InClass { error, .. } if matches!(*error, ReflectError::InClass { .. }))
    );
    assert!(
        classes
            .by_path("Core.Below")
            .is_none_or(|c| !classes.layout(c).is_defined())
    );
    assert_eq!(index.layout_failures().len(), 4);
}

/// A map property is a `Map` kind: with no references (every map of the installed code packages) both sides are
/// `None`; references, where present, are typed key first. Its value is `None`, and a subclass is laid out after it.
#[test]
fn map_properties_get_a_map_kind() {
    let tree = Tree::new("maps");
    let mut d =
        Declarations::with_imports(&["Class", "MapProperty", "NameProperty", "IntProperty"]);
    let mapped = d.cls("Mapped", 0);
    d.prop_with(&PropSpec {
        tail: Some(ints(&[0, 0])),
        ..PropSpec::new("Map", mapped, "Lookup")
    });
    let keyed = d.p.next_export();
    d.prop_with(&PropSpec {
        tail: Some(ints(&[keyed + 1, keyed + 2])),
        ..PropSpec::new("Map", mapped, "Index")
    });
    d.prop("Name", keyed, "Index", 0);
    d.prop("Int", keyed, "Index", 0);
    let below = d.cls("Below", mapped);
    d.prop("Int", below, "Extra", 0);
    tree.write("Core.upk", &d.build());

    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (2, 2),
        "{:?}",
        summary.failures
    );
    let mapped_id = classes.by_path("Core.Mapped").unwrap();
    let kind_of = |names: &mut NameTable, n: &str| {
        let p = classes.prop(mapped_id, names.intern(n)).unwrap();
        classes.decl(p).unwrap().kind.clone()
    };
    assert_eq!(
        kind_of(&mut names, "Lookup"),
        PropKind::Map {
            key: None,
            value: None
        }
    );
    assert_eq!(
        kind_of(&mut names, "Index"),
        PropKind::Map {
            key: Some(Box::new(PropKind::Name)),
            value: Some(Box::new(PropKind::Int))
        }
    );
    assert_eq!(
        classes.zero_value(&kind_of(&mut names, "Lookup")),
        crate::value::Value::None
    );
    let below_id = classes.by_path("Core.Below").unwrap();
    assert!(classes.layout(below_id).is_defined());
    assert_eq!(classes.layout(below_id).first_slot(), 2);
}

/// A super chain that holds a non-class is the C++ "export is not a class", returned again on every later call.
#[test]
fn a_super_that_is_not_a_class_fails_the_load() {
    let tree = Tree::new("notclass");
    let mut d = Declarations::new(&["Int"]);
    let odd = d.prop("Int", 0, "Loose", 0);
    let child = d.cls("Child", odd);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut index = CodeIndex::new();
    let expected = Err(ReflectError::NotAClass("Loose".into()));
    assert_eq!(index.class_at(&mut store, id, child), expected);
    assert_eq!(index.class_at(&mut store, id, child), expected, "kept");
}

/// An enum holds at most 1024 names and ends exactly at its serial size.
#[test]
fn enums_are_checked() {
    let mut d = Declarations::new(&[]);
    let good = d.enum_(0, "EGood", &["A", "B"]);
    let bad = d.enum_(0, "EBad", &["A"]);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&d.none);
    payload.extend(ints(&[0, MAX_ENUM_COUNT + 1]));
    let huge = d.p.add_export(d.import("Enum"), "EHuge", payload);
    let pkg = Package::from_decoded(PathBuf::new(), "Core".into(), d.build()).unwrap();
    assert_eq!(read_enum(&pkg, good).unwrap(), ["A", "B"]);
    assert_eq!(read_enum(&pkg, huge), Err(ReflectError::EnumCount));
    let mut long = pkg.clone();
    long.exports[bad as usize - 1].size += 1;
    assert_eq!(read_enum(&long, bad), Err(ReflectError::EnumTrailing));
}

#[test]
fn errors_print_the_cxx_texts_and_chain_their_causes() {
    use std::error::Error as _;
    assert_eq!(
        ReflectError::NotAClass("Thing".into()).to_string(),
        "export is not a class: Thing"
    );
    let wrapped = ReflectError::InClass {
        path: "Core.A".into(),
        error: Box::new(ReflectError::NotAProperty),
    };
    assert_eq!(wrapped.to_string(), "class Core.A");
    assert_eq!(
        wrapped.source().unwrap().to_string(),
        "not a property export"
    );
}
