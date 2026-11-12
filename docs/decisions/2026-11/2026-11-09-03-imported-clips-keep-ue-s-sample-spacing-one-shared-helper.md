## 2026-11-09: Imported clips keep UE's sample spacing; one shared helper gives a clip the game's keys / rate (animation timing)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/CONTROLS.md`, "Clip length". Refines the 2026-11-07 melee entry ("A swing holds for the game's clip length").

- **Kept:** the four MD5 importers' `max(1, keys - 1)` frame count. N bone-track keys one frame interval apart span N - 1 intervals in UE; that is UE's sample spacing and keeps loop seams (idle, walk, perch) without a dwell.
- **Change:** the melee-only `OpenWillowMelee::HeldSeconds` became `OpenWillowClip::GameSeconds` (`OpenWillowClipTime.h`): the clip's UE length plus one frame interval, the keys / rate the package reports. Melee uses it as before (0.70 s for 21 keys at 30 fps), and the `melee_swing_rule` self-check now checks the helper. No timing changes.
- **Choice:** the host bridges UE's length to the game's only where it models a game duration, and only melee has a measured one. Reload, target hit and death, lootable open, the walker's land and Phaselock are listed as candidates, **UNVERIFIED**, not changed.
- **Not known:** where the game places its keys inside its length; a one-key clip's game length (the clamp makes it look like two keys).
- **Checked:** `tests/anim_import_frames_test.py` (the exact importer argument) and the UE module build. The in-engine self-check and `check_controls.py --melee-clip` are for the hub's run; no re-import.
