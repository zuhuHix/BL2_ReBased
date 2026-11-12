## 2026-09-14: Sky census locates the native dome without a new policy

The user asked for the native skybox to be located using only already-decoded
data before any material-parameter parsing. AI-assisted `tools/sky_census.py`
reuses `Scene` from `prepare_level.py` (level traversal factored into
`Scene.levels`), the existing `--scene-records`, `--properties` (Texture2D at
the established offset 4), `--payload`, `--mesh` and `--texture` modes, and the
existing cooked-resource reader. No serialization offset, bounds check or
material policy changed; the census only restates what the Material v1 diffuse
rule would select and why.

Findings on the installed game, manifest evidence only:

- Neither `Sanctuary_P` nor `Ash_P` streams a `_Skybox` package. The dome is
  `Prop_Skybox.Meshes.Sky_Dome` (265 vertices, 480 triangles, two UV sets)
  placed by a `StaticMeshCollectionActor` (Sanctuary: `_Light` sublevel,
  scale 5000x5000x6000; Ash: persistent level, scale 1000). Its material instance (`Mati_Sky_Dynamic_INST`
  in Sanctuary, `Mati_AshSkyTempSunset` in Ash) inherits from the unlit
  `Common_Materials.Sky.Mat_SkyTimeOfDay_Master`, whose named samplers are
  `Transition_Track` (`Sky_TransitionBL2Default_Dif`, PF_A8R8G8B8 256x256),
  `clouds` (`Clouds_01`) and `Masks` (`Sky_Multi`/`Sky_Multi2`), with scalars
  such as `Time_of_Day` and `sky_brightness`. The existing policy already
  selects the transition texture as diffuse; the decoder gap closed today was
  the blocker, not the placement.
- Sanctuary's second sky layer, `Prop_Skybox.Meshes.SanctuarySky` in
  `Sanctuary_Outer`, is an `InterpActor` whose three overrides are the
  `*_Teleported` story-state materials with several `_Dif` textures each; the
  policy correctly refuses to pick one. Its mesh defaults resolve to
  `SanctuarySkybox_Diff` (DXT1 2048x2048) uniquely. Which layer is active is a
  Kismet streaming question this project does not interpret.

Not done and not claimed: how the master combines its inputs, the meaning of
`Time_of_Day`, the second `Sky_Dome` placement with a concrete material and
negative Z scale, any host import, and any in-game comparison. See
[verification](docs/verification/SKY_CENSUS.md).

