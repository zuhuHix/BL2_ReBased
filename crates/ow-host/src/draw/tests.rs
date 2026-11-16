//! Owner: lane H2-DRAW. Tests of the draw plan on an invented scene (AGENTS.md rule 9): every path and value is made up.

use super::*;
use crate::frame;
use crate::material::{Blend, DiffuseSource, MaterialLibrary};
use crate::scene::{
    ActorPose, ComponentPose, ExportKey, HiddenFlags, MaterialStatus, MaterialTexture, Placement,
    ScaleSource, Scene, SceneAssets, SceneMesh, TextureSource,
};
use ow_assets::mesh::{MeshAsset, MeshLod, MeshSection, MeshVertex};
use ow_core::units::Rotator;

/// A one-section triangle mesh, or a two-section one whose second section is empty.
fn mesh(sections: usize) -> SceneMesh {
    let mut lod = MeshLod::default();
    for p in [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]] {
        let mut v = MeshVertex::default();
        v.position = p;
        v.normal = [0.0, 0.0, 1.0];
        v.uvs = vec![[0.5, 0.25]];
        lod.vertices.push(v);
    }
    lod.indices = vec![0, 1, 2];
    for i in 0..sections {
        let mut s = MeshSection::default();
        s.face_count = u32::from(i == 0);
        s.max_vertex = 2;
        lod.sections.push(s);
    }
    let mut asset = MeshAsset::default();
    asset.lods.push(lod);
    SceneMesh {
        holder: None,
        status: MeshStatus::Decoded(asset),
        section_materials: vec![Some("Mats.Base".to_string()); sections],
    }
}

fn texture(path: &str, parameter: Option<&str>, depth: u32) -> MaterialTexture {
    MaterialTexture {
        path: path.to_string(),
        parameter: parameter.map(str::to_string),
        from: TextureSource::Instance,
        depth,
    }
}

fn material(textures: Vec<MaterialTexture>, two_sided: Option<bool>) -> SceneMaterial {
    SceneMaterial {
        class: "Engine.MaterialInstanceConstant".into(),
        parent: None,
        textures,
        two_sided,
        status: MaterialStatus::Read,
    }
}

fn attached(location: [f32; 3], draw_scale3d: [f32; 3]) -> Pose {
    Pose::Attached {
        actor: ActorPose {
            location,
            rotation: Rotator::new(0, 16384, 0),
            draw_scale: 1.0,
            draw_scale3d,
        },
        component: ComponentPose {
            translation: [0.0; 3],
            rotation: Rotator::default(),
            scale: 1.0,
            scale3d: [1.0; 3],
        },
    }
}

fn placement(mesh: &str, pose: Pose, hidden: bool, materials: Vec<Option<String>>) -> Placement {
    Placement {
        path: format!("TheWorld.PersistentLevel.{mesh}"),
        owner_class: "Engine.StaticMeshActor".into(),
        mesh: mesh.into(),
        pose,
        hidden: HiddenFlags {
            hidden_game: hidden,
            owner_hidden: false,
        },
        materials,
    }
}

