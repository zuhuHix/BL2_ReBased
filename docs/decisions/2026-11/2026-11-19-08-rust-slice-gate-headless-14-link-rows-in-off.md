## 2026-11-19: Rust slice gate: headless 14-link rows in Off and their checker

AI-assisted (Claude Code). Lane SLICE-RUN-OFF, step 5 of the slice plan (`docs/rust-port/SLICE_RUN_PLAN.md`). This lane
builds the headless measure of the slice gate (`docs/verification/SLICE_GATE.md`) in `ParityMode::Off`, so that every
later lane can measure itself against it. It adds no gameplay rule, binds no native, and loosens no `NotYet` guard or
bounds check. No dependency was added. There was no real-game session, and nothing here says anything about the real
game.

- **The command.** `ow-package --slice-gate --cooked <dir> [--past-errors]` lives in `crates/ow-cli/src/cmd/slice_gate.rs`
  (a Rust-only flag, family `SliceGate`). It refuses to run under `OW_PARITY=reference`.
  - It prints one JSON row per link, in gate order. Each row gives `n`, `link`, `grading`, `result` (ok, notyet or
    error), `driver`, `steps` and `stop`. The stop is `{step, native, record, text}`. Each row also gives `blocked_by`,
    `after_errors_in`, `error_lines` and `scope` (what the driver covers of the link).
  - Exit code: 0 for 14 ok; 3 when no link is an error but some are notyet; 1 otherwise.
