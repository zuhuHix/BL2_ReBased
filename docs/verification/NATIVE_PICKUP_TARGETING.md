# Native rules of the per-tick pickup targeting: which pickup the player sees (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-PICKUP-TARGETING (decision 2026-11-19-74), for slice link 7 (the
aimed pickup wins over the nearest). It answers open question 1 of NATIVE_PICKUP.md (#447): the per-pickup score, the
view cone, the distance term, the line check, and how `SawPickupable` and `ClearSeenPickupable` are raised.

**How it was read.** No bl-sdk session was granted, and no live evidence was recorded for this pass. The native was
read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and two small local listing scripts,
read-only, under the machine lock). Field names come from the cooked packages through `tools/ghidra/class_layout.py`;
for `WillowPickup` its layout is 16 bytes short of the native class, and every field named here is consistent once
that is allowed for. Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses,
offsets or call structure is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists
the live observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unported, or `NotYet` in Off, until a fresh
implementer codes it from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The pass: who runs it, and when it does nothing | native (the Willow HUD's per-tick pass; no script name) | high | UNVERIFIED |
| Candidates and eligibility (`WillowGlobals.PickupList`, `bPickupable`, the range, the 0.9 horizontal cone) | native | high | UNVERIFIED |
| The score (a vertical segment through the pickup, the view ray, the capture radius) and the ranking | native | high | UNVERIFIED |
| The line check and the pawn test before it | native | high | UNVERIFIED |
| `SawPickupable` / `ClearSeenPickupable`, and the 0.5 view check before them | native | high | UNVERIFIED |
| The shrink bookkeeping in the same pass (`BeginShrinking`) | native | medium | UNVERIFIED |
| Presentation in the same pass (hover particles, highlight colour, interaction icon) | native | medium (not read in detail) | UNVERIFIED |

## 1. Who runs it, and what it reads

- **The pass belongs to the player's HUD** (`WillowHUD`), not to the controller: a native per-tick pass of the HUD. It
  reads the HUD's owner (`HUD.PlayerOwner`, the player controller) and the HUD's own definition (a `HUDDefinition`).
  With no owner or no HUD definition, the pass does nothing at all this tick: no event is raised and nothing changes.
- **The view.** The pass asks the owner for its player view point: a location `V` and a rotation (pitch and yaw). That
  is the point and direction the player's camera shows. The headless world has no camera (hub ruling): this is the
  input a later host lane supplies. Everything below that says "view" means this location and rotation.
- **The HUD keeps two pickups between ticks:**
  - the **hover pickup** `H`: the pickup currently highlighted, the one the score favours;
  - the **reported pickup** `S`: the last pickup `SawPickupable` was raised for.

  It also keeps a nearest-pickup slot, a side duty (section 3), and a per-tick "seen item" slot used by the
  presentation.
- **The events** are raised on the object the HUD keeps for its owner. The controller's script handles both events
  (NATIVE_PICKUP.md section 4: `SawPickupable` records `CurrentSeenPickupable`).

## 2. When there is no scoring

- **The owner has no pawn** (`Controller.Pawn` is None): the pass only keeps its nearest-pickup slot, over the HUD's
  `PostRenderedActors`. It raises no event and leaves `H` and `S` as they are.
- **Scoring is suspended** when the owner controller is in its `Dead` state, or when its pawn, or the vehicle the pawn
  drives (`Pawn.DrivenVehicle`), answers a suspend query. For a `WillowPlayerPawn` that query always answers no.
  While suspended, no candidate is scored, so there is no winner (section 6 then applies).

## 3. Candidates and eligibility

The candidates are the entries of the game's globals' pickup list (`WillowGlobals.PickupList`), in list order. An entry
counts only when it is a `WillowPickup` and its `bPickupable` flag is set; any other entry is skipped.

For each counted pickup `P` (location `L`), when scoring is not suspended:
1. **The nearest-pickup slot** (side duty): the pickup with the smallest distance from `V` among those that have an
   inventory. Nothing on the targeting path reads it.
2. **The range or highlight gate:** `P` is considered when the globals definition has a hover particle
   (`GlobalsDefinition.PartItemHover` is set), or when the distance from `V` to `L` is below the HUD definition's
   `HUDDefinition.PickupRange`.
   - Correction to NATIVE_PICKUP.md section 4: the range is the HUD definition's `PickupRange`, not
     `GlobalsDefinition.PickupRadius`.
   - With a hover particle set, the range does not gate scoring. It still gates the event (section 7).
3. **The cone:** the angle between the view's horizontal direction (the yaw alone) and the horizontal direction from
   `V` to `L` (height ignored) must have a cosine above 0.9.
4. A pickup that passes 2 and 3 is scored (section 4).

## 4. The score

For a candidate `P`, with "current" meaning `P` is the hover pickup `H`:
- **The centre** `C` is `P`'s location plus its pickup particle offset. That is the offset `WillowPickup`'s native
  `GetPickupParticleOffset` answers: the inventory definition's `WillowInventoryDefinition.PickupFlagOffset`, turned by
  `P`'s rotation (its x, y and z along `P`'s forward, right and up axes). The offset is zero when that vector is zero
  or there is no definition. Reading a non-zero offset also sets `P`'s `bHasPickupParticleOffset`.
