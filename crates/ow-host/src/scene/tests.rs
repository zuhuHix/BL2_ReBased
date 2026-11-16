//! Owner: lane H2-SCENE. Tests of the scene loader on invented packages (AGENTS.md rule 9): every name, value and
//! file below is made up for the test. The last test is the installed-game driver: `#[ignore]`d, it reads
//! `OW_BL2_ROOT` and writes its report under `local/p2/h2-scene/` (ignored), never into a tracked file.

use super::*;
use ow_core::ParityMode;
use ow_core::fmt::fmt_py_repr;
use ow_defs::identity::IdentityPolicy;
use std::fmt::Write as _;
use std::path::Path;

// ------------------------------------------------------------------------------------------ synthetic packages

use ow_package::testkit::reflect::Declarations;
use ow_package::testkit::{Export, PackageBuilder, ints, words};

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// `Core.upk`: `Object` with the native structs `Vector` and `Rotator`, `Component`, and `Package`.
fn core() -> Vec<u8> {
    let mut d = Declarations::new(&["Float", "Int"]);
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let vector = d.struct_in(object, "Vector", 0, None);
    for f in ["Z", "Y", "X"] {
        d.prop("Float", vector, f, 0);
    }
    let rotator = d.struct_in(object, "Rotator", 0, None);
    for f in ["Roll", "Yaw", "Pitch"] {
        d.prop("Int", rotator, f, 0);
    }
    let component = d.cls("Component", object);
    d.default_object(component, "Component");
    let package = d.cls("Package", object);
    d.default_object(package, "Package");
    d.build()
}

/// `Engine.upk`: the classes the scene reads, each with the properties it reads, and their defaults (`DrawScale` 1
/// and `DrawScale3D` (1, 1, 1) on `Actor`; `Scale` 1 and `Scale3D` (1, 1, 1) on `PrimitiveComponent`). Like the
/// installed `Engine.upk`, it exports no `StaticMesh` class.
fn engine() -> Vec<u8> {
    let mut d = Declarations::new(&["Bool", "Float", "Struct", "Object", "Array", "Name"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let component = d.p.add_import("Core", "Class", core, "Component");
    let vector = d.p.add_import("Core", "ScriptStruct", object, "Vector");
    let rotator = d.p.add_import("Core", "ScriptStruct", object, "Rotator");
    let one = f32s(&[1.0, 1.0, 1.0]);

    let actor = d.cls("Actor", object);
    d.prop("Struct", actor, "Location", vector);
    d.prop("Struct", actor, "Rotation", rotator);
    d.prop("Float", actor, "DrawScale", 0);
    d.prop("Struct", actor, "DrawScale3D", vector);
    d.prop("Bool", actor, "bHidden", 0);
    let mut cdo = ints(&[0]);
    cdo.extend(d.p.float_tag("DrawScale", 1.0));
    cdo.extend(d.p.struct_tag("DrawScale3D", "Vector", &one));
    cdo.extend_from_slice(&d.none);
    d.p.add_export(actor, "Default__Actor", cdo);

    let actor_component = d.cls("ActorComponent", component);
    d.default_object(actor_component, "ActorComponent");
    let primitive = d.cls("PrimitiveComponent", actor_component);
    for flag in DUPLICATE_FLAGS {
        d.prop("Bool", primitive, flag, 0);
    }
    d.prop("Struct", primitive, "Translation", vector);
    d.prop("Struct", primitive, "Rotation", rotator);
    d.prop("Float", primitive, "Scale", 0);
    d.prop("Struct", primitive, "Scale3D", vector);
    let mut cdo = ints(&[0]);
    cdo.extend(d.p.float_tag("Scale", 1.0));
    cdo.extend(d.p.struct_tag("Scale3D", "Vector", &one));
    cdo.extend_from_slice(&d.none);
    d.p.add_export(primitive, "Default__PrimitiveComponent", cdo);

    let mesh_component = d.cls("StaticMeshComponent", primitive);
    // Typed as a Core.Object reference here; the scene reads the reference from the tags anyway.
    d.prop("Object", mesh_component, "StaticMesh", object);
    d.array_of(mesh_component, "Materials", "Object", object);
    d.default_object(mesh_component, "StaticMeshComponent");

    let collection = d.cls("StaticMeshCollectionActor", actor);
    d.array_of(collection, "StaticMeshComponents", "Object", mesh_component);
    d.default_object(collection, "StaticMeshCollectionActor");
    let light = d.cls("LightComponent", actor_component);
    d.default_object(light, "LightComponent");
    let point = d.cls("PointLightComponent", light);
    d.default_object(point, "PointLightComponent");
    let streaming = d.cls("LevelStreaming", object);
    d.prop("Name", streaming, "PackageName", 0);
    d.default_object(streaming, "LevelStreaming");
    let kismet = d.cls("LevelStreamingKismet", streaming);
    d.default_object(kismet, "LevelStreamingKismet");
    for (name, super_ref) in [
        ("StaticMeshActor", actor),
        ("InterpActor", actor),
        ("Emitter", actor),
        ("PlayerStart", actor),
        ("Terrain", actor),
        ("StaticLightCollectionActor", actor),
        ("TerrainComponent", primitive),
        ("ModelComponent", primitive),
        ("Level", object),
        ("World", object),
    ] {
        let class = d.cls(name, super_ref);
        d.default_object(class, name);
    }
    let texture = d.cls("Texture", object);
    d.default_object(texture, "Texture");
    let texture2d = d.cls("Texture2D", texture);
    d.default_object(texture2d, "Texture2D");
    let cube = d.cls("TextureCube", texture);
    d.default_object(cube, "TextureCube");

    let interface = d.cls("MaterialInterface", object);
    d.default_object(interface, "MaterialInterface");
    let material = d.cls("Material", interface);
    d.array_of(material, "Expressions", "Object", object);
    d.default_object(material, "Material");
    let instance = d.cls("MaterialInstance", interface);
    d.prop("Object", instance, "Parent", interface);
    let parameter = d.struct_in(instance, "TextureParameterValue", 0, None);
    d.prop("Object", parameter, "ParameterValue", texture);
    d.prop("Name", parameter, "ParameterName", 0);
    d.array_of(instance, "TextureParameterValues", "Struct", parameter);
    d.default_object(instance, "MaterialInstance");
    let constant = d.cls("MaterialInstanceConstant", instance);
    d.default_object(constant, "MaterialInstanceConstant");
    let expression = d.cls("MaterialExpression", object);
    d.default_object(expression, "MaterialExpression");
    let sample = d.cls("MaterialExpressionTextureSample", expression);
    d.prop("Object", sample, "Texture", texture);
    d.default_object(sample, "MaterialExpressionTextureSample");
    d.build()
}

/// `WillowGame.upk`: the two game owner classes the tests use.
fn willow_game() -> Vec<u8> {
    let mut d = Declarations::new(&[]);
    let engine = d.p.add_import("Core", "Package", 0, "Engine");
    let actor = d.p.add_import("Core", "Class", engine, "Actor");
    for name in ["WillowInteractiveObject", "WillowMissionPickupSpawner"] {
        let class = d.cls(name, actor);
        d.default_object(class, name);
    }
    d.build()
}

/// A map package's class imports, by class name.
struct Classes {
    refs: Vec<(&'static str, i32)>,
}

impl Classes {
    fn new(p: &mut PackageBuilder) -> Self {
        let core = p.add_import("Core", "Package", 0, "Core");
        let engine = p.add_import("Core", "Package", 0, "Engine");
        let willow = p.add_import("Core", "Package", 0, "WillowGame");
        let mut refs = vec![("Package", p.add_import("Core", "Class", core, "Package"))];
        for name in [
            "World",
            "Level",
            "StaticMeshActor",
            "StaticMeshComponent",
            "StaticMeshCollectionActor",
            "Emitter",
            "PlayerStart",
            "PointLightComponent",
            "Terrain",
            "TerrainComponent",
            "Model",
            "LevelStreamingKismet",
            "StaticMesh",
            "Material",
            "MaterialInstanceConstant",
            "MaterialExpressionTextureSample",
            "Texture2D",
            "TextureCube",
        ] {
            refs.push((name, p.add_import("Core", "Class", engine, name)));
        }
        for name in ["WillowInteractiveObject", "WillowMissionPickupSpawner"] {
            refs.push((name, p.add_import("Core", "Class", willow, name)));
        }
        Classes { refs }
    }

    fn get(&self, name: &str) -> i32 {
        self.refs
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, r)| r)
            .unwrap_or_else(|| panic!("no class import {name}"))
    }
}

/// `prefix` zero bytes, the tags, then `None`.
fn payload(p: &mut PackageBuilder, prefix: usize, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; prefix];
    for t in tags {
        out.extend_from_slice(t);
    }
    out.extend_from_slice(&p.none());
    out
}

