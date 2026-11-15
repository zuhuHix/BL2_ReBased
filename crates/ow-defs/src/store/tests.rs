//! Owner: lane W3-DEFS. Tests of the definition store on invented packages (AGENTS.md rule 9): every name, value and
//! file below is made up for the test.

use super::*;
use crate::identity::LoadSet;
use crate::overlay::{OverlayKind, Patch, PatchValue};
use ow_core::PackageName;
use ow_package::testkit::reflect::{Declarations, PropSpec};
use ow_package::testkit::{PackageBuilder, ints};

/// The class package: `Core.Object`, `Core.Component`, struct `Pair { A, Label }`, enum `EKind`, and class `Thing`
/// with `ClipSize`, `Damage`, `Next` (a `Thing`), `Pair`, `Slots[3]`, `List` (ints) and `Kind` (an `EKind`); its
/// default object sets `ClipSize` 1.
fn core() -> Vec<u8> {
    let mut d = Declarations::new(&["Int", "Float", "Object", "Struct", "Array", "Name", "Byte"]);
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let component = d.cls("Component", object);
    d.default_object(component, "Component");
    let pair = d.struct_in(object, "Pair", 0, None);
    d.prop("Name", pair, "Label", 0);
    d.prop("Int", pair, "A", 0);
    let kind = d.enum_(object, "EKind", &["K_One", "K_Two"]);
    let thing = d.cls("Thing", object);
    d.prop("Int", thing, "ClipSize", 0);
    d.prop("Float", thing, "Damage", 0);
    d.prop("Object", thing, "Next", thing);
    d.prop("Struct", thing, "Pair", pair);
    d.prop_with(&PropSpec {
        dim: 3,
        ..PropSpec::new("Int", thing, "Slots")
    });
    d.array_of(thing, "List", "Int", 0);
    d.prop_with(&PropSpec {
        dim: 3,
        type_ref: pair,
        ..PropSpec::new("Struct", thing, "Pairs")
    });
    d.prop("Byte", thing, "Kind", kind);
    let mut payload = ints(&[0]);
    payload.extend(d.p.int_tag("ClipSize", 1));
    payload.extend_from_slice(&d.none);
    d.p.add_export(thing, "Default__Thing", payload);
    d.build()
}

/// `Engine.Actor`, `Engine.ActorComponent` (a `Core.Component`) and `Engine.Level`.
fn engine() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let component = d.p.add_import("Core", "Class", core, "Component");
    for (name, super_ref) in [
        ("Actor", object),
        ("ActorComponent", component),
        ("Level", object),
    ] {
        let class = d.cls(name, super_ref);
        d.default_object(class, name);
    }
    d.build()
}

/// The class imports a definition package needs: (`Core.Thing`, `Engine.ActorComponent`, `Engine.Actor`,
/// `Engine.Level`, `Core.Package`).
fn class_imports(p: &mut PackageBuilder) -> (i32, i32, i32, i32, i32) {
    let core = p.add_import("Core", "Package", 0, "Core");
    let thing = p.add_import("Core", "Class", core, "Thing");
    let package = p.add_import("Core", "Class", core, "Package");
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let actor_component = p.add_import("Core", "Class", engine, "ActorComponent");
    let actor = p.add_import("Core", "Class", engine, "Actor");
    let level = p.add_import("Core", "Class", engine, "Level");
    (thing, actor_component, actor, level, package)
}

const TAIL: [u8; 3] = [0xAB, 0xCD, 0xEF];

/// `GD_Test.upk`: `Part_A` (ClipSize 7, Next -> Part_B, Pair.Label, then three untagged bytes), `Part_B` (Damage
/// 2.5, Next -> Part_A), `Part_C` (Next -> an import of a package no file holds), `Comp_A` (an actor component, tags
/// from 8 bytes in), `Comp_Bad` (an actor component whose tags start at 4: the derived prefix reads garbage),
/// `TheLevel` (a level) holding `Placed` (an actor, tags from 26 bytes in).
fn gd_test() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let (thing, actor_component, actor, level, _) = class_imports(&mut p);
    let missing = p.add_import("Core", "Package", 0, "GD_Missing");
    let nowhere = p.add_import("Core", "Thing", missing, "Nowhere");
    let none = p.none();

    let mut a = ints(&[0]);
    a.extend(p.int_tag("ClipSize", 7));
    a.extend(p.object_tag("Next", 2));
    let mut pair = p.name_tag("Label", "L");
    pair.extend_from_slice(&none);
    a.extend(p.struct_tag("Pair", "Pair", &pair));
    a.extend_from_slice(&none);
    a.extend_from_slice(&TAIL);
    assert_eq!(p.add_export(thing, "Part_A", a), 1);

    let mut b = ints(&[0]);
    b.extend(p.float_tag("Damage", 2.5));
    b.extend(p.object_tag("Next", 1));
    b.extend_from_slice(&none);
    assert_eq!(p.add_export(thing, "Part_B", b), 2);

    let mut c = ints(&[0]);
    c.extend(p.object_tag("Next", nowhere));
    c.extend_from_slice(&none);
    assert_eq!(p.add_export(thing, "Part_C", c), 3);

    let mut comp = ints(&[0, 0]);
    comp.extend_from_slice(&none);
    assert_eq!(p.add_export(actor_component, "Comp_A", comp), 4);
    let mut bad = ints(&[0]);
    bad.extend_from_slice(&none);
    assert_eq!(p.add_export(actor_component, "Comp_Bad", bad), 5);

    let level_ref = p.add_export(level, "TheLevel", {
        let mut l = ints(&[0]);
        l.extend_from_slice(&none);
        l
    });
    let mut placed = vec![0u8; 26];
    placed.extend_from_slice(&none);
    let mut export = ow_package::testkit::Export::new(actor, "Placed", placed);
    export.outer = level_ref;
    assert_eq!(p.add_export_with(export), 7);
    p.build()
}

