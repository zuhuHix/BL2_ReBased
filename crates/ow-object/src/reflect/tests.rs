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
/// `implements_resolved` (lane IMPL-ANIM-NODE-INIT): no answer before `implements` resolved the entries, then the same
/// answer, with nothing resolved by asking.
#[test]
fn implements_resolved_answers_only_from_resolved_entries() {
    let tree = Tree::new("implements_resolved");
    tree.write("Core.upk", &interface_fixture().0.build());
    let mut store = tree.store();
    let mut index = CodeIndex::new();
    let mut key = |path: &str| index.find_class(&mut store, path).unwrap();
    let thing = key("Core.IThing");
    let (implementer, plain) = (key("Core.Impl"), key("Core.Plain"));
    assert_eq!(index.implements_resolved(implementer, thing), None, "not resolved yet");
    assert_eq!(index.implements_resolved(implementer, thing), None, "asking resolves nothing");
    assert!(index.implements(&mut store, implementer, thing));
    assert_eq!(index.implements_resolved(implementer, thing), Some(true));
    assert!(!index.implements(&mut store, plain, thing));
    assert_eq!(index.implements_resolved(plain, thing), Some(false));
}

/// Review r1 of #575, item 2: a derived class whose own interface entry is not resolved yet gives no answer, even when
/// its base class's hit is already cached (`implements` resolves the derived entry first).
#[test]
fn implements_resolved_does_not_answer_over_an_unresolved_derived_entry() {
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let thing = d.real_class("IThing", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let other = d.real_class("IOther", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let base = d.real_class("Base", obj, obj, 2, &[thing], Some("VfTable_IThing"));
    d.real_class("DerivedOwn", base, obj, 2, &[other], Some("VfTable_IOther"));
    let tree = Tree::new("implements_resolved_derived");
    tree.write("Core.upk", &d.build());
    let mut store = tree.store();
    let mut index = CodeIndex::new();
    let mut key = |path: &str| index.find_class(&mut store, path).unwrap();
    let (thing, base, derived) = (key("Core.IThing"), key("Core.Base"), key("Core.DerivedOwn"));
    assert!(index.implements(&mut store, base, thing), "the base's hit, cached");
    assert_eq!(index.implements_resolved(base, thing), Some(true));
    assert_eq!(index.implements_resolved(derived, thing), None, "the derived entry is not resolved yet");
    assert!(index.implements(&mut store, derived, thing));
    assert_eq!(index.implements_resolved(derived, thing), Some(true));
}

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

/// A super chain that holds a non-class is the C++ "export is not a class" on the call that loads the class. As the
/// C++ `loadClass`, the class stays registered without its super class, so a later `class_at` returns it; only its
/// layout is refused.
#[test]
fn a_class_whose_super_is_not_a_class_stays_registered() {
    let tree = Tree::new("notclass");
    let mut d = Declarations::with_imports(&["Class", "Function", "IntProperty"]);
    let odd = d.prop("Int", 0, "Loose", 0);
    let child = d.cls("Child", odd);
    let function = d.import("Function");
    let mut f = Export::new(function, "Act", Vec::new());
    f.outer = child;
    d.p.add_export_with(f);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut index = CodeIndex::new();
    let refusal = ReflectError::NotAClass("Loose".into());
    assert_eq!(index.class_at(&mut store, id, child), Err(refusal.clone()));
    let key = index
        .class_at(&mut store, id, child)
        .unwrap()
        .expect("kept, as the C++ keeps it");
    assert_eq!(index.class(key).super_class, None);
    assert!(
        index.class(key).functions.is_empty(),
        "the C++ stops before the functions"
    );
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let error = index
        .define_layout(&mut store, &mut names, &mut classes, key)
        .unwrap_err();
    assert!(matches!(error, ReflectError::InClass { error, .. } if *error == refusal));
}

/// The root cause behind the class, struct and enum wrappers.
fn root(mut e: ReflectError) -> ReflectError {
    while let ReflectError::InClass { error, .. }
    | ReflectError::InStruct { error, .. }
    | ReflectError::InEnum { error, .. } = e
    {
        e = *error;
    }
    e
}

/// An array whose inner declaration is itself, an array and a map that name each other, and two arrays in two
/// packages that name each other through imports: each is a typed cycle, not an endless recursion.
#[test]
fn cyclic_property_references_are_refused() {
    let tree = Tree::new("propcycle");
    let mut d = Declarations::with_imports(&["Class", "ArrayProperty", "MapProperty"]);
    let selfish = d.cls("Selfish", 0);
    let own = d.p.next_export();
    d.prop("Array", selfish, "Me", own);
    let mutual = d.cls("Mutual", 0);
    let array = d.p.next_export();
    d.prop("Array", mutual, "List", array + 1);
    d.prop_with(&PropSpec {
        tail: Some(ints(&[array, 0])),
        ..PropSpec::new("Map", array, "List")
    });
    tree.write("Core.upk", &d.build());

    // Left.Owner.Y names Right.Holder.X as its inner declaration, and Right.Holder.X names Left.Owner.Y.
    let across = |this: &str,
                  class: &str,
                  array: &str,
                  other: &str,
                  other_class: &str,
                  other_array: &str| {
        let mut d = Declarations::with_imports(&["Class", "ArrayProperty"]);
        let package = d.p.add_import("Core", "Package", 0, other);
        let holder = d.p.add_import("Core", "Class", package, other_class);
        let inner = d.p.add_import("Core", "ArrayProperty", holder, other_array);
        let c = d.cls(class, 0);
        d.prop("Array", c, array, inner);
        tree.write(&format!("{this}.upk"), &d.build());
    };
    across("Left", "Owner", "Y", "Right", "Holder", "X");
    across("Right", "Holder", "X", "Left", "Owner", "Y");

    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!((summary.classes, summary.defined), (2, 0));
    assert_eq!(
        root(summary.failures[0].1.clone()),
        ReflectError::PropertyCycle("Me".into())
    );
    assert_eq!(
        root(summary.failures[1].1.clone()),
        ReflectError::PropertyCycle("List".into())
    );
    let left = index.package(&mut store, "Left").unwrap();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, left)
        .unwrap();
    assert_eq!(summary.defined, 0);
    assert_eq!(
        root(summary.failures[0].1.clone()),
        ReflectError::PropertyCycle("Y".into())
    );
}

/// A chain of `n` classes, each the super class of the next: returns the package bytes and the references.
fn class_chain(n: usize) -> (Vec<u8>, Vec<i32>) {
    let mut d = Declarations::new(&["Int"]);
    let mut refs = Vec::with_capacity(n);
    let mut previous = 0;
    for i in 0..n {
        previous = d.cls(&format!("C{i}"), previous);
        refs.push(previous);
    }
    (d.build(), refs)
}

/// Super-class loading, layouts and struct inheritance nest at most `MAX_NESTING` deep: past it, a typed
/// `TooDeep`, not a stack overflow. A chain just inside the bound loads and lays out.
#[test]
fn nesting_past_the_budget_is_a_typed_error() {
    let tree = Tree::new("deep");
    let (bytes, refs) = class_chain(MAX_NESTING + 8);
    tree.write("Core.upk", &bytes);
    let (mut store, id) = load(&tree, "Core.upk");
    let last = *refs.last().unwrap();
    // Loading the deepest class first nests once per super class.
    assert_eq!(
        CodeIndex::new().class_at(&mut store, id, last),
        Err(ReflectError::TooDeep(MAX_NESTING))
    );
    // Loaded from the root up, each load is shallow; the layout of the last then nests once per super class.
    let mut index = CodeIndex::new();
    for &r in &refs {
        index.class_at(&mut store, id, r).unwrap();
    }
    let key = index.loaded_class(id, last).unwrap();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let error = index
        .define_layout(&mut store, &mut names, &mut classes, key)
        .unwrap_err();
    assert_eq!(root(error), ReflectError::TooDeep(MAX_NESTING));
    assert_eq!(index.max_nesting(), MAX_NESTING);

    // Just inside the bound: a chain of MAX_NESTING - 1 classes lays out from its last class.
    let inside = Tree::new("deep-inside");
    let (bytes, refs) = class_chain(MAX_NESTING - 1);
    inside.write("Core.upk", &bytes);
    let (mut store, id) = load(&inside, "Core.upk");
    let mut index = CodeIndex::new();
    let key = index
        .class_at(&mut store, id, *refs.last().unwrap())
        .unwrap()
        .unwrap();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    assert!(
        index
            .define_layout(&mut store, &mut names, &mut classes, key)
            .is_ok()
    );

    // A struct whose super chain is past the bound, held by value.
    let structs = Tree::new("deep-structs");
    let mut d = Declarations::with_imports(&["Class", "ScriptStruct", "StructProperty"]);
    let holder = d.cls("Holder", 0);
    let mut previous = 0;
    for i in 0..MAX_NESTING + 8 {
        previous = d.struct_in(holder, &format!("S{i}"), previous, None);
    }
    d.prop("Struct", holder, "Deep", previous);
    structs.write("Core.upk", &d.build());
    let (mut store, id) = load(&structs, "Core.upk");
    let mut index = CodeIndex::new();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!(summary.defined, 0);
    assert_eq!(
        root(summary.failures[0].1.clone()),
        ReflectError::TooDeep(MAX_NESTING)
    );
}

/// A class whose attribute companions do not hold is not published: `define` checks them before the class is
/// defined, so it cannot be allocated half linked.
#[test]
fn a_class_whose_companions_fail_is_not_published() {
    let tree = Tree::new("halfdefined");
    let mut d = Declarations::with_imports(&[
        "Class",
        "FloatAttributeProperty",
        "ArrayProperty",
        "ObjectProperty",
    ]);
    let class = d.cls("Speedy", 0);
    let speed = d.p.next_export();
    d.prop_with(&PropSpec {
        flags: PropertyDecl::STACK_ATTRIBUTE,
        tail: Some(ints(&[speed + 2, speed + 1])),
        ..PropSpec::new("FloatAttribute", class, "Speed")
    });
    // The base without the plain-attribute flag: the companions do not form a triple.
    d.prop_with(&PropSpec::new("FloatAttribute", class, "SpeedBase"));
    d.array_of(class, "SpeedStack", "Object", class);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut space = crate::heap::ObjectSpace::new();
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut space.names, &mut space.classes, id)
        .unwrap();
    assert_eq!(summary.defined, 0);
    assert!(matches!(
        root(summary.failures[0].1.clone()),
        ReflectError::Layout(LayoutError::BadAttribute { .. })
    ));
    let speedy = space.classes.by_path("Core.Speedy").unwrap();
    assert!(!space.classes.layout(speedy).is_defined());
    assert!(
        space
            .alloc(speedy, "S", None, crate::heap::Origin::Runtime)
            .is_err()
    );
}