- **A vertical segment** through `C`:
  - top: `C` raised by `GlobalsDefinition.ItemHoverProjectionHeight`, times 1.25 when current;
  - bottom: `C` lowered by half the height of `P`'s components' bounding box, times 0.75, or times 1.5 when current.
- **The view ray:** from `V` along the full view direction (pitch and yaw), with a length of 100 times the distance
  from `V` to the segment's top.
- **The distance `d`:** between the closest points of the two lines (segment and ray). It only counts when both closest
  points lie inside their pieces. With either piece of near-zero length, the lines parallel, or a closest point
  outside its piece, there is no score.
- **The capture radius:** `R = GlobalsDefinition.HoverWidthPickup × 0.1`, times 1.5 when current.
- **The score:** `(R − d) / R` when there is a `d` and it is at most `R`, else 0. The score lies between 0 and 1, and
  is 1 when the ray passes through the segment.
- **The ranking value:** a candidate with a score above 0 gets the value `(score, plus 0.25 when current) / |L − V|`.
  The distance is to the pickup's location `L`, not to `C`. The highest value wins; on a tie the earlier candidate in
  list order is kept. A candidate whose score is 0 is never chosen.

## 5. The line check

When there is a winner `W` (the pass runs these only in a game world, which the port always is):
1. **The pawn test:** if the owner's pawn is a `WillowPlayerPawn` and it is `bDeleteMe`, `bIsInjured` or `bIsDead`,
   there is no winner this tick.
2. **The trace:** if the horizontal distance from `V` to `W` is more than `HUDDefinition.PickupRange × 0.25`, a world
   line check runs from a point 25 units back from `W` toward `V` (along the direction from `V` to `W`) to `V`. If it
   is blocked by anything other than `W` or `W`'s `Base`, there is no winner. Within that horizontal distance no trace
   runs.

## 6. No winner

When there is no winner (no candidate, suspended scoring, or a refusal in section 5):
- if a pickup was reported (`S` set), `ClearSeenPickupable` is raised and `S` is emptied;
- if there was a hover pickup, its highlight is switched off and `H` is emptied.

## 7. A winner: the events

With a winner `W`:
1. **The pickup's item:** `W`'s inventory and its definition are read. With no definition, nothing more happens this
   tick: no event, and `H` and `S` stay as they were.
2. **A second view check:** the cosine between the view direction and the direction from `V` to `W` must be above
   0.5. It is horizontal when the globals definition has a hover particle, and in three dimensions otherwise. When it
   fails, nothing more happens this tick.
3. **The hover pickup** becomes `W`. A different previous hover pickup has its highlight switched off first.
4. **In range** (the distance from `V` to `W` below `HUDDefinition.PickupRange`): if `W` is not the reported pickup,
   `SawPickupable(W)` is raised and `S` becomes `W`. Raising it again for the same pickup does not happen.
5. **Out of range:** no event, and `S` keeps its old value. A pickup reported earlier stays reported, and
   `ClearSeenPickupable` is not raised. The presentation shows the hover particle instead.
6. The highlight, its colour and the interaction icon follow: presentation (section 9).

## 8. The shrink bookkeeping (a side duty of the same pass)

- **When it runs:** the HUD has a `WorldInfo`, the world is not a network client, and a further query on the HUD
  answers no. That query was not read.
- **What it counts:** each counted pickup whose `LifeSpan` is above the `WillowPickup` class default's
  `PickupShrinkDuration`. The one with the smallest such `LifeSpan` is tracked.
- **The event:** after the list, if the count is above a limit the HUD holds (not named here),
  `BeginShrinking` is raised on the tracked pickup.

## 9. Presentation in the same pass

Hover particles, the highlight colour (by the item's rarity) and the interaction icon (distance bands of the item's
interaction icon definition) are drawn from the same winner. They change nothing the targeting or the take reads.
They were not read in detail.

