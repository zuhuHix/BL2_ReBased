## 2026-10-02: first native analysis: registration tables, query tooling, mission and behavior dispatch notes

AI-assisted. Tooling and behaviour notes only; no executor or parsing change yet. Policy: LEGAL.md "Analysing the
executable". Raw output stays in the ignored analysis folder; nothing here is listing or address.

- **Machinery** (`tools/ghidra/`, method in `docs/NATIVE_ANALYSIS.md` "Native registration and queries"): each native
  class has a table of name/function pairs (`<Class>exec<Function>`); fixed script native numbers bind by name. The
  table scan finds 6,877 natives in 770 tables, and all 199 numbered script natives resolve. A batch query names and
  decompiles functions by registered name, native number, string, callers or virtual slot (virtual natives such as
  `Behavior_*.ApplyBehaviorToContext` share one exec function and are reached through the class's virtual table).
  `tools/ghidra/class_layout.py` computes 32-bit field offsets from the packages (oracle: `Core.Object` 0x3C).
  About 15 s per run.
- **Behaviour notes** (`docs/verification/NATIVE_MISSION_DISPATCH.md`, all UNVERIFIED, read from native code, not
  confirmed in the game):
  - Behavior link id byte: a signed occasion selector; the event caller passes a filter (-1 = all links); a behavior
    chooses outputs by id, and the default output is followed only when `bSupportsDefaultOutputLink` is set. There
    is no once-per-event deduplication (a behavior reached by two links runs twice); events honour `bEnabled`,
    `MaxTriggerCount` and `ReTriggerDelay`; threads run depth-first.
  - Kismet: ops run from a stack, at most 1,000 per frame; a link's delay is the input's plus the output's; an input
    hit twice runs the op twice; an activated event fires all its outputs.
  - MissionTracker: updates are queued; an objective completes when progress reaches `ObjectiveCount`; a completed
    set makes the mission ready to turn in when `bCanCompleteMission`, else activates the next set when
    `bAutoEnableNextSet`, else waits for a behavior; `AdvanceObjectiveSet` only moves to `NextSet`. Mission event link
    ids: a census over 133 missions puts all 3,420 links in the predicted id ranges (structural only).
  - Consequence for the slice: the host fires every `Default` link on accept, which is why `TargetBack` appears 3 s
    after accepting; natively the first set is activated by the kickoff dialog's Finished output.
- Not yet implemented in `src/`; the host behaviour is unchanged. Checks: CTest 10/10, packages 9/9.