/// Companions are found by reference and must be those very declarations: a reference to another class's property
/// of the same name is refused, never linked to the own declaration of that name.
#[test]
fn companions_must_be_the_declarations_they_name() {
    let tree = Tree::new("companions");
    let mut d = Declarations::with_imports(&[
        "Class",
        "FloatAttributeProperty",
        "ArrayProperty",
        "ObjectProperty",
    ]);
    let triple = |d: &mut Declarations, class: i32, base_at: i32| {
        let speed = d.p.next_export();
        d.prop_with(&PropSpec {
            flags: PropertyDecl::STACK_ATTRIBUTE,
            tail: Some(ints(&[
                speed + 2,
                if base_at == 0 { speed + 1 } else { base_at },
            ])),
            ..PropSpec::new("FloatAttribute", class, "Speed")
        });
        d.prop_with(&PropSpec {
            flags: PropertyDecl::PLAIN_ATTRIBUTE,
            ..PropSpec::new("FloatAttribute", class, "SpeedBase")
        });
        d.array_of(class, "SpeedStack", "Object", class);
        speed + 1
    };
    let other = d.cls("Other", 0);
    let other_base = triple(&mut d, other, 0);
    let wrong = d.cls("Wrong", 0);
    triple(&mut d, wrong, other_base);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    assert_eq!((summary.classes, summary.defined), (2, 1));
    assert_eq!(
        root(summary.failures[0].1.clone()),
        ReflectError::CompanionNotOwn {
            property: "Speed".into(),
            what: "base value",
            target: (id, other_base),
        }
    );
}