fn add(p: &mut PackageBuilder, class: i32, name: &str, outer: i32, payload: Vec<u8>) -> i32 {
    let mut export = Export::new(class, name, payload);
    export.outer = outer;
    p.add_export_with(export)
}

/// A static mesh with one LOD: 3 vertices, one section of one triangle using material `material` (the shape of
/// `ow-assets`' own mesh fixture, invented values).
fn mesh_payload(p: &mut PackageBuilder, material: i32) -> Vec<u8> {
    let mut native = vec![0u8; 28 + 4 + 24];
    native.extend(words(&[6, 0, 8, 0, 18, 0, 0, 0, 1]));
    native.extend(words(&[0, 0, 0, 0, 1]));
    native.extend(material.to_le_bytes());
    native.extend(words(&[0, 0, 1, 0, 1, 0, 2, 0, 0]));
    native.push(0);
    native.extend(words(&[12, 3, 12, 3]));
    native.extend(f32s(&[0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0]));
    native.extend(words(&[1, 16, 3, 1, 16, 3]));
    for (u, v) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0)] {
        native.extend([255, 128, 128, 255, 128, 128, 255, 255]);
        native.extend(f32s(&[u, v]));
    }
    native.extend(words(&[0, 0]));
    native.extend(words(&[3, 2, 3]));
    for i in [0u16, 1, 2] {
        native.extend(i.to_le_bytes());
    }
    let mut out = ints(&[0]);
    out.extend(p.none());
    out.extend(native);
    out
}

/// A PF_G8 texture of 4 x 4 pixels with one inline mip (the record shape of `ow-assets`' texture fixture).
fn texture_payload(p: &mut PackageBuilder, shade: u8) -> Vec<u8> {
    let mut out = ints(&[0]);
    let detail = p.fname("EPixelFormat", 0);
    let value = p.fname("PF_G8", 0);
    out.extend(p.tag_detail("Format", "ByteProperty", &detail, &value));
    out.extend(p.none());
    out.extend(vec![0u8; 16]);
    out.extend(words(&[1, 0, 16, 16, 0]));
    out.extend([shade; 16]);
    out.extend(words(&[4, 4]));
    out
}

fn vector_tag(p: &mut PackageBuilder, name: &str, v: [f32; 3]) -> Vec<u8> {
    p.struct_tag(name, "Vector", &f32s(&v))
}

fn rotator_tag(p: &mut PackageBuilder, name: &str, r: [i32; 3]) -> Vec<u8> {
    p.struct_tag(name, "Rotator", &ints(&r))
}

/// One collection tail row: an affine matrix with translation `at`, Scale3D `scale3d`, scale 1, flags 0.
fn tail_row(at: [f32; 3], scale3d: [f32; 3]) -> Vec<u8> {
    let mut row = f32s(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, at[0], at[1], at[2], 1.0,
    ]);
    row.extend(f32s(&scale3d));
    row.extend(f32s(&[1.0]));
    row.extend(ints(&[0]));
    row
}

