## 2026-10-02: weapon paint pass 2: texture colour space from data, reflection term; albedo checked against screenshots

AI-assisted. Tooling and editor importer only; nothing compared against the running game.

- Textures default to sRGB (`Default__Texture`); only `p_Masks` and the normal maps switch it off. The detail atlas was
  imported as linear, which washed out grime and rust. The preparer now reads each texture's SRGB flag and the
  importer and thumbnail renderer follow it. The by-eye `DISPLAY_SCALE` is removed; the shader colour is used unscaled
  (clamped to 1, keeping hue).
- The environment term (`P_SimpleReflect`, `p_ReflectColor`, `p_ReflectionChannelScale`, `p_ReflectColorScale`) is
  drawn as the compiled shader combines it: it brightens surfaces that are already bright and cannot turn a dark base
  silver.
- Re-checked from the compiled shader: the shadow amount is the clamped product of the light/dark map's green and
  `p_ShadowsIntensity`, moving the tone from Midtone toward Shadow; the zone mask is the lower half of `p_Masks`
  (97% of its texels are flat, against 74% in the upper half).
- Numeric check (unlit model albedo against in-game wiki inspect screenshots, median sRGB of matching regions; the
  references include the game's lighting): the Jakobs wood and frame match closely; the Maliwan barrel has the same tint and is about 40% darker (reflection and
  lighting omitted in the render). So the blue cast seen in first person most likely comes from the host's lighting
  and its fixed metallic 0.35 / roughness 0.55, not from the paint formula (UNVERIFIED).
- Checks: `tests/weapon_paint_test.py` 29 passed, six guns re-imported with 0 errors, quest suite 73/73 and resume 10/10
  (`run-first-20261002-010701`). Critic (in-game stills and thumbnails vs wiki screenshots): Maliwan 6.5/10 (same),
  Jakobs 5.0/10 (was 4.5).