/// `A { array<B> }` with `B { A }` lays out in either load order: B, reached through an array, is defined after the
/// class that reached it, so the cycle is checked along by-value edges only.
#[test]
fn a_struct_cycle_through_an_array_loads_in_either_order() {
    for (tag, a_first) in [("cycle-a", true), ("cycle-b", false)] {
        let tree = Tree::new(tag);
        let mut d = Declarations::with_imports(&[
            "Class",
            "ScriptStruct",
            "StructProperty",
            "ArrayProperty",
        ]);
        let owner = d.cls("Owner", 0);
        let a = d.struct_in(owner, "A", 0, None);
        let b = d.struct_in(owner, "B", 0, None);
        d.array_of(a, "Bs", "Struct", b);
        d.prop("Struct", b, "Inner", a);
        let (first, second) = if a_first { (a, b) } else { (b, a) };
        let one = d.cls("HoldFirst", 0);
        d.prop("Struct", one, "Held", first);
        let two = d.cls("HoldSecond", 0);
        d.prop("Struct", two, "Held", second);
        tree.write("Core.upk", &d.build());
        let (mut store, id) = load(&tree, "Core.upk");
        let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
        let mut index = CodeIndex::new();
        let summary = index
            .define_layouts(&mut store, &mut names, &mut classes, id)
            .unwrap();
        assert_eq!(
            (summary.classes, summary.defined),
            (3, 3),
            "{tag}: {:?}",
            summary.failures
        );
        let a_id = classes.struct_by_path("Core.Owner.A").unwrap();
        let b_id = classes.struct_by_path("Core.Owner.B").unwrap();
        let kinds = |s: StructId| -> Vec<PropKind> {
            classes
                .struct_layout(s)
                .unwrap()
                .fields()
                .map(|(_, f)| f.kind.clone())
                .collect()
        };
        assert_eq!(
            kinds(a_id),
            [PropKind::Array(Box::new(PropKind::Struct(b_id)))],
            "{tag}"
        );
        assert_eq!(kinds(b_id), [PropKind::Struct(a_id)], "{tag}");
        assert_eq!(
            (index.struct_of(id, a), index.struct_of(id, b)),
            (Some(a_id), Some(b_id))
        );
    }
}