/// `Test_P.upk`, export by export (the numbers the tests use):
/// 1 `TheWorld`, 2 `PersistentLevel`; 3 `StaticMeshActor_0` at (10, 20, 30), yaw 16384, `DrawScale` 2, holding
/// 4 `StaticMeshComponent_0` (mesh `Meshes.Box`, translation (1, 2, 3), material override `Mati_Red`),
/// 5 `StaticMeshComponent_1` (`HiddenGame`), 6 `StaticMeshComponent_2` (mesh `Missing.Meshes.Gone`, which no file
/// holds) and 7 `StaticMeshComponent_3` (no mesh); 8 `StaticMeshActor_1` (`bHidden`) holding 9
/// `StaticMeshComponent_4`; 10 `StaticMeshCollectionActor_0` listing 11 (own `Scale3D` 0.5) and 12 (the tail's 2);
/// 13 `Emitter_0` holding 14; 15 `WillowInteractiveObject_0` holding two identical components 16 and 17; 18
/// `WillowMissionPickupSpawner_0` whose `PreviewMesh` is 19; 20 `PlayerStart_0`; 21 `PointLightComponent_0`; 22
/// `Terrain_0` with 23 `TerrainComponent_0`; 24 `Model_0`; 25 `LevelStreamingKismet_0` naming `Test_Sub`; then the
/// assets: 26 `Meshes`, 27 `Meshes.Box`, 28 `Materials`, 29 `Mati_Red` (parent `Mat_Base`, textures `Tex_Red` and
/// `Cube_Sky`), 30 `Mat_Base` with expression 31 (texture `Tex_Base`), 32 `Tex_Red`, 33 `Tex_Base`, 34 `Cube_Sky`.
/// `variant.tail` spoils collection 10's tail; `variant.extras` appends the exports of [`extras`].
fn test_p(variant: Variant) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let c = Classes::new(&mut p);
    let gone_package = p.add_import("Core", "Package", 0, "Missing");
    let gone_group = p.add_import("Core", "Package", gone_package, "Meshes");
    let gone = p.add_import("Engine", "StaticMesh", gone_group, "Gone");
    let (box_mesh, mati_red, mat_base, tex_red, tex_base, cube) = (27, 29, 30, 32, 33, 34);
    let mesh_component = c.get("StaticMeshComponent");

    let world = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("World"), "TheWorld", 0, world), 1);
    let level = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("Level"), "PersistentLevel", 1, level), 2);

    let tags = [
        vector_tag(&mut p, "Location", [10.0, 20.0, 30.0]),
        rotator_tag(&mut p, "Rotation", [0, 16384, 0]),
        p.float_tag("DrawScale", 2.0),
    ];
    let actor = payload(&mut p, 26, &tags);
    assert_eq!(
        add(
            &mut p,
            c.get("StaticMeshActor"),
            "StaticMeshActor_0",
            2,
            actor
        ),
        3
    );
    let tags = [
        p.object_tag("StaticMesh", box_mesh),
        vector_tag(&mut p, "Translation", [1.0, 2.0, 3.0]),
        p.array_tag("Materials", 1, &ints(&[mati_red])),
    ];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_0", 3, comp),
        4
    );
    let tags = [
        p.object_tag("StaticMesh", box_mesh),
        p.bool_tag("HiddenGame", true),
    ];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_1", 3, comp),
        5
    );
    let tags = [p.object_tag("StaticMesh", gone)];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_2", 3, comp),
        6
    );
    let comp = payload(&mut p, 8, &[]);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_3", 3, comp),
        7
    );

    let tags = [p.bool_tag("bHidden", true)];
    let actor = payload(&mut p, 26, &tags);
    assert_eq!(
        add(
            &mut p,
            c.get("StaticMeshActor"),
            "StaticMeshActor_1",
            2,
            actor
        ),
        8
    );
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_4", 8, comp),
        9
    );

    let tags = [p.array_tag("StaticMeshComponents", 2, &ints(&[11, 12]))];
    let mut collection = payload(&mut p, 4, &tags);
    let row0 = collection.len();
    collection.extend(tail_row([500.0, 0.0, 0.0], [0.5, 0.5, 0.5]));
    collection.extend(tail_row([600.0, 0.0, 0.0], [2.0, 2.0, 2.0]));
    match variant.tail {
        Tail::Good => {}
        Tail::Short => collection.truncate(collection.len() - 4),
        // The first row's Scale3D.X, then its matrix[3] (the affine column).
        Tail::NonFinite => {
            collection[row0 + 64..row0 + 68].copy_from_slice(&f32::NAN.to_le_bytes())
        }
        Tail::NonAffine => collection[row0 + 12..row0 + 16].copy_from_slice(&0.5f32.to_le_bytes()),
    }
    let class = c.get("StaticMeshCollectionActor");
    assert_eq!(
        add(&mut p, class, "StaticMeshCollectionActor_0", 2, collection),
        10
    );
    let tags = [
        p.object_tag("StaticMesh", box_mesh),
        vector_tag(&mut p, "Scale3D", [0.5, 0.5, 0.5]),
    ];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_5", 10, comp),
        11
    );
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_6", 10, comp),
        12
    );

    let emitter = payload(&mut p, 26, &[]);
    assert_eq!(add(&mut p, c.get("Emitter"), "Emitter_0", 2, emitter), 13);
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_7", 13, comp),
        14
    );

    let io = payload(&mut p, 26, &[]);
    let class = c.get("WillowInteractiveObject");
    assert_eq!(add(&mut p, class, "WillowInteractiveObject_0", 2, io), 15);
    for (name, index) in [("StaticMeshComponent_8", 16), ("StaticMeshComponent_9", 17)] {
        let tags = [p.object_tag("StaticMesh", box_mesh)];
        let comp = payload(&mut p, 8, &tags);
        assert_eq!(add(&mut p, mesh_component, name, 15, comp), index);
    }
    let tags = [p.object_tag("PreviewMesh", 19)];
    let spawner = payload(&mut p, 26, &tags);
    let class = c.get("WillowMissionPickupSpawner");
    assert_eq!(
        add(&mut p, class, "WillowMissionPickupSpawner_0", 2, spawner),
        18
    );
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(&mut p, 8, &tags);
    assert_eq!(
        add(&mut p, mesh_component, "StaticMeshComponent_10", 18, comp),
        19
    );

    let tags = [
        vector_tag(&mut p, "Location", [100.0, 200.0, 300.0]),
        rotator_tag(&mut p, "Rotation", [0, 8192, 0]),
    ];
    let start = payload(&mut p, 26, &tags);
    assert_eq!(
        add(&mut p, c.get("PlayerStart"), "PlayerStart_0", 2, start),
        20
    );
    let light = payload(&mut p, 8, &[]);
    let class = c.get("PointLightComponent");
    assert_eq!(add(&mut p, class, "PointLightComponent_0", 2, light), 21);
    let terrain = payload(&mut p, 26, &[]);
    assert_eq!(add(&mut p, c.get("Terrain"), "Terrain_0", 2, terrain), 22);
    let part = payload(&mut p, 8, &[]);
    let class = c.get("TerrainComponent");
    assert_eq!(add(&mut p, class, "TerrainComponent_0", 22, part), 23);
    let model = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("Model"), "Model_0", 2, model), 24);
    let tags = [p.name_tag("PackageName", "Test_Sub")];
    let streaming = payload(&mut p, 4, &tags);
    let class = c.get("LevelStreamingKismet");
    assert_eq!(
        add(&mut p, class, "LevelStreamingKismet_0", 1, streaming),
        25
    );

    let group = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("Package"), "Meshes", 0, group), 26);
    let mesh = mesh_payload(&mut p, mat_base);
    assert_eq!(add(&mut p, c.get("StaticMesh"), "Box", 26, mesh), box_mesh);
    let group = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("Package"), "Materials", 0, group), 28);
    let mut parameters = p.name_tag("ParameterName", "Diffuse");
    parameters.extend(p.object_tag("ParameterValue", tex_red));
    parameters.extend(p.none());
    parameters.extend(p.name_tag("ParameterName", "Sky"));
    parameters.extend(p.object_tag("ParameterValue", cube));
    parameters.extend(p.none());
    let tags = [
        p.object_tag("Parent", mat_base),
        p.array_tag("TextureParameterValues", 2, &parameters),
    ];
    let instance = payload(&mut p, 4, &tags);
    let class = c.get("MaterialInstanceConstant");
    assert_eq!(add(&mut p, class, "Mati_Red", 28, instance), mati_red);
    let tags = [p.array_tag("Expressions", 1, &ints(&[31]))];
    let base = payload(&mut p, 4, &tags);
    assert_eq!(
        add(&mut p, c.get("Material"), "Mat_Base", 28, base),
        mat_base
    );
    let tags = [p.object_tag("Texture", tex_base)];
    let sample = payload(&mut p, 4, &tags);
    let class = c.get("MaterialExpressionTextureSample");
    assert_eq!(
        add(
            &mut p,
            class,
            "MaterialExpressionTextureSample_0",
            30,
            sample
        ),
        31
    );
    let red = texture_payload(&mut p, 200);
    assert_eq!(add(&mut p, c.get("Texture2D"), "Tex_Red", 28, red), tex_red);
    let base = texture_payload(&mut p, 50);
    assert_eq!(
        add(&mut p, c.get("Texture2D"), "Tex_Base", 28, base),
        tex_base
    );
    let sky = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, c.get("TextureCube"), "Cube_Sky", 28, sky), cube);
    if variant.extras {
        extras(&mut p, &c, box_mesh);
    }
    p.build()
}

