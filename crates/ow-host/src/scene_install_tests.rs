//! Owner: lane H2-SCENE (moved by lane LINK-2-HEADLESS-MOVER, decision 2026-11-19-66). The scene loader's two
//! installed-game drivers, `#[ignore]`d: they read `OW_BL2_ROOT` and write their reports under ignored `local/p2/`,
//! never into a tracked file. The loader moved to `ow_assets::scene`; these stay here because they use `ow-host`'s
//! install, material and lighting modules.

use ow_assets::scene::*;
use ow_core::fmt::fmt_py_repr;
use ow_core::{DetMap, DetSet, OrderedMap, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use std::fmt::Write as _;
use std::path::Path;

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

/// A `fmt::Write` sink that keeps only the FNV-1a 64 digest and the length of what is written.
struct Digest {
    hash: u64,
    bytes: u64,
}

impl Digest {
    fn of(value: &dyn std::fmt::Debug) -> (u64, u64) {
        let mut d = Digest {
            hash: 0xcbf2_9ce4_8422_2325,
            bytes: 0,
        };
        let _ = write!(d, "{value:?}");
        (d.hash, d.bytes)
    }
}

impl std::fmt::Write for Digest {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for b in s.bytes() {
            self.hash ^= u64::from(b);
            self.hash = self.hash.wrapping_mul(0x0100_0000_01b3);
        }
        self.bytes += s.len() as u64;
        Ok(())
    }
}

/// Lane FIX-SCENE-LOAD: Sanctuary's load times, and digests of everything the loads answer (the whole `Debug` output
/// of the scene, the material library and the lights), so that a speed-up can be shown to change no answer. Writes
/// `local/p2/scene-load/<OW_RUN_TAG or "run">.txt`. Run with
/// `cargo test --release -p ow-host --locked -- --ignored sanctuary_load_digest_on_the_installed_game --nocapture`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/scene-load/"]
fn sanctuary_load_digest_on_the_installed_game() {
    use crate::lighting::SceneLights;
    use crate::material::MaterialLibrary;
    let Some(root) = std::env::var_os("OW_BL2_ROOT") else {
        panic!("set OW_BL2_ROOT to the Borderlands 2 install folder");
    };
    let install = crate::find_install(Path::new(&root)).expect("an install");
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/scene-load");
    std::fs::create_dir_all(&out).expect("the output folder");
    let tag = std::env::var("OW_RUN_TAG").unwrap_or_else(|_| "run".into());
    let started = std::time::Instant::now();
    let mut defs = DefStore::open(
        install.cooked.clone(),
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .expect("the store");
    let opened = started.elapsed();
    let mut space = ObjectSpace::new();
    let assets = SceneAssets::new(install.cooked.clone());
    let map = PackageName::parse("Sanctuary_P").unwrap();
    let scene = Scene::load(&mut defs, &mut space, &assets, &map).expect("the scene");
    let loaded = started.elapsed();
    let library = MaterialLibrary::read(&mut defs, &mut space, &scene);
    let materials = started.elapsed();
    let lights = SceneLights::read(&mut defs, &mut space, &scene);
    let lit = started.elapsed();
    let ms = |d: std::time::Duration| d.as_millis();
    let mut r = String::new();
    let _ = writeln!(
        r,
        "# Sanctuary load ({tag}; local, timings and digests only)"
    );
    let _ = writeln!(
        r,
        "store open {} ms; Scene::load {} ms; MaterialLibrary::read {} ms; SceneLights::read {} ms; total {} ms",
        ms(opened),
        ms(loaded - opened),
        ms(materials - loaded),
        ms(lit - materials),
        ms(lit)
    );
    for (what, value) in [
        ("scene", &scene as &dyn std::fmt::Debug),
        ("materials", &library),
        ("lights", &lights),
    ] {
        let (hash, bytes) = Digest::of(value);
        let _ = writeln!(r, "digest {what}: {hash:016x} over {bytes} bytes");
    }
    std::fs::write(out.join(format!("{tag}.txt")), &r).expect("the report");
    print!("{r}");
}
