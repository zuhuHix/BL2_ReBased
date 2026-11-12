## 2026-10-02: real-game ground truth: driver tooling and the first capture session

AI-assisted. Tooling (`tools/real_game/`) and records; no change to `src/`, `CMakeLists.txt`, the reader or the
host. The maintainer allowed unattended launches of the installed game for captures (orchestrator brief, 2026-10-02).
Details and method: `docs/verification/REALGAME_GROUND_TRUTH.md`; everything recorded stays under ignored
`local/realgame/`.

- **Tooling.** `tools/real_game/realgame.ps1` (run lock shared with UE runs, save backup, launch, window capture,
  scan-code keys including arrows, click/wheel/drag, QPC-stamped burst capture, `Invoke-GamePy[File]`) and
  `openwillow_realgame`, our own Library mod for the community SDK that runs command files on the game thread, with
  `block_saves()`. Command scripts: `scripts/weapon_cards.py` (spawn by balance or exact definition, weapon record,
  card trace in the uitrace row format), `scripts/phaselock.py` (per-frame lift-skill sampler, cast and weapon-call
  marks, damage immunity, `face`). `golden_cards.py` joins records, card trace and screenshots;
  `golden_card_compare.py` (written by a subagent, reviewed) evaluates `tools/weapon_stats.py` on the exact rolled
  parts; `tests/golden_card_compare_test.py` 12 synthetic tests. Smoke test: the channel answered at the main menu
  15 s after launch and the driver was removed afterwards.
- **Saves.** Backed up first; all 22 save files are byte-identical to the backup after the session (four files the
  game had written at start, character selection and one load, while the first hook version was broken, were restored
  from it). One game crash came from a command reusing an invalidated weapon reference; nothing was written.
- **Confirmed in game** (each with how; the notes' other rules stay `UNVERIFIED`):
  - level curve: `GetExpPointsRequiredForLevel` for 1-80 matches the single-precision formula at every level 1-59;
  - "next level at" and skill points `max(0, L − 4)` at levels 2, 8, 17, 70;
  - max health base `80 × 1.13^L` (health pool base value) at levels 2, 8, 17, 70; the HUD adds the Badass Rank;
  - Fire mission XP 395 at stage 8 (`MissionDefinition.GetExperienceReward`);
  - Phaselock bob: the note's sine-from-cast plus `VInterpTo` speed 1 reproduces a lifted enemy over 435 frames to
    0.001 units RMS; a cast during a manual reload starts and aborts the reload; a cast during a swap put-down is
    refused (2/2, with controls).
- **Observed, for the lanes:**
  - Phaselock presentation timings (hand orb ≈ 0.45 s after the key, not 0.25 s; dark-cored violet bubble; cast
    vignette, target burst and cyan release ring that the host lacks); skill 5.7 s = 0.7 + 3.9 + 1.1 at level 8.
  - Weapon cards (69, exact parts): every printed stat matches on 53/69 with the evaluator before `bb2a444` and 52/69 after it (fire rate fixed; one 1.75 reload now prints 1.8, the game 1.7); damage prints rounded up;
    the live weapon-type objects differ from the cooked `Startup.upk` decode (Bandit pistol magazine 36 vs 30, Dahl
    pistol 16 vs 12, Bandit shotgun 10 vs 9 and reload 4.1 vs 4.4) and no live hotfix entry touches them (source
    open); single precision fixes the 1.25 fire-rate display; the game falls back to the type's title; no level line
    at stage 1 or on mission weapons.
  - Paint: an independent critic agent scored the host 4.5/10 (Maliwan) and 3/10 (Jakobs) against real inspect
    captures (earlier 6.5 and 5.0 were against wiki screenshots): too dark even unlit, wrong Maliwan orange hue,
    no element glow.
  - Inventory open in the original game: ~126-156 ms to the first page frame (screen capture, upper bound).
- **Not done:** downed/injured Phaselock checks, the auto-aim radius thresholds, a real mission turn-in, the
  level-54 save. No UE suite was run (Smart App Control blocks the module DLL; no rebuild attempted).

