## 2026-11-07: reload clips and arms sets for the view table's weapon types, as one data step (gunlook)

AI-assisted (Claude Opus 5.5; critic Luna). Record: `docs/verification/WEAPON_VISUALS.md` section 22.

- **New tool:** `tools/weapon_reload_library.py`, for the types in `view_types.json` (DLC types are not in it). A clip is found by file name as before. On a machine that runs it, guns get reload motion and the assault rifle, SMG, shotgun and sniper get their own arms sets instead of the pistol's, so slice frames move.
- **One host rule changes:** a reload clip now plays over the reload time after skills, as the refill timer does. With no reload-speed bonus nothing changes. Built only by the next slot; UNVERIFIED in the game.
- **Checked:** host frames against lane E's real idle and reload frames; critic idle 8.0-8.8, reload 8.3 / 5.8 / 4.5 (pistol, rifle, SMG), target reached on idle and the pistol's reload only. The firing suite passes. UNVERIFIED beyond those frames; each type's default clip only, the fast variants are not imported.
