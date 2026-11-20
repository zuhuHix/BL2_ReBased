## 2026-11-19: `GearboxMind.GetAIComponent` returns the mind's own `AIComponent`

AI-assisted (Claude Code). Analyst lane ANALYST-GEARBOXMIND-GETAICOMPONENT (link 9). Documentation only: no code
changed, and the lane implements none of it (clean room). **Every rule is UNVERIFIED in the running game.**

- **The note:** `docs/verification/NATIVE_GEARBOXMIND_GETAICOMPONENT.md`, with a `NATIVE_INDEX.md` row.
  NATIVE_AI_HOLDS.md's `GetAIComponent` section now points at it.
- **What was read** (local Ghidra, read-only, under the machine lock; raw output under ignored `local/analysis/`):
  - the script native calls the mind's native `IAIInterface` implementation, which returns the mind's `AIComponent`
    property, None included. It writes, loads and draws nothing;
  - `WillowMind` uses the same implementation for this entry;
  - the field was placed with structural checks. Exactly one `MapProperty` size makes `GearboxMind` end at its
    registered class size. With that size, the interface table lands on `VfTable_IIAIInterface` and the read lands on
    `AIComponent`.
- **Proposed Off answer for a separate implementer:** replace `ow_combat::holds`' NotYet binding with a plain read of
  the receiver's `AIComponent`, returned as it is. The read does not call `loaded`. Malformed calls are
  `VmError::Script`. Two cases are `NotYet { NotRead }`: an unloaded receiver handle (not loaded first), and a field
  that names a class default's template. The template test reads only the heap (the outer's `Origin::Default`, or a
  `Default__` name) and decodes nothing, and it also runs in the hold preflight. Reference's `hit.rs` stand-in is
  unchanged. The tests are in the note's section 3 (review r1 of #594).
- **Local probe (never pushed):** a merge of #535, #579, #564, #548, #555, #560, #523 and the link 9 spawn branches.
  Two probe-only patches: #589's answer for `IAIInterface`, and this answer bound in link 9's set-up. Installed Off
  strict `--slice-gate`:
  - 1 ok / 3 NotYet / 10 errors. Link 9 is "no evidence": the cast is blocked, because the returned
    `WillowAIComponent` answers no `IFlagProvider` in a world without #524's component rule;
  - with #524 added, 1 ok / 4 NotYet / 9 errors. Link 9 is NotYet at `cast`, at `WillowAIComponent`'s own
    `QueryInterface` answer (not read), the next stop.
- **Live check (bl-sdk; not run, no session granted):** the note's last section.