/// `S { array<T> }`, held by value first by a class whose companions fail, then by a valid class. `S` is cached
/// as defined, but `T`'s deferred definition belonged to the failed class: the valid class queues it again and
/// defines it before it is published, or, when `T` cannot be defined, is refused with `T`'s error.
#[test]
fn a_reused_struct_brings_its_deferred_dependencies() {
    for (tag, t_breaks) in [("deps-ok", false), ("deps-fail", true)] {
        let tree = Tree::new(tag);
        let mut d = Declarations::with_imports(&[
            "Class",
            "ScriptStruct",
            "StructProperty",
            "ArrayProperty",
            "IntProperty",
            "WeirdProperty",
            "FloatAttributeProperty",
            "ObjectProperty",
        ]);
        let owner = d.cls("Owner", 0);
        let s_ref = d.struct_in(owner, "S", 0, None);
        let t_ref = d.struct_in(owner, "T", 0, None);
        d.array_of(s_ref, "Ts", "Struct", t_ref);
        if t_breaks {
            d.prop("Weird", t_ref, "Odd", 0);
        } else {
            d.prop("Int", t_ref, "X", 0);
        }
        // Bad holds S, then fails while resolving its companions (the stack reference names nothing), before the
        // structs it deferred are defined.
        let bad = d.cls("Bad", 0);
        d.prop("Struct", bad, "Held", s_ref);
        let speed = d.p.next_export();
        d.prop_with(&PropSpec {
            flags: PropertyDecl::STACK_ATTRIBUTE,
            tail: Some(ints(&[999, speed + 1])),
            ..PropSpec::new("FloatAttribute", bad, "Speed")
        });
        d.prop_with(&PropSpec {
            flags: PropertyDecl::PLAIN_ATTRIBUTE,
            ..PropSpec::new("FloatAttribute", bad, "SpeedBase")
        });
        let good = d.cls("Good", 0);
        d.prop("Struct", good, "Held", s_ref);
        tree.write("Core.upk", &d.build());

        let (mut store, id) = load(&tree, "Core.upk");
        let mut space = crate::heap::ObjectSpace::new();
        let mut index = CodeIndex::new();
        let mut layout = |r: i32, space: &mut crate::heap::ObjectSpace| {
            let key = index.class_at(&mut store, id, r).unwrap().unwrap();
            index.define_layout(&mut store, &mut space.names, &mut space.classes, key)
        };
        let bad_error = layout(bad, &mut space).unwrap_err();
        assert!(
            matches!(
                root(bad_error),
                ReflectError::Unresolved { reference: 999, .. }
            ),
            "{tag}"
        );
        let s_id = space.classes.struct_by_path("Core.Owner.S").unwrap();
        assert!(
            space.classes.struct_layout(s_id).is_some(),
            "{tag}: S stays defined and cached"
        );
        let t_id = space.classes.struct_by_path("Core.Owner.T").unwrap();
        assert!(
            space.classes.struct_layout(t_id).is_none(),
            "{tag}: T was only declared"
        );

        let good_outcome = layout(good, &mut space);
        if t_breaks {
            let error = good_outcome.unwrap_err();
            assert_eq!(
                root(error),
                ReflectError::UnknownPropertyClass("WeirdProperty".into())
            );
            let good_id = space.classes.by_path("Core.Good").unwrap();
            assert!(
                !space.classes.layout(good_id).is_defined(),
                "refused, never half defined"
            );
        } else {
            let good_id = good_outcome.unwrap();
            assert!(
                space.classes.struct_layout(t_id).is_some(),
                "T defined before Good was published"
            );
            let object = space
                .alloc(good_id, "G", None, crate::heap::Origin::Runtime)
                .unwrap();
            let held = crate::ids::PropAddr::scalar(space.prop(good_id, "Held").unwrap());
            let element = space
                .classes
                .default_value(&PropKind::Struct(t_id))
                .unwrap();
            let ts = crate::value::Value::array(vec![element]);
            assert_eq!(
                space.write_in(object, held, &[crate::heap::Step::Field(0)], ts),
                Ok(()),
                "a non-empty array of T can be written"
            );
        }
    }
}

