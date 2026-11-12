## 2026-10-02: weapon paint from Master_Gun's compiled shader data; MIC static parameters decoded

AI-assisted. Tooling, editor importer and two read-only CLI modes (`src/cli.cpp`: `--payload-file <index> <out>`,
`--names`); no change to `src/package.cpp`, the container code or `CMakeLists.txt`, no bounds check changed. Nothing
compared against the running game.

- **Static parameters.** The bytes after a MaterialInstanceConstant's properties are its fully resolved static
  parameter set; `tools/material_static_parameters.py` decodes them exactly (sizes meet) in 631 of 631 MICs in
  `Startup.upk`. On the slice chains they only pick channels: `p_WeapClassSelect` the detail atlas channel (B for
  pistols, matching the earlier guess), `p_PatternChannel` and `p_DecalChannel` the pattern and decal channels;
  `sw_FlipDecalOnRightSide` is off. The hypothesis that undecoded static overrides caused the blotches is refuted.
- **Shader data.** `RefShaderCache-PC-D3D-SM3.upk` keeps, per static permutation of Master_Gun, the uniform expression
  set (which parameter feeds which constant and sampler) and the compiled ps_3_0 shaders. A small token reader written
  from Microsoft's public D3D9 bytecode description (`research/d3d9_bytecode.py`; its listings are game-derived and stay
  under `local/`) and a diff of three permutations give the colour model described in our own words in
  `tools/weapon_paint_model.py`: `p_Masks` holds two stacked maps (lower half zone mask, upper half highlight/shadow
  map); zone tones go Midtone -> Hilight -> Shadow and are blended over `p_DColor`; pattern and decal multiply or
  replace by squared mask weights; the decal UV is shifted, rotated by `p_DecalRotate` x pi and scaled about the
  centre; the result is multiplied by the selected detail channel.
- **The bug:** the previous reading sampled `p_Masks` over its full height, so the light/dark map became zone weights
  (the camouflage blotches). Also fixed: detail used as a tone selector, `p_DColor` and the two intensities ignored,
  decal placement and rotation, the single-channel decal path.
- UNVERIFIED: the whole reading until compared with the running game; `DISPLAY_SCALE` (0.4) chosen by eye; the
  environment reflection (`P_SimpleReflect`), emissive, digistruct and lighting are not modelled; the material
  resource words before the static parameters are not interpreted.
- Checks: `tests/weapon_paint_test.py` 25 passed (invented values), CTest 10/10, packages 9/9, six guns re-imported
  with 0 errors, quest suite 73/73 and resume 10/10 afterwards (`run-first-20261002-005221`). Visual: an independent
  critic agent compared host stills with in-game inspect screenshots from the Borderlands wiki (kept under ignored
  `local/paint_research/ref_online/` with sources): Maliwan uncommon pistol **6.5/10** (was 4), Jakobs common pistol
  **4.5/10** (was 2). Its remaining findings: a cool blue cast on bare metal and on the Jakobs wood (wood reads grey,
  not brown), the Maliwan barrel looks painted rather than chrome (no reflection term), orange slightly too wide on
  the Maliwan grip. No real-game capture yet (the game would not launch under the logged-in Steam account).

