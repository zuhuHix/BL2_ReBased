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
/// default object sets `ClipSize` 1 and `List` [7, 8].
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
    payload.extend(d.p.array_tag("List", 2, &ints(&[7, 8])));
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
/// `TheLevel` (a level) holding `Placed` (an actor, tags from 26 bytes in), `Sub` (a subobject of `Part_A`) and
/// `Inner` (inside an imported package object `GD_Ext`).
fn gd_test() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let (thing, actor_component, actor, level, _) = class_imports(&mut p);
    let ext = p.add_import("Core", "Package", 0, "GD_Ext");
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
    let mut sub = ow_package::testkit::Export::new(thing, "Sub", {
        let mut b = ints(&[0]);
        b.extend(p.int_tag("ClipSize", 3));
        b.extend_from_slice(&none);
        b
    });
    sub.outer = 1;
    assert_eq!(p.add_export_with(sub), 8);
    let mut inner = ow_package::testkit::Export::new(thing, "Inner", {
        let mut b = ints(&[0]);
        b.extend_from_slice(&none);
        b
    });
    inner.outer = ext;
    assert_eq!(p.add_export_with(inner), 9);
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
        World::with_extra(tag, policy, mode, Vec::new())
    }

    /// [`World::new`] with `extra` files added to the package set.
    fn with_extra(tag: &str, policy: IdentityPolicy, mode: ParityMode, extra: Vec<(&'static str, Vec<u8>)>) -> World {
        let dir = std::env::temp_dir().join(format!("ow-defs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut files = vec![
            ("Core.upk", core()),
            ("Engine.upk", engine()),
            ("GD_Test.upk", gd_test()),
            ("Startup.upk", startup()),
            ("Broken.upk", vec![1, 2, 3]),
        ];
        files.extend(extra);
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

/// The file ids of `names`, loaded through the store's package store.
fn file_ids(t: &mut World, names: &[&str]) -> Vec<PackageId> {
    names.iter().map(|n| t.defs.code_mut().1.load(&PackageName::parse(n).unwrap()).unwrap()).collect()
}

/// Lane LINK1-HEAD-MESH (the hub's ruling): a handle reached from a file that holds its own copy of the path loads that
/// file's copy, and stays the one object for the path. `GD_Test.Part_A` has two holders: `GD_Test.upk` (ClipSize 7,
/// the policy's) and `Startup.upk` (ClipSize 99). Then the conflict: from `GD_Test.upk`, with Startup's copy resident,
/// a typed `NotRead` naming both files; and a file that holds no export at the path does nothing.
#[test]
fn a_file_s_own_copy_loads_into_the_handle_and_a_resident_other_copy_is_a_conflict() {
    let mut t = World::off("filecopy");
    let ids = file_ids(&mut t, &["Startup", "GD_Test", "Engine"]);
    let (startup, gd, engine) = (ids[0], ids[1], ids[2]);
    let b = t.get("GD_Test.Part_B");
    let a = t.object(b, "Next");
    assert!(t.defs.is_unloaded(&t.w, a), "an unloaded handle for the path");
    assert_eq!(t.defs.load_export_of_file(&mut t.w, a, engine).unwrap(), FileCopy::NotInFile);
    assert!(t.defs.is_unloaded(&t.w, a), "a file without the path does nothing");
    assert_eq!(t.defs.load_export_of_file(&mut t.w, a, startup).unwrap(), FileCopy::Loaded);
    assert_eq!(t.read(a, "ClipSize"), Value::Int(99), "Startup's own copy");
    assert_eq!(t.get("GD_Test.Part_A"), a, "still one object per path");
    assert_eq!(t.defs.load_export_of_file(&mut t.w, a, startup).unwrap(), FileCopy::Loaded, "again: nothing changes");
    match t.defs.load_export_of_file(&mut t.w, a, gd) {
        Err(DefError::NotYet(n)) => {
            assert_eq!(n.missing, ow_core::Missing::NotRead);
            assert!(n.what.contains("resident from Startup") && n.what.contains("GD_Test holds its own copy"), "{}", n.what);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(t.read(a, "ClipSize"), Value::Int(99), "the conflict changes nothing");
}

/// Other callers keep the identity policy's holder: an ordinary load of the same handle is `GD_Test.upk`'s copy, and a
/// later call from `Startup.upk` is then the conflict.
#[test]
fn ordinary_loads_keep_the_policy_holder() {
    let mut t = World::off("filecopy-policy");
    let ids = file_ids(&mut t, &["Startup", "GD_Test"]);
    let b = t.get("GD_Test.Part_B");
    let a = t.object(b, "Next");
    t.defs.ensure_loaded(&mut t.w, a).unwrap();
    assert_eq!(t.read(a, "ClipSize"), Value::Int(7));
    assert_eq!(t.defs.load_export_of_file(&mut t.w, a, ids[1]).unwrap(), FileCopy::Loaded, "the resident copy is GD_Test's");
    assert!(matches!(t.defs.load_export_of_file(&mut t.w, a, ids[0]), Err(DefError::NotYet(_))));
}

/// Review of #727, MED: an overlay patching the path keeps the re-point refused (its patches are placed by the
/// policy's holder), nothing changed, and the policy's load still takes the pending patch. An overlay pushed after a
/// re-point applies to the re-pointed copy.
#[test]
fn a_re_point_waits_for_no_overlay_and_an_overlay_after_it_applies_to_the_file_s_copy() {
    let mut t = World::off("filecopy-overlay");
    let ids = file_ids(&mut t, &["Startup"]);
    let b = t.get("GD_Test.Part_B");
    let a = t.object(b, "Next");
    t.overlay(OverlayKind::Test, "GD_Test.Part_A Damage = 5.0\n");
    match t.defs.load_export_of_file(&mut t.w, a, ids[0]) {
        Err(DefError::NotYet(n)) => {
            assert_eq!(n.missing, ow_core::Missing::NotRead);
            assert!(n.what.contains("has an overlay installed"), "{}", n.what);
        }
        other => panic!("{other:?}"),
    }
    assert!(t.defs.is_unloaded(&t.w, a), "nothing changed");
    t.defs.ensure_loaded(&mut t.w, a).unwrap();
    assert_eq!((t.read(a, "ClipSize"), t.read(a, "Damage")), (Value::Int(7), Value::Float(5.0)), "the policy's copy, patched");

    let mut t = World::off("filecopy-overlay-after");
    let ids = file_ids(&mut t, &["Startup"]);
    let b = t.get("GD_Test.Part_B");
    let a = t.object(b, "Next");
    assert_eq!(t.defs.load_export_of_file(&mut t.w, a, ids[0]).unwrap(), FileCopy::Loaded);
    let o = t.overlay(OverlayKind::Test, "GD_Test.Part_A Damage = 5.0\n");
    assert_eq!((t.read(a, "ClipSize"), t.read(a, "Damage")), (Value::Int(99), Value::Float(5.0)), "Startup's copy, patched");
    let status = t.defs.overlay_status(o).unwrap().clone();
    assert_eq!((status.applied, status.pending), (1, 0), "applied at once, not left pending");
}

/// Two files for the failure cases: `GD_Brk.upk` holds `Bad` (its own archetype: the store refuses its load),
/// `Good` (ClipSize 5) and `Holder1`/`Holder2` naming them; `GD_BrkCopy.upk` holds its own `GD_Brk.Bad` (good) and
/// `GD_Brk.Good` (its own archetype). `GD_Brk.upk` comes first in the scan, so its copies are the policy's.
fn brk_files() -> Vec<(&'static str, Vec<u8>)> {
    let cyclic = |p: &mut PackageBuilder, thing: i32, name: &str, me: i32, outer: i32| {
        let none = p.none();
        let mut b = ints(&[0]);
        b.extend_from_slice(&none);
        let mut e = ow_package::testkit::Export::new(thing, name, b);
        e.archetype = me;
        e.outer = outer;
        p.add_export_with(e)
    };
    let good = |p: &mut PackageBuilder, thing: i32, name: &str, clip: i32, outer: i32| {
        let none = p.none();
        let mut b = ints(&[0]);
        b.extend(p.int_tag("ClipSize", clip));
        b.extend_from_slice(&none);
        let mut e = ow_package::testkit::Export::new(thing, name, b);
        e.outer = outer;
        p.add_export_with(e)
    };
    let mut p = PackageBuilder::new_empty();
    let (thing, _, _, _, _) = class_imports(&mut p);
    assert_eq!(cyclic(&mut p, thing, "Bad", 1, 0), 1);
    assert_eq!(good(&mut p, thing, "Good", 5, 0), 2);
    for (name, r) in [("Holder1", 1), ("Holder2", 2)] {
        let none = p.none();
        let mut b = ints(&[0]);
        b.extend(p.object_tag("Next", r));
        b.extend_from_slice(&none);
        p.add_export(thing, name, b);
    }
    let brk = p.build();
    let mut p = PackageBuilder::new_empty();
    let (thing, _, _, _, package) = class_imports(&mut p);
    assert_eq!(p.add_export(package, "GD_Brk", ints(&[0])), 1);
    assert_eq!(good(&mut p, thing, "Bad", 6, 1), 2);
    assert_eq!(cyclic(&mut p, thing, "Good", 3, 1), 3);
    vec![("GD_Brk.upk", brk), ("GD_BrkCopy.upk", p.build())]
}

/// Review of #727, LOW: a handle whose load failed returns that stored error whichever file is asked (one holding the
/// path, one not); and when the file's own copy fails to load, the handle goes back to the policy's holder, which then
/// loads.
#[test]
fn a_failed_handle_keeps_its_error_and_a_failed_file_copy_restores_the_policy_holder() {
    let mut t = World::with_extra("filecopy-failed", IdentityPolicy::FirstInScanOrder, ParityMode::Off, brk_files());
    let ids = file_ids(&mut t, &["GD_BrkCopy", "Engine"]);
    let holder = t.get("GD_Brk.Holder1");
    let bad = t.object(holder, "Next");
    let stored = t.defs.ensure_loaded(&mut t.w, bad).unwrap_err();
    assert!(t.defs.is_failed(&t.w, bad));
    for f in ids.iter().copied() {
        assert_eq!(t.defs.load_export_of_file(&mut t.w, bad, f).unwrap_err(), stored);
    }
    let holder = t.get("GD_Brk.Holder2");
    let good = t.object(holder, "Next");
    assert!(t.defs.load_export_of_file(&mut t.w, good, ids[0]).is_err(), "the file's own copy fails");
    assert!(t.defs.is_unloaded(&t.w, good) && !t.defs.is_failed(&t.w, good), "back to an unloaded handle");
    t.defs.ensure_loaded(&mut t.w, good).unwrap();
    assert_eq!(t.read(good, "ClipSize"), Value::Int(5), "the policy's copy loads");
}

/// `GD_Skip.upk` (lane IMPL-AIMOFFSET-PROFILES): `Thing` with a `ClipSize` tag whose size does not fit an int (the
/// decoder skips it and logs), a decoded empty `List`, and no `Damage` tag; `Holder` names it.
fn skip_file() -> Vec<(&'static str, Vec<u8>)> {
    let mut p = PackageBuilder::new_empty();
    let (thing, _, _, _, _) = class_imports(&mut p);
    let none = p.none();
    let mut b = ints(&[0]);
    b.extend(p.tag("ClipSize", "IntProperty", &ints(&[1, 2])));
    b.extend(p.array_tag("List", 0, &[]));
    b.extend_from_slice(&none);
    assert_eq!(p.add_export(thing, "Thing", b), 1);
    let mut h = ints(&[0]);
    h.extend(p.object_tag("Next", 1));
    h.extend_from_slice(&none);
    p.add_export(thing, "Holder", h);
    vec![("GD_Skip.upk", p.build())]
}

/// Lane IMPL-AIMOFFSET-PROFILES: a declared property the decoder skips is marked on the loaded object, through `find`
/// and through a handle's load alike; a decoded empty array, a property the export does not tag and an object the
/// store did not load are not. The log line and the slot (the class default) are as before, in both modes.
#[test]
fn a_skipped_property_is_marked_and_a_decoded_or_absent_one_is_not() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut t = World::with_extra("skipmark", IdentityPolicy::FirstInScanOrder, mode, skip_file());
        let (clip, list, damage) = (t.key("ClipSize"), t.key("List"), t.key("Damage"));
        let holder = t.get("GD_Skip.Holder");
        let thing = t.object(holder, "Next");
        assert!(!t.defs.is_skipped(&t.w, thing, clip), "{mode:?}: an unloaded handle has no mark");
        t.defs.ensure_loaded(&mut t.w, thing).unwrap();
        assert!(t.defs.is_skipped(&t.w, thing, clip), "{mode:?}: the skip is the handle's");
        assert!(!t.defs.is_skipped(&t.w, thing, list), "{mode:?}: a decoded empty array");
        assert!(!t.defs.is_skipped(&t.w, thing, damage), "{mode:?}: a property the export does not tag");
        assert!(!t.defs.is_skipped(&t.w, holder, clip), "{mode:?}: another object");
        assert_eq!(t.read(thing, "ClipSize"), Value::Int(1), "{mode:?}: the class default stays");
        assert!(t.defs.take_log().iter().any(|l| l == "defaults: skipped Thing.ClipSize (IntProperty)"), "{mode:?}");
        let found = t.get("GD_Skip.Thing");
        assert_eq!(found, thing);
        assert!(t.defs.is_skipped(&t.w, found, clip));
    }
}

#[test]
fn candidate_lists_are_the_store_s_and_a_prefetched_holder_is_adopted_once() {
    // Lane FIX-SCENE-LOAD: the cached candidate lists are the package store's own, whatever the case or form of the
    // path, and a holder file the prefetch read is handed to the store under the id `load_path` gives.
    let mut t = World::off("prefetch");
    for path in [
        "GD_Test.Part_A",
        "gd_test.part_b",
        "GD_TEST.Part_A.Sub",
        "Startup.Other",
        "Nothing.Here",
    ] {
        let cached = t
            .defs
            .handles
            .borrow_mut()
            .candidates(&mut t.defs.packages, path);
        let fresh = t.defs.packages.global_candidates(path);
        assert_eq!(cached.to_vec(), fresh, "{path}");
    }
    // The prefetch kept only the files that hold the path it read for (GD_Test.Part_A: GD_Test and Startup); a
    // lookup releases them.
    let mut held: Vec<String> = t
        .defs
        .handles
        .borrow()
        .held
        .keys()
        .map(|f| f.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    held.sort();
    assert_eq!(held, ["GD_Test", "Startup"]);
    t.defs.handles.borrow_mut().release();
    let a = t.get("GD_Test.Part_A");
    assert_eq!(t.read(a, "ClipSize"), Value::Int(7));
    let file = t
        .defs
        .handles
        .borrow()
        .file_ids
        .iter()
        .find(|(f, _)| f.file_stem().is_some_and(|s| s == "GD_Test"))
        .map(|(f, &id)| (f.clone(), id))
        .expect("GD_Test's id is kept");
    assert_eq!(t.defs.packages.load_path(&file.0).unwrap(), file.1);
    assert!(t.defs.handles.borrow().held.is_empty());
    // A new store whose first lookup reads the candidates together: the holder is the scan order's first, kept
    // loaded once (adopted), and the same object as above.
    let mut u = World::off("prefetch-first");
    let a = u.get("GD_Test.Part_A");
    assert_eq!(u.read(a, "ClipSize"), Value::Int(7));
    assert!(u.defs.handles.borrow().held.is_empty());
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

/// `GD_Arch.upk`, invented for the archetype prefix test (decision 2026-11-16-36): `Character` (a package object)
/// holding the actor archetypes `Pawn_Arch` (tags from 4 bytes in, a stream that runs past 26 bytes so a 26-byte read
/// lands inside it) and `Pawn_Bad` (no tag stream from 4); `Pawn_Top`, an actor archetype with no outer; `TheLevel`
/// holding the placed actor `Holder` (26), which holds `Nested` (tags from 26) and `NestedAt4` (tags from 4).
fn gd_arch() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let (_, _, actor, level, package) = class_imports(&mut p);
    let none = p.none();
    let at4 = |p: &mut PackageBuilder| {
        let mut b = ints(&[0]);
        b.extend(p.name_tag("Label", "A_Label_Long_Enough"));
        b.extend(p.int_tag("Count", 3));
        b.extend_from_slice(&none);
        b
    };
    let at26 = || {
        let mut b = vec![0u8; 26];
        b.extend_from_slice(&none);
        b
    };
    assert_eq!(p.add_export(package, "Character", ints(&[0])), 1);
    let payload = at4(&mut p);
    assert!(payload.len() > 26 + none.len());
    let mut arch = ow_package::testkit::Export::new(actor, "Pawn_Arch", payload);
    arch.outer = 1;
    assert_eq!(p.add_export_with(arch), 2);
    let mut bad = ow_package::testkit::Export::new(actor, "Pawn_Bad", ints(&[0, i32::MAX, 0]));
    bad.outer = 1;
    assert_eq!(p.add_export_with(bad), 3);
    let payload = at4(&mut p);
    assert_eq!(p.add_export(actor, "Pawn_Top", payload), 4);
    assert_eq!(p.add_export(level, "TheLevel", ints(&[0])), 5);
    let mut holder = ow_package::testkit::Export::new(actor, "Holder", at26());
    holder.outer = 5;
    assert_eq!(p.add_export_with(holder), 6);
    let mut nested = ow_package::testkit::Export::new(actor, "Nested", at26());
    nested.outer = 6;
    assert_eq!(p.add_export_with(nested), 7);
    let payload = at4(&mut p);
    let mut nested4 = ow_package::testkit::Export::new(actor, "NestedAt4", payload);
    nested4.outer = 6;
    assert_eq!(p.add_export_with(nested4), 8);
    p.build()
}

#[test]
fn an_actor_archetype_outside_a_level_reads_from_4() {
    let dir = std::env::temp_dir().join(format!("ow-defs-arch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let files: [(&str, &[u8]); 3] = [
        ("Core.upk", &core()),
        ("Engine.upk", &engine()),
        ("GD_Arch.upk", &gd_arch()),
    ];
    let mut defs = DefStore::from_synthetic(
        &dir,
        &files,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut w = ObjectSpace::new();
    let arch_id = defs
        .packages
        .load(&PackageName::parse("GD_Arch").unwrap())
        .unwrap();
    let find = |defs: &mut DefStore, w: &mut ObjectSpace, path: &str| {
        defs.find(w, &ObjectPath::parse(path).unwrap())
    };
    // An archetype inside a package object, and one with no outer: both load through the store at 4.
    // (`load_export` names the export by its path in the file.)
    for (path, index, in_file) in [
        ("GD_Arch.Character.Pawn_Arch", 2, "Character.Pawn_Arch"),
        ("GD_Arch.Pawn_Top", 4, "GD_Arch.Pawn_Top"),
    ] {
        let o = find(&mut defs, &mut w, path).unwrap().unwrap();
        assert!(defs.is_loaded(&w, o), "{path}");
        assert_eq!(
            defs.load_export(&mut w, arch_id, index, PropertyPrefix::Ordinary)
                .unwrap(),
            o
        );
        assert_eq!(
            defs.load_export(&mut w, arch_id, index, PropertyPrefix::PlacedActor),
            Err(DefError::PrefixMismatch {
                object: in_file.into(),
                derived: PropertyPrefix::Ordinary,
                given: PropertyPrefix::PlacedActor,
            })
        );
    }
    // An archetype whose stream does not parse from 4 is NotYet: no other prefix is tried.
    assert!(matches!(
        find(&mut defs, &mut w, "GD_Arch.Character.Pawn_Bad"),
        Err(DefError::NotYet(_))
    ));
    // A placed actor still reads from 26, without a check.
    let holder = find(&mut defs, &mut w, "GD_Arch.TheLevel.Holder")
        .unwrap()
        .unwrap();
    assert!(defs.is_loaded(&w, holder));
    assert_eq!(
        defs.load_export(&mut w, arch_id, 6, PropertyPrefix::PlacedActor)
            .unwrap(),
        holder
    );
    // An actor inside a level but not directly: 26 only under the check, never 4.
    let nested = find(&mut defs, &mut w, "GD_Arch.TheLevel.Holder.Nested")
        .unwrap()
        .unwrap();
    assert!(defs.is_loaded(&w, nested));
    assert!(matches!(
        find(&mut defs, &mut w, "GD_Arch.TheLevel.Holder.NestedAt4"),
        Err(DefError::NotYet(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `DefStore::instance` (lane FIX-DIRECTOR-STORE, decision 2026-11-16-48): a new object on every call, decoded from
/// the export, whose references are the store's objects for their paths; the store's own object is untouched.
#[test]
fn an_instance_is_a_new_object_whose_references_are_the_store_s() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut t = World::new("instance", IdentityPolicy::FirstInScanOrder, mode);
        let gd = t
            .defs
            .packages
            .load(&PackageName::parse("GD_Test").unwrap())
            .unwrap();
        let first = t
            .defs
            .instance(&mut t.w, gd, 1, PropertyPrefix::Ordinary)
            .unwrap();
        let second = t
            .defs
            .instance(&mut t.w, gd, 1, PropertyPrefix::Ordinary)
            .unwrap();
        let stored = t.get("GD_Test.Part_A");
        assert_ne!(first, second, "{mode:?}: a new object on every call");
        assert_ne!(first, stored, "{mode:?}: not the store's object");
        assert_eq!(
            t.w.heap.get(first).unwrap().origin,
            Origin::Export {
                package: gd,
                index: 1
            }
        );
        assert!(!t.defs.is_loaded(&t.w, first) && !t.defs.is_unloaded(&t.w, first));
        assert_eq!(t.read(first, "ClipSize"), Value::Int(7));
        // Its reference is the store's object for the path, which loads like any definition's reference.
        let next = t.object(first, "Next");
        assert_eq!(next, t.object(stored, "Next"), "{mode:?}");
        t.defs.ensure_loaded(&mut t.w, next).unwrap();
        assert_eq!(next, t.get("GD_Test.Part_B"));
        assert_eq!(t.read(next, "Damage"), Value::Float(2.5));
        // Writing to an instance changes neither the other instance nor the definition.
        let clip = t.key("ClipSize");
        t.w.write(first, PropAddr::scalar(clip), Value::Int(99))
            .unwrap();
        assert_eq!(t.read(second, "ClipSize"), Value::Int(7));
        assert_eq!(t.read(stored, "ClipSize"), Value::Int(7));
        // The prefix is the store's: an actor component at 8, and any other given prefix refused.
        assert!(
            t.defs
                .instance(&mut t.w, gd, 4, PropertyPrefix::Component)
                .is_ok()
        );
        assert_eq!(
            t.defs.instance(&mut t.w, gd, 4, PropertyPrefix::Ordinary),
            Err(DefError::PrefixMismatch {
                object: "GD_Test.Comp_A".into(),
                derived: PropertyPrefix::Component,
                given: PropertyPrefix::Ordinary,
            })
        );
        // Not an export: the C++ text.
        let e = t
            .defs
            .instance(&mut t.w, gd, 0, PropertyPrefix::Ordinary)
            .unwrap_err();
        assert_eq!(e.to_string(), "unsupported exported object request");
        // A class default object is not a definition.
        let core = t
            .defs
            .packages
            .load(&PackageName::parse("Core").unwrap())
            .unwrap();
        let cdo_index = t.defs.code.find_export(&t.defs.packages, core, "Default__Thing");
        assert!(cdo_index > 0);
        assert!(matches!(
            t.defs
                .instance(&mut t.w, core, cdo_index, PropertyPrefix::Ordinary),
            Err(DefError::NotADefinition(_))
        ));
    }
}

/// An actor archetype outside a level (the director's and the dummy's kind) instantiates at 4, as the C++ reads it.
#[test]
fn an_actor_archetype_instance_reads_from_4() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let dir = std::env::temp_dir().join(format!(
            "ow-defs-arch-instance-{mode:?}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files: [(&str, &[u8]); 3] = [
            ("Core.upk", &core()),
            ("Engine.upk", &engine()),
            ("GD_Arch.upk", &gd_arch()),
        ];
        let mut defs =
            DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut w = ObjectSpace::new();
        let id = defs
            .packages
            .load(&PackageName::parse("GD_Arch").unwrap())
            .unwrap();
        let a = defs
            .instance(&mut w, id, 2, PropertyPrefix::Ordinary)
            .unwrap();
        let b = defs
            .instance(&mut w, id, 2, PropertyPrefix::Ordinary)
            .unwrap();
        assert_ne!(a, b);
        assert!(matches!(
            defs.instance(&mut w, id, 2, PropertyPrefix::PlacedActor),
            Err(DefError::PrefixMismatch { .. })
        ));
        assert!(matches!(
            defs.instance(&mut w, id, 3, PropertyPrefix::Ordinary),
            Err(DefError::NotYet(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
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
    assert_eq!(
        things,
        [
            "Part_A",
            "Part_B",
            "Part_C",
            "Part_A.Sub",
            "GD_Ext.Inner",
            "GD_Test.Part_A"
        ]
    );
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
    let out = local_out();
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

#[test]
fn a_colon_path_and_its_dotted_form_have_one_holder() {
    let mut t = World::off("colon");
    // The `:` form first: its answer is cached under the dotted key, so it must be the dotted form's answer.
    let sub = t.get("GD_Test.Part_A:Sub");
    assert_eq!(t.read(sub, "ClipSize"), Value::Int(3));
    assert_eq!(t.get("GD_Test.Part_A.Sub"), sub);
    let holders = t
        .defs
        .owners(&ObjectPath::parse("GD_Test.Part_A:Sub").unwrap())
        .unwrap();
    assert_eq!(holders.holders.len(), 1);
    assert_eq!(holders.holders[0].export, 8);
    // Its outer is Part_A's object.
    let a = t.get("GD_Test.Part_A");
    assert_eq!(t.w.heap.get(sub).unwrap().outer, Some(a));
}

#[test]
fn a_code_outer_through_an_import_stays_none() {
    let mut t = World::off("codeouter");
    let inner = t.get("GD_Ext.Inner");
    assert_eq!(t.w.heap.get(inner).unwrap().outer, None);
    let log = t.defs.take_log();
    assert!(
        !log.iter().any(|l| l.starts_with("defs: outer of")),
        "{log:?}"
    );
}

#[test]
fn a_live_capture_skips_an_empty_value_for_a_property_the_export_does_not_tag() {
    // tools/real_game/live_overlay.py: `if live in (None, [], {}) and key not in props: continue`.
    let mut t = World::off("liveskip");
    let capture = r#"{"GD_Test.Part_B": {"class": "Thing", "properties": {
        "ClipSize": null, "List": [], "Pair": {}, "Next": null}}}"#;
    let o = Overlay::from_live_capture("cap", capture).unwrap();
    let id = t.defs.push_overlay(&mut t.w, o).unwrap();
    let b = t.get("GD_Test.Part_B");
    let status = t.defs.overlay_status(id).unwrap().clone();
    assert_eq!(
        (status.applied, status.pending, status.failed.len()),
        (4, 0, 0)
    );
    // Not tagged by Part_B: the inherited values stay, ClipSize 1 and the non-empty List [7, 8] of the class default:
    // the capture's `[]` does not overwrite it.
    assert_eq!(t.read(b, "ClipSize"), Value::Int(1));
    assert_eq!(
        t.read(b, "List"),
        Value::array(vec![Value::Int(7), Value::Int(8)])
    );
    let list = t.key("List");
    assert_eq!(
        t.defs.origin(
            b,
            PropAddr {
                prop: list,
                index: 0
            }
        ),
        ValueOrigin::Cooked
    );
    let clip = t.key("ClipSize");
    assert_eq!(
        t.defs.origin(
            b,
            PropAddr {
                prop: clip,
                index: 0
            }
        ),
        ValueOrigin::Cooked
    );
    // Tagged by Part_B: the empty value is a patch, as the script applies it.
    assert_eq!(t.read(b, "Next"), Value::Object(None));
    let next = t.key("Next");
    assert_eq!(
        t.defs.origin(
            b,
            PropAddr {
                prop: next,
                index: 0
            }
        ),
        ValueOrigin::Overlay(id)
    );
    // The same patches in the test format are not skipped: null does not fit an int.
    let mut t = World::off("liveskip-text");
    let id = t.overlay(OverlayKind::Test, "GD_Test.Part_B ClipSize = null\n");
    t.get("GD_Test.Part_B");
    assert_eq!(t.defs.overlay_status(id).unwrap().failed.len(), 1);
}

#[test]
fn a_broken_outer_chain_hides_only_the_exports_after_it() {
    // Cycle.upk: `Early`, then two exports that are each other's outer (no path). The C++ findExport finds `Early`
    // before it meets them, and fails for a path it has not found by then.
    let mut p = PackageBuilder::new_empty();
    let (thing, ..) = class_imports(&mut p);
    let none = p.none();
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    p.add_export(thing, "Early", payload.clone());
    let a = p.add_export(thing, "LoopA", payload.clone());
    let b = p.add_export(thing, "LoopB", payload);
    p.set_outer(a, b);
    p.set_outer(b, a);
    let cycle = p.build();
    let dir = std::env::temp_dir().join(format!("ow-defs-cycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let files: [(&str, &[u8]); 2] = [("Core.upk", &core()), ("Cycle.upk", &cycle)];
    let mut defs = DefStore::from_synthetic(
        &dir,
        &files,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut w = ObjectSpace::new();
    let early = ObjectPath::parse("Cycle.Early").unwrap();
    let holders = defs.owners(&early).unwrap();
    assert_eq!((holders.holders.len(), holders.failures.len()), (1, 0));
    assert!(defs.find(&mut w, &early).unwrap().is_some());
    let absent = ObjectPath::parse("Cycle.Absent").unwrap();
    let holders = defs.owners(&absent).unwrap();
    assert_eq!((holders.holders.len(), holders.failures.len()), (0, 1));
    assert_eq!(defs.find(&mut w, &absent).unwrap(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The output folder of the real-game checks, `OW_DEFS_LOCAL_OUT`, refused unless it lies inside this workspace's own
/// ignored `local/` folder (the workspace root is two levels above `CARGO_MANIFEST_DIR`): what they write is game data
/// (AGENTS.md rule 1). The path is resolved through its nearest existing ancestor, and a `..` in the rest is refused.
fn local_out() -> PathBuf {
    let out = PathBuf::from(std::env::var("OW_DEFS_LOCAL_OUT").expect("OW_DEFS_LOCAL_OUT"));
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let local = std::fs::canonicalize(workspace.join("local"))
        .expect("the workspace's local/ folder exists (it holds the real-game outputs)");
    let absolute = std::path::absolute(&out).expect("an absolute form of OW_DEFS_LOCAL_OUT");
    let mut existing = absolute.as_path();
    let mut rest = Vec::new();
    while !existing.exists() {
        rest.push(
            existing
                .file_name()
                .expect("a path below an existing folder"),
        );
        existing = existing.parent().expect("a path below an existing folder");
    }
    let mut resolved = std::fs::canonicalize(existing).unwrap();
    for name in rest.iter().rev() {
        assert!(
            *name != "..",
            "OW_DEFS_LOCAL_OUT holds '..': {}",
            out.display()
        );
        resolved.push(name);
    }
    assert!(
        resolved.starts_with(&local),
        "OW_DEFS_LOCAL_OUT must be inside the workspace's local/ folder ({}): {}",
        local.display(),
        out.display()
    );
    out
}

/// A local timing on the installed game, not run by default: every object path listed in the file
/// `OW_DEFS_PATHS` (one per line, under local/) found and loaded into ONE store over `OW_DEFS_REAL_ROOT`, the time of
/// the first lookup (the global index) and of the loads written to `loads.txt` in `OW_DEFS_LOCAL_OUT`.
#[test]
#[ignore = "needs the installed game (OW_DEFS_REAL_ROOT), a path list (OW_DEFS_PATHS) and OW_DEFS_LOCAL_OUT under local/"]
fn real_loads_in_one_store() {
    use std::fmt::Write as _;
    let out = local_out();
    let root = PathBuf::from(std::env::var("OW_DEFS_REAL_ROOT").expect("OW_DEFS_REAL_ROOT"));
    let paths =
        std::fs::read_to_string(std::env::var("OW_DEFS_PATHS").expect("OW_DEFS_PATHS")).unwrap();
    let paths: Vec<ObjectPath> = paths
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| ObjectPath::parse(l.trim()).unwrap())
        .collect();
    let mut defs = DefStore::open(root, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut w = ObjectSpace::new();
    let mut text = String::new();
    let start = std::time::Instant::now();
    let (mut loaded, mut failed) = (0, 0);
    let mut first = None;
    let mut slowest = std::time::Duration::ZERO;
    for path in &paths {
        let t = std::time::Instant::now();
        match defs.find(&mut w, path) {
            Ok(Some(_)) => loaded += 1,
            _ => failed += 1,
        }
        let took = t.elapsed();
        if first.is_none() {
            first = Some(took);
        } else if took > slowest {
            slowest = took;
        }
    }
    let total = start.elapsed();
    let rest = total - first.unwrap_or_default();
    let _ = write!(
        text,
        "paths {}\nloaded {loaded}\nfailed {failed}\nheap objects {}\nfirst lookup (global index and first load) {} ms\n\
         the other {} loads {} ms, {} us each on average, slowest {} ms\ntotal {} ms\n",
        paths.len(),
        w.heap.len(),
        first.unwrap_or_default().as_millis(),
        paths.len().saturating_sub(1),
        rest.as_millis(),
        rest.as_micros() / paths.len().saturating_sub(1).max(1) as u128,
        slowest.as_millis(),
        total.as_millis()
    );
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("loads.txt"), text).unwrap();
}

/// One loaded definition's slots as text (decision 2026-11-16-36's sample): references by their path in their
/// package (or their name), names and classes by text, floats through `ow_core::fmt`.
fn render(defs: &DefStore, w: &ObjectSpace, v: &Value, out: &mut String) {
    use std::fmt::Write as _;
    let object = |o: ObjRef, out: &mut String| match w.heap.get(o) {
        Ok(x) => match x.origin {
            Origin::Resource { package, index } | Origin::Export { package, index } => {
                match defs.packages.get(package).path_of(index) {
                    Ok(p) => out.push_str(&p),
                    Err(e) => {
                        let _ = write!(out, "<{e}>");
                    }
                }
            }
            _ => out.push_str(&w.names.text(x.name)),
        },
        Err(e) => {
            let _ = write!(out, "<{e}>");
        }
    };
    match v {
        Value::None => out.push_str("none"),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => out.push_str(&ow_core::fmt::fmt_shortest_cpp(f64::from(*f))),
        Value::Bool(b) => {
            let _ = write!(out, "{b}");
        }
        Value::Byte(b) => {
            let _ = write!(out, "b{b}");
        }
        Value::Str(s) => {
            let _ = write!(out, "{s:?}");
        }
        Value::Name(n) => {
            let _ = write!(out, "n:{}", w.names.text(*n));
        }
        Value::Object(o) | Value::Interface(o) => match o {
            Some(o) => object(*o, out),
            None => out.push_str("null"),
        },
        Value::Class(c) => match c {
            Some(c) => {
                let _ = write!(out, "c:{}", w.classes.layout(*c).path().as_str());
            }
            None => out.push_str("null"),
        },
        Value::Struct(s) => {
            out.push('{');
            for f in &s.fields {
                render(defs, w, f, out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for item in items.iter() {
                render(defs, w, item, out);
                out.push(',');
            }
            out.push(']');
        }
        Value::Delegate(d) => match d {
            Some(d) => {
                if let Some(o) = d.object {
                    object(o, out);
                }
                let _ = write!(out, ".{}", w.names.text(d.function));
            }
            None => out.push_str("unbound"),
        },
    }
}

/// A 64-bit FNV-1a digest of `text`.
fn digest(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn mode_tag(mode: ParityMode) -> &'static str {
    match mode {
        ParityMode::Off => "off",
        _ => "reference",
    }
}

/// A local check on the installed game, not run by default (decision 2026-11-16-36): for every export of each package
/// named in `OW_DEFS_FILES` (comma separated) under the cooked root `OW_DEFS_REAL_ROOT`, the derived prefix (or the
/// error), and in each parity mode `load_export` at that prefix in one store, its outcome and a digest of the loaded
/// slots, written to `prefixes.tsv` and `loads-<mode>.tsv` in `OW_DEFS_LOCAL_OUT` (under local/: the paths are game
/// data). Run once before and once after a prefix change and compare the files.
#[test]
#[ignore = "needs the installed game (OW_DEFS_REAL_ROOT), OW_DEFS_FILES and OW_DEFS_LOCAL_OUT under local/"]
fn real_prefix_sample() {
    use std::fmt::Write as _;
    let out = local_out();
    let root = PathBuf::from(std::env::var("OW_DEFS_REAL_ROOT").expect("OW_DEFS_REAL_ROOT"));
    let files = std::env::var("OW_DEFS_FILES").expect("OW_DEFS_FILES");
    let files: Vec<&str> = files.split(',').filter(|f| !f.is_empty()).collect();
    std::fs::create_dir_all(&out).unwrap();
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut defs =
            DefStore::open(root.clone(), IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut w = ObjectSpace::new();
        let (mut prefixes, mut loads) = (String::new(), String::new());
        for name in &files {
            let id = defs
                .packages
                .load(&PackageName::parse(name).unwrap())
                .unwrap();
            let pkg = defs.packages.shared(id);
            for index in 1..=i32::try_from(pkg.exports.len()).unwrap() {
                let path = pkg.path_of(index).unwrap_or_else(|e| format!("<{e}>"));
                let derived = defs.run(&mut w, |l| l.derive_prefix(id, index, &path));
                let derived_text = match &derived {
                    Ok(p) => p.bytes().to_string(),
                    Err(DefError::NotYet(_)) => "NotYet".to_string(),
                    Err(e) => format!("error {e}"),
                };
                let _ = writeln!(prefixes, "{name}\t{index}\t{path}\t{derived_text}");
                let outcome = match derived {
                    Ok(p) => match defs.load_export(&mut w, id, index, p) {
                        Ok(o) => {
                            let slots = &w.heap.get(o).unwrap().slots;
                            let mut text = String::new();
                            for v in slots {
                                render(&defs, &w, v, &mut text);
                                text.push('|');
                            }
                            format!("ok {} slots {:016x}", slots.len(), digest(&text))
                        }
                        Err(DefError::NotYet(_)) => "NotYet".to_string(),
                        Err(e) => format!("error {}", e.to_string().replace(['\t', '\n'], " ")),
                    },
                    Err(_) => "no prefix".to_string(),
                };
                let _ = writeln!(loads, "{name}\t{index}\t{path}\t{outcome}");
            }
        }
        if mode == ParityMode::Off {
            std::fs::write(out.join("prefixes.tsv"), prefixes).unwrap();
        }
        std::fs::write(out.join(format!("loads-{}.tsv", mode_tag(mode))), loads).unwrap();
    }
}

/// A local check on the installed game, not run by default (decision 2026-11-16-36): every path of the census of
/// `OW_DEFS_REAL_ROOT` whose first holder's class derives from `Engine.Pawn` (class defaults are not census entries),
/// that holder's derived prefix, and `find` in one store per parity mode; one line per path in `pawns-<mode>.tsv` and
/// the counts in `pawns-summary.txt`, in `OW_DEFS_LOCAL_OUT` (under local/).
#[test]
#[ignore = "needs the installed game (OW_DEFS_REAL_ROOT) and OW_DEFS_LOCAL_OUT under local/"]
fn real_pawn_archetypes() {
    use std::fmt::Write as _;
    let out = local_out();
    let root = PathBuf::from(std::env::var("OW_DEFS_REAL_ROOT").expect("OW_DEFS_REAL_ROOT"));
    std::fs::create_dir_all(&out).unwrap();
    let mut summary = String::new();
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut defs =
            DefStore::open(root.clone(), IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut w = ObjectSpace::new();
        let census = defs.census().unwrap();
        let pawn = {
            let (code, store) = defs.code_mut();
            code.find_class(store, "Engine.Pawn").unwrap()
        };
        let mut is_pawn: FixedHashMap<String, bool> = FixedHashMap::default();
        let mut lines = String::new();
        let mut counts: std::collections::BTreeMap<(String, String), u32> = Default::default();
        for entry in &census.entries {
            let Some(first) = entry.holders.first() else {
                continue;
            };
            if !is_pawn.contains_key(&first.class) {
                let (code, store) = defs.code_mut();
                let yes = code
                    .find_class(store, &first.class)
                    .is_ok_and(|k| code.is_child_of(k, pawn));
                is_pawn.insert(first.class.clone(), yes);
            }
            if !is_pawn[&first.class] {
                continue;
            }
            let id = defs.packages.load_path(&first.file).unwrap();
            let derived = defs.run(&mut w, |l| l.derive_prefix(id, first.export, &entry.path));
            let derived = match derived {
                Ok(p) => p.bytes().to_string(),
                Err(DefError::NotYet(_)) => "NotYet".to_string(),
                Err(e) => format!("error {e}"),
            };
            let found = match ObjectPath::parse(&entry.path) {
                Ok(path) => match defs.find(&mut w, &path) {
                    Ok(Some(o)) if defs.is_loaded(&w, o) => "loaded".to_string(),
                    Ok(Some(_)) => "not loaded".to_string(),
                    Ok(None) => "no holder".to_string(),
                    Err(DefError::NotYet(_)) => "NotYet".to_string(),
                    Err(e) => format!("error {}", e.to_string().replace(['\t', '\n'], " ")),
                },
                Err(e) => format!("bad path {e}"),
            };
            *counts.entry((derived.clone(), found.clone())).or_default() += 1;
            let _ = writeln!(lines, "{}\t{}\t{derived}\t{found}", entry.path, first.class);
        }
        let tag = mode_tag(mode);
        std::fs::write(out.join(format!("pawns-{tag}.tsv")), lines).unwrap();
        let _ = writeln!(
            summary,
            "mode {tag}: census files {}, failures {}",
            census.files.len(),
            census.failures.len()
        );
        for ((derived, found), n) in &counts {
            let _ = writeln!(summary, "  prefix {derived}\t{found}\t{n}");
        }
    }
    std::fs::write(out.join("pawns-summary.txt"), summary).unwrap();
}