/// Invented `Engine.upk` (class Actor with enum EKind and struct Params, which gets `extra` more Int fields) and
/// `Game.upk` (Pawn under Engine.Actor, holding Engine.Actor.Params through an import). Returns the Engine path and
/// the references Actor, Params, EKind, and Game's Actor import.
fn two_package_tree(tree: &Tree, extra: usize) -> (PathBuf, [i32; 4]) {
    let mut engine = Declarations::new(&["Int", "Struct", "Byte"]);
    let actor = engine.cls("Actor", 0);
    let kinds = engine.enum_(actor, "EKind", &["K_A"]);
    engine.prop("Byte", actor, "Kind", kinds);
    // Held is the last export, so the first declaration read (descending order): the struct is reached first.
    let params = engine.struct_in(actor, "Params", 0, None);
    engine.prop("Int", params, "X", 0);
    engine.prop("Struct", actor, "Held", params);
    for n in 0..extra {
        engine.prop("Int", params, &format!("Extra{n}"), 0);
    }
    let engine_path = tree.write("Engine.upk", &engine.build());
    let mut game = Declarations::new(&["Struct"]);
    let package = game.p.add_import("Core", "Package", 0, "Engine");
    let actor_import = game.p.add_import("Core", "Class", package, "Actor");
    let params_import = game
        .p
        .add_import("Core", "ScriptStruct", actor_import, "Params");
    let pawn = game.cls("Pawn", actor_import);
    game.prop("Struct", pawn, "Also", params_import);
    tree.write("Game.upk", &game.build());
    (engine_path, [actor, params, kinds, actor_import])
}

/// REFLECT-FIX: one file reached under two package ids (a load after `scan()`; the store's global fallback hands
/// out a fresh id the same way, decision 2026-11-15-05) is one class, one struct and one enum. Before the fix the
/// second id loaded Engine.Actor again and its layout was refused with `AlreadyDefined`, as W3-DECODE's Sanctuary
/// run saw for `ImpactResponseParameters`.
#[test]
fn one_file_under_two_ids_is_one_declaration() {
    let tree = Tree::new("two-ids");
    let (engine_path, [actor, params, kinds, actor_import]) = two_package_tree(&tree, 0);
    let mut store = tree.store();
    let first = store.load_path(&engine_path).unwrap();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, first)
        .unwrap();
    assert_eq!((summary.classes, summary.defined), (1, 1));

    store.scan().unwrap();
    let second = store.load_path(&engine_path).unwrap();
    assert_ne!(first, second, "the store hands out a second id");
    assert_eq!(store.get(first).source, store.get(second).source);

    // Through the second id directly, then through an import that the store now answers with it.
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, second)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (1, 1),
        "{:?}",
        summary.failures
    );
    let game = store.load_path(&tree.0.join("Game.upk")).unwrap();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, game)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (1, 1),
        "{:?}",
        summary.failures
    );
    assert_eq!(index.classes().count(), 2, "Actor once, Pawn once");
    assert_eq!(
        index.resolve_ref(&mut store, game, actor_import),
        Some((first, actor)),
        "answered under the id the index met first"
    );
    let actor_key = index.loaded_class(first, actor).unwrap();
    assert_eq!(index.loaded_class(second, actor), Some(actor_key));
    assert_eq!(index.class(actor_key).package, first);
    let struct_id = classes.struct_by_path("Engine.Actor.Params");
    assert!(struct_id.is_some());
    assert_eq!(index.struct_of(first, params), struct_id);
    assert_eq!(index.struct_of(second, params), struct_id);
    assert_eq!(
        index.enum_of(second, kinds),
        classes.enum_by_path("Engine.Actor.EKind")
    );
    assert_eq!(
        index.cached_ref(&store, second, actor),
        Some(Some((first, actor)))
    );
    let pawn_id = classes.by_path("Game.Pawn").unwrap();
    assert_eq!(
        classes.layout(pawn_id).super_class(),
        classes.by_path("Engine.Actor")
    );
}

/// The same path with other bytes (the file was rewritten between the two loads) is not the same declaration: the
/// second id is not folded into the first, and its class is refused as defined twice, as before.
#[test]
fn a_rewritten_file_under_a_second_id_stays_apart() {
    let tree = Tree::new("two-ids-rewritten");
    let (engine_path, [actor, ..]) = two_package_tree(&tree, 0);
    let mut store = tree.store();
    let first = store.load_path(&engine_path).unwrap();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    index
        .define_layouts(&mut store, &mut names, &mut classes, first)
        .unwrap();
    two_package_tree(&tree, 1);
    store.scan().unwrap();
    let second = store.load_path(&engine_path).unwrap();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, second)
        .unwrap();
    assert_eq!((summary.classes, summary.defined), (1, 0));
    assert!(matches!(
        root(summary.failures[0].1.clone()),
        ReflectError::Layout(LayoutError::AlreadyDefined { .. })
    ));
    assert_ne!(
        index.loaded_class(second, actor),
        index.loaded_class(first, actor)
    );
}

