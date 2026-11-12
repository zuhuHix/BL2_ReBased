## 2026-10-01: stock slice world placement, values, Phaselock data, audio chain and NPC assets

AI-assisted. Tooling only; no parsing behaviour change. None of this is checked against the original game.

- World (`tools/prepare_slice_world.py`, record `docs/verification/SLICE_WORLD_PLACEMENT.md`): the GoToRange
  trigger is `WillowWaypoint_9`, a cylinder (357.81 / 145.31) completed by readable `WillowWaypoint.Touch`
  script; Marcus is the placed `WillowAIPawn_13` (not population-spawned, correcting the earlier census) with a
  scripted move-node walk 12 -> 26 -> 35 -> 40 and door Play/Reverse arrival events; the dummy comes from
  `PopulationOpportunityDen_13` at `WillowPopulationPoint_40`; the target Matinee `SeqAct_Interp_0` is driven
  by the dummy's own `MoveTargetForward`/`SendTargetBack` behaviours (the mission's `TargetForward`/`TargetBack`
  events have no Kismet listener); respawn follows the script rule in `GetBestPlayerPlacementPoint`. 20
  structural oracles pass. UNVERIFIED: native navigation, population spawning, Matinee frame, station activation.
- Values (`tools/slice_values.py`): XP reward percentage 0.05 (the amount is native; the candidate formula is
  UNVERIFIED); health 80 x 1.13^L (which of two constants applies is UNVERIFIED).
- Phaselock (`tools/prepare_action_skill.py`, record `docs/verification/PHASELOCK_STOCK_DATA.md`): lift 0.7 s,
  lock attribute 5 x target time scale, fade 1.1 s, cooldown pool 13 s paused while the target is held,
  Suspension +0.5 s per grade. The script was read, never run; the timeline, modifier rule and cooldown
  semantics are UNVERIFIED.
- Audio (`tools/audio_census.py`, `tools/audio_slice_chain.py`, record `docs/verification/SLICE_AUDIO_CHAIN.md`):
  16 slice events resolve to Wwise Vorbis media; containers tile exactly; ids are FNV-1 of the lower-cased
  name (16/16 events, 99/99 banks). Nothing installed decodes the media; a decoder is a new external tool and
  therefore a maintainer decision. No tool was downloaded.
- Assets (`tools/seed_slice_npc_assets.ps1`, record `docs/verification/SLICE_NPC_ASSETS.md`): Marcus, the target
  dummy and the Maliwan pistol candidate fragments extracted with UModel build 1590 and imported locally
  (12 jobs, 0 failed). Textures cross-check against our decoder; meshes are identity-only; materials are
  UModel's texture guess, not verified graphs.

