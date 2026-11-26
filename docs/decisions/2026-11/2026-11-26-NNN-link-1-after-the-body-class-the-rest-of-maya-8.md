## 2026-11-26: Link 1 after the body class: the rest of maya:8

AI-assisted (Claude Code)

Lane ANALYST-MAYA9-AND-AFTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** #771 predicts that `InitializeBehaviorProviders` completes. The hub asked what link 1 meets after it,
  through `maya:8` and `players:1`, and whether links 3, 4, 5, 13 and 14 then reach their own steps.
- **What:**
  - the new note `docs/verification/NATIVE_MAYA9_AND_AFTER.md`;
  - six NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (four natives and their class-table slots);
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off strict `--slice-gate` on main 3b8df87 with a call trace, each earlier stop
    stubbed in turn. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/maya9/`.
- **Findings** (UNVERIFIED in game):
  - the stop chain is in the health set-up's `ApplyCharacterClassDefaults`;
  - after lane E's two stops it meets, in order: `IsPlayerPawn` (unbound); `RunStance0` unloaded;
    `SetDefaultStance`; `AttachComponent` in the combat world; the `MaterialInstanceConstant` keys; the shield idle
    hook; `ApplyFlagInitializationData`; the playthrough effects;
  - the four natives read are no-ops for Maya: no shield, an empty flag list, playthrough 0, and the run stance's
    provider not registered;
  - with every stop stubbed, link 1 is ok, and links 3, 4, 13 and 14 stop at their own spawn steps.
- **Proposed:**
  - Off answers for the four natives;
  - bind the three existing rules where they are missing;
  - a labelled harness preload of the body class's objects (the hub's call).
- **Clean room:** a different lane implements this note.