/// Review of PR #306: the second id comes from the store's global fallback, with no `scan()` in between. Game.upk
/// imports `Actor` inside its own export `Ghost`, so the index hands the import to the store (it looks up an
/// all-import chain itself, without the fallback). The root package name, Ghost, matches no file, so the store finds
/// `Ghost.Actor` in Engine.upk (which holds an export Ghost with the class Actor inside it) and answers with a fresh
/// id for that file. The declaration is still one class, filed under the id the index met first.
#[test]
fn a_global_fallback_answer_is_filed_under_the_first_id() {
    let tree = Tree::new("fallback-id");
    let mut engine = Declarations::new(&["Int"]);
    let package_class = engine.p.add_import("Core", "Class", 0, "Package");
    let ghost = engine.p.add_export(package_class, "Ghost", ints(&[0]));
    let actor = engine.cls("Actor", 0);
    engine.p.set_outer(actor, ghost);
    engine.prop("Int", actor, "Health", 0);
    let engine_path = tree.write("Engine.upk", &engine.build());
    let mut game = Declarations::new(&[]);
    let game_package_class = game.p.add_import("Core", "Class", 0, "Package");
    let local_ghost = game.p.add_export(game_package_class, "Ghost", ints(&[0]));
    let actor_import = game.p.add_import("Core", "Class", local_ghost, "Actor");
    let pawn = game.cls("Pawn", actor_import);
    tree.write("Game.upk", &game.build());

    let mut store = tree.store();
    let first = store.load_path(&engine_path).unwrap();
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, first)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (1, 1),
        "{:?}",
        summary.failures
    );

    let game_id = store.load_path(&tree.0.join("Game.upk")).unwrap();
    let summary = index
        .define_layouts(&mut store, &mut names, &mut classes, game_id)
        .unwrap();
    assert_eq!(
        (summary.classes, summary.defined),
        (1, 1),
        "{:?}",
        summary.failures
    );

    // Ids are positions in the store's list of loaded packages (store.rs): Engine.upk 0, Game.upk 1, and 2 the fresh
    // id the fallback handed out for Engine.upk while the index resolved Pawn's super class. (A second store lookup
    // answers from the store's path cache, with the first id.)
    assert_eq!((first, game_id), (0, 1));
    let fallback: PackageId = 2;
    assert_eq!(store.get(fallback).source, store.get(first).source);
    assert_eq!(store.get(fallback).path_of(actor).unwrap(), "Ghost.Actor");
    assert_eq!(index.filed_id(fallback), first);

    // The index: one Actor, under the first id.
    assert_eq!(index.classes().count(), 2, "Actor once, Pawn once");
    assert_eq!(
        index.resolve_ref(&mut store, game_id, actor_import),
        Some((first, actor))
    );
    let actor_key = index.loaded_class(first, actor).unwrap();
    assert_eq!(index.loaded_class(fallback, actor), Some(actor_key));
    assert_eq!(index.class(actor_key).package, first);
    let pawn_id = classes.by_path("Game.Pawn").unwrap();
    let pawn_key = index.loaded_class(game_id, pawn).unwrap();
    assert_eq!(index.layout_id(pawn_key), Some(pawn_id));
    assert_eq!(
        classes.layout(pawn_id).super_class(),
        index.layout_id(actor_key)
    );
}

