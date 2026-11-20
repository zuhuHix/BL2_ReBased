## 2026-11-19: GearboxMind's QueryInterface on link 9: an own-words note and a proposed Off answer

AI-assisted (Claude Code)

Lane ANALYST-GEARBOXMIND-QUERYINTERFACE, analyst only, with Claude Opus 5.5. The file number is a placeholder until
the maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md` and one NATIVE_INDEX row. They come from a local,
  read-only Ghidra reading under the machine lock (class registrations, constructor table stores, interface-virtual
  entries and the start-up name registration) and the layout tool. Raw output stays under ignored
  `local/analysis/gmqi/`.
- **Findings** (UNVERIFIED in game):
  - `GearboxMind` and `WillowMind` do not override the interface virtual; they answer by `AIController`'s rule;
  - the rule, in order:
    - a re-entry guard;
    - `IWorldBody` and `ISoundBehavior` (compared by name) go straight to the controlled pawn when there is one;
    - then the plain answer, the pawn, `PlayerReplicationInfo`, else none;
  - the pawn accessor is `Controller.Pawn` for `AIController` and `GearboxMind`. `WillowMind` overrides it to read a
    field of its own, `MyWillowPawn` by fit;
  - nothing is written, loaded or drawn in the body read.
- **On link 9:** the target's `WillowMind` lists `IAIInterface`, so it answers itself.
- **Proposed Off answer:**
  - register the rule for `Engine.AIController` and drop `GearboxMind` from `OPEN_ANSWERS`;
  - an accessor table by class;
  - a resident-only preflight;
  - Reference unchanged; tests listed.
- **The partial writes** (queue item PREFLIGHT-ORDER-LINK9-CAST): the mark is `FlagExpressionEvaluator.Evaluate`'s.
  Inside that call a local probe saw 8 lazy definition loads, the Phaselock flags and their resolver sub-objects, and
  40 committed heap writes, with 0 draws. These are the evaluator's own loads, not an earlier native's writes. Two
  options are recorded for the hub: preload them at set-up, or a residency check first.
- **Probe** (local, never pushed, on #628's head): with the mind answering by the plain rule, installed Off
  `--slice-gate`, strict, gives ok 4, notyet 8, error 2. Link 9 passes 1 of 2 steps and stops at the unbound
  `GearboxMind.GetAIComponent()`.
- **Clean room:** a third lane implements this note. Its author implemented #602.
