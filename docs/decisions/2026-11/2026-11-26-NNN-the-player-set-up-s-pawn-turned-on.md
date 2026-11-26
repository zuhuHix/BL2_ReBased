## 2026-11-26: The player set-up's Pawn turned on

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-PLAYER-PAWN-FLAG, a harness lane on the hub's ruling, on main c1058de6
(#805 in). No native code was read. **Both changes are UNVERIFIED stand-ins.** Reference is unchanged.

- **What was decided (Off only):**
  - the slice gate's combat set-up turns on `PlayerPawnDuringSetUp`, the player set-up's controller `Pawn` (the hub's
    ruling on #796). It was deferred, off by default, until #805 bound `SetCollisionSize`, `SetHardAttach` and
    `SetBase` for the trade proxy (queue 0aq). It now logs its own set-up line (`run::PLAYER_PAWN_SETUP`). Every other
    world keeps it off;
  - #805's `NothingTouched` stand-in for `SetCollisionSize`'s touch update applied only in a world with no collision
    service. #807 gives the combat world link 9's `NoLevel` service, so link 1 would stop at the touch rule's
    `NotRead`. Under the same setting, a service that holds no level geometry now takes the stand-in too, with its own
    line (`NOTHING_TOUCHED_NO_LEVEL_LINE`). A service with geometry, and `NoLevel` without the setting, keep the
    `NotRead`. This answers NATIVE_SPAWN_SETBASEVALUE.md's open question 4 the way the lane needed it; **it needs the
    hub's confirmation**.
- **Verified (host only):**
  - `ow-world` proxy_natives: under the setting `NoLevel` takes the stand-in with its line; a service with geometry,
    and `NoLevel` without the setting, refuse with nothing written;
  - `ow-combat` run: the set-up line is logged once;
  - `cargo test --workspace -j 8 --no-fail-fast` 4,051 passed / 0 failed / 59 ignored;
  - the installed Off strict `--slice-gate` under the machine lock, base c1058de6 against this head: both **8 ok / 14,
    X = 8.5/14 = 60.7%, every row the same**. Links 1 and 6 pass. Link 1's row shows the `Pawn` set and the no-geometry
    touch line. Links 3, 4, 5, 13 and 14 stop at the spawn's `SetBaseValue` (NATIVE_SPAWN_SETBASEVALUE.md), so Maya's
    class-level modifiers are not yet exercised by a swing.
- **Not in this change:** #807's three review LOWs (queue 0as). The lane could not reach that review: it is not on
  the PR and the hub's thread is on an environment not linked to this machine.
- **UNVERIFIED:** whether the game's touch update finds nothing here; both settings are harness inputs.