/// The read-only lookups answer from what earlier calls loaded, and nothing else.
#[test]
fn read_only_lookups_answer_from_the_cache() {
    let tree = Tree::new("readonly");
    let mut d = Declarations::with_imports(&["Class", "Enum", "ByteProperty"]);
    let engine = d.p.add_import("Core", "Package", 0, "Engine");
    let asked = d.p.add_import("Core", "Class", engine, "Asked");
    let never = d.p.add_import("Core", "Class", engine, "Never");
    let thing = d.cls("Thing", 0);
    let kinds = d.enum_(thing, "EKind", &["K_A"]);
    d.prop("Byte", thing, "Kind", kinds);
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut index = CodeIndex::new();
    assert_eq!(index.loaded_class(id, thing), None);
    assert_eq!(index.cached_ref(&store, id, asked), None, "not asked yet");
    assert_eq!(index.resolve_ref(&mut store, id, asked), None);
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    index
        .define_layouts(&mut store, &mut names, &mut classes, id)
        .unwrap();
    let key = index.loaded_class(id, thing).unwrap();
    assert_eq!(index.layout_id(key), classes.by_path("Core.Thing"));
    assert_eq!(
        index.enum_of(id, kinds),
        classes.enum_by_path("Core.Thing.EKind")
    );
    assert_eq!(index.struct_of(id, thing), None);
    assert_eq!(
        index.cached_ref(&store, id, asked),
        Some(None),
        "asked: known to resolve to nothing"
    );
    assert_eq!(index.cached_ref(&store, id, never), None, "never asked");
    assert_eq!(index.cached_ref(&store, id, thing), Some(Some((id, thing))));
    assert_eq!(index.cached_ref(&store, id, 99), Some(None));
    assert_eq!(index.cached_ref(&store, id, 0), Some(None));
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

// ---- child_declarations and states (lane W3-INTERP, additive) ----------------------------------------------------

/// A class with one function (parameters, a local, a return value) and one state declaring a function, plus a class
/// with a function whose child has an unknown property class. Invented names and values.
fn function_fixture() -> (Declarations, i32, i32, i32) {
    let mut d = Declarations::with_imports(&[
        "Class",
        "ScriptStruct",
        "Function",
        "State",
        "IntProperty",
        "FloatProperty",
        "StructProperty",
        "ArrayProperty",
        "ObjectProperty",
        "WeirdProperty",
    ]);
    let object = d.cls("Object", 0);
    let vec = d.struct_in(object, "Vec", 0, None);
    d.prop("Float", vec, "X", 0);
    let pair = d.struct_in(object, "PairList", 0, None);
    d.prop("Int", pair, "N", 0);
    // Children in reverse declaration order, then the function: ReturnValue, B, V, A, then the local L (first).
    let function = d.p.next_export() + 6;
    let parm = 0x80;
    d.prop_with(&PropSpec {
        flags: parm | 0x100 | 0x400,
        ..PropSpec::new("Int", function, "ReturnValue")
    });
    d.prop_with(&PropSpec {
        flags: parm | 0x100 | 0x10,
        ..PropSpec::new("Float", function, "B")
    });
    d.prop_with(&PropSpec {
        flags: parm,
        type_ref: vec,
        ..PropSpec::new("Struct", function, "V")
    });
    d.prop_with(&PropSpec {
        flags: parm,
        dim: 2,
        ..PropSpec::new("Int", function, "A")
    });
    d.array_of(function, "L", "Struct", pair);
    let mut f = Export::new(d.import("Function"), "Compute", vec![0u8; 8]);
    f.outer = object;
    assert_eq!(d.p.add_export_with(f), function);
    let mut state = Export::new(d.import("State"), "Waiting", vec![0u8; 8]);
    state.outer = object;
    let state = d.p.add_export_with(state);
    let mut in_state = Export::new(d.import("Function"), "Tick", vec![0u8; 8]);
    in_state.outer = state;
    d.p.add_export_with(in_state);
    let odd = d.p.next_export() + 1;
    d.prop("Weird", odd, "Q", 0);
    let mut broken = Export::new(d.import("Function"), "Broken", vec![0u8; 8]);
    broken.outer = object;
    assert_eq!(d.p.add_export_with(broken), odd);
    (d, object, function, odd)
}

#[test]
fn child_declarations_type_a_functions_children_in_declaration_order() {
    let tree = Tree::new("children");
    let (mut d, _, function, _) = function_fixture();
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let decls = index
        .child_declarations(&mut store, &mut names, &mut classes, id, function)
        .unwrap();
    let shown: Vec<(String, u64, i32)> = decls
        .iter()
        .map(|p| (names.text(p.name).into_owned(), p.flags, p.array_dim))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("L".to_string(), 0, 1),
            ("A".to_string(), 0x80, 2),
            ("V".to_string(), 0x80, 1),
            ("B".to_string(), 0x80 | 0x100 | 0x10, 1),
            ("ReturnValue".to_string(), 0x80 | 0x100 | 0x400, 1),
        ]
    );
    let vec = classes.struct_by_path("Core.Object.Vec").unwrap();
    let pair = classes.struct_by_path("Core.Object.PairList").unwrap();
    assert_eq!(decls[2].kind, PropKind::Struct(vec));
    assert_eq!(decls[0].kind, PropKind::Array(Box::new(PropKind::Struct(pair))));
    assert_eq!(decls[3].kind, PropKind::Float);
    // The struct reached only through the array is defined before the call returns, not just declared.
    assert!(classes.struct_layout(pair).is_some_and(|l| l.slot_count() == 1));
    assert_eq!(decls[1].source, Some((id, function - 3)));
    assert!(decls.iter().all(|p| p.attr.is_none()));
    // A second call reads the same declarations again (nothing cached per outer).
    let again = index
        .child_declarations(&mut store, &mut names, &mut classes, id, function)
        .unwrap();
    assert_eq!(again, decls);
}

#[test]
fn child_declarations_refuse_an_unknown_property_class_and_answer_empty_for_no_children() {
    let tree = Tree::new("children-bad");
    let (mut d, object, _, odd) = function_fixture();
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let (mut names, mut classes) = (NameTable::new(), ClassTable::new());
    let mut index = CodeIndex::new();
    let error = index
        .child_declarations(&mut store, &mut names, &mut classes, id, odd)
        .unwrap_err();
    assert_eq!(error, ReflectError::UnknownPropertyClass("WeirdProperty".into()));
    // A struct export's own fields and an export with no property children.
    let vec = d.p.next_export(); // past the last export: no children at all
    assert_eq!(
        index
            .child_declarations(&mut store, &mut names, &mut classes, id, vec)
            .unwrap(),
        Vec::new()
    );
    let _ = object;
}

