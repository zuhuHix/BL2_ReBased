# Native AI action sequences: atomic sections, "is running", "ready to finish", and the time ranges (draft, 2026-11-16)

AI-assisted (Claude), analyst lane W7-AI part 2. Read from the game executable with Ghidra (`tools/ghidra/run.ps1`, read-only
project), written in our own words; no decompiler output, pseudo-code, addresses or listing structure is reproduced here.
Field names and offsets come from the cooked packages (`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the
running game**; each section ends with the live observation that would confirm it.

**Clean room.** The lane that wrote this note does not implement it. In the Rust port these natives answer `NotYet`
until a fresh implementer codes them from this note alone (decision 2026-11-16-66).

## Summary

| Native | Script signature (from the package) | Used by the bullymong's actions | Confidence | Status |
|---|---|---|---|---|
| `ActionSequence.StartAtomic` / `StopAtomic` / `IsAtomic` | `native function StartAtomic()`, `StopAtomic()`, `bool IsAtomic()` | `Action_AnimAttack` state `Attack` (begin and end) | high | UNVERIFIED |
| `ActionSequence.IsRunning` | `native function bool IsRunning()` | `Action_GenericAttack.CheckForNewZone` | high | UNVERIFIED |
| `ActionSequence.IsActionReadyToFinish` | `native function bool IsActionReadyToFinish()` | not called by the bullymong's scripts | medium | UNVERIFIED |
| `ActionSequence.RangeIsValid` / `GetRangeValue` | `native function bool RangeIsValid(...)`, `float GetRangeValue(...)` | action scripts that read a min/max range | high | UNVERIFIED |
| `Action_GenericAttack.InitData` | `native function InitData()` | `Action_GenericAttack.Init` | low (not resolved) | not read |

## The atomic section

- **State:** three flags of the action object, in this order of one flag word: `bInitialized`, `bDoWorkFinished`,
  `bInAtomicSection` (`ActionSequence`, from the package layout).
- **`StartAtomic()`:** when `bInitialized` is set, `bInAtomicSection` becomes true; otherwise nothing changes.
- **`StopAtomic()`:** when `bInitialized` is set, `bInAtomicSection` becomes false; otherwise nothing changes.
- **`IsAtomic()`:** true exactly when both `bInitialized` and `bInAtomicSection` are set.
- **Edge cases:** an action that was never initialised can neither enter nor leave an atomic section, and always
  answers false. Calls do not nest or count: a second `StartAtomic` changes nothing, and one `StopAtomic` ends it.
- **Who sets `bInitialized`** was not read here (the AI's start-up, which creates the action objects; see "Not read").
- **Relevance:** NATIVE_AI_AWARENESS.md's periodic state update leaves a running action alone when "both its running
  and its non-interruptible flag" are set. That note does not name the flags. Reading: the non-interruptible flag is
  `bInAtomicSection` (the only flag these natives write). Which flag is "running" stays open (see "Not read").
- **Live confirmation:** during a bullymong's melee swing (`Action_AnimAttack` in state `Attack`), read the action's
  `bInAtomicSection` (true) and `bInitialized` (true), and after the swing ends (false, true); call `IsAtomic` on a
  freshly copied, uninitialised action (false).

## IsRunning

- **Returns** true when the action's `AI` interface is set, the AI's component has a current state, and that state's
  node names this action as its `Action` (`AIComponent.CurrentState` points at an `AITreeData` node). Otherwise false.
- **So:** "running" means "this action is the AI tree's current leaf", not "its state code is running". An action
  that is set up but not selected answers false.
- **Live confirmation:** on a bullymong in a fight, `IsRunning` on the running melee action true, on the patrol action
  false; after the tree switches, the two answers swap in the same frame.

## IsActionReadyToFinish

- **Returns** true when the action has no latent action pending in its state code (a field of the object's state
  frame reads zero; read as the frame's pending latent action, UNVERIFIED reading of the field) **and**, when the
  action has a `ParentSequence`, that parent answers true in turn (the same question is asked up the parent chain).
  An action whose state code is waiting in a latent call answers false.
- **Subclasses:** the `WillowActionSequencePawn` class may override the answer; its function table was not resolved
  by the tooling, so the override (if any) is not read.
- **Live confirmation:** call it on an action sleeping in state code (false) and on one that is not (true).

## RangeIsValid and GetRangeValue

- **Valid range:** both ends greater than zero and the minimum not greater than the maximum (the same validity
  NATIVE_AI_AWARENESS.md gives for the tree's timed ranges).
- **`RangeIsValid(range)`:** true for a valid range, else false.
- **`GetRangeValue(range)`:** 0.0 for a range that is not valid; otherwise `min + (max - min) * r / 32767`, where `r`
  is one draw of the C runtime's `rand()` (0 to 32767), computed in single precision. **The divisor here is 32767**, so
  the maximum itself can come out. NATIVE_AI_AWARENESS.md gives 32768 for the tree's own draws (its timed ranges); that
  is a different code path and was not re-read here (see "Not read").
- **Live confirmation:** call `GetRangeValue` on a range of (10, 10) (10.0) and on (20, 30) many times (all values
  within 20 to 30, the maximum reachable).

## Not read

- `Action_GenericAttack.InitData`: the native only dispatches to a virtual function of the action; the class's
  function table was not found by the tooling, so its body is not read.
- Which flag is the "running" flag of the periodic update (NATIVE_AI_AWARENESS.md); who sets `bInitialized`,
  `bDoWorkFinished` and the action's context fields (`AI`, `MyGearboxMind`, `MyGearboxPawn`, `WillowAI`,
  `MyWillowMind`, `MyWillowPawn`) when the AI creates its action objects; and what "back to its default script state"
  at a state's stop does.
- Whether the tree's own random draws use 32768 (NATIVE_AI_AWARENESS.md) or 32767 (as `GetRangeValue` does).
- `Sleep`, `ExecuteSequence`, `RunSubSequence`, `InterruptLatentAction`, the rule-set natives, `IsInLabel`,
  `GetActionSequence`.
