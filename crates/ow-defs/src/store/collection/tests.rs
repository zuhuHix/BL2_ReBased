//! Owner: lane FIX-DEFS-COLLECTION. Tests of the collection actors, the engine-native references under
//! `ParityMode::Off` and the archetypes, on invented packages (AGENTS.md rule 9): every name, value and file below is
//! made up for the test.

use std::path::PathBuf;

use ow_core::{ObjectPath, PackageName, ParityMode};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::testkit::reflect::Declarations;
use ow_package::testkit::{Export, PackageBuilder, ints};

use crate::identity::IdentityPolicy;
use crate::store::{
    CollectionKind, CollectionPose, DefError, DefStore, LIGHT_ENTRY_BYTES, MESH_ENTRY_BYTES,
};

/// `Core.Object` and `Core.Component`, with their default objects.
fn core() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let component = d.cls("Component", object);
    d.default_object(component, "Component");
    d.build()
}

/// A toy `Engine`: `Actor`, `ActorComponent`, `Level`; `StaticMeshComponent` (an actor component with `Count`,
/// `Scale` and `StaticMesh`, an object of the class `Engine.StaticMesh`, which this package imports and does not
/// export); `MeshActor`, whose default object holds the subobject `MeshComponent0` (tags from 8: `Count` 5, `Scale`
/// 2, its archetype `Default__StaticMeshComponent`); and the two collection actors with their component arrays.
/// `Default__StaticMeshComponent` sets `Count` 1. With `level_exported` false, `Level` is not exported (an
/// engine-native class, as `Engine.Level` is in the installed game).
fn engine(level_exported: bool) -> Vec<u8> {
    let mut d = Declarations::new(&["Int", "Float", "Object", "Array"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let component = d.p.add_import("Core", "Class", core, "Component");
    let engine = d.p.add_import("Core", "Package", 0, "Engine");
    let mesh_class = d.p.add_import("Core", "Class", engine, "StaticMesh");
    let actor = d.cls("Actor", object);
    d.default_object(actor, "Actor");
    let actor_component = d.cls("ActorComponent", component);
    d.default_object(actor_component, "ActorComponent");
    if level_exported {
        let level = d.cls("Level", object);
        d.default_object(level, "Level");
    } else {
        // `Engine.Level` imported from this very package, which does not export it: an engine-native class.
        d.p.add_import("Core", "Class", engine, "Level");
    }
    let smc = d.cls("StaticMeshComponent", actor_component);
    d.prop("Int", smc, "Count", 0);
    d.prop("Float", smc, "Scale", 0);
    d.prop("Object", smc, "StaticMesh", mesh_class);
    let none = d.none;
    let mut payload = ints(&[0]);
    payload.extend(d.p.int_tag("Count", 1));
    payload.extend_from_slice(&none);
    let smc_default = d.p.add_export(smc, "Default__StaticMeshComponent", payload);
    let mesh_actor = d.cls("MeshActor", actor);
    let mesh_actor_default = d.default_object(mesh_actor, "MeshActor");
    let mut payload = ints(&[0, 0]);
    payload.extend(d.p.int_tag("Count", 5));
    payload.extend(d.p.float_tag("Scale", 2.0));
    payload.extend_from_slice(&none);
    let mut sub = Export::new(smc, "MeshComponent0", payload);
    sub.outer = mesh_actor_default;
    sub.archetype = smc_default;
    d.p.add_export_with(sub);
    let mesh_collection = d.cls("StaticMeshCollectionActor", actor);
    d.array_of(mesh_collection, "StaticMeshComponents", "Object", smc);
    d.default_object(mesh_collection, "StaticMeshCollectionActor");
    let light_collection = d.cls("StaticLightCollectionActor", actor);
    d.array_of(
        light_collection,
        "LightComponents",
        "Object",
        actor_component,
    );
    d.default_object(light_collection, "StaticLightCollectionActor");
    d.build()
}

/// An 84-byte mesh entry: an affine matrix whose translation is `(x, 0, 0)`, Scale3D (1, 2, 3), scale `scale` and
/// the int `flag`; `w` is entry 15 (1 for an affine matrix).
fn mesh_entry(x: f32, scale: f32, flag: i32, w: f32) -> Vec<u8> {
    let mut m = [0f32; 20];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[12] = x;
    m[15] = w;
    m[16..19].copy_from_slice(&[1.0, 2.0, 3.0]);
    m[19] = scale;
    let mut b: Vec<u8> = m.iter().flat_map(|v| v.to_le_bytes()).collect();
    b.extend(flag.to_le_bytes());
    assert_eq!(b.len(), MESH_ENTRY_BYTES);
    b
}

/// `Map_Test.upk`, export numbers asserted below:
/// 1 `TheLevel`; 2 `SMCA` (a mesh collection: components [3, null, 4], three entries); 3 `Comp_1` (archetype 11; tags
/// `Scale` 3 and `StaticMesh` -> the exported mesh 6); 4 `Comp_2` (archetype the imported template
/// `Engine.Default__MeshActor.MeshComponent0`; `StaticMesh` -> `Env_Pkg.Meshes.Rock`, an import no file holds); 5
/// `SLCA` (a light
/// collection: [7], one 64-byte entry); 6 `LocalMesh` (an `Engine.StaticMesh` export); 7 `Light_1`; 8 `Short` (a mesh
/// collection of one component with an 80-byte tail); 9 `Skewed` (one entry whose entry 15 is 2); 10 `Comp_Chain`
/// (archetype 11); 11 `Arch_Comp` (a component archetype outside the level: `Scale` 4, its archetype 16); 12 and 13
/// `Cycle_A` / `Cycle_B` (each the other's archetype); 14 `Odd` (its archetype the level, an unrelated class); 15
/// `Comp_3` (`StaticMesh` -> the same import as `Comp_2`); 16 `Arch_Base` (an archetype: `Count` 6); 17 `Comp_4` (the
/// same template archetype as `Comp_2`); 18 `No_Holder` (its archetype an import no file holds); 19 .. 36 `Deep_0` ..
/// `Deep_17` (a chain of 17 archetypes from `Deep_0`); 37 `Comp_Code` (`StaticMesh` -> an import of class
/// `Core.Function`, which the toy `Core` does not export).
fn map() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let core = p.add_import("Core", "Package", 0, "Core");
    p.add_import("Core", "Class", core, "Package");
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", engine, name);
    let level = class(&mut p, "Level");
    let smc = class(&mut p, "StaticMeshComponent");
    let mesh_collection = class(&mut p, "StaticMeshCollectionActor");
    let light_collection = class(&mut p, "StaticLightCollectionActor");
    let actor_component = class(&mut p, "ActorComponent");
    let mesh_class = class(&mut p, "StaticMesh");
    let mesh_actor_default = p.add_import("Engine", "MeshActor", engine, "Default__MeshActor");
    let subobject = p.add_import(
        "Engine",
        "StaticMeshComponent",
        mesh_actor_default,
        "MeshComponent0",
    );
    let env = p.add_import("Core", "Package", 0, "Env_Pkg");
    let meshes = p.add_import("Core", "Package", env, "Meshes");
    let rock = p.add_import("Engine", "StaticMesh", meshes, "Rock");
    let no_arch = p.add_import("Engine", "StaticMeshComponent", meshes, "NoArch");
    let func = p.add_import("Core", "Function", meshes, "SomeFunc");
    let none = p.none();
    let at = |prefix: usize, tags: &[Vec<u8>]| {
        let mut b = vec![0u8; prefix];
        for t in tags {
            b.extend_from_slice(t);
        }
        b.extend_from_slice(&none);
        b
    };
    let add = |p: &mut PackageBuilder, class, name: &str, outer, archetype, payload| {
        let mut e = Export::new(class, name, payload);
        e.outer = outer;
        e.archetype = archetype;
        p.add_export_with(e)
    };

    assert_eq!(add(&mut p, level, "TheLevel", 0, 0, at(4, &[])), 1);
    let mut payload = at(
        4,
        &[p.array_tag("StaticMeshComponents", 3, &ints(&[3, 0, 4]))],
    );
    payload.extend(mesh_entry(10.0, 1.5, 7, 1.0));
    payload.extend(mesh_entry(20.0, 1.0, 0, 1.0));
    payload.extend(mesh_entry(30.0, 0.5, -1, 1.0));
    assert_eq!(add(&mut p, mesh_collection, "SMCA", 1, 0, payload), 2);
    let tags = [p.float_tag("Scale", 3.0), p.object_tag("StaticMesh", 6)];
    assert_eq!(add(&mut p, smc, "Comp_1", 2, 11, at(8, &tags)), 3);
    let tags = [p.object_tag("StaticMesh", rock)];
    assert_eq!(add(&mut p, smc, "Comp_2", 2, subobject, at(8, &tags)), 4);
    let mut payload = at(4, &[p.array_tag("LightComponents", 1, &ints(&[7]))]);
    payload.extend((0..LIGHT_ENTRY_BYTES).map(|i| i as u8));
    assert_eq!(add(&mut p, light_collection, "SLCA", 1, 0, payload), 5);
    assert_eq!(add(&mut p, mesh_class, "LocalMesh", 0, 0, at(4, &[])), 6);
    assert_eq!(add(&mut p, actor_component, "Light_1", 5, 0, at(8, &[])), 7);
    let mut payload = at(4, &[p.array_tag("StaticMeshComponents", 1, &ints(&[3]))]);
    payload.extend(&mesh_entry(1.0, 1.0, 0, 1.0)[..80]);
    assert_eq!(add(&mut p, mesh_collection, "Short", 1, 0, payload), 8);
    let mut payload = at(4, &[p.array_tag("StaticMeshComponents", 1, &ints(&[4]))]);
    payload.extend(mesh_entry(1.0, 1.0, 0, 2.0));
    assert_eq!(add(&mut p, mesh_collection, "Skewed", 1, 0, payload), 9);
    assert_eq!(add(&mut p, smc, "Comp_Chain", 1, 11, at(8, &[])), 10);
    let tags = [p.float_tag("Scale", 4.0)];
    assert_eq!(add(&mut p, smc, "Arch_Comp", 0, 16, at(8, &tags)), 11);
    assert_eq!(add(&mut p, smc, "Cycle_A", 1, 13, at(8, &[])), 12);
    assert_eq!(add(&mut p, smc, "Cycle_B", 1, 12, at(8, &[])), 13);
    assert_eq!(add(&mut p, smc, "Odd", 1, 1, at(8, &[])), 14);
    let tags = [p.object_tag("StaticMesh", rock)];
    assert_eq!(add(&mut p, smc, "Comp_3", 1, 0, at(8, &tags)), 15);
    let tags = [p.int_tag("Count", 6)];
    assert_eq!(add(&mut p, smc, "Arch_Base", 0, 0, at(8, &tags)), 16);
    assert_eq!(add(&mut p, smc, "Comp_4", 1, subobject, at(8, &[])), 17);
    assert_eq!(add(&mut p, smc, "No_Holder", 1, no_arch, at(8, &[])), 18);
    // Deep_0 (in the level) .. Deep_17: each the next one's instance, a chain of 17 archetypes from Deep_0.
    for k in 0..18 {
        let (outer, archetype) = (if k == 0 { 1 } else { 0 }, if k < 17 { 20 + k } else { 0 });
        assert_eq!(
            add(
                &mut p,
                smc,
                &format!("Deep_{k}"),
                outer,
                archetype,
                at(8, &[])
            ),
            19 + k
        );
    }
    let tags = [p.object_tag("StaticMesh", func)];
    assert_eq!(add(&mut p, smc, "Comp_Code", 1, 0, at(8, &tags)), 37);
    p.build()
}

struct World {
    dir: PathBuf,
    defs: DefStore,
    w: ObjectSpace,
    map: PackageId,
}

impl World {
    fn new(tag: &str, mode: ParityMode) -> World {
        World::with_engine(tag, mode, true)
    }

    fn with_engine(tag: &str, mode: ParityMode, level_exported: bool) -> World {
        let dir = std::env::temp_dir().join(format!(
            "ow-defs-collection-{tag}-{mode:?}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files: [(&str, &[u8]); 3] = [
            ("Core.upk", &core()),
            ("Engine.upk", &engine(level_exported)),
            ("Map_Test.upk", &map()),
        ];
        let mut defs =
            DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let map = defs
            .packages
            .load(&PackageName::parse("Map_Test").unwrap())
            .unwrap();
        World {
            dir,
            defs,
            w: ObjectSpace::new(),
            map,
        }
    }

    fn find(&mut self, name: &str) -> Result<ObjRef, DefError> {
        let path = ObjectPath::parse(&format!("Map_Test.TheLevel.{name}")).unwrap();
        Ok(self.defs.find(&mut self.w, &path)?.unwrap())
    }

    fn read(&mut self, o: ObjRef, prop: &str) -> Value {
        let p = self
            .defs
            .key(&mut self.w, "Engine.StaticMeshComponent", prop)
            .unwrap();
        self.w.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn mesh(&mut self, o: ObjRef) -> Option<ObjRef> {
        match self.read(o, "StaticMesh") {
            Value::Object(m) => m,
            other => panic!("StaticMesh: {other:?}"),
        }
    }

    fn class_path(&self, o: ObjRef) -> String {
        let class = self.w.heap.get(o).unwrap().class;
        self.w.classes.layout(class).path().to_string()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const MODES: [ParityMode; 2] = [ParityMode::Off, ParityMode::Reference];

/// Fix 1: a collection actor in a level reads from 4 (not the placed actor's 26), and its components come back with
/// their tail entries, in both modes.
#[test]
fn a_collection_actor_reads_from_4_and_lists_its_components() {
    for mode in MODES {
        let mut t = World::new("list", mode);
        let smca = t.find("SMCA").unwrap();
        assert!(t.defs.is_loaded(&t.w, smca));
        assert_eq!(
            t.defs
                .load_export(&mut t.w, t.map, 2, PropertyPrefix::Ordinary)
                .unwrap(),
            smca
        );
        assert!(matches!(
            t.defs
                .load_export(&mut t.w, t.map, 2, PropertyPrefix::PlacedActor),
            Err(DefError::PrefixMismatch { .. })
        ));
        let comp_1 = t.find("SMCA.Comp_1").unwrap();
        let comp_2 = t.find("SMCA.Comp_2").unwrap();
        let c = t.defs.collection(&mut t.w, smca).unwrap();
        assert_eq!(c.kind, CollectionKind::StaticMesh);
        let components: Vec<_> = c.entries.iter().map(|e| e.component).collect();
        assert_eq!(components, [Some(comp_1), None, Some(comp_2)]);
        let poses: Vec<_> = c
            .entries
            .iter()
            .map(|e| match &e.pose {
                CollectionPose::Mesh {
                    matrix,
                    scale3d,
                    scale,
                    unassigned,
                } => (matrix[12], matrix[15], *scale3d, *scale, *unassigned),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            poses,
            [
                (10.0, 1.0, [1.0, 2.0, 3.0], 1.5, 7),
                (20.0, 1.0, [1.0, 2.0, 3.0], 1.0, 0),
                (30.0, 1.0, [1.0, 2.0, 3.0], 0.5, -1),
            ]
        );

        let slca = t.find("SLCA").unwrap();
        let light = t.find("SLCA.Light_1").unwrap();
        let c = t.defs.collection(&mut t.w, slca).unwrap();
        assert_eq!(c.kind, CollectionKind::StaticLight);
        assert_eq!(c.entries.len(), 1);
        assert_eq!(c.entries[0].component, Some(light));
        let raw: Vec<u8> = (0..LIGHT_ENTRY_BYTES).map(|i| i as u8).collect();
        assert_eq!(
            c.entries[0].pose,
            CollectionPose::Light {
                raw: raw.try_into().unwrap()
            }
        );
    }
}

/// Fix 1: a tail that is not exactly one entry per component, or a matrix that is not affine, is an error; an object
/// that is not a collection actor is not read as one.
#[test]
fn a_collection_outside_the_recorded_layout_is_an_error() {
    for mode in MODES {
        let mut t = World::new("bad", mode);
        let short = t.find("Short").unwrap();
        let e = t.defs.collection(&mut t.w, short).unwrap_err();
        assert_eq!(
            e.to_string(),
            "collection actor Short: a tail of 80 bytes for 1 components of 84 bytes"
        );
        let skewed = t.find("Skewed").unwrap();
        assert!(matches!(
            t.defs.collection(&mut t.w, skewed),
            Err(DefError::Collection { reason, .. }) if reason == "a non-affine entry matrix"
        ));
        let comp = t.find("SMCA.Comp_2").unwrap();
        assert!(matches!(
            t.defs.collection(&mut t.w, comp),
            Err(DefError::NotADefinition(_))
        ));
    }
}

/// Fix 2: a `StaticMesh` reference (a class no package exports) is the store's handle for the mesh's path in both
/// modes: an export of the map, or an import no file holds, one handle per path. It stays a stand-in of a class marked
/// native, and never loads.
#[test]
fn a_static_mesh_reference_is_a_handle_for_its_path_in_both_modes() {
    for mode in MODES {
        let mut t = World::new("mesh", mode);
        let comp_1 = t.find("SMCA.Comp_1").unwrap();
        let local = t.mesh(comp_1).expect("the exported mesh");
        assert_eq!(t.class_path(local), "Engine.StaticMesh");
        assert!(t.w.classes.is_native(t.w.heap.get(local).unwrap().class));
        assert!(matches!(
            t.w.heap.get(local).unwrap().origin,
            Origin::Resource { index: 6, .. }
        ));
        assert!(!t.defs.is_loaded(&t.w, local));
        assert!(matches!(
            t.defs.load(&mut t.w, local),
            Err(DefError::NotADefinition(_))
        ));
        t.defs.ensure_loaded(&mut t.w, local).unwrap();

        let comp_2 = t.find("SMCA.Comp_2").unwrap();
        let rock = t.mesh(comp_2).expect("the imported mesh");
        assert_eq!(t.class_path(rock), "Engine.StaticMesh");
        assert_eq!(t.w.names.text(t.w.heap.get(rock).unwrap().name), "Rock");
        assert!(
            t.defs
                .take_log()
                .contains(&"defs: no holder for Env_Pkg.Meshes.Rock".to_string())
        );
        let comp_3 = t.find("Comp_3").unwrap();
        // One handle for the path (Off keys it by the path, Reference by the import both components name).
        assert_eq!(t.mesh(comp_3), Some(rock));
    }
}

/// Fix 3: under `ParityMode::Off` a placed component takes its archetype's values (an archetype over its own
/// archetype), then its own tags; under `ParityMode::Reference` its class defaults, as the C++. `DefStore::instance`
/// follows the same rule. An archetype inside a class default object (a template) is not applied, and is logged once.
#[test]
fn a_placed_object_takes_its_archetype_s_values_in_off_only() {
    for mode in MODES {
        let mut t = World::new("arch", mode);
        let off = mode == ParityMode::Off;
        let comp_1 = t.find("SMCA.Comp_1").unwrap();
        assert_eq!(t.read(comp_1, "Count"), Value::Int(if off { 6 } else { 1 }));
        assert_eq!(t.read(comp_1, "Scale"), Value::Float(3.0));
        let chain = t.find("Comp_Chain").unwrap();
        assert_eq!(t.read(chain, "Count"), Value::Int(if off { 6 } else { 1 }));
        assert_eq!(
            t.read(chain, "Scale"),
            Value::Float(if off { 4.0 } else { 0.0 })
        );
        let copy = t
            .defs
            .instance(&mut t.w, t.map, 3, PropertyPrefix::Component)
            .unwrap();
        assert_ne!(copy, comp_1);
        assert_eq!(t.read(copy, "Count"), Value::Int(if off { 6 } else { 1 }));
        assert_eq!(t.read(copy, "Scale"), Value::Float(3.0));
        t.defs.take_log();
        // The template's Count 5 and Scale 2 are not read: the class defaults stay.
        let comp_2 = t.find("SMCA.Comp_2").unwrap();
        assert_eq!(t.read(comp_2, "Count"), Value::Int(1));
        assert_eq!(t.read(comp_2, "Scale"), Value::Float(0.0));
        // A second component with the same template: not applied, and not logged again.
        let comp_4 = t.find("Comp_4").unwrap();
        assert_eq!(t.read(comp_4, "Count"), Value::Int(1));
        let line = "defs: archetype Engine.Default__MeshActor.MeshComponent0 not applied: \
                    a class default object's subobject, its prefix not recorded";
        let log = t.defs.take_log();
        assert_eq!(
            log.iter().filter(|l| *l == line).count(),
            usize::from(off),
            "{log:?}"
        );
    }
}

/// Fix 3: an archetype chain that is a cycle, or an archetype of an unrelated class, is a `NotYet` under
/// `ParityMode::Off` (never class defaults in its place); `ParityMode::Reference` does not follow archetypes.
#[test]
fn an_archetype_that_cannot_be_followed_is_not_yet_in_off() {
    for mode in MODES {
        let mut t = World::new("arch-bad", mode);
        for (name, why) in [
            ("Cycle_A", "its archetype chain is a cycle"),
            (
                "Odd",
                "is not Engine.StaticMeshComponent or a class it derives from",
            ),
            ("No_Holder", "no package holds it"),
            ("Deep_0", "more than 16 archetypes deep"),
        ] {
            match (mode, t.find(name)) {
                (ParityMode::Off, Err(DefError::NotYet(n))) => {
                    assert!(n.to_string().contains(why), "{name}: {n}");
                }
                (ParityMode::Reference, Ok(o)) => assert!(t.defs.is_loaded(&t.w, o)),
                (mode, other) => panic!("{name} in {mode:?}: {other:?}"),
            }
        }
    }
}

/// Review r1, M1: a reference to a code object whose class no installed package exports (`Core.Function`) is left to
/// the decoder in `ParityMode::Off`, as before: the property is skipped, and no class is defined at that path.
#[test]
fn a_reference_to_code_is_not_made_a_native_stand_in() {
    let mut t = World::new("code", ParityMode::Off);
    let comp = t.find("Comp_Code").unwrap();
    assert_eq!(t.mesh(comp), None);
    assert!(
        t.w.classes
            .by_path("Core.Function")
            .is_none_or(|c| !t.w.classes.layout(c).is_defined())
    );
}

/// Fix 2: where `Engine.Level` is not exported (as in the installed game), a placed object's outer is the level's
/// handle in both modes: a native-class stand-in that never loads. Under `ParityMode::Off` it was `None` before.
#[test]
fn a_level_of_an_engine_native_class_is_the_outer_in_both_modes() {
    for mode in MODES {
        let mut t = World::with_engine("level", mode, false);
        let smca = t.find("SMCA").unwrap();
        let level =
            t.w.heap
                .get(smca)
                .unwrap()
                .outer
                .expect("the level as outer");
        assert_eq!(t.class_path(level), "Engine.Level");
        assert!(t.w.classes.is_native(t.w.heap.get(level).unwrap().class));
        assert!(!t.defs.is_loaded(&t.w, level));
        // The component inside the collection has the collection as its outer, and still reads from 8.
        let comp_1 = t.find("SMCA.Comp_1").unwrap();
        assert_eq!(t.w.heap.get(comp_1).unwrap().outer, Some(smca));
        assert!(t.defs.collection(&mut t.w, smca).is_ok());
    }
}
