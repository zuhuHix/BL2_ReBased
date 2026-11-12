## 2026-10-22: Phaselock presentation rounds 17-20: capture FOV, camera pitch, stock dark interior, live mesh scale

AI-assisted (Claude orchestrating, Codex GPT-6 Luna implementing and critiquing). Host, tools and docs
(`OpenWillowPhaselockFx.*`, `OpenWillowCombatTarget.cpp`, `import_phaselock_fx.py`, `prepare_phaselock_target.py` and its
test, `run_phaselock_shots.ps1`, a few additive lines in the shared `OpenWillowWalker.cpp`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 17" to "Round 20".

- **Captures:** the `-owfov` value was converted from 4:3 to 16:9 a second time, so round 17's host frames were about 94
  degrees wide instead of 77.6; with the corrected input the bubble rim measures 420 px against the game's 422 px (no scale
  fitted). The capture camera is now pitched like the game's (`-owphaselockpitch`, set after target selection so the cast
  still hits).
- **0.80 s line:** not a host bug in kind. The game frame has the same cut where the ground passes through the huge
  `InitialFlash` sprites' soft-particle fade; the host's sat higher because its camera was level. The flash overbright is
  back to the 0.4 fit (round 16's 1.0 over-bleached the view; `UNVERIFIED`).
- **Bubble interior:** the bubble's `ModulateBlack` uses the shader's own values (cap 1, fade floor 1) instead of the round
  7-14 compensations, which had made it see-through.
- **Mesh scale:** the bullymong uses the live component scale 1.0 and the plain mesh bounds (307 / 245 / 200 uu against the
  game's 290 / 230-260 / 193), replacing 0.7 and the "bounds divided by scale" reading. The earlier frames favouring 0.7
  predate the FOV fix. Archetype (0.7) and live (1.0) still disagree; `UNVERIFIED`. A 0.9 multiplier fitted from a critic's
  estimate in round 19 was removed again.
- **Facing:** the test pawn's yaw offset is 0 (the held pawn faces the player, as in the game frames); the stock rule was
  not read (`UNVERIFIED`).
- **Visual review:** independent critic, frames only: 5.9 (round 17, three frames), 5.6, about 5.8, about 5.4 (six frames).
  The 8/10 target was not reached. Open: the held pose (folded, limbs high, against compact and head-forward), the 0.80 s
  shell that hides the target in the game, the bullymong's brown colour (no material graph found), the irregular cyan rim,
  the palm orb's cobalt core and strands. The eye motif low in the view is the HUD's action-skill indicator.
- **Checks:** CTest 10/10; `verify_packages` 9/9; Python tests 49 files and JS tests 2 files pass; quest suite 79/0 and
  11/0; Phaselock shots 28 frames (cast hits the bullymong). No flaky test seen.

