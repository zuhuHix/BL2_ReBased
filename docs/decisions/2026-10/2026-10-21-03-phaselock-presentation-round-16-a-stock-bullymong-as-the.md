## 2026-10-21: Phaselock presentation round 16: a stock bullymong as the lifted target, cast-arm blends, intro flash

AI-assisted (Claude). Host, tools and docs (`OpenWillowCombatTarget.*`, new `OpenWillowTargetAnimInstance.*`,
`OpenWillowArmsAnimInstance.*`, `OpenWillowPhaselockFx.cpp`, `import_phaselock_fx.py`, a few lines in the shared
`OpenWillowWalker.cpp`; new `prepare_phaselock_target.py`, `import_phaselock_target.ps1`, `phaselock_target_editor.py`,
`real_game/scripts/phaselock_target_anim.py`); no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` "Round 16".

- **Target animation (maintainer item 2):** the slice dummy has no `PhaseLock_*` clips, so the host's lifted-target chain
  had never run. `-owcombatpawn=Bullymong` puts the stock bullymong (all four clips) in as the test target, with the stock
  move blends (lift in over 0.4 s, others 0.1 s, fall stretched to `DropTime`), the bubble sized from its own bounds (no
  `-owbubbleradius` stand-in) and the mesh centring. Observed in the real game (SDK probe, two casts): the mesh is lowered
  119.5 uu through the hold and the base pose blends out over about 0.4 s after the cast; the host's centring is fitted to
  the first (58.7 uu bias), the second matches the stock blend.
- **Mesh scale open:** the live component reads `Scale` 1.0, the archetype 0.7; frame sizes favour 0.7, the live head
  height favours 1.0. The host uses 0.7 and divides the bounds by it (`UNVERIFIED` reading).
- **Cast arm (item 3):** stock 0.1 s blends with the blend-out ending; the round 8 rate 0.85 stays (the arm drops about
  0.1 s early at rate 1). The hand smear was UE5's per-bone motion blur, now off on the arms and the stock pawn.
- **Bubble (item 1):** the intro's `Mat_SirenGlowMOD` flashes use the shader's own overbright (1) instead of the hand's
  0.4 fit.
- **Visual review:** three independent critic rounds against the matched real-game frames: overall 5.8, 5.7, 5.5 (bubble
  6.0 / 5.5 / 5.5, target 4.5 / 4.5 / 4.5, arm 7.0 / 7.0 / 6.5). The 8/10 target was not reached. Open: the held pose shows
  the back where the game shows the front, the lift still reads early at 0.5 s, the bullymong's skin colour, the opening
  cyan sweep and the plain rim, the eye motif under the target (HUD, not this lane).
- **Checks:** CTest 10/10; `verify_packages` 9/9; `tests/prepare_phaselock_target_test.py` 4 OK, `particle_system_test`
  OK, `prepare_action_skill_test` 9 OK; Phaselock shots 28 frames; quest suite 79/0 and 11/0 (a first run failed checks
  45 and 47 because this worktree had no turn-in reward data; passed after `weapon_slice_gear.py --reward-only`).

