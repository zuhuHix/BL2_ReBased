## 2026-11-16: Rust host materials: blend modes, the sky and the packages' lights

AI-assisted (Claude Code). Lane H8-MATERIALS. Stage 2 of H2 (decision 2026-11-16-51) drew every section with its
named diffuse, opaque, under one constant sun. This lane reads more of each material from the packages: the fallback
diffuse rules for materials that name no diffuse, the blend mode and two-sidedness, the sky dome's inputs, and the
scene's own lights. The renderer draws masked, translucent, additive and modulate surfaces as such, and the sky first.
Nothing here claims BL2's look, which stays **UNVERIFIED**. There was no real-game session. No dependency was added,
no bounds check was loosened, and `ow-package` and `ow-assets` are unchanged.

- **What was added.**
  - `crates/ow-host/src/material.rs` (`MaterialLibrary`, `MaterialInfo`, `Blend`, `DiffuseSource`, `SkyInputs`,
    `decode_texture`) and `crates/ow-host/src/lighting.rs` (`SceneLights`, `Sun`, `SkyLight`, `LightmapCensus`), each
    with its tests.
  - `draw.rs`: `DrawPlan::build_with(scene, library)`. A batch now keys a material slot (`MaterialSlot`: diffuse
    candidates or a constant, blend, unlit, clip, sky inputs). `build(scene)` is the named-only plan of stage 2.
  - `render.rs`: one bind group per slot (colour texture, data texture, material uniform), pipelines by pass (sky,
    opaque, translucent, additive, modulate), `Lighting` from the packages, and blended instances drawn back to front.
  - `main.rs` (`scene_window`): reads the library and lights after the scene, plans with them, and prints the counts
    below.
  - `play.rs` (H4-PLAY's, decision 2026-11-16-57) and its part of `main.rs`: the `--play` window draws with the
    materials and lights too. `PlayDrawer::new` takes a `Lighting`, and the frame and its screenshots pass the view's
    eye for the back-to-front sort. The simulation is untouched. On pc-hummer, under the machine lock, `--walk`'s
    script was run three ways: `--walk`, a headless `--play-script`, and a `--play --play-script --no-vsync` window
    with materials. All three gave the same nine segment digests and the end digest `fd6551c9598c8ec7`, which is
    GATES.md's H4-PLAY baseline. The window ran 35,727 frames, median 0.476 ms, p95 0.538 ms.
  - `scene.rs`: visibility only. `parameter_name`, `full_path`, `read`, `read_object`, `read_objects`, `read_bool`,
    `read_float`, `struct_field` and `read_rotator` are now `pub(crate)`, so the material and light readers share
    them. No behaviour changed.

- **Where the rules come from.** The tool first, then the records. No native code was read.
  - **Fallback diffuse rules.** These follow `tools/prepare_level.py`, `Scene.material`, in its order:
    - the named parameter diffuse, as in stage 2;
    - a sole unnamed `MaterialExpressionTextureSampleParameter2D_<n>` whose texture ends in `_Dif`
      (`unnamed_diffuse_candidate`);
    - from the base material's cooked texture list, a sole `_Dif`/`_Diff` `Texture2D`;
    - for opaque or masked materials only, a sole non-auxiliary `Texture2D` (`cooked_diffuse_candidate`);
    - for an opaque material with an empty cooked list, the constant of an unconnected `DiffuseColor` input
      (`unconnected_diffuse_constant`).
  - **The cooked list** is read with the tool's recovered prefix (`cooked_texture_references`): two empty arrays, a
    resource int, a GUID, a resource int, then the reference array. It is refused unless the arrays are empty and the
    count fits. Each reference is resolved as a package reference (`PackageStore::resolve`, the tool's `--resolve`),
    not through the definition store's identity by path: Sanctuary's `Prop_Wire.Meshes.HangingWires_1A` is both a
    material and a texture, and the store answers the material.
  - **Blend mode, lighting model, `TwoSided` and `OpacityMaskClipValue`** come from the base `Material` of the chain
    (`Scene.material_metadata`). The clip value defaults to 0.3333 when not serialized.
  - **The sky** follows `docs/verification/NATIVE_SKY_APPROXIMATION.md`. For a chain whose base is
    `Common_Materials.Sky.Mat_SkyTimeOfDay_Master`:
    - sky = strip(`Time_of_Day`/256, v) x `sky_brightness`;
    - the clouds' colour is the strip at row 0.95 x `sky_brightness` x `p_CouldBrightness`;
    - the mix is by saturate(clouds.R x `cloud_cap_opacity`);
    - the strip is sampled as sRGB, the clouds as linear;
    - the dome is drawn first, unlit, with no depth test and no depth write.
  - **Lights** follow `Scene.lighting_manifest`:
    - the `DominantDirectionalLight` actor's rotation, and its component's `LightColor` and `Brightness`;
    - the component's tags are found by the tool's anchored read: the one tag stream that starts at a
      `LightmassSettings` name, holds `Brightness` and `LightColor`, and ends at the export's end;
    - the `SkyLightComponent`'s upper and lower colours and brightnesses, with class defaults applied;
    - `FColor` to linear is (byte/255)^2.2, as UE3's `FLinearColor` does it (public UE3 behaviour, UNVERIFIED for
      BL2).
  - **Not ported.** The tool's inspected, path-specific rules (`REGULAR_DIFFUSE_FALLBACKS`,
    `INSPECTED_COLOR_FALLBACKS`, the glacier surface, scene recipes, tints, emissive calibrations). They name single
    materials from screenshot inspection.

