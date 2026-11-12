## 2026-10-22: Sanctuary matched-viewpoint rounds 1-4: WiringMesh collision refresh, source lighting, decal default-deny, host calibration

AI-assisted (Claude), lane A. Details, counts and per-round history: `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`;
collision record: `COLLISION_WALKING_VERIFICATION.md` (2026-10-22).

- **Collision refresh (`tools/refresh_collision.py`).** Accepts `GearboxFramework.WiringMesh` exports as well as
  `StaticMesh` (`MESH_CLASSES`). Before this, the Sanctuary_P cable mesh raised "Ambiguous mesh" and the refreshed scene had no
  volumes. Synthetic test `test_refresh_accepts_wiring_mesh`. Still skips the one UModel-prepared skeletal pillar. **Verified:** the
  refresh printed supported 312, absent 115, unsupported 2, hulls 518, blocking volumes 5, non-blocking 6, live mismatches 0;
  the live capture matches the committed record (`DynamicBlockingVolume_1`/`_2` collide, `_0`/`_3` do not). The full UE import
  now spawns `OpenWillow_Volumes` itself.
- **Scene manifest (`tools/prepare_level.py`, `scene.json`).** Gains `lighting` (decoded SkyLight, HeightFog, WorldInfo and
  PostProcessVolume settings, and the directional light component after its `LightmassSettings` anchor), `decals` with
  default-deny exclusions and a reason per actor and material, and `diffuse_tint` (`diffuse_tint_v1`, `partial_unverified`) for 18
  Sanctuary building materials, read from `Master_World` instances. **Verified:** the decoded records and counts (33 kept, 78
  excluded, 182 unsupported; 46 decal-like mesh placements stay on the mesh path). **UNVERIFIED:** that the game multiplies Base Color by
  `p_diffmod` in the way the host does.
- **Schema (`tools/level-arrays.schema`).** Declares `StaticLightCollectionActor.LightComponents` as an object array. This is bulk
  decode metadata only; the resolved elements are point and spot light components, 1,023 of them, of which 128 are spawned.
  The schema is keyed by property name only, so it applies to every package that has such a property. **Declaration
  census (verified, all 914 base-game packages, 0 failures):** 129 packages carry the name; the only exports named
  `LightComponents` are `StaticLightCollectionActor.LightComponents`, a `Core.ArrayProperty`, and its inner
  `Core.ComponentProperty` (an object-reference property). A tagged property's type comes from its declaring class, so every
  serialized `LightComponents` is that array of object references. The same declaration census over the 1,096 DLC `.upk` files
  (the `--include-dlc` scope) finds no declaration of that name (151 files carry it as instances; 2 texture payload files
  with no package magic cannot be opened). An instance decode pass (stopped at a 30-minute budget after
  666 packages) agreed: 119 of 119 occurrences decoded as `ObjectProperty` arrays owned by `StaticLightCollectionActor`.
  Scratch scripts under `local/laneA-census/` (ignored).
- **Light array tail (`UNVERIFIED` layout).** The 64-byte-per-element tail after a `StaticLightCollectionActor`'s properties is
  size-checked and skipped, not decoded. Its per-component layout (presumed 4x4 transform, as for `StaticMeshCollectionActor`)
  is `UNVERIFIED`. Placement uses each component's decoded `CachedParentToWorld`. Reader coverage: `tests/properties_test.py`
  (a real-reader N = 2 `ObjectProperty` array with a 128-byte tail, and refusals when the declared size disagrees).
- **Malformed light arrays contribute nothing.** A `LightComponents` array that fails any check now stages its records and adds
  them only after the whole array validates (`tools/prepare_level.py`, `static_light_collection_records`). Synthetic test:
  `test_malformed_light_array_contributes_no_lights`.
- **Lighting is Sanctuary-only.** `scene.json` gets `lighting` only when the persistent map is `Sanctuary_P`
  (`sanctuary_lighting_enabled`); other maps have no `lighting` key. Decals stay generic and default-deny: a non-allowlisted
  material is excluded for any map, so a non-Sanctuary map places no decals. Synthetic test:
  `test_sanctuary_lighting_policy_is_limited_to_sanctuary_persistent_map`.
- **Host opt-in (documented, implemented by the other lane-A agent).** `OPENWILLOW_SANCTUARY_OPT_IN` lists `grade`,
  `diffuse_tint`, `xmas_bulbs`, `static_lights`, `graffiti_decals`; all default OFF, Sanctuary_P only, and the 0.14 cloud cap
  stays on. Source inputs are still recorded in `scene.json`.
- **Test values replaced.** The visual-rules test uses invented colours and intensities (the 1.25 directional brightness,
  50/100/150 sky colour, post-process 4.0/1.3-1.5/0.05-0.07/2.5) and checks calibration formulas against the host constants,
  not literal calibrated outputs. The grafitto ink check is structural, and a far plane is 900.
- **Colour conversion.** UE3 FColor values are converted to linear with a 2.2 power curve, following the UE3 FLinearColor
  conversion as we understand it. **UNVERIFIED** against the game's output.
- **Host calibration (`host/ue5/scene_import_rules.py`).** The sun's 5.0 lux per source brightness unit, sky light 1.5, sky tint
  (3.0, 2.1, 1.0), exposure bias -1.5 EV, white balance 8200 K, saturation, contrast, shadow lift, the cloud-cap 0.14, the 25 cd
  per brightness unit with the 3.0 ceiling, the 0.01 orange-point scale, and the graffiti ink and 128 cm depth cap are
  **screenshot calibrations tuned against the matched cameras. They are not decoded from the game** and are `UNVERIFIED`.
- **Verifier crash fix (`host/ue5/verify_level.py`, decal check loop).** A module-level bound method of a UE actor
  (`actor.get_actor_right_vector`) was kept alive to interpreter shutdown. After a successful run the UE Python plugin crashed
  in `python311.dll` (access violation), so `-ImportOnly` reported `Saved scene verification failed: 3` although the assertions
  passed. The loop now reads the axis vectors as values; the full gate exits 0. The root cause inside the plugin's teardown is
  not analysed.
- **Visual review:** independent critic (Codex GPT-6-Luna, host and reference paths only, eight matched cameras). Baseline before
  round 1 4.5/10; round 1 4.7; round 2 4.1 (decal noise regression); round 3 4.3; round 4 4.4; after review 1, the default state (round-4 guesses opt-in and off) 5.0. **The 8/10 gate was not reached;
  the lane stopped after four rounds as briefed.** Remaining gaps are listed in the gaps document, round-4 ranking.
- **Checks:** CTest 10/10; packages 9/9; Python tests 42 pass (the 8 reader-argument suites run inside CTest); JS 9 and 32/32;
  UE import gate exit 0 after the crash fix; OpenWillow.Walking and BspWalking pass; TerrainWalking 23/24 (the known
  pre-existing Terrain_2 hole endpoint). Not run: RouteWalking/CollisionShapes (no route traces on this machine), live-game
  gameplay.

