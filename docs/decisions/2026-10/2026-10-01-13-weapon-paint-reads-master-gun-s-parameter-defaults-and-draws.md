## 2026-10-01: weapon paint reads Master_Gun's parameter defaults and draws a decal layer

AI-assisted. Tooling and editor importer; no package parsing change; nothing compared against the original game.

- Master_Gun's graph is stripped, but its 43 parameter expressions survive in `Startup.upk` and decode completely
  with the owned reader (`ParameterName`, and `DefaultValue` where it differs from the class default; an absent value
  is taken from the expression class default in `Engine.upk`, and the scalar default, which serializes nothing, is
  read as zero). `tools/prepare_weapon_paint.py --reader/--package` uses them only for scalar and vector parameters
  that no MIC sets and records each value's source. Reader and UModel agree on all 116 MIC parameter values of the
  nine slice MICs; UModel exports nothing for the Material itself. This makes the two Jakobs common chains
  preparable (their B and C zone colours are the base defaults), so all five slice guns and the mission pistol are
  now painted instead of grey.
- The decal parameters are emitted and the importer draws a decal by this reading: placement UV1 x scale + offset
  with the installed texture address modes; weight = zone weights dotted with the mask channels, times decal alpha;
  colour multiplies the zone colours, or replaces them as `p_ReplaceDecal` goes to 1. Evidence is structural only:
  of six candidate placements only this one puts the clamped Hyperion stripe along the gun; the Jakobs decal and
  pattern zone weights are exact complements; two chains' decal colours normalise their decal's mean to about 1.
- UNVERIFIED: the whole paint and decal reading. `p_DecalRotate` and the flip switch are not applied; static
  parameter overrides (undecoded trailing bytes on the MICs) are unknown; the replace mode is inferred from its name.
- Checks: `tests/weapon_paint_test.py` 9 passed; imports ran for six guns ("decal used" on each); quest suite 73/73
  and resume 10/10 afterwards (`run-first-20261001-224649`). Visual, by an independent critic agent that could not
  fetch real first-person images and judged from text descriptions of the skins plus one unrelated local reference
  (so low to medium confidence): Jakobs common pistol 2/10, Maliwan fire pistol 4/10 (2/10 before the decal). Its
  main finding is that both guns show irregular blotchy patches where the real skins have clean zones, so the zone
  mask reading itself is suspect. That is open work, not a result.

