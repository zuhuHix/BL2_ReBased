## 2026-09-18: Material inference honesty fixes (stale metadata, auxiliary suffixes)

Two game-free fixes from a code-only pipeline review of zuhu's Sanctuary
screenshots (view-dependent shading, untextured town center). No new
serialization, offsets, or bounds checks; `src/` untouched.

- A failed diffuse texture decode no longer leaves `diffuse_inference` /
  `diffuse_inference_method` / `surface_approximation` metadata claiming a
  diffuse that never decoded (`tools/prepare_level.py`, channel loop). The
  material renders the neutral fallback, and the manifest now says so too;
  refresh previously counted these as fixed while the audit counted them as
  gaps. Synthetic test: `test_failed_diffuse_decode_clears_stale_inference`.
- `AUXILIARY_TEXTURE` now also excludes `_detail`, `_rough(ness)`,
  `_height`, `_bump`, `_opacity`, `_illum`, `_lightmap`, `_gloss`,
  `_metal(lic)`, `_ao`, `_cavity`, `_displacement`, `_reflection` and `_env`
  suffixes (plus `_\d+` variants). A lone utility map previously became
  BaseColor with fixed roughness 0.65, producing wrong albedo with
  view-dependent shading. Synthetic assertions extend the existing
  `sole_cooked_resource_texture` cases.

Evidence: `ctest` 6/6 plus all pure-Python suites pass on a game-less PC;
`tools/verify_packages.py` not run (needs the install). Sanctuary/Ash
re-measurement against the real game remains open, as do the documented
approximations (fixed 0.65 roughness, planar/UV0 mappings, sky graph).

