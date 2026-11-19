## 2026-11-19: Native skill manager: GetSkillManager and the first call on it, read as analyst

AI-assisted (Claude Code). Lane FIX-SKILLMGR-A, analyst only. Number 11 was given by the hub. No crate code, no
dependency, no parsing change. There was no real-game session; every rule here is **UNVERIFIED** in the running game.

- **Why.** SLICE-RUN-OFF's gate run (decision 2026-11-19-08) shows link 1 (`spawn_maya`) stopping first at `maya:8` on
  the unbound `WillowGameInfo.GetSkillManager()`, which also holds links 3, 4, 5, 13 and 14. COMBAT_DEATH.md only
  listed the native. Confirmed on main 86c28d00: installed Off `--combat-run --cooked <dir> maya:8` gives exit 1 and
  the one error `maya:8: unbound native WillowGameInfo.GetSkillManager()`.
- **What was read.** Three read-only Ghidra queries (one session at a time on pc-hummer; none other was running):
  - `GetSkillManager` and its callees;
  - `SkillEffectManager.DeactivateAllSkillsForInstigator`. It is the next native on that path: the `maya` step runs
    the controller's `ApplyCharacterClassDefaults`, whose first statement calls it on the manager.

  The script callers were found with `research/script_disasm.py`. Raw output stays under ignored `local/analysis/`.
- **Outcome, in NATIVE_SKILL_MANAGER.md (own words):**
  - `GetSkillManager` returns the game info's `GlobalSkillManager`. On the first call, when that is None and the game
    info is the authority, it spawns a `SkillEffectManager` actor at the origin, with the game info read as its owner,
    and caches it after a class check.
  - It returns None when the game info is not the authority, or when the spawn fails (the next call retries).
  - `DeactivateAllSkillsForInstigator` calls one by-name script function on each active skill of that instigator. The
    name was not resolved. On a fresh manager it does nothing.
- **Records.** NATIVE_SKILL_MANAGER.md and two NATIVE_INDEX rows. Both natives stay unbound in Off (the gameplay stub
  policy refuses them) until a fresh implementer codes them from the note alone.
- **Verified.** The clean-room scan of the note finds nothing. `python tools/decisions_index.py --check --baseline
  origin/main` passes. No code changed, so no test numbers move.
- **Open.**
  - The implementer needs a world that can spawn the manager actor with the game info as owner. In Off, a world that
    cannot spawn should answer `NotYet`, not None.
  - The skill-walk's function name is an analyst question.
  - The live checks are in the note.
