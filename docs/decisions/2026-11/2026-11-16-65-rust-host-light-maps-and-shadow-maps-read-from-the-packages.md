## 2026-11-16: Rust host light maps and shadow maps read from the packages (H9)

AI-assisted (Claude Code). Lane H9-LIGHTMAPS. Decision 2026-11-16-58 left faces away from the sun nearly black: the
host had only the sun and a sky term. Decision 58 counted the levels' light maps but did not read them. This lane reads
each placed static mesh component's light map and the dominant light's shadow map from the packages, and decodes their
textures. The host now draws with them. The data path is checked against the installed packages. How the game combines
the data is **UNVERIFIED**, and so is the renderer's decode, which is a labelled fit. There was no real-game session.
No native code was read and no clean-room note was needed. No dependency was added, no bounds check was loosened, and
`ow-package` and `ow-container` are unchanged.

Edits: docs/decisions/2026-11/2026-11-16-58-rust-host-materials-blend-modes-the-sky-and-the-packages.md
(H8 review r2 low L1: "Neither was counted on Sanctuary" now says that neither difference changes a diffuse, since
none differs from the tool outside the 15 inspected materials, by the material-by-material comparison.)

- **What was added.**
  - `crates/ow-host/src/lightmap.rs` (+ tests):
    - `parse_lods`, the component tail parser;
    - `SceneLighting::read`, each placement's light map and sun shadow map, and each mesh's
      `LightMapCoordinateIndex`;
    - `SceneLighting::decode` for the lighting textures.
  - `crates/ow-assets/src/texture.rs`: `read_lighting_texture` for `LightMapTexture2D` and `ShadowMapTexture2D`.
    - It reads them as a `Texture2D`'s payload, then requires the payload to end exactly at the class's trailer.
    - `read_texture`, the C++ port, is unchanged and still refuses both classes, so its parity with `src/assets.cpp`
      holds.
    - Two new error texts.
  - `draw.rs`:
    - `DrawPlan::build_lit(scene, library, lighting)`, with `build_with` its call with no lighting;
    - each vertex carries the mesh's light-map UV set, and each instance an `InstanceLight`;
    - batches are split by `LightingSlot` (the atlas textures, the shadow atlas, or a component's per-vertex samples);
    - plan counters.
  - `render.rs`:
    - a third bind group (two coefficient atlases and the shadow atlas, uploaded linear, clamped sampler);
    - a per-vertex light buffer (a zero buffer for other draws);
    - the shader's indirect and shadow terms.
  - `lighting.rs`:
    - `Sun::light_guid`, which picks the shadow map;
    - the census now tries `read_lighting_texture`.
  - `main.rs`: the scene window and `--play` read the baked lighting and plan with it. Lighting textures go through the
    texture source under a `lightmap:<file>.<path>` key, because atlas names repeat across level files.

- **How the layout was found.** No record, tool, C++ reader or listing in this repository decodes light maps, so the
  layout was fitted to the installed packages. It was checked with structural oracles, as AGENTS.md rule 4 asks, over
  all ten Sanctuary level files. A local probe (not committed) printed tags, tails and words, and the checks below were
  run on everything.
  - **`LightMapTexture2D`** reads as a `Texture2D`: the same tags, 16 bytes, the mip count and 24-byte mip records,
    inline or streamed from the `Lighting` texture cache.
    - After the records, all 136 `LightMapTexture2D` exports have 40 bytes.
    - All 945 `Texture2D` and 65 `ShadowMapTexture2D` exports whose records parsed have 36.
    - So a light map is a `Texture2D` with one more 32-bit word; its meaning is not read.
    - All 136 light maps and 65 shadow textures decode: DXT1 and G8.
  - **A `StaticMeshComponent`'s untagged tail**, after its tags at the component prefix, is:
    - a count of LODs. For each LOD:
      - an array of shadow-map references (`ShadowMap2D`);
      - an array of shadow-vertex-buffer references (`ShadowMap1D`);
      - a light-map type word: 0 none, 1 per vertex, 2 texture;
      - for types 1 and 2, an array of 16-byte light GUIDs;
    - **type 2:** three coefficient slots, each a texture reference and a 3-float scale, then a UV scale and a UV bias
      (2 floats each);
    - **type 1:** a reference to the component itself, a bulk array of 8-byte samples (two `FColor`s per vertex),
      three 3-float scales, and a second bulk array, empty on Sanctuary. A bulk array is a flags word, a count, a byte
      size, the absolute file offset of its data, then the data;
    - then a flag byte, 1 when per-vertex override colours follow. Those are a stride of 4, a count, an element size
      of 4, the count again, and 4 bytes per vertex.
  - **The oracles:**
    - all 4,890 `StaticMeshComponent` tails in the ten files are consumed exactly to their export's end;
    - all 467 type-1 bulk offsets equal their data's own position in the file;
    - every type-2 slot 0 and slot 1 names a `LightMapTexture2D`, and every slot 2 is empty (3,581 of each);
    - the 12 override-colour flags of 1 each meet a well-formed colour stream;
    - in the scene, all 4,446 placements' components parse (0 refused): 3,581 texture light maps, 462 per-vertex and
      403 with none;
    - every per-vertex light map's owner is the component itself, and its sample count equals its mesh's LOD 0 vertex
      count (0 mismatches).
  - **Shadow maps.**
    - A `ShadowMap2D` is fully tagged: `Texture` (a `ShadowMapTexture2D`), `CoordinateScale`, `CoordinateBias`,
      `LightGuid` and `bIsShadowFactorTexture`.
    - All 1,540 in the three main level files carry the `DominantDirectionalLight` component's own `LightGuid`.
    - All are serialized with `bIsShadowFactorTexture` false; the class default is true.
    - Their G8 textures look like distance fields (smooth greys with edges at mid-grey). That reading is UNVERIFIED.
    - The 142 `ShadowMap1D` (per-vertex shadows) are counted, not read.
  - **Light-map UVs.** `StaticMesh.LightMapCoordinateIndex` is 1 on 377 of Sanctuary's meshes, 2 on 2, and absent
    on 51. An absent one is taken as 0, the property's zero value; that is UNVERIFIED as the class default.
  - **What the data says, read from names and values only.**
    - Slot 0's textures are named `EnvironmentLightmap*` and always carry scale (1, 1, 1).
    - Slot 1's are named `DominantBouncemap*`, with a per-component scale (for example 5.46 and 14.25). The sun's
      `LightmassSettings.IndirectLightingScale` is 5.
    - So slot 0 holds environment (sky and static point) light and slot 1 bounced sun light. That reading is
      UNVERIFIED.
  - **UModel** (`-nolightmap` in `docs/TOOLING.md`) was not used as an oracle; nothing was copied from it.

- **The renderer's decode (a fit, UNVERIFIED).**
  - For an instance with a light map, the sky term is replaced by indirect light: coefficient 0 x scale 0 +
    coefficient 1 x scale 1.
    - For atlases, the texels are sampled at `lightmap_uv * uv_scale + uv_bias` and taken as linear (byte/255).
    - For a per-vertex light map, its two samples are interpolated, as linear B, G, R bytes.
  - The sun term is multiplied by the shadow map's visibility.
    - The shadow UV is `lightmap_uv * CoordinateScale + CoordinateBias`.
    - For a distance-field map, visibility is smoothstep(0.45, 0.55, texel): a host choice of half-width 0.05.
    - For a shadow-factor map, visibility is the texel.
  - The same exposure as decision 58 applies.
  - Instances with no light map (movers such as `InterpActor`s, and 403 components with none) keep the sky term. That
    is the meanwhile fallback ambient the lane brief allowed, also UNVERIFIED.
  - What is not read:
    - whether the atlases are gamma encoded;
    - how the game weights the two coefficients;
    - the game's distance-field penumbra;
    - the shadow vertex buffers;
    - the third coefficient (always empty on Sanctuary; counted if ever set).
  - The shipped engine shader sources under the installed game's `Engine/Shaders` would settle the decode. They are
    UE3 source, which LEGAL.md rule 2 keeps unread, so they were not opened. Whether reading them is allowed is the
    maintainer's call.

- **Tests** (synthetic, AGENTS.md rule 9):
  - `lightmap/tests.rs`:
    - a texture light map;
    - a per-vertex one with its bulk-offset, second-array and flag refusals, and override colours;
    - exact consumption, the smallest LOD, a count past the tail, an unknown type and a non-finite scale.
  - `ow-assets`: `lighting_textures_read_as_textures_and_end_at_their_trailer`.
  - `draw/tests.rs`: `baked_lighting_goes_to_each_instance_and_splits_the_batches`.
  - `render/tests.rs` (GPU, ignored by default): `light_maps_and_shadow_maps_light_the_surface_as_the_fit_says`.
    - Coefficients 0 and 1 with their scales, no light map, per-vertex samples, and a distance-field shadow that lets
      the sun through or blocks it.
    - The output pixels are checked against the fit.
  - H8 review r2 lows L2 and L3: `material/tests.rs` `a_refused_cooked_list_keeps_the_scalars_and_a_constant_is_linear`.
    - It builds a synthetic definition store.
    - A refused cooked list keeps the instance's `Time_of_Day` scalar. A mutation that restored the old early return
      made it fail.
    - An unconnected `DiffuseColor` of red 128 gives linear 0.2195.
  - The installed driver `sanctuary_lighting_on_the_installed_game` (ignored by default) writes its counts under
    ignored `local/p2/h9-lightmaps/`.

- **Results** (pc-hummer, RTX 5070 Ti, Vulkan, release `ow-host scene --map Sanctuary_P`, 3000 frames, no vsync).
  - Every run was under the machine lock, and all exited 0. Logs and screenshots are under ignored
    `local/p2/h9-lightmaps/`.
  - "Before" is main 60413ba, built in a temporary worktree.
  - **Lit placements:** 3,580 by a texture light map, 462 per vertex, 1,535 with the sun's shadow map. None was left
    unapplied for missing UVs or a vertex-count mismatch.
  - **Lighting slots:** 578. They split the batches, so a frame now issues 1,883 instanced draws (662 before).
  - **Lighting textures:** 201, uploaded linear. Texture bytes are 1,941,085,888, up from 1,815,865,592.
  - **Frame time, median / p95:** 1.234 / 1.387 ms at 1280x720 and 1.241 / 1.397 ms at 1920x1080, against
    0.472 / 0.533 and 0.471 / 0.531 ms before. The extra draws cost about 0.8 ms; merging lighting atlases into
    texture arrays would bring the draws back down, but nothing in this lane needed it.
  - **Load time:** reading the lighting takes about 35 ms after the scene loads.
  - **The look, from the player start:**
    - faces away from the sun now show their texture under baked light: the walls and shop fronts on the left,
      Scooter's sign, the building fronts;
    - the ground carries the sun's baked shadows;
    - the sky is unchanged;
    - still wrong: the large box in front (`InterpActor_34`'s BoxLrg, a mover with no light map) stays dark, and
      surfaces with no diffuse (the spire, the glacier meshes) are drawn grey and now read almost white under their
      light maps.

- **UNVERIFIED**, each settled only by the running game:
  - the coefficient decode (linear texels, the sum of two scaled coefficients) and which light each slot holds;
  - the distance-field reading and its 0.05 half-width;
  - `LightMapCoordinateIndex` 0 where absent;
  - the sky-term fallback for meshes with no light map;
  - the exposure (decision 58).

- **What a real-game screenshot from the same player start would settle** (`WillowCoopPlayerStart_0` + 100 uu, the
  level's default time of day):
  - the brightness of shadowed faces against lit ones;
  - the width of the sun's shadow edges on the ground;
  - whether the box in front is drawn at all.
  A bl-sdk read of a light-mapped surface's final colour would settle the decode formula.
