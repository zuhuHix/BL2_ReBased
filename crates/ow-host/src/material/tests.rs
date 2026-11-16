//! Owner: lane H8-MATERIALS. Tests of the material reads on invented values (AGENTS.md rule 9); the last test is the
//! installed-game driver, `#[ignore]`d, writing only under `local/p2/h8-materials/`.

use super::*;
use crate::scene::{MaterialStatus, MaterialTexture, TextureSource};
use std::fmt::Write as _;

fn words(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn a_cooked_resource_prefix_gives_its_texture_references() {
    // Two empty arrays, a resource int, a 16-byte GUID, a resource int, two references, then opaque bytes.
    let mut tail = words(&[0, 0, 7, 1, 2, 3, 4, 9, 2, -5, 12]);
    tail.extend([0xAB; 10]);
    assert_eq!(cooked_texture_references(&tail), Ok(vec![-5, 12]));
    let mut errors = words(&[1, 0, 7, 1, 2, 3, 4, 9, 0]);
    assert!(cooked_texture_references(&errors).is_err());
    errors = words(&[0, 0, 7, 1, 2, 3, 4, 9, 3, -5]);
    assert_eq!(
        cooked_texture_references(&errors),
        Err("invalid cooked material texture count".to_string())
    );
    assert!(cooked_texture_references(&words(&[0, 0, 7])).is_err());
}

#[test]
fn texture_names_follow_the_tools_suffix_rules() {
    for (name, diffuse) in [
        ("Pkg.Tex.Wall_Dif", true),
        ("Pkg.Tex.Wall_DIFF", true),
        ("Pkg.Tex.Wall_Dif_04", true),
        ("Pkg.Tex.Wall_Diffuse", false),
        ("Pkg.Tex.Wall_N", false),
    ] {
        assert_eq!(is_diffuse_name(name), diffuse, "{name}");
    }
    for (name, auxiliary) in [
        ("Pkg.Tex.Wall_N", true),
        ("Pkg.Tex.Wall_Spec_2", true),
        ("Pkg.Tex.CloudNoise", true),
        ("Pkg.Tex.Wall_Base", false),
        ("Pkg.Tex.Wall_Dif", false),
    ] {
        assert_eq!(is_auxiliary_name(name), auxiliary, "{name}");
    }
    assert!(is_unnamed_sample(
        "MaterialExpressionTextureSampleParameter2D_12"
    ));
    assert!(!is_unnamed_sample(
        "MaterialExpressionTextureSampleParameter2D_"
    ));
    assert!(!is_unnamed_sample("p_Diffuse"));
}

fn texture(path: &str, parameter: Option<&str>, depth: u32) -> MaterialTexture {
    MaterialTexture {
        path: path.to_string(),
        parameter: parameter.map(str::to_string),
        from: TextureSource::Expression,
        depth,
    }
}

fn scene_material(textures: Vec<MaterialTexture>) -> SceneMaterial {
    SceneMaterial {
        class: "Engine.Material".into(),
        parent: None,
        textures,
        two_sided: None,
        status: MaterialStatus::Read,
    }
}

fn info(blend: Blend, cooked: &[&str]) -> MaterialInfo {
    MaterialInfo {
        blend,
        cooked_textures: cooked.iter().map(|s| s.to_string()).collect(),
        ..MaterialInfo::empty()
    }
}

#[test]
fn a_material_with_no_named_diffuse_takes_the_tools_fallbacks_in_order() {
    let planar = |_: &str| Some("Engine.Texture2D".to_string());
    // A named diffuse wins over everything.
    let named = scene_material(vec![texture("T.Named", Some("p_Diffuse"), 0)]);
    assert_eq!(
        diffuse_source(&named, &info(Blend::Opaque, &["T.Other_Dif"]), &planar),
        DiffuseSource::Named
    );
    // A sole unnamed sample parameter whose texture ends in _Dif.
    let unnamed = scene_material(vec![
        texture(
            "T.Wall_Dif_02",
            Some("MaterialExpressionTextureSampleParameter2D_4"),
            0,
        ),
        texture(
            "T.Wall_N",
            Some("MaterialExpressionTextureSampleParameter2D_5"),
            0,
        ),
    ]);
    assert_eq!(
        diffuse_source(&unnamed, &info(Blend::Opaque, &[]), &planar),
        DiffuseSource::Unnamed("T.Wall_Dif_02".into())
    );
    // The cooked list: a sole _Dif texture, whatever the blend.
    let none = scene_material(vec![]);
    assert_eq!(
        diffuse_source(
            &none,
            &info(Blend::Translucent, &["T.A_Dif", "T.A_N"]),
            &planar
        ),
        DiffuseSource::CookedDif("T.A_Dif".into())
    );
    // Two _Dif textures: no guess.
    assert_eq!(
        diffuse_source(
            &none,
            &info(Blend::Opaque, &["T.A_Dif", "T.B_Dif"]),
            &planar
        ),
        DiffuseSource::None
    );
    // A sole plain texture, for opaque and masked materials only.
    assert_eq!(
        diffuse_source(
            &none,
            &info(Blend::Masked, &["T.Leaves", "T.Leaves_N"]),
            &planar
        ),
        DiffuseSource::CookedSole("T.Leaves".into())
    );
    assert_eq!(
        diffuse_source(
            &none,
            &info(Blend::Translucent, &["T.Leaves", "T.Leaves_N"]),
            &planar
        ),
        DiffuseSource::None
    );
    // A cube map is not a planar texture.
    let cube = |_: &str| Some("Engine.TextureCube".to_string());
    assert_eq!(
        diffuse_source(&none, &info(Blend::Opaque, &["T.Env_Dif"]), &cube),
        DiffuseSource::None
    );
}

#[test]
fn blend_names_and_the_sky_inputs() {
    assert_eq!(Blend::from_name("BLEND_Masked"), Some(Blend::Masked));
    assert_eq!(Blend::from_name("BLEND_SoftMasked"), None);
    assert!(Blend::Masked.writes_depth() && !Blend::Additive.writes_depth());
    let sky = scene_material(vec![
        texture("T.Strip_Base", Some("Transition_Track"), 1),
        texture("T.Strip_Override", Some("Transition_Track"), 0),
        texture("T.Clouds", Some("clouds"), 1),
    ]);
    let mut i = MaterialInfo::empty();
    i.scalars = vec![
        ("Time_of_Day".into(), 64.0),
        ("sky_brightness".into(), 1.5),
        ("p_CouldBrightness".into(), 0.5),
        ("cloud_cap_opacity".into(), 1.0),
    ];
    let inputs = sky_inputs(&sky, &i).unwrap();
    assert_eq!(
        (inputs.strip.as_str(), inputs.clouds.as_str()),
        ("T.Strip_Override", "T.Clouds")
    );
    assert_eq!(
        (
            inputs.column(),
            inputs.sky_brightness,
            inputs.cloud_brightness
        ),
        (0.25, 1.5, 0.5)
    );
    i.scalars[0].1 = 300.0;
    assert!(
        sky_inputs(&sky, &i).is_none(),
        "a time past the strip is refused"
    );
}

// ------------------------------------------------------------------------------------------ installed driver

/// Sanctuary's materials and lights on the installed game (`OW_BL2_ROOT`): counts by diffuse source, blend mode and
/// lighting model, expression classes, the lights and the lightmap census. Writes `local/p2/h8-materials/report.md`
/// and `materials.tsv` (game-derived, ignored). Run with
/// `cargo test --release -p ow-host --locked -- --ignored sanctuary_materials_on_the_installed_game`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h8-materials/"]
fn sanctuary_materials_on_the_installed_game() {
    use crate::lighting::SceneLights;
    use crate::scene::SceneAssets;
    use ow_defs::identity::IdentityPolicy;
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let install = crate::find_install(std::path::Path::new(&root)).expect("an install");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h8-materials");
    std::fs::create_dir_all(&out).expect("the output folder");
    let mut defs = DefStore::open(
        install.cooked.clone(),
        IdentityPolicy::FirstInScanOrder,
        ow_core::ParityMode::Off,
    )
    .expect("the store");
    let mut space = ObjectSpace::new();
    let map = ow_core::PackageName::parse("Sanctuary_P").unwrap();
    let scene = Scene::load(
        &mut defs,
        &mut space,
        &SceneAssets::new(install.cooked.clone()),
        &map,
    )
    .expect("the scene");
    let started = std::time::Instant::now();
    let library = MaterialLibrary::read(&mut defs, &mut space, &scene);
    let read_ms = started.elapsed().as_millis();
    let lights = SceneLights::read(&mut defs, &mut space, &scene);

    let mut r = String::new();
    let _ = writeln!(r, "# H8 materials: Sanctuary_P (local, counts only)");
    let _ = writeln!(r, "material read {read_ms} ms");
    let _ = writeln!(r, "materials {}", library.materials.len());
    let mut tally: ow_core::DetMap<(String, String), usize> = ow_core::DetMap::new();
    let mut tsv = String::new();
    for (path, m) in library.materials.iter() {
        for (what, key) in [
            ("diffuse", m.diffuse.label().to_string()),
            (
                "blend",
                m.blend_name.clone().unwrap_or_else(|| "(none)".into()),
            ),
            (
                "lighting",
                m.lighting_model.clone().unwrap_or_else(|| "(none)".into()),
            ),
            ("two_sided", format!("{:?}", m.two_sided)),
            (
                "error",
                m.error.clone().map_or("none".into(), |_| "some".into()),
            ),
            ("sky", m.sky.is_some().to_string()),
        ] {
            *tally.entry((what.to_string(), key)).or_insert(0) += 1;
        }
        for class in &m.expression_classes {
            *tally
                .entry(("expression".into(), class.clone()))
                .or_insert(0) += 1;
        }
        let _ = writeln!(
            tsv,
            "{path}\t{}\t{}\t{}\t{}\t{}",
            m.diffuse.label(),
            m.diffuse.texture().unwrap_or("-"),
            m.blend_name.as_deref().unwrap_or("-"),
            m.lighting_model.as_deref().unwrap_or("-"),
            m.error.as_deref().unwrap_or("-")
        );
    }
    for ((what, key), n) in &tally {
        let _ = writeln!(r, "  {what}: {key}: {n}");
    }
    let errors: Vec<&str> = library
        .materials
        .values()
        .filter_map(|m| m.error.as_deref())
        .collect();
    for e in errors.iter().take(10) {
        let _ = writeln!(r, "  error example: {e}");
    }
    let _ = writeln!(
        r,
        "textures named only by cooked lists: {}",
        library.textures.len()
    );
    if let Some(sky) = library.materials.values().find_map(|m| m.sky.as_ref()) {
        let _ = writeln!(
            r,
            "sky: strip {} clouds {} time {} column {} brightness {} cloud {} cap {}",
            sky.strip,
            sky.clouds,
            ow_core::fmt::fmt_py_repr(f64::from(sky.time_of_day)),
            ow_core::fmt::fmt_py_repr(f64::from(sky.column())),
            ow_core::fmt::fmt_py_repr(f64::from(sky.sky_brightness)),
            ow_core::fmt::fmt_py_repr(f64::from(sky.cloud_brightness)),
            ow_core::fmt::fmt_py_repr(f64::from(sky.cloud_cap_opacity))
        );
    }
    let f = |v: f32| ow_core::fmt::fmt_py_repr(f64::from(v));
    match &lights.sun {
        Some(s) => {
            let _ = writeln!(
                r,
                "sun ({} found): {}:{} rotation {} {} {} colour {} {} {} brightness {} tags at {} toward {} {} {}",
                lights.suns,
                s.level,
                s.path,
                s.rotation.pitch,
                s.rotation.yaw,
                s.rotation.roll,
                f(s.color[0]),
                f(s.color[1]),
                f(s.color[2]),
                f(s.brightness),
                s.tags_at,
                f(s.toward()[0]),
                f(s.toward()[1]),
                f(s.toward()[2])
            );
        }
        None => {
            let _ = writeln!(r, "sun: none ({} found)", lights.suns);
        }
    }
    match &lights.sky {
        Some(s) => {
            let _ = writeln!(
                r,
                "sky light ({} found): {}:{} colour {} {} {} x {} lower {} {} {} x {}",
                lights.skies,
                s.level,
                s.path,
                f(s.color[0]),
                f(s.color[1]),
                f(s.color[2]),
                f(s.brightness),
                f(s.lower_color[0]),
                f(s.lower_color[1]),
                f(s.lower_color[2]),
                f(s.lower_brightness)
            );
        }
        None => {
            let _ = writeln!(r, "sky light: none ({} found)", lights.skies);
        }
    }
    let l = &lights.lightmaps;
    let _ = writeln!(
        r,
        "lightmaps: LightMapTexture2D {} ShadowMap2D {} ShadowMap1D {} decodable {:?}",
        l.light_map_textures, l.shadow_maps_2d, l.shadow_maps_1d, l.decodable
    );
    for e in &lights.errors {
        let _ = writeln!(r, "  light error: {e}");
    }
    // Per material slot of the plan: its section instances and the nearest instance origin's distance from the first
    // player start, to tell which slots draw what the player-start screenshot shows.
    let plan = crate::draw::DrawPlan::build_with(&scene, Some(&library));
    let start = scene
        .player_starts
        .values()
        .next()
        .map_or([0.0; 3], |p| p.pose.location);
    let mut per_slot: Vec<(usize, f32)> = vec![(0, f32::MAX); plan.slots.len()];
    for batch in &plan.batches {
        let Some(i) = batch.key.material else {
            continue;
        };
        let Some(slot) = per_slot.get_mut(i as usize) else {
            continue;
        };
        for instance in &batch.instances {
            let o = instance.world.cols[3];
            let squared: f64 = (0..3)
                .map(|k| f64::from(o[k] - start[k]) * f64::from(o[k] - start[k]))
                .sum();
            let d = ow_core::num::sqrt_f64(squared) as f32;
            slot.0 += 1;
            slot.1 = slot.1.min(d);
        }
    }
    let mut slots_tsv = String::from(
        "path	diffuse	first texture	constant	blend	unlit	sky	instances	nearest uu	holder
",
    );
    for (slot, (n, d)) in plan.slots.iter().zip(&per_slot) {
        let label = library
            .materials
            .get(slot.path.as_str())
            .map_or("-", |m| m.diffuse.label());
        let holder = slot
            .diffuse
            .first()
            .and_then(|c| library.texture(&scene, &c.path))
            .and_then(|t| t.holder)
            .map_or("-".to_string(), |h| format!("{} {}", h.file, h.export));
        let _ = writeln!(
            slots_tsv,
            "{}	{label}	{}	{}	{:?}	{}	{}	{n}	{}	{holder}",
            slot.path,
            slot.diffuse.first().map_or("-", |c| c.path.as_str()),
            slot.constant.is_some(),
            slot.blend,
            slot.unlit,
            slot.sky.is_some(),
            f(*d)
        );
    }
    std::fs::write(out.join("slots.tsv"), &slots_tsv).expect("slots.tsv");
    std::fs::write(out.join("report.md"), &r).expect("report.md");
    std::fs::write(out.join("materials.tsv"), &tsv).expect("materials.tsv");
    print!("{r}");
}
