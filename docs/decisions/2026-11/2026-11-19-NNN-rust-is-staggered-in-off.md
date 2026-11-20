## 2026-11-19: WillowPawn.IsStaggered and WillowAIPawn.IsStaggered in Off

AI-assisted (Claude Code). Lane IMPL-IS-STAGGERED, slice link 9. The decision number is pending (the landing run
numbers it). Coded from docs/verification/NATIVE_IS_STAGGERED.md alone, the note as final at b8b22422 (PR #611; clean
room: lane ANALYST-ISSTAGGERED wrote it). The executable and the analysis output were not opened. Synthetic fixtures
only. Based on main c687c135.

- **What changes:**
  - `ow-combat` `hit/pools.rs` binds `WillowPawn.IsStaggered()` and `WillowAIPawn.IsStaggered()` in Off, next to
    `SetDefaultPhysics` and `GetExpLevel`, through the keep-existing `bind_fn`.
  - Both go to `is_staggered`. Reference keeps the stub policy.
- **The rule, in the note's order:**
  1. The receiver must be set and a `WillowPawn` or a child, by the class table's `by_path` and `is_child_of` only.
     Otherwise a `VmError::Script`.
  2. **S1**, `NotYet { NotRead }`, with two texts: an unloaded handle, and a slotless stand-in.
  3. The class:
     - exactly `WillowAIPawn`, or a script-only child of it: the AI rule;
     - a native descendant (`NATIVE_AI_PAWN_DESCENDANTS`, carried with the binding from
       `research/native_by_class.json`: `WillowAICranePawn`), or a child of one: **S2**;
     - exactly `WillowPawn`: false;
     - any other child of `WillowPawn`, script-only ones included: **S2**.
  4. The AI rule reads by name, with no offset: `StaggerState` must be declared as a `WillowAIPawn.StaggerStateData`
     struct, and its member `bIsStaggered` must be a bool. Otherwise a `VmError::Script` naming it.

  It writes, loads and draws nothing, so no refusal is marked "stopped after partial writes" from inside it.
- **Verified:** host unit tests on invented packages, covering the note's nine test items. Every refusal leaves #518's
  counters unchanged. The installed Off strict slice gate is identical to main's, row by row.
- **Not verified:** nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
