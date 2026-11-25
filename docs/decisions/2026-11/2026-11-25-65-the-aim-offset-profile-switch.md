## 2026-11-25: The aim-offset profile switch

AI-assisted (Claude Code)

Lane ANALYST-LINK1-AFTER-HOLD, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** a look-ahead for link 1 past `GetWeaponHoldDef` (#728). The first native with no note on `maya:8`'s path is
  `AnimNodeAimOffset.SetActiveProfileByName`, from `WillowPawn.SetAimOffsetProfile('Unarmed')`.
- **What:**
  - the new note `docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md`;
  - a correction marker in NATIVE_ANIM_NODE_INIT.md section 1;
  - one NATIVE_INDEX row and one GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (both execs, their slots in both class tables, the
    rebuild in outline);
  - the layout tool;
  - `ow-package --properties` and the export's payload names.

  Raw output stays under ignored `local/analysis/aimprof/`.
- **Findings** (UNVERIFIED in game):
  - by name: the first matching profile of `TemplateNode`'s list (or the node's own) goes to the index switch;
  - by index: a new valid index is written to `CurrentProfileIndex`, then the required bones are rebuilt;
  - Maya's template node holds 7 binary-serialized profiles (`Unarmed` index 3), not an empty list;
  - so her call sets index 3.
- **Proposed Off answer:**
  - a preflight;
  - `LayerAbsent` while the profiles are not decoded;
  - the rebuild's sort and parent pass `NotRead` before any write;
  - tests listed.
- **Review r1 (#730):**
  - the interim rule is conservative: an empty `Profiles` on the list's owner is `LayerAbsent` until the store can
    report "decoded and empty", because today's decoder leaves a skipped property as the empty class default;
  - a lazy `SkelComponent` is `NotRead`, and any set mesh goes to the rebuild's `NotRead` unresolved;
  - all declarations are checked first;
  - the rebuild empties both arrays before its gate;
  - the 7 entries' sizes add up to the array's bytes.
- **Clean room:** a different lane implements this note.