/// Six placements: two of `Meshes.A` at different places (one batch), one mirrored (its own batch), one hidden, one
/// of a mesh that did not decode, one of `Meshes.B` through a collection with a material override.
fn scene() -> Scene {
    let mut scene = Scene::empty("Test_P", SceneAssets::new("."));
    scene.meshes.insert("Meshes.A".into(), mesh(1));
    scene.meshes.insert("Meshes.B".into(), mesh(2));
    let gone = SceneMesh {
        holder: None,
        status: MeshStatus::NotYet(ow_core::NotYet::not_read("an invented gap", "test")),
        section_materials: Vec::new(),
    };
    scene.meshes.insert("Meshes.Gone".into(), gone);
    scene.materials.insert(
        "Mats.Base".into(),
        material(
            vec![texture("Tex.Base_Dif", Some("p_Diffuse"), 0)],
            Some(false),
        ),
    );
    scene.materials.insert(
        "Mats.Glass".into(),
        material(
            vec![texture("Tex.Glass_Dif", Some("Diffuse"), 0)],
            Some(true),
        ),
    );
    let mut matrix = [0f32; 16];
    for i in [0, 5, 10, 15] {
        matrix[i] = 1.0;
    }
    matrix[12] = 500.0;
    let collection = Pose::Collection {
        matrix,
        scale3d: [2.0, 2.0, 2.0],
        scale: 0.5,
        scale_source: ScaleSource::CollectionTail,
    };
    let rows = [
        placement(
            "Meshes.A",
            attached([1.0, 2.0, 3.0], [1.0; 3]),
            false,
            vec![],
        ),
        placement(
            "Meshes.A",
            attached([9.0, 2.0, 3.0], [1.0; 3]),
            false,
            vec![],
        ),
        placement(
            "Meshes.A",
            attached([0.0; 3], [-1.0, 1.0, 1.0]),
            false,
            vec![],
        ),
        placement("Meshes.A", attached([0.0; 3], [1.0; 3]), true, vec![]),
        placement("Meshes.Gone", attached([0.0; 3], [1.0; 3]), false, vec![]),
        placement(
            "Meshes.B",
            collection,
            false,
            vec![Some("Mats.Glass".into())],
        ),
    ];
    for (i, p) in rows.into_iter().enumerate() {
        let key = ExportKey {
            level: 0,
            export: i32::try_from(i).unwrap() + 1,
        };
        scene.placements.insert(key, p);
    }
    scene
}