- **Drivers: the existing commands, unchanged.**
  - The combat run (`ow_combat::run::Run` over the mission bridge's player, as `--combat-run` builds it) drives links 1,
    3, 4, 5, 13 and 14 on one world:
    - link 1: `maya:8` and `players:1`;
    - link 3: a baby bullymong at level 6 and one lethal scripted hit;
    - link 4: an adult at level 7 and one melee swing;
    - link 5: the death roll of link 3's kill;
    - link 13: `cash:3000` and three more swings;
    - link 14: `bleed:50`.

    These follow decision 2026-11-16-54's S-d and S-f.
  - The Fire slice (`FireMissionSlice`, as `--slice-run` builds it, including its rerun on the definition store's copy
    refusal) drives links 10 to 12 on a second world. It uses the step list of `tools/verify_fire_event_order.py`.
  - Links 2, 6, 7, 8 and 9 have no headless driver. Each is `notyet` with its reason:
    - 2: movement and the fight area are host-side, and the map world is not brought up for play;
    - 6 and 7: pickup is host-side;
    - 8: no run step joins a looted gun to a fire;
    - 9: Phaselock has no cast step.
  - The two worlds are separate. The gate's single session is not modelled, because the slice bridge refuses a world
    that already has actors and the map world is not brought up for play (decision 2026-11-19-04).
  - In `ow-cli`, `slice.rs` made `open`, `load_set`, `run_step`, `Failure` and `COPY_REFUSAL` crate-visible. No
    behaviour changed.
- **Grading.**
  - **Strict, the default and the gate.** A link stops at its first `NotYet`, error line, failed step or step error. With
    no stop it is ok only if its evidence is there:
    - a row of the combat report: `kill.`, `ratio.`, `loot.roll.`, `down.total.` or `down.respawn_health.`;
    - for the Fire links, the mission status after the link: 1, 2 or 3.

    Without the evidence the link is an error. A link whose prerequisite is not ok is not run: it takes that link's
    result and stop, with `blocked_by`.
  - **`--past-errors`, informational only.** An error line (which the drivers record and go on past, as `--slice-run`
    does) is kept in `error_lines` and makes the link an error, but the steps go on. Only a hard stop blocks. So each
    row shows how far its own steps get.
  - A stop text with a `NotYet` display (any of the four `Missing` phrases) is notyet, with its subject and record.
    Anything else is an error, with the unbound native's key when the text names one.
- **The checker.** `tools/check_slice_run_off.py` reads the rows. It takes the 14 names, in order, from
  `tools/check_slice_gate.py`'s `LINKS`; that host-log checker is kept for its own format.
  - It checks the structure:
    - exactly 14 rows, `n` 1 to 14, and the right names;
    - every field present, with the right type;
    - one grading for all rows;
    - a stop present exactly when the result is not ok;
    - `blocked_by` and `after_errors_in` name an earlier link that is not ok, and a blocked row carries that link's
      result;
    - no ok row without a driver.
  - It prints the per-link table and the counts, and can write them as markdown and JSON.
  - It passes only 14 ok under strict grading (exit 0). A past-errors run never passes. A malformed run exits 2.
- **Proof.**
  - **Synthetic.** The checker has 10 tests in `tests/check_slice_run_off_test.py`, which is not in ctest. They cover
    the pass, the counts, past-errors never passing, order and names, each field's type, the stop rule, one grading,
    `blocked_by`, a link without a driver, and the exit codes and outputs.
  - The command has 9 Rust unit tests:
    - the 14 names match the existing checker, in order;
    - prerequisites come earlier and share a driver;
    - `NotYet` texts split into subject and record for all four kinds, and an unbound native's key is read;
    - driverless links are notyet, and an unrun driven link is an error;
    - blocking, and missing evidence as an error;
    - the exit codes;
    - the row JSON, field by field;
    - the pawn placeholders;
    - past-errors against strict grading.
  - **Installed, on main 86c28d00** (pc-hummer, Off, about 7 s per run; rows under ignored `local/p2/slice-run-off/`):
    - **Strict: 0 ok, 5 notyet, 9 error.**
      - The combat links stop at link 1's `maya:8` on the unbound `WillowGameInfo.GetSkillManager()`; links 3, 4, 5,
        13 and 14 are blocked by it.
      - The Fire links stop at link 10's `use` on the unbound `AIComponent.ActivateEvent(name)`; links 11 and 12 are
        blocked.
      - The five links without a driver are notyet.
    - **Past-errors: 0 ok, 12 notyet, 2 error.**
      - Link 1 has its error line, as above.
      - Links 3 and 4 run their own spawn steps and stop `NotYet` there, on the game replication info query, which
        needs the map load's handler (`architecture.md` 3.8-3.9, `NATIVE_ATTRIBUTES.md` section 5). Links 5, 13 and 14
        are blocked behind them.
      - Link 10 has two error lines: `AIComponent.ActivateEvent` at `use`, and
        `MissionTracker.GetObjectivesProgress(object,array,bool)` at `accept`.
      - Link 11 runs `range`, `spawn` and its ticks, and stops `NotYet` at `hit:fire`: "Pawn IAIInterface routing is
        unread" (`NATIVE_AI_HOLDS.md`). Link 12 is blocked.
    - **Cross-checks.**
      - `--combat-run` with link 1's and link 3's steps records the same three errors: the `GetSkillManager` line, the
        replication info `NotYet` at the spawn, and "no pawn 0" at the hit.
      - `--slice-run` with the Fire steps runs through `spawn` (status 1 from `accept` on) and exits 3 at `hit:fire` on
        the same `NotYet`.
      - `tools/verify_fire_event_order.py` cannot parse that cut-off output; it was not changed here.
- **For the hub.** These rows replace the hand-built stop lists. The first stops are:
  - `WillowGameInfo.GetSkillManager`, for the combat group;
  - the game replication info handler, at every Off spawn;
  - `AIComponent.ActivateEvent` and `GetObjectivesProgress`, for the mission, both analyst-read (decision 2026-11-16-81);
  - the pawn's `IAIInterface` routing, at the fire hit.

  The five driverless links each need a driver lane before they can count.
- **UNVERIFIED:** everything about the real game. The step lists are the earlier lanes' own, and the evidence keys say
  that the driver recorded the outcome, not that the outcome matches the game.