## 10. Off answers, step by step (for an implementer)

Each step is answered with a value (a plain read or a computed rule), None, or a named `NotYet`.

| Step | Off answer |
|---|---|
| The HUD and its owner (section 1) | the world has no HUD: `NotYet { LayerAbsent }` naming the HUD (its owner and its `HUDDefinition`) until a host lane provides one; which `HUDDefinition` the player's HUD holds is open question 2 |
| No owner or no HUD definition | value: nothing happens |
| The view point `V` and rotation | `NotYet { LayerAbsent }` naming the view (the camera, hub ruling) until a host lane supplies it |
| No pawn | value: no event, `H` and `S` unchanged; the nearest slot is not needed (no reader on the targeting path) |
| Suspended: the controller in its `Dead` state | value: no scoring, section 6 |
| Suspended: the pawn's query | value: no for a `WillowPlayerPawn`; a pawn of another class, or a driven vehicle, `NotYet { NotRead }` |
| Candidates, `bPickupable`, the class test | values: plain reads of `WillowGlobals.PickupList` and the flag |
| The range gate, `PartItemHover`, `PickupRange` | values: plain reads |
| The 0.9 horizontal cone | value: the rule of section 3 |
| The particle offset | value: `PickupFlagOffset` turned by the pickup's rotation; also sets `bHasPickupParticleOffset` |
| The bounding-box height | `NotYet { LayerAbsent }` naming the pickup's component bounds, while the world has none |
| The score, the radius, the ranking | values: the formulas of section 4 (plain reads of `HoverWidthPickup` and `ItemHoverProjectionHeight`) |
| The pawn test before the trace | value: the three flags |
| The trace | value: no trace within `PickupRange × 0.25` horizontally; otherwise `NotYet { LayerAbsent }` naming collision |
| The definition, the 0.5 check, the range | values: section 7 |
| `SawPickupable` / `ClearSeenPickupable` | values: raised as sections 6 and 7 say; the script side is NATIVE_PICKUP.md's |
| The shrink bookkeeping | value: nothing when no counted pickup's `LifeSpan` is above `PickupShrinkDuration`; otherwise `NotYet { NotRead }` (the enable query and the HUD's limit are open question 3) |
| Presentation | value: nothing (nothing the rules read changes) |

## 11. Link 7 with the score resolved

Link 7's fixture: the aimed gun, and a nearer decoy about 90 degrees off the aim (NATIVE_PICKUP.md section 4).
- **The decoy** fails the 0.9 cone, so it is never scored.
- **The aimed gun** is scored when it is `bPickupable`, within `PickupRange` (or the globals definition has a hover
  particle), and inside the cone. Its score is 1 when the view ray passes through its segment, and lower as the ray
  passes further off, down to 0 at the capture radius.
  - The segment's extent needs the gun's bounds height and `ItemHoverProjectionHeight`.
  - The radius needs `HoverWidthPickup`.
  - Being the only scored candidate, it wins with any positive score.
- **It is reported** (`SawPickupable`) when it also passes the pawn test, the trace (when more than a quarter of
  `PickupRange` away horizontally), the definition read and the 0.5 view check, and lies within `PickupRange`.

## Open questions

1. **UNVERIFIED:** the suspend query of a pawn of another class and of a driven vehicle (section 2), and the HUD
   presentation step the pass calls with the winner, which writes nothing the targeting reads.
2. **UNVERIFIED:** which `HUDDefinition` the player's HUD holds, and so the `PickupRange` value, and whether the
   slice's globals definition has a hover particle (`PartItemHover`).
3. **UNVERIFIED:** the shrink bookkeeping's enable query on the HUD and the limit the HUD holds (section 8).
4. **UNVERIFIED:** that the object the events are raised on is the owner controller (consistent with the script
   side, not traced to where the HUD sets it).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Read the HUD's definition and its `PickupRange`, and the globals definition's `HoverWidthPickup`,
  `ItemHoverProjectionHeight` and `PartItemHover`.
- Drop one gun. Sweep the aim slowly across it at a fixed distance and record when `SawPickupable` fires. The
  horizontal edge should match a cosine of 0.9, and the near-miss edge the capture radius.
- Aim at a gun just outside `PickupRange` and record that no `SawPickupable` fires, and that a pickup reported earlier
  stays reported (no `ClearSeenPickupable`).
- Put a wall between the view and a gun further than a quarter of `PickupRange`, and record that it is not reported.
- Two guns in the cone at different distances, both hit by the ray: record that the nearer one is reported (the
  ranking divides by distance).