- **Counts against the tool** (installed Sanctuary_P; oracle: the tool's `scene.json` from lane H2).
  - The library reads 381 materials.
  - Diffuse sources: named 249, unnamed sample 11, cooked `_Dif` 61, cooked sole 19, constant 1, none 40.
  - Each agrees with the tool material by material, except 15 that the tool settles with its inspected rules
    (5 scene recipes, 7 inspected colour fallbacks, 1 regular fallback, 2 glacier).
  - Blend mode and lighting model agree for all 377 paths both hold.
  - By material: opaque 309, masked 40, translucent 23, additive 6, modulate 3. Unlit 39, non-directional 2.
  - Count agreement is the tool's behaviour, not the game's.

- **Expression classes that survive cooking** (Sanctuary, over the base materials' `Expressions`):
  VectorParameter 4654, StaticSwitchParameter 2270, TextureSampleParameter2D 2130, ScalarParameter 1540,
  StaticComponentMaskParameter 703, TextureSampleParameterNormal 472, TextureSampleParameterCube 6. Only parameter
  expressions survive; the graphs are stripped, so no general graph compiler was built.

- **Where a node-graph evaluator would fit.** An evaluator, of the surviving parameters or of a hand-authored
  replacement graph, would replace `DiffuseSource` and the sky inputs with a compiled shader per material. The seam
  is `draw::MaterialSlot` and its bind group in `render.rs`. MaterialCraft 0.4.1 was named as a design reference
  only. Nothing was copied from it; using it needs a `THIRD_PARTY.md` entry, which is the maintainer's call.

- **Translucent surfaces with no readable opacity are not drawn** (a choice of this lane, UNVERIFIED).
  - Drawn with the diffuse alpha as opacity, two kinds covered the scene opaquely. A CPU pick through the
    player-start camera found which:
    - Sanctuary's water decals (`DecalPlane_LiquidDev` with `FX_ENV_Water.Materials.Mat_SanctuaryWater`:
      translucent, no diffuse) were drawn as white patches on the ground;
    - a cloud layer (`Blocking_Plane` with `Env_Ice.Materials.Mat_CloudLayer_Light`) was drawn as a yellow-and-black
      band on the skyline. Its fallback diffuse, `FX_Shared_Smoke.Textures.Basic_Dust_Dif`, is DXT1, a yellow puff
      on black with no alpha.
  - Their opacity is the stripped graph's. So the plan leaves out a translucent slot with neither a diffuse nor a
    constant (`PlanStats::translucent_skipped`), and the renderer leaves out a translucent slot whose chosen diffuse
    has alpha 255 at every texel of its largest uploaded level (`UploadStats::translucent_without_alpha`).
  - Masked surfaces are still drawn; one whose alpha is all 255 discards nothing.
  - Tests: `a_translucent_material_with_nothing_to_draw_is_not_batched`, and a case in
    `blend_modes_draw_as_their_material_says`.

- **Lightmaps: counted, not decoded.** Sanctuary's levels hold 136 `LightMapTexture2D`, 1567 `ShadowMap2D` and 151
  `ShadowMap1D` exports. The `ow-assets` texture reader refuses the first lightmap ("export is not Texture2D or
  TerrainWeightMapTexture"). The components' lightmap data and UV placement are in untagged bytes no reader here
  knows. So faces away from the sun get only the sky term and read dark.

- **Results** (pc-hummer, RTX 5070 Ti, Vulkan, release `ow-host scene --map Sanctuary_P`, 3000 frames, no vsync,
  under the machine lock; logs and screenshots under ignored `local/p2/h8-materials/`). Both runs exited 0.
  - **Plan:** 349 material slots, 681 batches, 4384 section instances, 662 instanced draws per frame, and 41 blended
    instances drawn one at a time.
  - **Untextured section instances** (no material / material not in the scene / no diffuse):
    - before, named diffuse only: 9 / 0 / 820;
    - after: 9 / 0 / 149, with 617 textured by a fallback rule and 53 by a constant colour.
  - **Section instances by blend:** 4100 opaque, 234 masked, 99 translucent, 5 additive, 6 modulate, 1 sky.
  - **Translucent instances not drawn:** 61 with no diffuse, and 8 (3 slots) whose diffuse has no alpha.
  - **The sky is drawn:** strip `Sky_TransitionBL2Default_Dif`, clouds `Clouds_01`, `Time_of_Day` 170, column 0.664.
  - **Lights:**
    - sun `Sanctuary_Light`'s `DominantDirectionalLight_1`: brightness 7, linear colour (1.0, 0.966, 0.828), tags at
      byte 4327 (the tool's offset);
    - sky light: colour bytes (89, 134, 195), lower brightness 0.1, brightness 1 (the class default);
    - exposure 0.1409.
  - **Textures:** 247 uploaded (1,815,865,592 bytes), none failed.
  - **Frame time, median / p95:** 0.486 / 0.599 ms at 1280x720, 0.499 / 0.598 ms at 1920x1080. Stage 2's r1 runs had
    0.401 / 0.486 and 0.401 / 0.493 ms. The difference is the blended single draws and the longer fragment shader.
  - **Load time:**
    - the scene loads in about 44 s, as in stage 2;
    - the material read adds about 87 s on top. Nearly all of that is in `PackageStore::resolve` for 214 distinct
      cooked texture imports, of which only 2 are in the scene's levels;
    - the store's global fallback loads each candidate file afresh on a miss (`try_file`). A per-reference cache in
      `ow-host` saved nothing, so it was not kept;
    - a cache of parsed candidate files belongs in `ow-package`, which is outside this lane: a follow-up.
  - **Screenshots** from the player start: the sky shows a blue gradient with cloud bands. The white water patches
    and the yellow band are gone.
  - **What is still visibly wrong:**
    - faces away from the sun are dark (no lightmaps);
    - a spire is white (`Prop_SancBuildings_02.Material.Mati_SancSpire_Base`: opaque, no diffuse);
    - a blue ring in the sky: an additive aurora mesh whose material is a `MaterialInstanceConstant` driven by the
      level's day-night Kismet sequence, which the host does not run.

- **UNVERIFIED**, each settled only by the running game:
  - the exposure: one over the brightest sum, the sun at full incidence plus the upper sky (a viewer choice; BL2's
    exposure, tonemapping and lightmaps are not read);
  - the 2.2 power for `FColor` light colours;
  - the diffuse alpha as a masked surface's opacity mask;
  - the sky formula and its constants (the record's approximation of a stripped graph);
  - leaving out translucent surfaces with no readable opacity;
  - drawing the aurora at full strength.

- **What a real-game screenshot from the same player start would settle.** The camera is at
  `WillowCoopPlayerStart_0` + 100 uu, the time of day being the level's default:
  - how bright lit and shadowed faces are against each other (the exposure, and how much the lightmaps carry);
  - the sky's colour and where its cloud band sits;
  - whether the aurora ring shows at that time of day;
  - how the water decals and the skyline cloud layer look;
  - what the white spire's surface is.
  It would not settle per-material colours that the inspected rules tune.
