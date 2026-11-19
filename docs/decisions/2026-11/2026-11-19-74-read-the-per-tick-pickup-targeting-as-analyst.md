## 2026-11-19: Read the per-tick pickup targeting as analyst

AI-assisted (Claude Code).

Lane ANALYST-PICKUP-TARGETING, analyst only. Slice link 7 (the aimed pickup wins over the nearest) needs the native that
chooses the seen pickup: open question 1 of `NATIVE_PICKUP.md` (#447). The new own-words note
`docs/verification/NATIVE_PICKUP_TARGETING.md` and two `NATIVE_INDEX.md` rows record, as behaviour only:

- **Who runs it.** The pass is the player HUD's, not the controller's. It reads the HUD's owner, that owner's player
  view point, and the HUD's own definition.
- **Candidates.** The pickups of `WillowGlobals.PickupList` that are `bPickupable`, within `HUDDefinition.PickupRange`
  (or always, when the globals definition has a hover particle), and inside a 0.9 horizontal cone.
- **The score.** The view ray against a vertical segment through the pickup's particle-offset centre:
  - capture radius `HoverWidthPickup × 0.1`;
  - top at `ItemHoverProjectionHeight`, bottom at half the bounds height × 0.75;
  - larger for the current hover pickup, which also gets a 0.25 bonus;
  - divided by the distance; the highest value wins.
- **After the winner.**
  - A pawn test and a line check: 25 units back from the pickup to the view, skipped within a quarter of the range.
  - A 0.5 view check.
  - `SawPickupable` only within range and for a new pickup; `ClearSeenPickupable` only when there is no winner.
- **Off answers.** A per-step list (value, None or a named `NotYet`). The headless world has no camera (hub ruling)
  and no HUD or bounds: those inputs are named for a later host lane.
- **A correction to `NATIVE_PICKUP.md`.** The range is `HUDDefinition.PickupRange`, not
  `GlobalsDefinition.PickupRadius`.

The native was read from the executable with Ghidra: `tools/ghidra/run.ps1 -ReadOnly` and two small local listing
scripts, read-only, under the machine lock. Field names came from `tools/ghidra/class_layout.py` (a local copy that
accepts a second field-list head). Raw output stays under ignored `local/analysis/pickup-targeting/`. No bl-sdk
session was granted. Nothing is implemented. Every rule is UNVERIFIED in the running game; the note lists the live
checks and four open questions.
