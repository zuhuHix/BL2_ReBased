## 2026-11-04: gun look on the second PC: part library rebuilt, Inspect capture gets its own fitted light (gunlook)

AI-assisted (Claude Opus 5.5; Haiku 5.5 sub-agents; critic Luna). Record: `docs/verification/WEAPON_VISUALS.md` section 19.
No change to `src/`, `CMakeLists.txt` or package parsing.

- **Data step, no code:** the weapon part library was rebuilt on this PC with the documented chain (301/301 skins exported,
  300 imported, 6/6 meshes, 279 slots); 33 type x manufacturer guns assemble with no fallback (oracle 33/33 against the cooked
  prediction; no live check here). The 22 live-only Torgue glow colours are missing (no live census on this PC).
- **Behaviour change (host look):** the inventory preview capture (Inspect, thumbnails) uses its own light, ambient 3.2 /
  key 0.7 / fill 0.3 instead of the world look's 1.6 / 1.2 / 0.5, fitted on one gun (the slice Jakobs pistol) against the real
  Inspect frame: luma percentiles 25-75 within 6 of the real frame (were 21-28 darker). UNVERIFIED stand-in light; the
  world look is unchanged. `-owinspectlook` overrides it; `-owgunturn` adds turntable frames to `-owgunshots`.
- **Critic:** round 0 mean 7.6 (Inspect side 8.6), round 1 Inspect side 8.0 (different runs, not comparable); elemental glow
  parts and rarity skins read as correct (no real frames for them). Open: highlight roll-off, grip weave, the pale rim, a
  real-game reference session.