/// How collection 10's tail is spoilt.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Tail {
    #[default]
    Good,
    /// 4 bytes short.
    Short,
    /// A NaN in the first row.
    NonFinite,
    /// A non-zero affine column in the first row.
    NonAffine,
}

/// Which `Test_P` to build.
#[derive(Debug, Clone, Copy, Default)]
struct Variant {
    tail: Tail,
    extras: bool,
}

/// The exports after 34 (invented values): 35 `StaticMeshCollectionActor_1` (`bHidden`) listing 36, a null entry
/// and 11 (which collection 10 lists already); 36 `StaticMeshComponent_11`; 37 `WillowMissionPickupSpawner_1`
/// whose `PreviewMesh` is 4, not its own 38; 39 `StaticMeshComponent_13` of actor 3, with a payload shorter than a
/// component's prefix; 40 `WillowInteractiveObject_1` holding 41 and 42, which differ only in `CastShadow`.
fn extras(p: &mut PackageBuilder, c: &Classes, box_mesh: i32) {
    let mesh_component = c.get("StaticMeshComponent");
    let tags = [
        p.bool_tag("bHidden", true),
        p.array_tag("StaticMeshComponents", 3, &ints(&[36, 0, 11])),
    ];
    let mut collection = payload(p, 4, &tags);
    collection.extend(tail_row([700.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    collection.extend(tail_row([800.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    collection.extend(tail_row([900.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let class = c.get("StaticMeshCollectionActor");
    assert_eq!(
        add(p, class, "StaticMeshCollectionActor_1", 2, collection),
        35
    );
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(p, 8, &tags);
    assert_eq!(
        add(p, mesh_component, "StaticMeshComponent_11", 35, comp),
        36
    );

    let tags = [p.object_tag("PreviewMesh", 4)];
    let spawner = payload(p, 26, &tags);
    let class = c.get("WillowMissionPickupSpawner");
    assert_eq!(
        add(p, class, "WillowMissionPickupSpawner_1", 2, spawner),
        37
    );
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(p, 8, &tags);
    assert_eq!(
        add(p, mesh_component, "StaticMeshComponent_12", 37, comp),
        38
    );

    assert_eq!(
        add(p, mesh_component, "StaticMeshComponent_13", 3, ints(&[0])),
        39
    );

    let io = payload(p, 26, &[]);
    let class = c.get("WillowInteractiveObject");
    assert_eq!(add(p, class, "WillowInteractiveObject_1", 2, io), 40);
    let tags = [p.object_tag("StaticMesh", box_mesh)];
    let comp = payload(p, 8, &tags);
    assert_eq!(
        add(p, mesh_component, "StaticMeshComponent_14", 40, comp),
        41
    );
    let tags = [
        p.object_tag("StaticMesh", box_mesh),
        p.bool_tag("CastShadow", true),
    ];
    let comp = payload(p, 8, &tags);
    assert_eq!(
        add(p, mesh_component, "StaticMeshComponent_15", 40, comp),
        42
    );
}

/// `Test_Sub.upk`: `StaticMeshActor_0` holding `StaticMeshComponent_0` (export 4), whose mesh is an import of
/// `Meshes.Box`, which `Test_P` exports.
fn test_sub() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let c = Classes::new(&mut p);
    let group = p.add_import("Core", "Package", 0, "Meshes");
    let mesh = p.add_import("Engine", "StaticMesh", group, "Box");
    let world = payload(&mut p, 4, &[]);
    add(&mut p, c.get("World"), "TheWorld", 0, world);
    let level = payload(&mut p, 4, &[]);
    add(&mut p, c.get("Level"), "PersistentLevel", 1, level);
    let tags = [vector_tag(&mut p, "Location", [-5.0, 0.0, 0.0])];
    let actor = payload(&mut p, 26, &tags);
    add(
        &mut p,
        c.get("StaticMeshActor"),
        "StaticMeshActor_0",
        2,
        actor,
    );
    let tags = [p.object_tag("StaticMesh", mesh)];
    let comp = payload(&mut p, 8, &tags);
    add(
        &mut p,
        c.get("StaticMeshComponent"),
        "StaticMeshComponent_0",
        3,
        comp,
    );
    p.build()
}

/// A folder of its own under the system temp folder, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ow-host-scene-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn load(name: &str, variant: Variant) -> (Scratch, DefStore, Result<Scene, SceneError>) {
    let dir = Scratch::new(name);
    let files = [
        ("Core.upk", core()),
        ("Engine.upk", engine()),
        ("WillowGame.upk", willow_game()),
        ("Test_P.upk", test_p(variant)),
        ("Test_Sub.upk", test_sub()),
    ];
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs = DefStore::from_synthetic(
        &dir.0,
        &refs,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .expect("the store");
    let (defs, scene) = load_from(defs, &dir.0);
    (dir, defs, scene)
}

/// `Test_P` through `defs`, in a new object space.
fn load_from(mut defs: DefStore, dir: &Path) -> (DefStore, Result<Scene, SceneError>) {
    let mut space = ObjectSpace::new();
    let assets = SceneAssets::new(dir.to_path_buf());
    let map = PackageName::parse("Test_P").unwrap();
    let scene = Scene::load(&mut defs, &mut space, &assets, &map);
    (defs, scene)
}

fn key(level: u32, export: i32) -> ExportKey {
    ExportKey { level, export }
}

#[test]
fn placements_and_exclusions_follow_the_owner_rules() {
    let (_dir, _defs, scene) = load("placements", Variant::default());
    let scene = scene.expect("the scene");
    let levels: Vec<&str> = scene.levels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(levels, ["Test_P", "Test_Sub"]);
    let placed: Vec<ExportKey> = scene.placements.keys().copied().collect();
    assert_eq!(
        placed,
        [
            key(0, 4),
            key(0, 5),
            key(0, 6),
            key(0, 9),
            key(0, 11),
            key(0, 12),
            key(0, 16),
            key(1, 4)
        ]
    );
    let excluded: Vec<(ExportKey, Exclusion)> = scene
        .excluded
        .iter()
        .map(|(k, e)| (*k, e.reason.clone()))
        .collect();
    assert_eq!(
        excluded,
        [
            (key(0, 7), Exclusion::NoStaticMesh),
            (
                key(0, 14),
                Exclusion::UnsupportedOwner("Engine.Emitter".into())
            ),
            (key(0, 17), Exclusion::DuplicateComponent),
            (key(0, 19), Exclusion::MissionItemPreview),
        ]
    );
    let p = scene.placements.get(&key(0, 4)).unwrap();
    assert_eq!(
        (p.path.as_str(), p.owner_class.as_str(), p.mesh.as_str()),
        (
            "TheWorld.PersistentLevel.StaticMeshActor_0.StaticMeshComponent_0",
            "Engine.StaticMeshActor",
            "Meshes.Box"
        )
    );
}

#[test]
fn poses_keep_the_serialized_values_and_the_class_defaults() {
    let (_dir, _defs, scene) = load("poses", Variant::default());
    let scene = scene.expect("the scene");
    let Pose::Attached { actor, component } = scene.placements.get(&key(0, 4)).unwrap().pose else {
        panic!("an attached pose");
    };
    assert_eq!(actor.location, [10.0, 20.0, 30.0]);
    assert_eq!(actor.rotation, Rotator::new(0, 16384, 0));
    // DrawScale is the actor's own tag; DrawScale3D, Scale and Scale3D come from the class defaults.
    assert_eq!(
        (actor.draw_scale, actor.draw_scale3d),
        (2.0, [1.0, 1.0, 1.0])
    );
    assert_eq!(component.translation, [1.0, 2.0, 3.0]);
    assert_eq!((component.scale, component.scale3d), (1.0, [1.0, 1.0, 1.0]));

    let Pose::Collection {
        matrix,
        scale3d,
        scale,
        scale_source,
    } = scene.placements.get(&key(0, 11)).unwrap().pose
    else {
        panic!("a collection pose");
    };
    assert_eq!(&matrix[12..16], &[500.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        (scale3d, scale, scale_source),
        ([0.5; 3], 1.0, ScaleSource::Component)
    );
    let Pose::Collection {
        scale3d,
        scale_source,
        ..
    } = scene.placements.get(&key(0, 12)).unwrap().pose
    else {
        panic!("a collection pose");
    };
    assert_eq!(
        (scale3d, scale_source),
        ([2.0; 3], ScaleSource::CollectionTail)
    );

    let start = scene.player_starts.get(&key(0, 20)).unwrap();
    assert_eq!(
        (start.pose.location, start.pose.rotation.yaw),
        ([100.0, 200.0, 300.0], 8192)
    );
    assert_eq!(start.class, "Engine.PlayerStart");
}

#[test]
fn the_hidden_filter_reads_the_component_and_the_owner_flags() {
    let (_dir, _defs, scene) = load("hidden", Variant::default());
    let scene = scene.expect("the scene");
    let hidden: Vec<(ExportKey, HiddenFlags)> = scene
        .placements
        .iter()
        .filter(|(_, p)| p.is_hidden())
        .map(|(k, p)| (*k, p.hidden))
        .collect();
    assert_eq!(
        hidden,
        [
            (
                key(0, 5),
                HiddenFlags {
                    hidden_game: true,
                    owner_hidden: false
                }
            ),
            (
                key(0, 9),
                HiddenFlags {
                    hidden_game: false,
                    owner_hidden: true
                }
            ),
        ]
    );
    let s = scene.summary();
    assert_eq!((s.placements, s.hidden), (8, 2));
    // One triangle per placement of the decoded box; the placement of the missing mesh counts 0.
    assert_eq!((s.triangles_all, s.triangles_drawn), (7, 5));
}

#[test]
fn a_mesh_no_file_holds_is_not_yet_and_an_import_resolves_in_the_loaded_levels() {
    let (_dir, _defs, scene) = load("meshes", Variant::default());
    let scene = scene.expect("the scene");
    let paths: Vec<&str> = scene.meshes.keys().map(String::as_str).collect();
    assert_eq!(paths, ["Meshes.Box", "Missing.Meshes.Gone"]);
    let gone = scene.meshes.get("Missing.Meshes.Gone").unwrap();
    assert!(
        matches!(&gone.status, MeshStatus::NotYet(n) if n.what.contains("no installed package")),
        "{gone:?}"
    );
    assert_eq!(gone.holder, None);
    let mesh = scene.meshes.get("Meshes.Box").unwrap();
    assert_eq!(
        mesh.holder.as_ref().map(|h| (h.file.as_str(), h.export)),
        Some(("Test_P", 27))
    );
    assert_eq!(mesh.lod0_triangles(), Some(1));
    assert_eq!(
        mesh.section_materials,
        [Some("Materials.Mat_Base".to_string())]
    );
    // Test_Sub's import of Meshes.Box is the export Test_P holds.
    assert_eq!(scene.placements.get(&key(1, 4)).unwrap().mesh, "Meshes.Box");
    let gaps: Vec<(GapPart, &str)> = scene
        .gaps
        .iter()
        .map(|g| (g.part, g.object.as_str()))
        .collect();
    assert!(
        gaps.contains(&(GapPart::Mesh, "Missing.Meshes.Gone")),
        "{gaps:?}"
    );
}

#[test]
fn materials_name_their_textures_and_textures_decode_on_demand() {
    let (_dir, defs, scene) = load("textures", Variant::default());
    let scene = scene.expect("the scene");
    let red = scene.materials.get("Materials.Mati_Red").unwrap();
    assert_eq!(red.status, MaterialStatus::Read);
    assert_eq!(red.class, "Engine.MaterialInstanceConstant");
    assert_eq!(red.parent.as_deref(), Some("Materials.Mat_Base"));
    let named = |path: &str, parameter: Option<&str>, from| MaterialTexture {
        path: path.to_string(),
        parameter: parameter.map(str::to_string),
        from,
    };
    assert_eq!(
        red.textures,
        [
            named(
                "Materials.Tex_Red",
                Some("Diffuse"),
                TextureSource::Instance
            ),
            named("Materials.Cube_Sky", Some("Sky"), TextureSource::Instance),
            named("Materials.Tex_Base", None, TextureSource::Expression),
        ]
    );
    let placement = scene.placements.get(&key(0, 4)).unwrap();
    assert_eq!(
        scene.effective_materials(placement),
        [Some("Materials.Mati_Red".to_string())]
    );
    let other = scene.placements.get(&key(0, 11)).unwrap();
    assert_eq!(
        scene.effective_materials(other),
        [Some("Materials.Mat_Base".to_string())]
    );
    let s = scene.summary();
    assert_eq!(
        (s.textures_named, s.textures_all, s.textures_drawn),
        (3, 3, 3)
    );

    let texture = scene
        .decode_texture(&defs, "Materials.Tex_Red")
        .expect("a texture");
    assert_eq!((texture.mips.len(), texture.mips[0].width), (1, 4));
    assert_eq!(&texture.mips[0].rgba[..4], &[200, 200, 200, 255]);
    assert!(matches!(
        scene.decode_texture(&defs, "Materials.Cube_Sky"),
        Err(TextureError::NotYet(_))
    ));
    assert!(matches!(
        scene.decode_texture(&defs, "Materials.Nope"),
        Err(TextureError::NotInScene(_))
    ));
}

#[test]
fn lights_terrain_and_bsp_are_counted_and_named_as_gaps() {
    let (_dir, _defs, scene) = load("census", Variant::default());
    let scene = scene.expect("the scene");
    let lights: Vec<(&str, LightCount)> =
        scene.lights.iter().map(|(c, n)| (c.as_str(), *n)).collect();
    assert_eq!(
        lights,
        [(
            "Engine.PointLightComponent",
            LightCount {
                total: 1,
                in_collections: 0
            }
        )]
    );
    assert_eq!(scene.terrains.get(&key(0, 22)).unwrap().components, 1);
    assert_eq!(
        scene.models.get(&0),
        Some(&LevelModels {
            models: 1,
            components: 0
        })
    );
    let gaps: Vec<(GapPart, String)> = scene
        .summary()
        .gaps
        .into_iter()
        .map(|((part, what), n)| (part, format!("{n} {what}")))
        .collect();
    assert_eq!(
        gaps,
        [
            (
                GapPart::Mesh,
                "1 StaticMesh export: no installed package holds its path".to_string()
            ),
            (
                GapPart::Texture,
                "1 a texture class ow-assets does not read (it reads Texture2D and TerrainWeightMapTexture)"
                    .to_string()
            ),
            (GapPart::Terrain, "1 terrain height and layer data".to_string()),
            (GapPart::Bsp, "1 BSP model geometry".to_string()),
        ]
    );
}

#[test]
fn a_collection_tail_off_the_recorded_layout_is_a_gap_not_a_guess() {
    let (_dir, _defs, scene) = load(
        "broken",
        Variant {
            tail: Tail::Short,
            extras: false,
        },
    );
    let scene = scene.expect("the scene");
    assert!(!scene.placements.contains_key(&key(0, 11)));
    assert!(
        matches!(
            &scene.excluded.get(&key(0, 12)).unwrap().reason,
            Exclusion::CollectionUnreadable(r) if r.contains("tail of 164 bytes for 2 components")
        ),
        "{:?}",
        scene.excluded.get(&key(0, 12))
    );
    assert!(scene.gaps.iter().any(|g| g.part == GapPart::Collection));
}

#[test]
fn a_non_finite_or_non_affine_tail_is_a_gap() {
    for (tail, reason) in [
        (Tail::NonFinite, "a non-finite tail value"),
        (Tail::NonAffine, "a non-affine tail matrix"),
    ] {
        let variant = Variant {
            tail,
            extras: false,
        };
        let (_dir, _defs, scene) = load(&format!("{tail:?}"), variant);
        let scene = scene.expect("the scene");
        for export in [11, 12] {
            let excluded = &scene.excluded.get(&key(0, export)).unwrap().reason;
            assert_eq!(
                excluded,
                &Exclusion::CollectionUnreadable(reason.to_string()),
                "{tail:?}"
            );
        }
        assert!(scene.gaps.iter().any(|g| g.part == GapPart::Collection));
    }
}

#[test]
fn collection_listings_owner_previews_failed_loads_and_near_duplicates() {
    let variant = Variant {
        tail: Tail::Good,
        extras: true,
    };
    let (_dir, _defs, scene) = load("extras", variant);
    let scene = scene.expect("the scene");
    // Component 11 is listed by collections 10 and 35: a gap, and not placed.
    assert_eq!(
        scene.excluded.get(&key(0, 11)).map(|e| &e.reason),
        Some(&Exclusion::CollectionUnreadable(
            "listed by two collection entries".to_string()
        ))
    );
    let listed: Vec<&str> = scene
        .gaps
        .iter()
        .filter(|g| g.part == GapPart::Collection)
        .map(|g| g.object.as_str())
        .collect();
    assert_eq!(
        listed,
        ["Test_P:TheWorld.PersistentLevel.StaticMeshCollectionActor_0.StaticMeshComponent_5"]
    );
    // Collection 35's null entry is skipped; its own bHidden hides 36.
    let p = scene.placements.get(&key(0, 36)).unwrap();
    assert_eq!(
        p.hidden,
        HiddenFlags {
            hidden_game: false,
            owner_hidden: true
        }
    );
    let Pose::Collection { matrix, .. } = p.pose else {
        panic!("a collection pose");
    };
    assert_eq!(matrix[12], 700.0);
    // A spawner whose PreviewMesh names another component is an unsupported owner of its own component.
    assert_eq!(
        scene.excluded.get(&key(0, 38)).map(|e| &e.reason),
        Some(&Exclusion::UnsupportedOwner(
            "WillowGame.WillowMissionPickupSpawner".to_string()
        ))
    );
    assert!(matches!(
        scene.excluded.get(&key(0, 39)).map(|e| &e.reason),
        Some(Exclusion::LoadFailed(_))
    ));
    // Two components that differ in one flag of DUPLICATE_FLAGS are both placed.
    assert!(scene.placements.contains_key(&key(0, 41)));
    assert!(scene.placements.contains_key(&key(0, 42)));
}

#[test]
fn two_loads_give_the_same_scene() {
    let variant = Variant {
        tail: Tail::Good,
        extras: true,
    };
    let (dir, defs, first) = load("twice", variant);
    drop(defs);
    let again = DefStore::open(
        dir.0.clone(),
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let (_defs, second) = load_from(again, &dir.0);
    assert_eq!(first.expect("the first"), second.expect("the second"));
}

#[test]
fn a_map_no_file_holds_is_an_error() {
    let dir = Scratch::new("nomap");
    let core = core();
    let files: [(&str, &[u8]); 1] = [("Core.upk", &core)];
    let mut defs = DefStore::from_synthetic(
        &dir.0,
        &files,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut space = ObjectSpace::new();
    let map = PackageName::parse("Nope_P").unwrap();
    let result = Scene::load(
        &mut defs,
        &mut space,
        &SceneAssets::new(dir.0.clone()),
        &map,
    );
    assert!(
        matches!(
            result,
            Err(SceneError::Store(StoreError::PackageNotFound(_)))
        ),
        "{result:?}"
    );
}

// ------------------------------------------------------------------------------------------ installed driver

/// Floats as Python's `repr` prints the same `f64` (`ow_core::fmt`), so the TSV compares with the tool's JSON.
fn num(x: f32) -> String {
    fmt_py_repr(f64::from(x))
}

fn pose_columns(pose: &Pose) -> String {
    match pose {
        Pose::Attached { actor, component } => format!(
            "attached\t{} {} {}\t{} {} {}\t{} {} {} {}\t{} {} {}\t{} {} {}\t{} {} {} {}",
            num(actor.location[0]),
            num(actor.location[1]),
            num(actor.location[2]),
            actor.rotation.pitch,
            actor.rotation.yaw,
            actor.rotation.roll,
            num(actor.draw_scale),
            num(actor.draw_scale3d[0]),
            num(actor.draw_scale3d[1]),
            num(actor.draw_scale3d[2]),
            num(component.translation[0]),
            num(component.translation[1]),
            num(component.translation[2]),
            component.rotation.pitch,
            component.rotation.yaw,
            component.rotation.roll,
            num(component.scale),
            num(component.scale3d[0]),
            num(component.scale3d[1]),
            num(component.scale3d[2]),
        ),
        Pose::Collection {
            matrix,
            scale3d,
            scale,
            scale_source,
        } => {
            let m: Vec<String> = matrix.iter().map(|&v| num(v)).collect();
            format!(
                "collection\t{}\t{} {} {} {}\t{scale_source:?}",
                m.join(" "),
                num(scale3d[0]),
                num(scale3d[1]),
                num(scale3d[2]),
                num(*scale),
            )
        }
    }
}

/// Sanctuary on the installed game: `OW_BL2_ROOT` is the install folder. Run with
/// `cargo test --release -p ow-host --locked -- --ignored sanctuary_on_the_installed_game`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h2-scene/"]
fn sanctuary_on_the_installed_game() {
    let Some(root) = std::env::var_os("OW_BL2_ROOT") else {
        panic!("set OW_BL2_ROOT to the Borderlands 2 install folder");
    };
    let install = crate::find_install(Path::new(&root)).expect("an install");
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h2-scene");
    std::fs::create_dir_all(&out).expect("the output folder");
    let started = std::time::Instant::now();
    let mut defs = DefStore::open(
        install.cooked.clone(),
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .expect("the store");
    let mut space = ObjectSpace::new();
    let assets = SceneAssets::new(install.cooked.clone());
    let map = PackageName::parse("Sanctuary_P").unwrap();
    let scene = Scene::load(&mut defs, &mut space, &assets, &map).expect("the scene");
    let load_time = started.elapsed();

    let mut tsv = String::new();
    for (key, p) in scene.placements.iter() {
        let _ = writeln!(
            tsv,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            scene.levels[key.level as usize].name,
            p.path,
            p.owner_class,
            p.mesh,
            u8::from(p.hidden.hidden_game),
            u8::from(p.hidden.owner_hidden),
            pose_columns(&p.pose)
        );
    }
    std::fs::write(out.join("placements.tsv"), tsv).expect("placements.tsv");
    let mut excluded = String::new();
    for (key, e) in scene.excluded.iter() {
        let _ = writeln!(
            excluded,
            "{}\t{}\t{:?}",
            scene.levels[key.level as usize].name, e.path, e.reason
        );
    }
    std::fs::write(out.join("excluded.tsv"), excluded).expect("excluded.tsv");
    let mut meshes = String::new();
    for (path, m) in scene.meshes.iter() {
        let sections = match &m.status {
            MeshStatus::Decoded(mesh) => mesh.lods.first().map_or(0, |l| l.sections.len()),
            _ => 0,
        };
        let _ = writeln!(
            meshes,
            "{path}	{}	{}	{sections}",
            m.holder.as_ref().map_or("-", |h| h.file.as_str()),
            m.lod0_triangles().map_or(-1, |t| t as i64)
        );
    }
    std::fs::write(out.join("meshes.tsv"), meshes).expect("meshes.tsv");
    let mut materials = String::new();
    for p in scene.placements.values() {
        let effective = scene.effective_materials(p);
        let textures: Vec<&str> = effective
            .iter()
            .flatten()
            .filter_map(|m| scene.materials.get(m.as_str()))
            .flat_map(|m| m.textures.iter().map(|t| t.path.as_str()))
            .collect();
        let _ = writeln!(materials, "{}	{}", p.path, textures.join(" "));
    }
    std::fs::write(out.join("placement_textures.tsv"), materials).expect("placement_textures.tsv");

    // Every named texture decoded once and dropped.
    let decode_started = std::time::Instant::now();
    let mut decoded = 0usize;
    let mut texture_failures: DetMap<String, usize> = DetMap::new();
    let mut decoded_set = DetSet::new();
    for path in scene.textures.keys() {
        match scene.decode_texture(&defs, path) {
            Ok(_) => {
                decoded += 1;
                decoded_set.insert(path.clone());
            }
            Err(e) => *texture_failures.entry(e.to_string()).or_insert(0) += 1,
        }
    }
    let decode_time = decode_started.elapsed();

    let s = scene.summary();
    let mut r = String::new();
    let _ = writeln!(
        r,
        "# H2 scene report: {} (local, game-derived counts only)",
        scene.map
    );
    let _ = writeln!(
        r,
        "load {} ms, texture decode {} ms",
        load_time.as_millis(),
        decode_time.as_millis()
    );
    let levels: Vec<&str> = scene.levels.iter().map(|l| l.name.as_str()).collect();
    let _ = writeln!(r, "levels ({}): {}", levels.len(), levels.join(", "));
    let _ = writeln!(
        r,
        "placements {} hidden {} drawn {}",
        s.placements,
        s.hidden,
        s.placements - s.hidden
    );
    let hidden_game = scene
        .placements
        .values()
        .filter(|p| p.hidden.hidden_game)
        .count();
    let owner_hidden = scene
        .placements
        .values()
        .filter(|p| p.hidden.owner_hidden)
        .count();
    let movers_hidden = scene
        .placements
        .values()
        .filter(|p| p.is_hidden() && p.owner_class == "Engine.InterpActor")
        .count();
    let _ = writeln!(
        r,
        "hidden by HiddenGame {hidden_game}, by owner bHidden {owner_hidden}, hidden InterpActor placements {movers_hidden}"
    );
    let mut per_level: OrderedMap<String, usize> = OrderedMap::new();
    for (key, _) in scene.placements.iter() {
        let name = scene.levels[key.level as usize].name.clone();
        let n = per_level.get(name.as_str()).copied().unwrap_or(0);
        per_level.insert(name, n + 1);
    }
    for (name, n) in per_level.iter() {
        let _ = writeln!(r, "  {name}: {n}");
    }
    let mut owners: OrderedMap<String, usize> = OrderedMap::new();
    for p in scene.placements.values() {
        let n = owners.get(p.owner_class.as_str()).copied().unwrap_or(0);
        owners.insert(p.owner_class.clone(), n + 1);
    }
    for (name, n) in owners.iter() {
        let _ = writeln!(r, "  owner {name}: {n}");
    }
    let mut reasons: DetMap<String, usize> = DetMap::new();
    for e in scene.excluded.values() {
        let reason = match &e.reason {
            Exclusion::LoadFailed(t) => format!("LoadFailed: {t}"),
            other => format!("{other:?}"),
        };
        *reasons.entry(reason).or_insert(0) += 1;
    }
    let _ = writeln!(r, "excluded {}", scene.excluded.len());
    for (reason, n) in &reasons {
        let _ = writeln!(r, "  {n} {reason}");
    }
    let _ = writeln!(
        r,
        "meshes {} (drawn {}), decoded {}, not yet {}, refused {}",
        s.meshes, s.meshes_drawn, s.meshes_decoded, s.meshes_not_yet, s.meshes_refused
    );
    let mut refusals: DetMap<String, usize> = DetMap::new();
    for m in scene.meshes.values() {
        if let MeshStatus::Refused(e) = &m.status {
            *refusals.entry(e.to_string()).or_insert(0) += 1;
        }
    }
    for (reason, n) in &refusals {
        let _ = writeln!(r, "  mesh refused {n}: {reason}");
    }
    let _ = writeln!(
        r,
        "triangles (LOD 0) all {} drawn {}",
        s.triangles_all, s.triangles_drawn
    );
    let mut material_status: DetMap<String, usize> = DetMap::new();
    for m in scene.materials.values() {
        let k = match &m.status {
            MaterialStatus::Read => "read".to_string(),
            MaterialStatus::NotYet(n) => format!("not yet: {}", n.what),
            MaterialStatus::Failed(t) => format!("failed: {t}"),
        };
        *material_status.entry(k).or_insert(0) += 1;
    }
    let _ = writeln!(r, "materials {}", scene.materials.len());
    for (k, n) in &material_status {
        let _ = writeln!(r, "  {n} {k}");
    }
    let drawn_decoded = scene
        .placements
        .values()
        .filter(|p| !p.is_hidden())
        .flat_map(|p| scene.effective_materials(p))
        .flatten()
        .filter_map(|m| scene.materials.get(m.as_str()))
        .flat_map(|m| m.textures.iter().map(|t| &t.path))
        .filter(|t| decoded_set.contains(*t))
        .collect::<DetSet<_>>()
        .len();
    let _ = writeln!(
        r,
        "textures named {} (by all placements {}, by drawn {}), decoded {} (of the drawn set {}), failed {}",
        s.textures_named,
        s.textures_all,
        s.textures_drawn,
        decoded,
        drawn_decoded,
        s.textures_named - decoded
    );
    for (reason, n) in &texture_failures {
        let _ = writeln!(r, "  texture failed {n}: {reason}");
    }
    let mut classes: DetMap<String, usize> = DetMap::new();
    for t in scene.textures.values() {
        *classes.entry(t.class.clone()).or_insert(0) += 1;
    }
    for (class, n) in &classes {
        let _ = writeln!(r, "  texture class {class}: {n}");
    }
    let _ = writeln!(r, "player starts {}", scene.player_starts.len());
    for (key, p) in scene.player_starts.iter() {
        let _ = writeln!(
            r,
            "  {}:{} {} at {} {} {} yaw {}",
            scene.levels[key.level as usize].name,
            p.path,
            p.class,
            num(p.pose.location[0]),
            num(p.pose.location[1]),
            num(p.pose.location[2]),
            p.pose.rotation.yaw
        );
    }
    let _ = writeln!(r, "lights (components by class)");
    for (class, n) in scene.lights.iter() {
        let _ = writeln!(
            r,
            "  {class}: {} ({} in collections)",
            n.total, n.in_collections
        );
    }
    let _ = writeln!(r, "terrains {}", scene.terrains.len());
    for (key, t) in scene.terrains.iter() {
        let _ = writeln!(
            r,
            "  {}:{} components {}",
            scene.levels[key.level as usize].name, t.path, t.components
        );
    }
    let _ = writeln!(r, "models (BSP) by level");
    for (level, m) in scene.models.iter() {
        let _ = writeln!(
            r,
            "  {}: {} models, {} components",
            scene.levels[*level as usize].name, m.models, m.components
        );
    }
    let _ = writeln!(r, "gaps {}", scene.gaps.len());
    for ((part, what), n) in &s.gaps {
        let _ = writeln!(r, "  {part:?} {n}: {what}");
    }
    std::fs::write(out.join("report.md"), &r).expect("report.md");
    print!("{r}");
}
