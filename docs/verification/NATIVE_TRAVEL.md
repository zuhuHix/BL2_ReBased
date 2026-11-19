# Native travel: `WillowGameInfo.TravelPostCountdownInProcess` (2026-11-19)

AI-assisted (Claude Code). Lane MAP-LOAD-B, analyst only (clean room, docs/LANE_BRIEF.md; decision 2026-11-19-13). Read
locally from `Borderlands2.exe` with the project's Ghidra tooling (read-only queries and a local read-only table reader;
raw output only under ignored `local/analysis/`), in our own words. Nothing here was observed in the running game:
**UNVERIFIED** until it is. The lane that wrote this note does not implement it; it stays `NotYet` in Off until a fresh
implementer codes it from this note.

## Summary

| Native | Script signature (from the package) | Used by | Confidence | Status |
|---|---|---|---|---|
| `WillowGameInfo.TravelPostCountdownInProcess` | `native function bool TravelPostCountdownInProcess()` | six installed script functions: `WillowPawn.TakeDamage`, `WillowPawn.TakeRadiusDamage`, `WillowPlayerPawn.TakeDamage`, `WillowPawn.CausePlayerDeath`, `WillowPlayerController.CausePlayerDeath`, `PlayerKillVolume.Touch`; `--combat-run`'s swing steps reach it in Off (decision 2026-11-19-06) | high | UNVERIFIED |

## The rule

- The script native takes no arguments and returns what one virtual function of the game info returns. The script
  native's own code is shared by the linker with an unrelated native (`Pawn.IsDead`), so the name the native tables give
  is not its own; the virtual it calls was resolved in `WillowGameInfo`'s function table, read from that class's
  constructor.
- **The virtual answers whether the game info's `TravelPostCountdownTime` is greater than 0.** Nothing else is read or
  written. `TravelPostCountdownTime` is a script int of `WillowGameInfo` (named by the class layout; the layout's total
  size equals the size the class registration passes).
- So the native has no state of its own: it answers from a script field.

## What sets the field (installed script, for the implementer's context)

- The class default is 0, so a fresh game info answers **false**.
- `WillowGameInfo.InitiateTravel` sets it to 3 when a travel starts; `WillowGameInfo.ClearTravelData` sets it to 0; the
  script's post-countdown also writes it (not traced here). All of that is UnrealScript the VM runs.
- In a Sanctuary run with no travel started, the answer is therefore false throughout.

## Edge cases

- A game info that is not a `WillowGameInfo` cannot reach this native (it is declared on `WillowGameInfo`).
- A negative value answers false (the test is "greater than 0").
- A subclass replacing the virtual was not looked for; `WillowGameInfo`'s own table holds the field test.

## Live confirmation

With bl-sdk: read `TravelPostCountdownTime` and call `TravelPostCountdownInProcess()` on Sanctuary's game info before
and during a fast travel; expect false at 0, true while the field is above 0.
