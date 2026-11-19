//! Owner: lane W3-DECODE. Tests of `cdo.rs` on the invented `Core.upk` of `decode/tests.rs` (AGENTS.md rule 9): the
//! C++ `buildDefaults` order (root class first, each default object adding only what it changes) and
//! `instantiateExport` (class defaults, then the export's own tags from its prefix; the C++ refusals).

use ow_core::ParityMode;
use ow_package::testkit::ints;

use super::*;
use crate::decode::tests::{Tree, World, decls, get, stream, world};
use crate::value::Value;

#[test]
fn class_defaults_apply_from_the_root_class_down() {
    let mut w = world("cdo-order", false);
    let base = w.class("Base");
    let leaf = w.class("Leaf");
    let mut cx = w.cx(ParityMode::Off);
    let base_default = class_default(&mut cx, base).unwrap();
    let leaf_default = class_default(&mut cx, leaf).unwrap();
    assert_eq!(
        class_default(&mut cx, leaf).unwrap(),
        leaf_default,
        "built once"
    );
    let s = &w.space;
    // Default__Base sets A and B; Default__Leaf changes A and leaves B (its own B tag fails).
    assert_eq!(
        (get(s, base_default, "A"), get(s, base_default, "B")),
        (Value::Int(3), Value::Float(1.5))
    );
    assert_eq!(
        (get(s, leaf_default, "A"), get(s, leaf_default, "B")),
        (Value::Int(7), Value::Float(1.5))
    );
    let object = s.heap.get(leaf_default).unwrap();
    assert_eq!(s.names.text(object.name), "Default__Leaf");
    assert_eq!(
        object.origin,
        Origin::Default {
            class: object.class
        }
    );
}

#[test]
fn an_export_starts_from_its_class_default_and_applies_its_own_tags() {
    let mut w = world("cdo-export", false);
    let thing = w.export("Thing");
    let comp = w.export("Comp");
    let core = w.core;
    let mut cx = w.cx(ParityMode::Off);
    let o = instantiate_export(&mut cx, core, thing, PropertyPrefix::Ordinary).unwrap();
    let c = instantiate_export(&mut cx, core, comp, PropertyPrefix::Component).unwrap();
    // Both streams end at their export's end (no untagged tail).
    let size = |index: i32| w.store.get(core).object(index).unwrap().size as usize;
    assert_eq!((o.tags_end, c.tags_end), (size(thing), size(comp)));
    let (o, c) = (o.object, c.object);
    let s = &w.space;
    assert_eq!(
        (get(s, o, "D"), get(s, o, "A")),
        (Value::Int(2), Value::Int(7))
    );
    assert_eq!(get(s, c, "D"), Value::Int(3));
    let object = s.heap.get(o).unwrap();
    assert_eq!(s.names.text(object.name), "Thing");
    assert_eq!(
        object.origin,
        Origin::Export {
            package: core,
            index: thing
        }
    );
    assert_eq!(object.outer, None);
}

#[test]
fn the_cxx_refusals() {
    let mut w = world("cdo-refusals", false);
    let tiny = w.export("Tiny");
    let object = w.export("Object");
    let core = w.core;
    let mut cx = w.cx(ParityMode::Off);
    let err = |cx: &mut crate::decode::ValueCx<'_>, index: i32| {
        instantiate_export(cx, core, index, PropertyPrefix::Ordinary)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(err(&mut cx, 0), "unsupported exported object request");
    assert_eq!(err(&mut cx, -1), "unsupported exported object request");
    assert_eq!(err(&mut cx, 9999), "invalid object reference");
    // A Class export's class (Core.Class) is in no package here: the class resolves to nothing.
    assert_eq!(err(&mut cx, object), "cannot instantiate a null class");
    // Four bytes cannot hold the prefix and a tag.
    assert_eq!(err(&mut cx, tiny), "object property prefix outside export");
}

#[test]
fn a_failed_instantiation_leaves_no_object_behind() {
    let mut w = world("cdo-rollback", false);
    let tiny = w.export("Tiny");
    let broken = w.export("Broken");
    let core = w.core;
    let leaf = w.class("Leaf");
    let leaf_id = w.cx(ParityMode::Off).layout(leaf).unwrap();
    // Build the class default (and its stand-ins) first, so the counts below see only the instantiations.
    class_default(&mut w.cx(ParityMode::Off), leaf).unwrap();
    let before = w.space.heap.len();
    for (index, text) in [
        (tiny, "object property prefix outside export"),
        (broken, "negative property size or index"),
        (tiny, "object property prefix outside export"),
    ] {
        let err = instantiate_export(
            &mut w.cx(ParityMode::Off),
            core,
            index,
            PropertyPrefix::Ordinary,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), text);
        assert_eq!(w.space.heap.len(), before, "nothing piles up");
        assert_eq!(w.space.live(leaf_id).count(), 0, "no live Leaf instance");
    }
    // A class default whose own stream breaks keeps its earlier writes (next test); an export that succeeds stays.
    let thing = w.export("Thing");
    let ok = instantiate_export(
        &mut w.cx(ParityMode::Off),
        core,
        thing,
        PropertyPrefix::Ordinary,
    )
    .unwrap();
    assert_eq!(w.space.live(leaf_id).collect::<Vec<_>>(), [ok.object]);
}

#[test]
fn a_default_object_whose_stream_breaks_keeps_what_it_applied_and_logs_the_error() {
    let mut d = decls();
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let thing = d.cls("Thing", object);
    d.prop("Int", thing, "B", 0);
    d.prop("Int", thing, "A", 0);
    let a = d.p.int_tag("A", 4);
    let bad = d.p.tag_full("B", 0, "IntProperty", -1, &[], &[]);
    let mut payload = ints(&[0]);
    payload.extend(a);
    payload.extend(bad);
    d.p.add_export(thing, "Default__Thing", payload);
    let tags = vec![d.p.int_tag("B", 1)];
    let other = stream(&mut d, 4, &tags);
    d.p.add_export(thing, "Default__Thing", other);
    let tree = Tree::new("cdo-broken");
    tree.write("Core.upk", &d.build());
    let mut w = World::open(tree);
    let key = w.class("Thing");
    let o = class_default(&mut w.cx(ParityMode::Off), key).unwrap();
    // Only the first export of that name and class applies, as in the C++.
    assert_eq!(
        (get(&w.space, o, "A"), get(&w.space, o, "B")),
        (Value::Int(4), Value::Int(0))
    );
    assert_eq!(
        w.state.log,
        ["defaults: Default__Thing: negative property size or index"]
    );
}

/// The class-default export selection shared with `ow-world`'s instancing (decision 2026-11-19-44, review r3 of #435):
/// the exact name and the class reference, first in table order. An export of the same name but another class, or of
/// another spelling, before it is passed over.
#[test]
fn the_default_export_is_the_exact_name_of_the_class() {
    let entry = |name: &str, class: i32| ow_package::tables::ObjectEntry {
        name: name.to_string(),
        class,
        ..Default::default()
    };
    let exports = [
        entry("Default__Toy", 9),
        entry("default__toy", 3),
        entry("Default__Toy", 3),
        entry("Default__Toy", 3),
    ];
    assert_eq!(default_export_in(&exports, 3, "Default__Toy"), Some(3));
    assert_eq!(default_export_in(&exports, 7, "Default__Toy"), None);
    assert!(!is_default_export(&exports[0], 3, "Default__Toy"));
}