#[test]
fn a_class_lists_its_states_and_their_functions() {
    let tree = Tree::new("states");
    let (mut d, object, function, odd) = function_fixture();
    tree.write("Core.upk", &d.build());
    let (mut store, id) = load(&tree, "Core.upk");
    let mut index = CodeIndex::new();
    let key = index.class_at(&mut store, id, object).unwrap().unwrap();
    let info = index.class(key);
    assert_eq!(
        info.functions.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(),
        vec![("broken", odd), ("compute", function)]
    );
    let states: Vec<(&str, &str, i32, Vec<&str>)> = info
        .states()
        .iter()
        .map(|(k, s)| {
            (
                k.as_str(),
                s.name.as_str(),
                s.export,
                s.functions.keys().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(states, vec![("waiting", "Waiting", function + 1, vec!["tick"])]);
    // The state's function is not a class-level function.
    assert!(!info.functions.contains_key("tick"));
}

/// `implements_declared` (reviews r1 and r2 of #455) over [`interface_fixture`] plus invented classes: `Gone` is a
/// class import from a package no store holds (an entry that resolves to nothing); `Forward` names `[IThing, Gone]`
/// and `Backward` `[Gone, IThing]`; `BadEntry` names an export that is not a class; `Orphan` derives from `Gone`, and
/// `ViaOrphan` names `Orphan`; `LostParent` derives from `Gone` and names nothing.
#[test]
fn implements_declared_is_strict_order_free_and_keeps_its_errors() {
    let (mut d, _) = interface_fixture();
    let obj = 1; // `Object`, the fixture's first export
    let missing = d.p.add_import("Core", "Package", 0, "MissingPkg");
    let gone = d.p.add_import("Core", "Class", missing, "Gone");
    let not_a_class = d.p.add_export(obj, "NotAClassThing", ints(&[0, 0, 0, 0]));
    let tree = Tree::new("implements-declared");
    // The interface's export reference, read back from the fixture by name.
    let bytes_probe = d.build();
    tree.write("Core.upk", &bytes_probe);
    let iface_ref = {
        let mut store = tree.store();
        let id = store.load_path(&tree.0.join("Core.upk")).unwrap();
        CodeIndex::new().find_export(&store, id, "IThing")
    };
    d.real_class("Forward", obj, obj, 2, &[iface_ref, gone], None);
    d.real_class("Backward", obj, obj, 2, &[gone, iface_ref], None);
    d.real_class("BadEntry", obj, obj, 2, &[not_a_class], None);
    let orphan = d.real_class("Orphan", gone, obj, 2, &[], None);
    d.real_class("ViaOrphan", obj, obj, 2, &[orphan], None);
    d.real_class("LostParent", gone, obj, 2, &[], None);
    tree.write("Core.upk", &d.build());
    let mut store = tree.store();
    let mut index = CodeIndex::new();
    let mut key = |path: &str| index.find_class(&mut store, path).unwrap();
    let thing = key("Core.IThing");
    let names = ["Impl", "Derived", "Plain", "Forward", "Backward", "BadEntry", "ViaOrphan", "LostParent"];
    let keys: Vec<ClassKey> = names.iter().map(|n| key(&format!("Core.{n}"))).collect();
    let mut ask = |k: ClassKey| index.implements_declared(&mut store, k, thing);
    assert_eq!(ask(keys[0]), Ok(Some(true)), "own table");
    assert_eq!(ask(keys[1]), Ok(Some(true)), "a super class's table");
    assert_eq!(ask(keys[2]), Ok(Some(false)), "names nothing");
    // Item 2: an entry that resolves to nothing makes the answer unreadable in either order.
    assert_eq!(ask(keys[3]), Ok(None), "[IThing, Gone]");
    assert_eq!(ask(keys[4]), Ok(None), "[Gone, IThing]");
    // Item 1: a non-class entry is its typed error, on the first call and from the cache on the next.
    for call in 0..2 {
        match ask(keys[5]) {
            Err(ReflectError::NotAClass(path)) => assert!(path.contains("NotAClassThing"), "{path}"),
            other => panic!("call {call}: expected NotAClass, got {other:?}"),
        }
    }
    // Item 3: a target, or the source, whose super class reference resolves to nothing is unreadable, not absent.
    assert_eq!(ask(keys[6]), Ok(None), "the target's chain is incomplete");
    assert_eq!(ask(keys[7]), Ok(None), "the source's chain is incomplete");
}