#[test]
fn the_plan_uploads_each_mesh_once_and_batches_its_placements() {
    let plan = DrawPlan::build(&scene());
    let paths: Vec<&str> = plan.meshes.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["Meshes.A", "Meshes.B"]);
    let a = &plan.meshes[0];
    assert_eq!(a.vertex_bytes().len(), 3 * VERTEX_BYTES);
    assert_eq!(a.index_bytes(), [0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(
        &a.vertex_bytes()[24..32],
        &[0.5f32.to_le_bytes(), 0.25f32.to_le_bytes()].concat()[..]
    );
    assert_eq!(
        a.sections,
        [SectionRange {
            first_index: 0,
            index_count: 3
        }]
    );
    let s = plan.stats;
    assert_eq!(
        (
            s.placements,
            s.hidden,
            s.no_mesh,
            s.bad_transform,
            s.drawn,
            s.mirrored
        ),
        (6, 1, 1, 0, 4, 1)
    );
    // Meshes.B's second section has no triangles.
    assert_eq!((s.instances, s.empty_sections), (4, 1));
    let firsts: Vec<&str> = plan
        .slots
        .iter()
        .map(|c| c.diffuse[0].path.as_str())
        .collect();
    assert_eq!(firsts, ["Tex.Base_Dif", "Tex.Glass_Dif"]);
    assert_eq!(
        (
            s.untextured_no_material,
            s.untextured_unknown_material,
            s.untextured_no_diffuse
        ),
        (0, 0, 0)
    );
    let batches: Vec<(BatchKey, usize)> = plan
        .batches
        .iter()
        .map(|b| (b.key, b.instances.len()))
        .collect();
    let key = |mesh, material, two_sided, mirrored| BatchKey {
        mesh,
        section: 0,
        material: Some(material),
        two_sided,
        mirrored,
    };
    assert_eq!(
        batches,
        [
            (key(0, 0, false, false), 2),
            (key(0, 0, false, true), 1),
            (key(1, 1, true, false), 1),
        ]
    );
    // The two placements of one batch keep their own matrices, in placement order: yaw 16384 turns X onto Y.
    let first = &plan.batches[0].instances;
    assert_eq!(first[0].world.cols[3], [1.0, 2.0, 3.0, 1.0]);
    assert_eq!(first[1].world.cols[3], [9.0, 2.0, 3.0, 1.0]);
    assert!(first[0].world.cols[0][1] > 0.999);
}

#[test]
fn a_mirroring_placement_has_its_own_batch_key() {
    let plan = DrawPlan::build(&scene());
    let mirrored: Vec<&Batch> = plan.batches.iter().filter(|b| b.key.mirrored).collect();
    assert_eq!(mirrored.len(), 1);
    let m = mirrored[0].instances[0].world;
    // DrawScale3D (-1, 1, 1) under yaw 16384: the X axis points along -Y.
    assert!(m.cols[0][1] < -0.999);
    assert!(
        world_affine(&attached([0.0; 3], [-1.0, 1.0, 1.0]))
            .unwrap()
            .mirrors()
    );
    assert!(
        !world_affine(&attached([0.0; 3], [-1.0, -1.0, 1.0]))
            .unwrap()
            .mirrors()
    );
}

#[test]
fn a_collection_pose_scales_each_row_by_scale3d_times_scale() {
    let plan = DrawPlan::build(&scene());
    let b = plan.batches.iter().find(|b| b.key.mesh == 1).unwrap();
    let m = b.instances[0].world;
    assert_eq!(m.cols[0], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(m.cols[3], [500.0, 0.0, 0.0, 1.0]);
}

/// `n` through `m` (column vectors, 3x3 columns as given).
fn apply3(m: &[[f32; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (c, col) in m.iter().enumerate() {
        for (r, cell) in out.iter_mut().enumerate() {
            *cell += f64::from(col[r]) * v[c];
        }
    }
    out
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn normals_stay_perpendicular_under_a_non_uniform_scale() {
    // DrawScale3D (4, 1, 1), no rotation; a face at 45 degrees: normal (1, 1, 0), a tangent (1, -1, 0).
    let affine = frame::Placement::new([0.0; 3], Rotator::default(), 1.0, [4.0, 1.0, 1.0])
        .affine()
        .unwrap();
    let n = normal_matrix(&affine);
    let normal = apply3(&n, [1.0, 1.0, 0.0]);
    let tangent = affine.direction([1.0, -1.0, 0.0]);
    assert!(dot(normal, tangent).abs() < 1e-9, "{normal:?} {tangent:?}");
    // The transformed normal is (1, 4, 0) up to length; the model matrix would have given (4, 1, 0).
    assert!((normal[1] / normal[0] - 4.0).abs() < 1e-9);
    // A rotated, mirrored, non-uniform transform: still perpendicular, and still on the outward side.
    let affine = frame::Placement::new(
        [5.0; 3],
        Rotator::new(3000, 12000, -7000),
        1.5,
        [-2.0, 1.0, 3.0],
    )
    .affine()
    .unwrap();
    assert!(affine.mirrors());
    let n = normal_matrix(&affine);
    for (normal, tangent) in [
        ([1.0, 1.0, 0.0], [1.0, -1.0, 0.0]),
        ([0.0, 0.3, 1.0], [0.0, 1.0, -0.3]),
    ] {
        let turned = apply3(&n, normal);
        assert!(dot(turned, affine.direction(tangent)).abs() < 1e-5);
        assert!(dot(turned, affine.direction(normal)) > 0.0);
    }
}

#[test]
fn a_candidate_that_does_not_decode_is_skipped_for_the_next() {
    let m = material(
        vec![
            texture("Tex.Main", Some("p_Diffuse"), 0),
            texture("Tex.Layer", Some("Diffuse_A"), 0),
            texture("Common_Textures.Stub.StubGray", Some("Diffuse_B"), 1),
        ],
        None,
    );
    let candidates = diffuse_candidates(&m);
    let pick =
        |usable: &dyn Fn(&str) -> bool| pick_from(&candidates, usable).map(|c| c.path.as_str());
    assert_eq!(pick(&|_| true), Some("Tex.Main"));
    assert_eq!(pick(&|p| p != "Tex.Main"), Some("Tex.Layer"));
    // A stub is still taken when it is all that decodes.
    assert_eq!(
        pick(&|p| p.starts_with("Common")),
        Some("Common_Textures.Stub.StubGray")
    );
    assert_eq!(pick(&|_| false), None);
}

#[test]
fn untextured_sections_are_counted_by_cause() {
    let mut s = scene();
    s.materials.insert(
        "Mats.Plain".into(),
        material(vec![texture("Tex.N", Some("p_Normal"), 0)], Some(false)),
    );
    let mut bare = mesh(1);
    bare.section_materials = vec![None];
    s.meshes.insert("Meshes.Bare".into(), bare);
    let rows = [
        placement("Meshes.Bare", attached([0.0; 3], [1.0; 3]), false, vec![]),
        placement(
            "Meshes.A",
            attached([0.0; 3], [1.0; 3]),
            false,
            vec![Some("Mats.Nowhere".into())],
        ),
        placement(
            "Meshes.A",
            attached([0.0; 3], [1.0; 3]),
            false,
            vec![Some("Mats.Plain".into())],
        ),
    ];
    for (i, p) in rows.into_iter().enumerate() {
        let key = ExportKey {
            level: 1,
            export: i32::try_from(i).unwrap() + 1,
        };
        s.placements.insert(key, p);
    }
    let st = DrawPlan::build(&s).stats;
    assert_eq!(
        (
            st.untextured_no_material,
            st.untextured_unknown_material,
            st.untextured_no_diffuse
        ),
        (1, 1, 1)
    );
}

#[test]
fn a_mesh_with_no_vertex_or_an_index_past_them_is_refused() {
    let SceneMesh {
        status: MeshStatus::Decoded(asset),
        ..
    } = mesh(1)
    else {
        panic!("a decoded mesh");
    };
    let lod = &asset.lods[0];
    assert!(MeshData::from_lod("M", lod).is_some());
    let mut past = lod.clone();
    past.indices[2] = 3;
    assert!(MeshData::from_lod("M", &past).is_none());
    let mut empty = lod.clone();
    empty.vertices.clear();
    empty.indices.clear();
    empty.sections.clear();
    assert!(MeshData::from_lod("M", &empty).is_none());
}

/// A library over `scene()`'s materials: `Mats.Base` translucent with no named diffuse but a fallback texture, and
/// `Mats.Glass` the sky.
fn library(scene: &Scene) -> MaterialLibrary {
    use crate::material::{MaterialInfo, SkyInputs};
    let info = |blend, diffuse, sky| MaterialInfo {
        base: None,
        blend_name: None,
        blend,
        lighting_model: None,
        two_sided: Some(false),
        opacity_mask_clip: 0.5,
        cooked_textures: Vec::new(),
        expression_classes: Vec::new(),
        scalars: Vec::new(),
        diffuse,
        sky,
        error: None,
    };
    let mut l = MaterialLibrary::default();
    let _ = scene;
    l.materials.insert(
        "Mats.Base".into(),
        info(Blend::Translucent, DiffuseSource::Named, None),
    );
    let sky = SkyInputs {
        strip: "Tex.Strip".into(),
        clouds: "Tex.Clouds".into(),
        time_of_day: 128.0,
        sky_brightness: 1.0,
        cloud_brightness: 1.0,
        cloud_cap_opacity: 1.0,
    };
    l.materials.insert(
        "Mats.Glass".into(),
        info(Blend::Opaque, DiffuseSource::Named, Some(sky)),
    );
    l
}

#[test]
fn a_library_gives_each_slot_its_blend_and_the_sky_its_inputs() {
    let s = scene();
    let plan = DrawPlan::build_with(&s, Some(&library(&s)));
    let st = plan.stats;
    // Three Meshes.A instances of the translucent Mats.Base, one Meshes.B instance of the sky.
    assert_eq!(st.blend_count(Blend::Translucent), 3);
    assert_eq!((st.blend_count(Blend::Opaque), st.sky_instances), (0, 1));
    let base = &plan.slots[0];
    assert_eq!(
        (base.blend, base.clip, base.sky.is_some()),
        (Blend::Translucent, 0.5, false)
    );
    assert!(
        plan.slots[1]
            .sky
            .as_ref()
            .is_some_and(|k| k.column() == 0.5)
    );
    // Without a library: opaque, named only, as stage 2.
    let plain = DrawPlan::build(&s);
    assert_eq!(plain.stats.blend_count(Blend::Opaque), 4);
}

#[test]
fn a_fallback_texture_or_constant_fills_a_material_with_no_named_diffuse() {
    use crate::material::MaterialInfo;
    let mut s = scene();
    s.materials
        .insert("Mats.Plain".into(), material(vec![], Some(false)));
    s.materials
        .insert("Mats.Black".into(), material(vec![], Some(false)));
    let rows = [
        placement(
            "Meshes.A",
            attached([0.0; 3], [1.0; 3]),
            false,
            vec![Some("Mats.Plain".into())],
        ),
        placement(
            "Meshes.A",
            attached([0.0; 3], [1.0; 3]),
            false,
            vec![Some("Mats.Black".into())],
        ),
    ];
    for (i, p) in rows.into_iter().enumerate() {
        let key = ExportKey {
            level: 2,
            export: i32::try_from(i).unwrap() + 1,
        };
        s.placements.insert(key, p);
    }
    let named_only = DrawPlan::build(&s).stats;
    assert_eq!(named_only.untextured_no_diffuse, 2);
    let mut l = MaterialLibrary::default();
    let info = |diffuse| MaterialInfo {
        base: None,
        blend_name: None,
        blend: Blend::Opaque,
        lighting_model: None,
        two_sided: None,
        opacity_mask_clip: crate::material::DEFAULT_CLIP,
        cooked_textures: Vec::new(),
        expression_classes: Vec::new(),
        scalars: Vec::new(),
        diffuse,
        sky: None,
        error: None,
    };
    l.materials.insert(
        "Mats.Plain".into(),
        info(DiffuseSource::CookedDif("Tex.Plain_Dif".into())),
    );
    l.materials.insert(
        "Mats.Black".into(),
        info(DiffuseSource::Constant([0.0, 0.0, 0.0])),
    );
    let st = DrawPlan::build_with(&s, Some(&l)).stats;
    assert_eq!(
        (
            st.untextured_no_diffuse,
            st.fallback_textured,
            st.constant_coloured
        ),
        (0, 1, 1)
    );
}

#[test]
fn a_translucent_material_with_nothing_to_draw_is_not_batched() {
    use crate::material::MaterialInfo;
    let mut s = scene();
    s.materials
        .insert("Mats.Water".into(), material(vec![], Some(false)));
    let key = ExportKey {
        level: 2,
        export: 1,
    };
    s.placements.insert(
        key,
        placement(
            "Meshes.A",
            attached([0.0; 3], [1.0; 3]),
            false,
            vec![Some("Mats.Water".into())],
        ),
    );
    let mut l = MaterialLibrary::default();
    l.materials.insert(
        "Mats.Water".into(),
        MaterialInfo {
            base: None,
            blend_name: Some("BLEND_Translucent".into()),
            blend: Blend::Translucent,
            lighting_model: None,
            two_sided: None,
            opacity_mask_clip: crate::material::DEFAULT_CLIP,
            cooked_textures: Vec::new(),
            expression_classes: Vec::new(),
            scalars: Vec::new(),
            diffuse: DiffuseSource::None,
            sky: None,
            error: None,
        },
    );
    let before = DrawPlan::build(&s);
    let plan = DrawPlan::build_with(&s, Some(&l));
    let st = plan.stats;
    assert_eq!((st.translucent_skipped, st.untextured_no_diffuse), (1, 1));
    assert_eq!(st.instances + 1, before.stats.instances);
    let water = plan
        .slots
        .iter()
        .position(|m| m.path == "Mats.Water")
        .map(|i| u32::try_from(i).unwrap());
    assert!(water.is_some());
    assert!(plan.batches.iter().all(|b| b.key.material != water));
}

#[test]
fn blended_instances_are_ordered_far_to_near() {
    let items = [
        (0, 0, [10.0, 0.0, 0.0]),
        (0, 1, [300.0, 0.0, 0.0]),
        (1, 0, [-50.0, 0.0, 0.0]),
        (2, 0, [0.0, 300.0, 0.0]),
    ];
    // (0,1) and (2,0) tie at 300: batch order breaks the tie.
    assert_eq!(
        back_to_front(&items, [0.0; 3]),
        [(0, 1), (2, 0), (1, 0), (0, 0)]
    );
}

#[test]
fn the_diffuse_is_picked_by_parameter_name() {
    // A child's parameter replaces its parent's of the same name; the folded name is the key.
    let m = material(
        vec![
            texture("Tex.Child", Some("P_DIFFUSE"), 0),
            texture("Tex.Parent", Some("p_Diffuse"), 1),
        ],
        None,
    );
    assert_eq!(pick_diffuse(&m).as_deref(), Some("Tex.Child"));
    // A real texture replaces a stub; a stub never replaces a real one.
    let m = material(
        vec![
            texture("Common_Textures.Stub.StubGray", Some("p_Diffuse"), 1),
            texture("Tex.Real", Some("Diffuse_A"), 0),
        ],
        None,
    );
    assert_eq!(pick_diffuse(&m).as_deref(), Some("Tex.Real"));
    // Higher priority wins between real textures: p_Diffuse (100) over Diffuse_A (90), whatever the order.
    let m = material(
        vec![
            texture("Tex.Layer", Some("Diffuse_A"), 0),
            texture("Tex.Main", Some("p_Diffuse"), 0),
        ],
        None,
    );
    assert_eq!(pick_diffuse(&m).as_deref(), Some("Tex.Main"));
    // Aliases count; other channels and unnamed entries do not.
    let m = material(
        vec![
            texture("Tex.Normal", Some("p_Normal"), 0),
            texture("Tex.Unnamed", None, 0),
            texture("Tex.Alias", Some("Tex_Diff"), 0),
        ],
        None,
    );
    assert_eq!(pick_diffuse(&m).as_deref(), Some("Tex.Alias"));
    let m = material(vec![texture("Tex.Normal", Some("p_Normal"), 0)], None);
    assert_eq!(pick_diffuse(&m).as_deref(), None);
    assert_eq!(
        channel_for_parameter("EmissiveOpacityInput"),
        Some(Channel::Emissive)
    );
    assert_eq!(channel_for_parameter("p_Mask"), None);
}

/// Sanctuary's draw plan on the installed game (`OW_BL2_ROOT`): counts for the report, and the winding data check of
/// decision 51 (does the index order of a LOD 0 triangle run against its stored vertex normals?). Writes only
/// `local/p2/h2-draw/plan.md`. Run with
/// `cargo test --release -p ow-host --locked -- --ignored sanctuary_plan_on_the_installed_game`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h2-draw/"]
fn sanctuary_plan_on_the_installed_game() {
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use std::fmt::Write as _;
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let install = crate::find_install(std::path::Path::new(&root)).expect("an install");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h2-draw");
    std::fs::create_dir_all(&out).expect("the output folder");
    let mut defs = DefStore::open(
        install.cooked.clone(),
        IdentityPolicy::FirstInScanOrder,
        ow_core::ParityMode::Off,
    )
    .expect("the store");
    let mut space = ow_object::heap::ObjectSpace::new();
    let map = ow_core::PackageName::parse("Sanctuary_P").unwrap();
    let scene = Scene::load(
        &mut defs,
        &mut space,
        &SceneAssets::new(install.cooked.clone()),
        &map,
    )
    .expect("the scene");
    let plan = DrawPlan::build(&scene);

    // Winding: the face normal of the index order against the sum of the three stored normals; 0 against, 1
    // along, 2 flat. Unique triangles once each; weighted, each batch's section times its instances.
    let winding = |m: &MeshData, t: &[u32]| -> usize {
        let v = |i: u32| &m.vertices[i as usize];
        let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
        let e1 = [0, 1, 2].map(|k| f64::from(b.position[k]) - f64::from(a.position[k]));
        let e2 = [0, 1, 2].map(|k| f64::from(c.position[k]) - f64::from(a.position[k]));
        let cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let n = [0, 1, 2].map(|k| f64::from(a.normal[k] + b.normal[k] + c.normal[k]));
        let dot = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2];
        if dot < 0.0 {
            0
        } else if dot > 0.0 {
            1
        } else {
            2
        }
    };
    let mut unique = [0u64; 3];
    for m in &plan.meshes {
        for t in m.indices.chunks_exact(3) {
            unique[winding(m, t)] += 1;
        }
    }
    let (against, along, flat) = (unique[0], unique[1], unique[2]);
    let mut weighted = [0u64; 3];
    for b in &plan.batches {
        let m = &plan.meshes[b.key.mesh as usize];
        let range = m.sections[b.key.section as usize];
        let first = range.first_index as usize;
        let section = &m.indices[first..first + range.index_count as usize];
        for t in section.chunks_exact(3) {
            weighted[winding(m, t)] += b.instances.len() as u64;
        }
    }
    let s = plan.stats;
    let mut r = String::new();
    let _ = writeln!(r, "# H2 draw plan: Sanctuary_P (local, counts only)");
    let _ = writeln!(
        r,
        "placements {} hidden {} no_mesh {} bad_transform {} drawn {} mirrored {}",
        s.placements, s.hidden, s.no_mesh, s.bad_transform, s.drawn, s.mirrored
    );
    let _ = writeln!(
        r,
        "meshes {} material slots {} batches {} section instances {} empty sections {}",
        plan.meshes.len(),
        plan.slots.len(),
        plan.batches.len(),
        s.instances,
        s.empty_sections
    );
    let by = |f: &dyn Fn(&BatchKey) -> bool| plan.batches.iter().filter(|b| f(&b.key)).count();
    let _ = writeln!(
        r,
        "batches: mirrored {}, two-sided {}, untextured {}",
        by(&|k| k.mirrored),
        by(&|k| k.two_sided),
        by(&|k| k.material.is_none())
    );
    let mut groups = ow_core::DetSet::new();
    for b in &plan.batches {
        groups.insert((b.key.mesh, b.key.material));
    }
    let _ = writeln!(r, "distinct (mesh, texture) pairs {}", groups.len());
    let mut meshes = ow_core::DetSet::new();
    for b in &plan.batches {
        meshes.insert(b.key.mesh);
    }
    let _ = writeln!(r, "distinct meshes drawn {}", meshes.len());
    let materials_with_diffuse = scene
        .materials
        .values()
        .filter(|m| pick_diffuse(m).is_some())
        .count();
    let _ = writeln!(
        r,
        "materials {} with a named diffuse {}",
        scene.materials.len(),
        materials_with_diffuse
    );
    let _ = writeln!(
        r,
        "winding (unique LOD 0 triangles): against the stored normals {against}, along {along}, flat {flat}"
    );
    let _ = writeln!(
        r,
        "winding (weighted by drawn section instances): against {}, along {}, flat {}",
        weighted[0], weighted[1], weighted[2]
    );
    // Other batch keys, for comparing with B2's 784 instanced draws (its grouping is not on this PC): by effective
    // material instead of diffuse texture, and by mesh section alone.
    let mut by_material = ow_core::DetSet::new();
    let mut by_section = ow_core::DetSet::new();
    let mut by_mesh_material = ow_core::DetSet::new();
    for p in scene.placements.values() {
        if p.is_hidden() {
            continue;
        }
        let Some(affine) = world_affine(&p.pose) else {
            continue;
        };
        let effective = scene.effective_materials(p);
        for (slot, m) in effective.iter().enumerate() {
            by_material.insert((p.mesh.clone(), slot, m.clone(), affine.mirrors()));
            by_section.insert((p.mesh.clone(), slot));
            by_mesh_material.insert((p.mesh.clone(), m.clone()));
        }
    }
    let _ = writeln!(
        r,
        "other keys: (mesh, section, material, mirrored) {}, (mesh, section) {}, (mesh, material) {}",
        by_material.len(),
        by_section.len(),
        by_mesh_material.len()
    );
    std::fs::write(out.join("plan.md"), &r).expect("plan.md");
    print!("{r}");
}
