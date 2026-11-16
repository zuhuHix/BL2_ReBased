//! Owner: lane H9-LIGHTMAPS. Tests of the light map tail parser on invented bytes (AGENTS.md rule 9): every count,
//! reference, scale and sample is made up.

use super::*;

/// A tail under construction, with the absolute position it will sit at.
struct Tail {
    bytes: Vec<u8>,
    at: usize,
}

impl Tail {
    fn new(at: usize) -> Tail {
        Tail {
            bytes: Vec::new(),
            at,
        }
    }
    fn word(&mut self, v: i32) -> &mut Tail {
        self.bytes.extend(v.to_le_bytes());
        self
    }
    fn float(&mut self, v: f32) -> &mut Tail {
        self.bytes.extend(v.to_le_bytes());
        self
    }
    fn vector(&mut self, v: [f32; 3]) -> &mut Tail {
        v.iter().for_each(|f| {
            self.float(*f);
        });
        self
    }
    fn byte(&mut self, b: u8) -> &mut Tail {
        self.bytes.push(b);
        self
    }
    /// A bulk array of 8-byte samples whose offset word is `shift` away from the data's real position.
    fn bulk(&mut self, samples: &[[u8; 8]], shift: i32) -> &mut Tail {
        let n = i32::try_from(samples.len()).unwrap();
        self.word(0).word(n).word(n * 8);
        let data_at = i32::try_from(self.at + self.bytes.len() + 4).unwrap();
        self.word(data_at + shift);
        samples.iter().for_each(|s| self.bytes.extend(s));
        self
    }
}

/// One LOD with a texture light map: shadow map 5, two light GUIDs, textures 7 and 8.
fn texture_lod(t: &mut Tail) {
    t.word(1).word(5).word(0).word(2).word(2);
    t.bytes.extend([0xabu8; 32]);
    t.word(7).vector([1.0, 1.0, 1.0]);
    t.word(8).vector([4.5, 4.5, 4.5]);
    t.word(0).vector([0.0; 3]);
    t.float(0.25).float(0.5).float(0.125).float(0.0625).byte(0);
}

#[test]
fn a_texture_light_map_parses_to_its_fields() {
    let mut t = Tail::new(1000);
    t.word(1);
    texture_lod(&mut t);
    let lods = parse_lods(&t.bytes, t.at).unwrap();
    assert_eq!(lods.len(), 1);
    assert_eq!(
        (lods[0].shadow_maps.as_slice(), lods[0].override_colors),
        (&[5][..], None)
    );
    assert_eq!(
        lods[0].light_map,
        RawLightMap::Texture {
            guids: 2,
            textures: [7, 8, 0],
            scales: [[1.0; 3], [4.5; 3], [0.0; 3]],
            uv_scale: [0.25, 0.5],
            uv_bias: [0.125, 0.0625],
        }
    );
    // A component with no LODs is the one word; the smallest LOD is 13 bytes.
    assert_eq!(parse_lods(&0i32.to_le_bytes(), 0).unwrap(), Vec::new());
    let mut bare = Tail::new(0);
    bare.word(1).word(0).word(0).word(0).byte(0);
    assert_eq!(
        parse_lods(&bare.bytes, 0).unwrap()[0].light_map,
        RawLightMap::None
    );
}

#[test]
fn a_vertex_light_map_checks_its_bulk_offsets_and_override_colours() {
    let samples = [[1, 2, 3, 0, 4, 5, 6, 0], [7, 8, 9, 0, 10, 11, 12, 0]];
    let build = |shift: i32, second: &[[u8; 8]], flag: u8| {
        let mut t = Tail::new(5000);
        t.word(1).word(0).word(1).word(9).word(1).word(0).word(3);
        t.bulk(&samples, shift);
        t.vector([1.0; 3]).vector([2.0; 3]).vector([3.0; 3]);
        t.bulk(second, 0);
        t.byte(flag);
        if flag == 1 {
            t.word(4).word(2).word(4).word(2).word(-1).word(-2);
        }
        t
    };
    let t = build(0, &[], 1);
    let lods = parse_lods(&t.bytes, t.at).unwrap();
    assert_eq!(lods[0].shadow_vertex_buffers, vec![9]);
    assert_eq!(lods[0].override_colors, Some(2));
    match &lods[0].light_map {
        RawLightMap::Vertex {
            owner,
            samples: s,
            scales,
            ..
        } => {
            assert_eq!(
                (*owner, s.as_slice(), scales[1]),
                (3, &samples[..], [2.0; 3])
            );
        }
        other => panic!("{other:?}"),
    }
    // The offset word must be the data's own position.
    assert!(
        parse_lods(&build(4, &[], 0).bytes, 5000)
            .unwrap_err()
            .contains("is not its position")
    );
    // Read at another position, the same bytes do not check.
    assert!(parse_lods(&build(0, &[], 0).bytes, 5004).is_err());
    // A second sample array that is not empty is refused (Sanctuary has none to fit it to).
    assert!(
        parse_lods(&build(0, &samples, 0).bytes, 5000)
            .unwrap_err()
            .contains("second")
    );
    // A flag byte other than 0 or 1.
    assert!(
        parse_lods(&build(0, &[], 2).bytes, 5000)
            .unwrap_err()
            .contains("flag")
    );
}

