# Native rules behind the Fire slice's Off stops: objective progress, the AI event broadcast, a trigger's sprites (draft, 2026-11-16)

AI-assisted (Claude), analyst lane FIX-SLICE-OFF-1. Read from the game executable with Ghidra (`tools/ghidra/run.ps1`,
read-only project), written in our own words; no decompiler output, pseudo-code, addresses or listing structure is
reproduced here. Field names come from the cooked packages (`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED
in the running game**; each section ends with the live observation that would confirm it.

**Clean room.** The lane that wrote this note does not implement it. In the Rust port these natives stay unbound in Off
(the gameplay stub policy refuses them, as decision 2026-11-16-50 does for unread natives) and keep the C++ stub under
Reference, until a fresh implementer codes them from this note alone (decision 2026-11-16-81).

## Summary

| Native | Script signature (from the package) | Reached in the Fire slice | Confidence | Status |
|---|---|---|---|---|
| `MissionTracker.GetObjectivesProgress` | `(MissionDefinition Mission, out array<int> Progress, bool ...)` (key `(object,array,bool)`) | the mission screen's script, at accept and turn-in | medium | UNVERIFIED |
| `AIComponent.ActivateEvent` | `(name EventName)` with a result (key `(name)`) | an AI component's script event, from `player:` on | medium | UNVERIFIED |
| `Trigger.TriggerDetachSprites` | `()` | a trigger's script | low | not resolved |

## MissionTracker.GetObjectivesProgress

- **Inputs:** the mission definition, the out progress array, and a flag (its script name was not checked here).
- **Does:** finds the tracker's record for the mission. With no record, it returns and the array is left as passed.
  Otherwise the array receives the record's objective progress (one value per entry of the mission's `ObjectiveDefs`,
  read as a copy of the record's progress; the copy routine was not opened).
- **With the flag set**, a second pass walks the mission's objectives. Each objective with `bAllOrNothing` that fails
  one test (a routine of the objective, read as "is not complete", not proven) gets its entry, at its index in
  `ObjectiveDefs`, set to 0. So an all-or-nothing objective shows 0 until it is done. Other objectives keep their value.
- **Edge cases:** an objective missing from `ObjectiveDefs` changes nothing; an index past the array's length is
  skipped.
- **Live confirmation:** with bl-sdk, call `GetObjectivesProgress` on the Fire mission before and after the range
  objective, with the flag false and true, and compare with `MissionPlaythroughs`' progress; then the same on a mission
  with a partly counted `bAllOrNothing` objective (expected 0 with the flag, the count without it).

## AIComponent.ActivateEvent

- **Does nothing unless** the component's `AI` interface and its `AIDef` are both set.
- **Then:** if `CurrentState` (the running node) has an action object, that action gets the event first (through its
  own routine, read as raising `ActivateEvent(EventName)` on it, not opened). Then every node of `AIDef.NodeList` other
  than the running one whose `Action` is set gets the script event `ActivateEvent(EventName)`, in list order.
- **Result:** always false.
- This is the same order NATIVE_AI_MELEE.md gives for the target events (running action first, then every other action
  of the tree). That the walk reads the `Action` fields of `AIDef.NodeList` itself suggests the action objects are
  per-AI copies in that list, which part 2 of the AI port does differently (it copies the archetypes into its own
  brain): which objects the game's list holds is open.
- **Live confirmation:** hook `ActionSequence.ActivateEvent` on a bullymong's actions, call its component's
  `ActivateEvent('Invented')`, and read the order and the receivers; read whether `AIDef.NodeList[i].Action` is the
  archetype or a per-AI object.

## Trigger.TriggerDetachSprites

- The script native calls one virtual function of the trigger, with no arguments and no result. Which one (by its name,
  detaching the editor sprite components) was not resolved: its code is folded with an unrelated native's by the linker,
  so the name the tables give is not its own.
- **Live confirmation:** list a Sanctuary trigger's components before and after the call.

## Not read

- The copy of the record's progress, and the objective's "is complete" test, in `GetObjectivesProgress`.
- What the running action's first delivery in `ActivateEvent` does beyond the event.
- The trigger's virtual.
