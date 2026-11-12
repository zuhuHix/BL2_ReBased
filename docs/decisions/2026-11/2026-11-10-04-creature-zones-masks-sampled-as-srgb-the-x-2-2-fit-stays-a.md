## 2026-11-10: Creature zones: masks sampled as sRGB; the x^2.2 fit stays a host fit (creature colour lane 2)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Why the skin reads yellow and the spikes golden".

- **Found:** the host's zone and channel mapping matches Master_Creature's compiled shader (checked against the cache's uniform-expression
  list); not the cause. The worn spikes use the body's instance and sit in zone A with the torso, so "yellow skin" and "golden spikes" are one
  fault. The shader uses the zone vectors as stored. Both creature mask textures have `SRGB` set in the package (the normal maps do not), so
  the game most likely decodes them before the x2 light/dark scales (UE3's usual sampling; UNVERIFIED in game); the host read them raw
  (about 1.7 times the highlight amount).
- **Change:** `masks_srgb()` in `tools/creature_zone.py` (True for `Master_Creature` targets); `tools/phaselock_target_editor.py` imports
  those masks as sRGB. Other masters and the citizens are unchanged.
- **Critic (run Z55d, Luna, colour only, against the matched real frames):** sRGB masks with the vectors as stored 6.1/10 (too pale, cool);
  sRGB masks with the x^2.2 fit 6.6; the earlier raw masks with the fit 5.5 (yellow, golden spikes). Hub chose the second: the x^2.2 fit
  stays on for every target (`OPENWILLOW_ZONE_GAMMA=0` for an A/B), an UNVERIFIED host fit; the remaining paleness is most likely UE5's
  tone curve and the host's 0.15 specular (the shader has none), not checked yet.
- **Checked:** `tests/creature_zone_test.py` (synthetic), `tests/editor_inputs_test.py`. No parsing change.
- **Not verified:** the look against the running game beyond this critic score. That the game samples these masks decoded is read from the
  package's `SRGB` flag and UE3's usual sampling of such a texture; it was not seen in the running game (UNVERIFIED). The range dummy is a
  `Master_Creature` target too, so its masks become sRGB at its next zone step (`tools/seed_slice_npc_assets.ps1 -Steps zone`); the critic
  scored the Bullymong only, and the dummy was not re-imported or looked at.