/// `Startup.upk`: a `GD_Test` package object holding its own `Part_A` (ClipSize 99, Next -> an import of
/// `GD_Test.Part_B`), so the path `GD_Test.Part_A` has two holders.
fn startup() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let (thing, _, _, _, package) = class_imports(&mut p);
    let gd = p.add_import("Core", "Package", 0, "GD_Test");
    let part_b = p.add_import("Core", "Thing", gd, "Part_B");
    let none = p.none();
    assert_eq!(p.add_export(package, "GD_Test", ints(&[0])), 1);
    let mut a = ints(&[0]);
    a.extend(p.int_tag("ClipSize", 99));
    a.extend(p.object_tag("Next", part_b));
    a.extend_from_slice(&none);
    let mut export = ow_package::testkit::Export::new(thing, "Part_A", a);
    export.outer = 1;
    assert_eq!(p.add_export_with(export), 2);
    p.build()
}

/// A fresh folder with the package set (and a file that is not a package), and a store over it.
struct World {
    dir: PathBuf,
    defs: DefStore,
    w: ObjectSpace,
}

impl World {
    fn new(tag: &str, policy: IdentityPolicy, mode: ParityMode) -> World {
        let dir = std::env::temp_dir().join(format!("ow-defs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let files = [
            ("Core.upk", core()),
            ("Engine.upk", engine()),
            ("GD_Test.upk", gd_test()),
            ("Startup.upk", startup()),
            ("Broken.upk", vec![1, 2, 3]),
        ];
        let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        let defs = DefStore::from_synthetic(&dir, &files, policy, mode).unwrap();
        World {
            dir,
            defs,
            w: ObjectSpace::new(),
        }
    }

    fn off(tag: &str) -> World {
        World::new(tag, IdentityPolicy::FirstInScanOrder, ParityMode::Off)
    }

    fn find(&mut self, path: &str) -> Result<Option<ObjRef>, DefError> {
        self.defs
            .find(&mut self.w, &ObjectPath::parse(path).unwrap())
    }

    fn get(&mut self, path: &str) -> ObjRef {
        self.find(path).unwrap().unwrap()
    }

    fn key(&mut self, prop: &str) -> PropId {
        self.defs.key(&mut self.w, "Core.Thing", prop).unwrap()
    }

    fn read(&mut self, o: ObjRef, prop: &str) -> Value {
        let p = self.key(prop);
        self.w
            .read(o, PropAddr { prop: p, index: 0 })
            .unwrap()
            .clone()
    }

    fn read_at(&mut self, o: ObjRef, prop: &str, index: u32) -> Value {
        let p = self.key(prop);
        self.w.read(o, PropAddr { prop: p, index }).unwrap().clone()
    }

    fn object(&mut self, o: ObjRef, prop: &str) -> ObjRef {
        match self.read(o, prop) {
            Value::Object(Some(r)) => r,
            other => panic!("{prop}: {other:?}"),
        }
    }

    fn overlay(&mut self, kind: OverlayKind, text: &str) -> OverlayId {
        let o = Overlay::from_text("test", kind, text).unwrap();
        self.defs.push_overlay(&mut self.w, o).unwrap()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_definition_loads_in_place_and_its_references_are_unloaded_handles() {
    let mut t = World::off("inplace");
    let a = t.get("GD_Test.Part_A");
    assert!(t.defs.is_loaded(&t.w, a));
    assert_eq!(t.read(a, "ClipSize"), Value::Int(7));
    // A property the export does not tag keeps the class default's value.
    assert_eq!(t.read(a, "Damage"), Value::Float(0.0));
    let b = t.object(a, "Next");
    assert!(t.defs.is_unloaded(&t.w, b));
    assert!(!t.defs.is_loaded(&t.w, b));
    // An unloaded handle has no slots: reading one fails, it gives no guessed value.
    let damage = t.key("Damage");
    assert!(matches!(
        t.w.read(
            b,
            PropAddr {
                prop: damage,
                index: 0
            }
        ),
        Err(HeapError::SlotMissing { .. })
    ));
    assert_eq!(t.defs.load(&mut t.w, b).unwrap(), b);
    assert!(t.defs.is_loaded(&t.w, b));
    assert_eq!(t.read(b, "Damage"), Value::Float(2.5));
    // The cycle closes on the same object: loads never nest, so A was complete when it was handed out.
    assert_eq!(t.object(b, "Next"), a);
    assert_eq!(t.get("GD_Test.Part_B"), b);
    assert_eq!(t.get("gd_test.part_b"), b);
    assert!(matches!(
        t.w.heap.get(a).unwrap().origin,
        Origin::Export { index: 1, .. }
    ));
    // Loaded definitions are instances; the store's handles are not, until loaded.
    let thing = t.defs.class_id(&mut t.w, "Core.Thing").unwrap();
    let live: Vec<ObjRef> = t.w.live(thing).collect();
    assert_eq!(live, vec![a, b]);
}

#[test]
fn identity_is_by_path_whichever_file_holds_it() {
    // FirstInScanOrder: GD_Test.upk comes before Startup.upk in the scan, so its Part_A is the definition.
    let mut t = World::off("first");
    let a = t.get("GD_Test.Part_A");
    assert_eq!(t.read(a, "ClipSize"), Value::Int(7));
    // Loaded: Startup first, so Startup's copy is the definition; its import of GD_Test.Part_B is GD_Test.upk's
    // export, whose export reference back to Part_A is the path GD_Test.Part_A again: Startup's object.
    let set = LoadSet::new(vec![
        PackageName::parse("Startup").unwrap(),
        PackageName::parse("GD_Test").unwrap(),
    ]);
    let mut t = World::new("loaded", IdentityPolicy::Loaded(set), ParityMode::Off);
    let a = t.get("GD_Test.Part_A");
    assert_eq!(t.read(a, "ClipSize"), Value::Int(99));
    let b = t.object(a, "Next");
    t.defs.load(&mut t.w, b).unwrap();
    assert_eq!(t.object(b, "Next"), a);
    assert_eq!(t.get("GD_Test.Part_B"), b);
    // A path outside the load set is not found.
    let set = LoadSet::new(vec![PackageName::parse("Startup").unwrap()]);
    let mut t = World::new("loaded-only", IdentityPolicy::Loaded(set), ParityMode::Off);
    assert_eq!(t.find("GD_Test.Part_B").unwrap(), None);
    // A stem no file has is the store's error.
    let set = LoadSet::new(vec![PackageName::parse("Nothing").unwrap()]);
    let mut t = World::new("loaded-bad", IdentityPolicy::Loaded(set), ParityMode::Off);
    assert!(matches!(
        t.find("GD_Test.Part_B"),
        Err(DefError::Store(StoreError::PackageNotFound(_)))
    ));
}

#[test]
fn resolve_ref_goes_through_the_path() {
    let mut t = World::off("resolve");
    let a = t.get("GD_Test.Part_A");
    let startup = t
        .defs
        .packages
        .load(&PackageName::parse("Startup").unwrap())
        .unwrap();
    // Startup's own export at GD_Test.Part_A is the definition GD_Test.upk holds under FirstInScanOrder.
    assert_eq!(t.defs.resolve_ref(&mut t.w, startup, 2).unwrap(), Some(a));
    assert_eq!(t.defs.resolve_ref(&mut t.w, startup, 0).unwrap(), None);
    assert!(matches!(
        t.defs.resolve_ref(&mut t.w, startup, 40),
        Err(DefError::Store(StoreError::Package(_)))
    ));
}

#[test]
fn a_reference_with_no_holder_stays_a_stand_in_and_is_logged_once() {
    let mut t = World::off("noholder");
    let c = t.get("GD_Test.Part_C");
    let gone = t.object(c, "Next");
    assert!(!t.defs.is_unloaded(&t.w, gone));
    t.defs.ensure_loaded(&mut t.w, gone).unwrap();
    assert!(matches!(
        t.defs.load(&mut t.w, gone),
        Err(DefError::NotADefinition(_))
    ));
    let log = t.defs.take_log();
    assert_eq!(
        log.iter()
            .filter(|l| l.as_str() == "defs: no holder for GD_Missing.Nowhere")
            .count(),
        1,
        "{log:?}"
    );
    assert!(t.defs.take_log().is_empty());
    assert_eq!(t.find("GD_Missing.Nowhere").unwrap(), None);
}

#[test]
fn ensure_loaded_is_total_and_load_is_strict() {
    let mut t = World::off("total");
    let cdo = t.defs.class_default(&mut t.w, "Core.Thing").unwrap();
    assert_eq!(t.read(cdo, "ClipSize"), Value::Int(1));
    t.defs.ensure_loaded(&mut t.w, cdo).unwrap();
    assert!(matches!(
        t.defs.load(&mut t.w, cdo),
        Err(DefError::NotADefinition(_))
    ));
    // A Default__ path is the class default object.
    assert_eq!(t.find("Core.Default__Thing").unwrap(), Some(cdo));
    // Code is not a definition.
    assert!(matches!(
        t.find("Core.Thing"),
        Err(DefError::NotADefinition(_))
    ));
    // A stale handle is a heap error, in both.
    let thing = t.defs.class_id(&mut t.w, "Core.Thing").unwrap();
    let runtime = t.w.alloc(thing, "Spawned", None, Origin::Runtime).unwrap();
    t.defs.ensure_loaded(&mut t.w, runtime).unwrap();
    t.w.free(runtime).unwrap();
    assert!(matches!(
        t.defs.ensure_loaded(&mut t.w, runtime),
        Err(DefError::Heap(HeapError::Stale(_)))
    ));
    assert!(matches!(
        t.defs.load(&mut t.w, runtime),
        Err(DefError::Heap(_))
    ));
}

#[test]
fn the_prefix_comes_from_the_class_and_load_export_only_checks_it() {
    let mut t = World::off("prefix");
    let gd = t
        .defs
        .packages
        .load(&PackageName::parse("GD_Test").unwrap())
        .unwrap();
    // An actor component reads from 8; a placed actor (an Actor inside a Level) from 26.
    let comp = t.get("GD_Test.Comp_A");
    assert!(t.defs.is_loaded(&t.w, comp));
    let placed = t.get("GD_Test.TheLevel.Placed");
    assert!(t.defs.is_loaded(&t.w, placed));
    assert_eq!(
        t.defs
            .load_export(&mut t.w, gd, 4, PropertyPrefix::Component)
            .unwrap(),
        comp
    );
    assert_eq!(
        t.defs
            .load_export(&mut t.w, gd, 4, PropertyPrefix::Ordinary),
        Err(DefError::PrefixMismatch {
            object: "GD_Test.Comp_A".into(),
            derived: PropertyPrefix::Component,
            given: PropertyPrefix::Ordinary,
        })
    );
    // An actor component whose tags do not start at 8 does not decode there; the store does not try another prefix.
    let bad = t.find("GD_Test.Comp_Bad");
    assert!(bad.is_err(), "{bad:?}");
    // The same error comes back without decoding again or logging again.
    let log = t.defs.take_log();
    let again = t.find("GD_Test.Comp_Bad");
    assert_eq!(again.unwrap_err(), bad.unwrap_err());
    assert!(t.defs.take_log().is_empty(), "{log:?}");
    // Startup's Part_A is held by GD_Test.upk under FirstInScanOrder: not a second copy.
    let startup = t
        .defs
        .packages
        .load(&PackageName::parse("Startup").unwrap())
        .unwrap();
    assert!(matches!(
        t.defs
            .load_export(&mut t.w, startup, 2, PropertyPrefix::Ordinary),
        Err(DefError::NotADefinition(_))
    ));
}

#[test]
fn a_component_that_is_not_an_actor_component_needs_its_stream_to_parse() {
    // A class deriving from Core.Component but not Engine.ActorComponent: its derived prefix (8) is used only when
    // the stream parses from there; Comp_Bad's tags start at 4, so it is NotYet, never a guess.
    let mut d = Declarations::new(&[]);
    let core_package = d.p.add_import("Core", "Package", 0, "Core");
    let component = d.p.add_import("Core", "Class", core_package, "Component");
    let gadget = d.cls("Gadget", component);
    d.default_object(gadget, "Gadget");
    let none = d.none;
    let mut good = ints(&[0, 0]);
    good.extend_from_slice(&none);
    d.p.add_export(gadget, "Good", good);
    let mut bad = ints(&[0]);
    bad.extend_from_slice(&none);
    d.p.add_export(gadget, "Bad", bad);
    let extra = d.build();
    let dir = std::env::temp_dir().join(format!("ow-defs-gadget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let files: [(&str, &[u8]); 2] = [("Core.upk", &core()), ("Gadgets.upk", &extra)];
    let mut defs = DefStore::from_synthetic(
        &dir,
        &files,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut w = ObjectSpace::new();
    let good = defs
        .find(&mut w, &ObjectPath::parse("Gadgets.Good").unwrap())
        .unwrap()
        .unwrap();
    assert!(defs.is_loaded(&w, good));
    assert!(matches!(
        defs.find(&mut w, &ObjectPath::parse("Gadgets.Bad").unwrap()),
        Err(DefError::NotYet(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn raw_tail_is_the_bytes_after_the_none_tag() {
    let mut t = World::off("tail");
    let a = t.get("GD_Test.Part_A");
    assert_eq!(t.defs.raw_tail(a).unwrap().bytes(), &TAIL);
    let b = t.get("GD_Test.Part_B");
    assert!(t.defs.raw_tail(b).unwrap().bytes().is_empty());
    let c = t.get("GD_Test.Part_C");
    let gone = t.object(c, "Next");
    assert!(t.defs.raw_tail(gone).is_none());
}

#[test]
fn owners_census_and_exports_of_class() {
    let mut t = World::off("census");
    let holders = t
        .defs
        .owners(&ObjectPath::parse("GD_Test.Part_A").unwrap())
        .unwrap();
    let files: Vec<(&str, i32, &str, bool)> = holders
        .holders
        .iter()
        .map(|o| {
            (
                o.package.as_str(),
                o.export,
                o.class.as_str(),
                o.id.is_some(),
            )
        })
        .collect();
    assert_eq!(
        files,
        vec![
            ("GD_Test", 1, "Core.Thing", true),
            ("Startup", 2, "Core.Thing", true)
        ]
    );
    // Broken.upk cannot be read: it is a candidate for every path, and its failure is reported.
    assert_eq!(holders.failures.len(), 1);
    assert!(holders.failures[0].file.ends_with("Broken.upk"));

    let census = t.defs.census().unwrap();
    assert_eq!(census.files.len(), 5);
    assert_eq!(census.failures.len(), 1);
    assert!(census.failures[0].file.ends_with("Broken.upk"));
    let paths: Vec<&str> = census.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"Part_A") && paths.contains(&"GD_Test.Part_A"));
    assert!(
        census
            .class_defaults
            .iter()
            .any(|e| e.path == "Default__Thing")
    );
    assert!(census.entries.iter().all(|e| !e.path.contains("Default__")));
    let part_a = census.entries.iter().find(|e| e.path == "Part_A").unwrap();
    assert_eq!(part_a.holders[0].class, "Core.Thing");
    assert_eq!(part_a.holders[0].id, None);
    // A class the file exports itself prints as its bare path, as --exports prints it.
    let thing = census.entries.iter().find(|e| e.path == "Thing").unwrap();
    assert_eq!(thing.holders[0].class, "Class");
    let things: Vec<String> = t
        .defs
        .exports_of_class("Core.Thing")
        .unwrap()
        .iter()
        .map(|p| p.as_str().to_string())
        .collect();
    assert_eq!(things, ["Part_A", "Part_B", "Part_C", "GD_Test.Part_A"]);
    assert_eq!(
        t.defs
            .exports_of_class("Engine.ActorComponent")
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn overlays_patch_loaded_objects_now_and_others_when_they_load() {
    let mut t = World::off("overlay");
    let a = t.get("GD_Test.Part_A");
    let first = t.overlay(
        OverlayKind::Test,
        concat!(
            "GD_Test.Part_A ClipSize = 9\n",
            "GD_Test.Part_A Slots = [1, 2, 3]\n",
            "GD_Test.Part_B Slots[2] = 5\n",
            "GD_Test.Part_B Pair.Label = \"Patched\"\n",
            "GD_Test.Part_B Next = \"GD_Test.Part_C\"\n",
            "GD_Test.Part_B List = [4, 5]\n",
            "GD_Test.Part_B Kind = \"K_Two\"\n",
            "GD_Test.Part_B Damage = NaN\n",
            "GD_Test.Part_B Pairs[2].A = 4\n",
            "GD_Test.Part_B Missing = 1\n",
            "GD_Test.Part_B ClipSize = \"seven\"\n",
            "GD_Elsewhere.Part ClipSize = 1\n",
            "Core.Default__Thing ClipSize = 3\n",
        ),
    );
    // Part_A was loaded: its two patches applied at once.
    assert_eq!(t.read(a, "ClipSize"), Value::Int(9));
    assert_eq!(t.read_at(a, "Slots", 2), Value::Int(3));
    let status = t.defs.overlay_status(first).unwrap().clone();
    assert_eq!((status.applied, status.pending), (2, 10));
    assert_eq!(status.failed.len(), 1);
    assert!(matches!(
        status.failed[0],
        (12, DefError::NotADefinition(_))
    ));
    let clip = t.key("ClipSize");
    assert_eq!(
        t.defs.origin(
            a,
            PropAddr {
                prop: clip,
                index: 0
            }
        ),
        ValueOrigin::Overlay(first)
    );
    let damage = t.key("Damage");
    assert_eq!(
        t.defs.origin(
            a,
            PropAddr {
                prop: damage,
                index: 0
            }
        ),
        ValueOrigin::Cooked
    );
    let slots = t.key("Slots");
    assert_eq!(
        t.defs.origin(
            a,
            PropAddr {
                prop: slots,
                index: 3
            }
        ),
        ValueOrigin::NotADefinition
    );

    // Part_B loads now: its patches apply, the two that do not type are failed, the cooked values stay there.
    let b = t.get("GD_Test.Part_B");
    assert_eq!(t.read_at(b, "Slots", 2), Value::Int(5));
    assert_eq!(t.read_at(b, "Slots", 0), Value::Int(0));
    match t.read(b, "Pair") {
        Value::Struct(s) => assert!(
            s.fields
                .contains(&Value::Name(t.w.names.find("Patched").unwrap()))
        ),
        other => panic!("{other:?}"),
    }
    let c = t.object(b, "Next");
    assert_eq!(t.get("GD_Test.Part_C"), c);
    assert_eq!(
        t.read(b, "List"),
        Value::array(vec![Value::Int(4), Value::Int(5)])
    );
    assert_eq!(t.read(b, "Kind"), Value::Byte(1));
    assert!(matches!(t.read(b, "Damage"), Value::Float(f) if f.is_nan()));
    assert_eq!(t.read(b, "ClipSize"), Value::Int(1));
    // `Pairs[2].A` addresses element 2 of the static array (a PropAddr) and the field inside it.
    let pairs = t.key("Pairs");
    match t.read_at(b, "Pairs", 2) {
        Value::Struct(s) => assert!(s.fields.contains(&Value::Int(4))),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        t.defs.origin(
            b,
            PropAddr {
                prop: pairs,
                index: 2
            }
        ),
        ValueOrigin::Overlay(first)
    );
    assert_eq!(
        t.defs.origin(
            b,
            PropAddr {
                prop: pairs,
                index: 1
            }
        ),
        ValueOrigin::Cooked
    );
    let status = t.defs.overlay_status(first).unwrap().clone();
    assert_eq!((status.applied, status.pending), (9, 1));
    let failed: Vec<usize> = status.failed.iter().map(|(i, _)| *i).collect();
    assert_eq!(failed, vec![12, 9, 10]);

    // A later overlay wins, and its origin is the later one.
    let second = t.overlay(
        OverlayKind::TextMod,
        "GD_Test.Part_A ClipSize = 10\nCore.Default__Thing ClipSize = 3\n",
    );
    assert_eq!(t.read(a, "ClipSize"), Value::Int(10));
    assert_eq!(
        t.defs.origin(
            a,
            PropAddr {
                prop: clip,
                index: 0
            }
        ),
        ValueOrigin::Overlay(second)
    );
    // A class default patch is a missing feature for a text mod.
    assert!(matches!(
        t.defs.overlay_status(second).unwrap().failed[0],
        (1, DefError::NotYet(_))
    ));
    // A property path outside the grammar refuses the whole overlay.
    let bad = Overlay::new(
        "bad",
        OverlayKind::Test,
        vec![Patch::new(
            ObjectPath::parse("GD_Test.Part_A").unwrap(),
            "ClipSize[",
            PatchValue::Number("1".into()),
        )],
    );
    assert!(matches!(
        t.defs.push_overlay(&mut t.w, bad),
        Err(DefError::BadPropertyPath { .. })
    ));
    assert_eq!(t.defs.overlay_status(OverlayId::new(2)), None);
}

/// The overlay gate on a synthetic capture: a capture in the shape `weapon_dump.py` writes changes exactly the objects
/// whose live values differ from the cooked ones (the real "39 changed objects" run needs the local capture).
#[test]
fn a_synthetic_live_capture_changes_exactly_the_objects_that_differ() {
    let mut t = World::off("live");
    let paths = ["GD_Test.Part_A", "GD_Test.Part_B", "GD_Test.Part_C"];
    let props = ["ClipSize", "Damage", "Next"];
    let mut cooked = Vec::new();
    for p in paths {
        let o = t.get(p);
        cooked.push(props.map(|k| t.read(o, k)));
    }
    let capture = r#"{
        "GD_Test.Part_A": {"class": "Thing", "properties": {"ClipSize": 7, "Damage": 0.0}},
        "GD_Test.Part_B": {"class": "Thing", "properties": {"ClipSize": 12, "Next": "Thing'GD_Test.Part_A'"}},
        "GD_Test.Part_C": {"class": "Thing", "properties": {"Damage": 0.30000001192092896}},
        "GD_Elsewhere.Gone": {"class": "Thing", "properties": {"ClipSize": 1}}
    }"#;
    let o = Overlay::from_live_capture("live_weapon_data.json", capture).unwrap();
    let id = t.defs.push_overlay(&mut t.w, o).unwrap();
    let status = t.defs.overlay_status(id).unwrap().clone();
    assert_eq!(
        (status.applied, status.pending, status.failed.len()),
        (5, 1, 0)
    );
    let mut changed = Vec::new();
    for (p, before) in paths.iter().zip(&cooked) {
        let o = t.get(p);
        if props.map(|k| t.read(o, k)) != *before {
            changed.push(*p);
        }
    }
    assert_eq!(changed, ["GD_Test.Part_B", "GD_Test.Part_C"]);
    let c = t.get("GD_Test.Part_C");
    assert_eq!(t.read(c, "Damage"), Value::Float(0.3));
}

#[test]
fn the_reference_mode_keys_a_no_holder_stand_in_by_its_import() {
    let mut t = World::new(
        "reference",
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Reference,
    );
    let c = t.get("GD_Test.Part_C");
    let gone = t.object(c, "Next");
    assert!(matches!(
        t.w.heap.get(gone).unwrap().origin,
        Origin::Resource { index, .. } if index < 0
    ));
}

#[test]
fn keys_are_cached_and_unknown_properties_are_refused() {
    let mut t = World::off("keys");
    let k = t.key("ClipSize");
    assert_eq!(t.defs.key(&mut t.w, "core.thing", "clipsize").unwrap(), k);
    assert!(matches!(
        t.defs.key(&mut t.w, "Core.Thing", "NoSuch"),
        Err(DefError::NoSuchProperty { .. })
    ));
    assert!(t.defs.class_id(&mut t.w, "Core.NoClass").is_err());
}

#[test]
fn the_empty_store_finds_nothing() {
    let mut defs = DefStore::new_empty();
    let mut w = ObjectSpace::new();
    let path = ObjectPath::parse("GD_Test.Part").unwrap();
    assert_eq!(defs.find(&mut w, &path).unwrap(), None);
    assert_eq!(defs.census().unwrap(), Census::default());
    assert_eq!(defs.mode(), ParityMode::Off);
    assert_eq!(defs.policy(), &IdentityPolicy::FirstInScanOrder);
    let (_code, store) = defs.code_mut();
    assert_eq!(store.indexed_package_count(), 0);
}

#[test]
fn errors_print_their_layer_and_conversions_flatten() {
    let e = DefError::from(StoreError::PackageNotFound("X".into()));
    assert_eq!(e.to_string(), "package not found: X");
    assert!(std::error::Error::source(&e).is_none());
    let n = NotYet::not_read("x", "r");
    assert_eq!(
        DefError::from(DecodeError::NotYet(n.clone())),
        DefError::NotYet(n.clone())
    );
    assert_eq!(
        DefError::from(ReflectError::NotYet(n.clone())),
        DefError::NotYet(n.clone())
    );
    assert_eq!(
        DefError::from(StoreError::NotYet(n.clone())),
        DefError::NotYet(n)
    );
    let s = StoreError::PackageNotFound("P".into());
    assert_eq!(
        DefError::from(DecodeError::Reflect(ReflectError::Store(s.clone()))),
        DefError::Store(s.clone())
    );
    assert_eq!(
        DefError::from(DecodeError::Store(s.clone())),
        DefError::Store(s)
    );
    let n = NotYet::not_ported("lzo", "r");
    let p = PackageError::Container(ContainerError::NotYet(n.clone()));
    assert_eq!(
        DefError::from(StoreError::Package(p.clone())),
        DefError::NotYet(n.clone())
    );
    assert_eq!(DefError::from(DecodeError::Package(p)), DefError::NotYet(n));
    let h = HeapError::InstanceStandIn(Origin::Runtime);
    assert_eq!(
        DefError::from(DecodeError::Heap(h.clone())),
        DefError::Heap(h)
    );
}

/// A local check on the installed game, not run by default (`cargo test -p ow-defs -- --ignored`): the census of the
/// tree in `OW_DEFS_REAL_ROOT` written as tab-separated lines (path, then `file|export|class` per holder) to
/// `census.tsv` in the folder `OW_DEFS_LOCAL_OUT`, which must be under ignored `local/` (the lines are game data), and
/// `owners()` compared with the census's holder order on every 400th path that has holders in two or more files.
#[test]
#[ignore = "needs the installed game (OW_DEFS_REAL_ROOT) and an output folder under local/ (OW_DEFS_LOCAL_OUT)"]
fn real_census_and_owners() {
    use std::fmt::Write as _;
    let root = PathBuf::from(std::env::var("OW_DEFS_REAL_ROOT").expect("OW_DEFS_REAL_ROOT"));
    let out = PathBuf::from(std::env::var("OW_DEFS_LOCAL_OUT").expect("OW_DEFS_LOCAL_OUT"));
    let mut defs = DefStore::open(root, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let census = defs.census().unwrap();
    let mut text = String::new();
    for e in &census.entries {
        text.push_str(&e.path);
        for h in &e.holders {
            let _ = write!(text, "\t{}|{}|{}", h.file.display(), h.export, h.class);
        }
        text.push('\n');
    }
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("census.tsv"), text).unwrap();
    let mut summary = format!(
        "files {}\nentries {}\nclass_defaults {}\nfailures {}\n",
        census.files.len(),
        census.entries.len(),
        census.class_defaults.len(),
        census.failures.len()
    );
    let shared: Vec<&CensusEntry> = census
        .entries
        .iter()
        .filter(|e| {
            let mut files: Vec<&PathBuf> = e.holders.iter().map(|h| &h.file).collect();
            files.dedup();
            files.len() > 1
        })
        .collect();
    let (mut sampled, mut same_first, mut same_all) = (0, 0, 0);
    let mut mismatches = String::new();
    for e in shared.iter().step_by(400) {
        let Ok(path) = ObjectPath::parse(&e.path) else {
            continue;
        };
        let holders = defs.owners(&path).unwrap();
        sampled += 1;
        let mut census_files: Vec<&PathBuf> = e.holders.iter().map(|h| &h.file).collect();
        census_files.dedup();
        let owner_files: Vec<&PathBuf> = holders.holders.iter().map(|h| &h.file).collect();
        same_first += usize::from(owner_files.first() == census_files.first());
        same_all += usize::from(owner_files == census_files);
        if owner_files != census_files {
            // Game data, for the local summary only.
            let _ = writeln!(
                mismatches,
                "mismatch {}: census {census_files:?} owners {owner_files:?} failures {}",
                e.path,
                holders.failures.len()
            );
        }
    }
    let _ = write!(
        summary,
        "paths with holders in 2+ files {}\nowners sampled {sampled}\nsame first holder {same_first}\nsame holder list {same_all}\n",
        shared.len()
    );
    summary.push_str(&mismatches);
    std::fs::write(out.join("census_summary.txt"), summary).unwrap();
}

/// A census failure prints exactly what the reader's `--exports` prints after `ow-package: ` for the file (no path, no
/// wrapper): the texts below are the Rust `ow-package <file> --exports` stderr lines for the same bytes, checked locally.
#[test]
fn a_file_failure_prints_the_reader_s_exports_error() {
    let dir = std::env::temp_dir().join(format!("ow-defs-failure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Not a package: the reader tries it as a container, so the error has a cause.
    let not_a_package: &[u8] = &[1, 2, 3];
    // A plain package cut short inside its tables: one error, no cause.
    let mut short = PackageBuilder::new_empty();
    short.add_export(0, "Thing", ints(&[0]));
    let mut short = short.build();
    short.truncate(60);
    let files: [(&str, &[u8]); 3] = [
        ("Core.upk", &core()),
        ("NotAPackage.upk", not_a_package),
        ("Short.upk", &short),
    ];
    let mut defs = DefStore::from_synthetic(
        &dir,
        &files,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let census = defs.census().unwrap();
    let failure = |stem: &str| {
        census
            .failures
            .iter()
            .find(|f| f.file.file_stem().is_some_and(|s| s == stem))
            .unwrap_or_else(|| panic!("{stem} did not fail"))
    };
    assert_eq!(
        failure("NotAPackage").to_string(),
        "cannot decode the package container: truncated at byte 0: 1 more bytes needed"
    );
    // The error itself prints the first part; the container's text is its cause.
    assert_eq!(
        failure("NotAPackage").error.to_string(),
        "cannot decode the package container"
    );
    // Equal to the reader's own message for each failing file, built as the CLI builds it.
    for f in &census.failures {
        let reader = Package::load(&f.file).unwrap_err();
        let mut expected = reader.to_string();
        let mut cause = std::error::Error::source(&reader);
        while let Some(c) = cause {
            expected.push_str(": ");
            expected.push_str(&c.to_string());
            cause = c.source();
        }
        assert_eq!(f.to_string(), expected);
        assert!(!f.to_string().contains("Short.upk") && !f.to_string().contains("NotAPackage"));
    }
    assert_eq!(census.failures.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