#[test]
fn a_tail_must_be_consumed_exactly() {
    let mut t = Tail::new(0);
    t.word(1);
    texture_lod(&mut t);
    let mut longer = t.bytes.clone();
    longer.push(0);
    assert!(parse_lods(&longer, 0).unwrap_err().contains("left after"));
    let shorter = &t.bytes[..t.bytes.len() - 1];
    assert!(parse_lods(shorter, 0).unwrap_err().contains("ends before"));
    // A count past what is left, and an unknown light map type.
    assert!(
        parse_lods(&[2, 0, 0, 0], 0)
            .unwrap_err()
            .contains("past the tail")
    );
    let mut odd = Tail::new(0);
    odd.word(1).word(0).word(0).word(3).byte(0);
    assert!(parse_lods(&odd.bytes, 0).unwrap_err().contains("type 3"));
    // A non-finite scale.
    let mut nan = t.bytes.clone();
    let at = 4 * 7 + 32 + 4;
    nan[at..at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(parse_lods(&nan, 0).unwrap_err().contains("non-finite"));
}

#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h9-lightmaps/"]
fn sanctuary_lighting_on_the_installed_game() {
    use crate::lighting::SceneLights;
    use crate::scene::SceneAssets;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use std::fmt::Write as _;
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let install = crate::find_install(std::path::Path::new(&root)).expect("an install");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h9-lightmaps");
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
    let lights = SceneLights::read(&mut defs, &mut space, &scene);
    let guid = lights.sun.as_ref().and_then(|s| s.light_guid);
    let started = std::time::Instant::now();
    let lighting = SceneLighting::read(&defs, &scene, guid);
    let read_ms = started.elapsed().as_millis();
    let mut r = String::new();
    let _ = writeln!(r, "# H9 lighting: Sanctuary_P (local, counts only)");
    let _ = writeln!(r, "read {read_ms} ms; sun guid known: {}", guid.is_some());
    let _ = writeln!(r, "{:?}", lighting.census);
    let mut sets: ow_core::DetMap<usize, usize> = ow_core::DetMap::new();
    for v in lighting.uv_sets.values() {
        *sets.entry(*v).or_default() += 1;
    }
    let _ = writeln!(r, "meshes by LightMapCoordinateIndex: {sets:?}");
    let mut classes: ow_core::DetMap<String, (usize, usize)> = ow_core::DetMap::new();
    let mut failures = Vec::new();
    for (key, (class, _)) in lighting.textures.iter() {
        let e = classes.entry(class.clone()).or_default();
        match lighting.decode(&defs, &scene, key) {
            Ok(_) => e.0 += 1,
            Err(err) => {
                e.1 += 1;
                failures.push(format!("{key}: {err}"));
            }
        }
    }
    let _ = writeln!(
        r,
        "lighting textures (decoded, failed) by class: {classes:?}"
    );
    for f in failures.iter().take(10) {
        let _ = writeln!(r, "  decode failure: {f}");
    }
    for e in &lighting.errors {
        let _ = writeln!(r, "  refused: {e}");
    }
    // How the census's components become the plan's lit placements: hidden placements are not drawn.
    let plan = crate::draw::DrawPlan::build_lit(&scene, None, Some(&lighting));
    let (mut hidden_texture, mut hidden_shadowed) = (0usize, 0usize);
    for (key, p) in scene.placements.iter() {
        let Some(light) = lighting.components.get(key) else {
            continue;
        };
        if p.is_hidden() {
            hidden_texture +=
                usize::from(matches!(light.light_map, Some(LightMap::Texture { .. })));
            hidden_shadowed += usize::from(light.shadow.is_some());
        }
    }
    let st = plan.stats;
    let _ = writeln!(
        r,
        "plan: {} lit by a texture light map, {} per vertex, {} shadowed; hidden placements with a texture light \
         map {hidden_texture}, with a sun shadow map {hidden_shadowed}; not applied: {} light maps and {} shadow maps \
         without UVs, {} vertex mismatches",
        st.lit_texture,
        st.lit_vertex,
        st.shadowed,
        st.lightmap_no_uvs,
        st.shadow_no_uvs,
        st.lightmap_vertex_mismatch
    );
    std::fs::write(out.join("report.md"), &r).expect("report.md");
    print!("{r}");
    // Review r1 of PR #365: the run checks what decision 65 claims of Sanctuary.
    assert_eq!(lighting.census.refused, 0, "{:?}", lighting.errors);
    assert_eq!(lighting.census.uv_set_unread, 0, "{:?}", lighting.errors);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(classes.get("LightMapTexture2D"), Some(&(136, 0)));
    assert_eq!(classes.get("ShadowMapTexture2D").map(|c| c.1), Some(0));
    assert_eq!(
        st.lit_texture + hidden_texture + st.lightmap_no_uvs,
        lighting.census.texture_light_maps
    );
    assert_eq!(
        st.shadowed + hidden_shadowed + st.shadow_no_uvs,
        lighting.census.shadowed
    );
}
