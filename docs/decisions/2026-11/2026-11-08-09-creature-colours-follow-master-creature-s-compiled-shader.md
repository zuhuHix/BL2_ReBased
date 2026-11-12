## 2026-11-08: Creature colours follow Master_Creature's compiled shader: light/dark scales, no gamma fit, zones on the range dummy (showcase follow-up)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Master_Creature's colour model".

- **Found:** the bullymong and the range dummy are both `Master_Creature` instances. Its compiled pixel shader (shader cache, read in our own words)
  scales the light/dark mask by `p_HighlightsIntensity` and `p_ShadowsIntensity` (master default 2.0) and uses the zone vectors as stored. The host
  used no scale and, since round 23, raised the vectors to the power 2.2; the dummy had no zone colours at all.
- **Change:** `M_OW_NPC_Zone` gains the two scalars (default 1.0, so the citizens are unchanged); `tools/creature_zone.py` sets them and the stored
  vectors on creature instances; the gamma fit is now opt-in (`OPENWILLOW_ZONE_GAMMA=1`); `seed_slice_npc_assets.ps1 -Steps zone` gives the dummy the zone material.
- **Not verified:** nothing has run in the editor or the game yet. The master's other terms are not drawn. No parsing behaviour changed.

