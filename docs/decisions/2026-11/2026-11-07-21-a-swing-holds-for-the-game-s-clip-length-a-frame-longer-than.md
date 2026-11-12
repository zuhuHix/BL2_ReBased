## 2026-11-07: A swing holds for the game's clip length, a frame longer than UE's (controls)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/CONTROLS.md`, "Melee".

- **Change:** with the first-person `Melee` clip imported, a swing holds the weapons for the clip's UE length plus one frame interval, not for its UE length.
- **Why:** the first row after the import graded 0.67 s against 0.70 s. The package gives the clip 21 keys and a length of 0.70 s, and gives all 5 first-person Siren clips read a length of keys / 30. The md5anim has 21 frames at 30 fps. The importer keeps 21 keys, and UE counts 20 intervals, 0.667 s, which the host's `GetPlayLength` returned. It is the right clip; the two engines count length differently.
- **Not changed:** the grader's 0.70 s and its tolerance, which come from the live reads and the package. The importer's interval count, which is what UE needs for those keys. Other clips the host times by `GetPlayLength` are also a frame short: not changed here.
- **Not known:** how the game spaces the keys inside its 0.70 s. **UNVERIFIED**, at most one frame.
- **Checked:** `tests.check_controls_test` only. The host change is not compiled or run.

