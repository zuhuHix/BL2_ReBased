# Native rules of the per-tick pickup targeting: which pickup the player sees (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-PICKUP-TARGETING (decision 2026-11-19-74), for slice link 7 (the
aimed pickup wins over the nearest). It answers open question 1 of NATIVE_PICKUP.md (#447): the per-pickup score, the
view cone, the distance term, the line check, and how `SawPickupable` and `ClearSeenPickupable` are raised.

**How it was read.** No bl-sdk session was granted, and no live evidence was recorded for this pass. The native was
read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and two small local listing scripts,
read-only, under the machine lock). Field names come from the cooked packages through `tools/ghidra/class_layout.py`;
for `WillowPickup` its layout is 16 bytes short of the native class, and every field named here is consistent once
that is allowed for. The HUD script that sets the events' recipient was read with `research/script_disasm.py`.
Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets or call structure
is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the live observations
that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unported, or a named `NotYet` in Off, until a fresh
implementer codes it from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The pass: who runs it, its order, and when it does nothing | native (the Willow HUD's per-tick pass; no script name) | high | UNVERIFIED |
| Candidates and eligibility (`WillowGlobals.PickupList`, `bPickupable`, the range, the 0.9 horizontal cone) | native | high | UNVERIFIED |
| The score (a vertical segment through the pickup, the view ray, the capture radius), its cutoffs, and the ranking | native | high | UNVERIFIED |
| The line check and the pawn test before it | native | high | UNVERIFIED |
| `SawPickupable` / `ClearSeenPickupable`: the 0.5 view check, the recipient (`WillowHUD.WPlayerOwner`) and the order of events and writes | native, and the HUD's script for the recipient | high; medium for the recipient field | UNVERIFIED |
| The shrink bookkeeping in the same pass (`BeginShrinking`) | native | medium | UNVERIFIED |
| Presentation in the same pass (hover particles, highlight colour, interaction icon) | native | medium (not read in detail) | UNVERIFIED |

## 1. Who runs it, what it reads, and in what order

- **The pass belongs to the player's HUD** (`WillowHUD`), not to the controller: a native per-tick pass of the HUD. It
  reads the HUD's owner (`HUD.PlayerOwner`, the player controller) and the HUD's own definition (`WillowHUD.HUDDef`, a
  `HUDDefinition`).
- **The view.** The pass asks the owner for its player view point: a location `V` and a rotation (pitch and yaw). That
  is the point and direction the player's camera shows. The headless world has no camera (hub ruling): this is the
  input a later host lane supplies. Everything below that says "view" means this location and rotation. The view's
  direction comes from the engine's sine table for the rotation's angle units (16384 steps per turn), not from exact
  trigonometry.
- **The HUD keeps two pickups between ticks:**
  - the **hover pickup** `H` (`WillowHUD.CachedHoverPickup`): the pickup currently highlighted, the one the score
    favours;
  - the **reported pickup** `S` (`WillowHUD.CachedBestPickup`): the last pickup `SawPickupable` was raised for.

  It also keeps a nearest-pickup slot (`WillowHUD.ClosestPickup`), a side duty (section 3), and a per-tick "seen
  item" slot used by the presentation (`WillowHUD.SavedLookAtInventory`). These four names fit the native's accesses in
  the class's declared field order (medium confidence for the names; the roles are read).
- **The order at the start of a tick:**
  1. the nearest-pickup slot is emptied;
  2. with no owner (`PlayerOwner` None) or no HUD definition (`HUDDef` None), the pass ends: no event, and `H`, `S`
     and the seen-item slot stay as they were;
  3. the view point and rotation are read from the owner. This comes before the pawn test, so both branches of
     section 2 need the view;
  4. the pawn test of section 2, then scoring (sections 3 and 4).
- **The events' recipient** is the HUD's `WPlayerOwner`, not `PlayerOwner` itself. The HUD's script sets it once, in
  `WillowHUD.PostBeginPlay`, to `PlayerOwner` cast to `WillowPlayerController`, so for Maya's HUD it is her controller
  (read from the script). The field is identified by its place in the class's declared order, two fields before
  `HUDDef` (`WPlayerOwner`, `WPRI`, `HUDDef`): medium confidence for the field, high for the script that sets it. The
  native raises both events on it **without a None check**. The controller's script handles both
  (NATIVE_PICKUP.md section 4: `SawPickupable` records `CurrentSeenPickupable`).

## 2. When there is no scoring

- **The owner has no pawn** (`Controller.Pawn` is None): the pass only refills its nearest-pickup slot, over the HUD's
  `PostRenderedActors`, using the view point. It raises no event and leaves `H` and `S` as they are.
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
   `HUDDefinition.PickupRange` (strictly below; the comparison is of squared distances, in single precision).
   - Correction to NATIVE_PICKUP.md section 4: the range is the HUD definition's `PickupRange`, not
     `GlobalsDefinition.PickupRadius`.
   - With a hover particle set, the range does not gate scoring. It still gates the event (section 7).
3. **The cone:** the cosine between the view's horizontal direction (the yaw alone, the pitch taken as 0) and the
   horizontal direction from `V` to `L` (height dropped) must be strictly above 0.9.
   - **The cutoff:** the horizontal offset from `V` to `L` is made a unit vector only when its squared length is above
     1e-8. Below that it is used as it is, so its cosine is at most about 1e-4 and the pickup **fails the cone**. A
     pickup straight above or below the view point (zero horizontal separation) is therefore never scored.
   - **Precision:** the native normalises with a fast single-precision reciprocal square root refined once, so its
     cosine can differ from an exact one in the last bits. Off answer: the cosine in single precision with an exact
     square root; a case within a few units in the last place of 0.9 is not decided bit for bit (UNVERIFIED).
4. A pickup that passes 2 and 3 is scored (section 4).

## 4. The score

For a candidate `P`, with "current" meaning `P` is the hover pickup `H`:
- **The centre** `C` is `P`'s location plus its pickup particle offset. That is the offset `WillowPickup`'s native
  `GetPickupParticleOffset` answers: the inventory definition's `WillowInventoryDefinition.PickupFlagOffset`, turned by
  `P`'s rotation (its x, y and z along `P`'s forward, right and up axes). The offset is zero when that vector is zero
  or there is no definition. Reading a non-zero offset also sets `P`'s `bHasPickupParticleOffset`.
- **A vertical segment** through `C`, from its bottom to its top:
  - top: `C` raised by `GlobalsDefinition.ItemHoverProjectionHeight`, times 1.25 when current;
  - bottom: `C` lowered by half the height of `P`'s components' bounding box, times 0.75, or times 1.5 when current.
- **The view ray:** from `V` along the full view direction (pitch and yaw), with a length of 100 times the distance
  from `V` to the segment's top.
- **The distance `d`:** between the closest points of the two lines through the segment and the ray, in single
  precision. It only counts when both closest points lie inside their pieces. The cutoffs, all definite:
  - **a near-zero piece:** a piece (segment or ray) whose three components (end minus start) are each below 0.0001 in
    magnitude gives no `d`. The segment is near-zero when its top and bottom coincide, for example a zero hover
    height with a zero bounds height;
  - **parallel lines:** no `d` when the magnitude of `|s|² |r|² − (s·r)²` is below 1e-8, with `s` and `r` the segment
    and ray vectors (single precision). A view pitched straight up or down along the segment is this case;
  - **inside:** each closest point's position along its piece, as a fraction from its start, must lie in 0 to 1,
    both ends included; otherwise no `d`.
- **The capture radius:** `R = GlobalsDefinition.HoverWidthPickup × 0.1`, times 1.5 when current.
- **The score:** `(R − d) / R` when there is a `d` and it is at most `R`, else 0. With a positive `R` the score lies
  between 0 and 1, and is 1 when the ray passes through the segment.
  - **A zero or negative capture radius** (`HoverWidthPickup` at most 0): the candidate is never chosen. A `d` above
    `R` scores 0; with `R` exactly 0 and `d` exactly 0 the native divides 0 by 0, a value that is not a number and
    never ranks. Off answer: score 0.
- **The ranking value:** a candidate with a score above 0 gets the value `(score, plus 0.25 when current) / |L − V|`.
  The distance is to the pickup's location `L`, not to `C`. The best value starts at 0 and a candidate replaces it
  only when its value is strictly greater: the highest value wins, on an exact tie the earlier candidate in list order
  is kept, and a candidate whose score is 0 is never chosen. `|L − V|` cannot be 0 for a scored candidate (it already
  failed the cone).

## 5. The line check

When there is a winner `W` (the pass runs these only in a game world, which the port always is):
1. **The pawn test:** if the owner's pawn is a `WillowPlayerPawn` and it is `bDeleteMe`, `bIsInjured` or `bIsDead`,
   there is no winner this tick.
2. **The trace exemption:** the trace runs only when the horizontal distance from `V` to `W` is strictly more than
   `HUDDefinition.PickupRange × 0.25`. Within that distance no trace runs and `W` stays the winner.
3. **The trace:** a world line check from a point 25 units back from `W` toward `V` (along the direction from `V` to
   `W`, pitch and yaw) to `V`. A hit on `W` itself or on `W`'s `Base` is allowed. A hit on anything else means no
   winner. No hit keeps `W`.

## 6. No winner

When there is no winner (no candidate, suspended scoring, or a refusal in section 5), in this order:
1. the seen-item slot is emptied;
2. if a pickup was reported (`S` set), `ClearSeenPickupable` is raised on `WPlayerOwner`, **then** `S` is emptied. The
   controller's handler runs while the HUD still holds the old `S`;
3. if there was a hover pickup, its highlight is switched off and `H` is emptied.

## 7. A winner: the events

With a winner `W`, in this order:
1. **The seen-item slot** takes `W`'s inventory, and the HUD's presentation step is called with `W` (open question
   1; it writes nothing the targeting reads).
2. **The pickup's item:** `W`'s inventory's definition is read. With no definition, nothing more happens this tick: no
   event, and `H` and `S` stay as they were.
3. **A second view check:** the cosine between the view direction and the direction from `V` to `W` must be above
   0.5. It is horizontal when the globals definition has a hover particle, and in three dimensions otherwise. When it
   fails, nothing more happens this tick (`H` and `S` unchanged).
4. **The hover pickup:** when `W` is not already `H`, the old hover pickup's highlight is switched off first, then
   `H` becomes `W`. When `W` is `H`, nothing changes here.
5. **In range** (the distance from `V` to `W` strictly below `HUDDefinition.PickupRange`): if `W` is not `S`,
   `SawPickupable(W)` is raised on `WPlayerOwner`, **then** `S` becomes `W`. When `W` is already `S`, no event and no
   write.
6. **Out of range:** no event, and `S` keeps its old value. A pickup reported earlier stays reported, and
   `ClearSeenPickupable` is not raised. The presentation shows the hover particle instead, and the seen-item slot is
   emptied.
7. The highlight, its colour and the interaction icon follow: presentation (section 9).

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

Each step is answered with a value (a plain read or a computed rule), None, or a named `NotYet`. Its kind is given in
words: "layer absent" (a part the headless world does not have yet) or "not read".

**Plan first, then write (review r2 of #461).** The native writes as it goes: it empties the nearest-pickup slot
before it asks for the view, refills it and sets `bHasPickupParticleOffset` while it scores, and then raises
`BeginShrinking`, writes the seen-item slot, `H` and `S`, and raises the two events. Off does not. It runs the whole
tick read-only first, on what it read, and writes only once that plan is complete:
1. **The plan** takes every read and every decision of sections 1 to 8 in the native's order: the HUD, its owner and
   `HUDDef`, the view, `WPlayerOwner`, the pawn and the suspend test, every candidate's eligibility, offset, bounds and
   score, the ranking, the pawn test and the trace, the winner's definition, the 0.5 check, the range, which event (if
   any) the tick raises, and the shrink bookkeeping. Each cheap, certain blocker on the path the plan takes is decided
   here, before any write:
   - no HUD, and no camera (layer absent);
   - a suspend query on a pawn that is not a `WillowPlayerPawn` (not read);
   - the bounds of each candidate that reaches scoring (layer absent);
   - the trace, when the winner is beyond a quarter of `PickupRange` (layer absent);
   - a None `WPlayerOwner`, but **only when the plan raises `SawPickupable` or `ClearSeenPickupable`** (below);
   - the shrink bookkeeping when its count is reached (not read).
   A blocker off the path is not reached: a candidate that fails the cone needs no bounds, and a winner within a
   quarter of the range needs no trace.
2. **Why the plan is exact.** Nothing the pass reads depends on what it writes in the same tick: the nearest-pickup
   slot and `bHasPickupParticleOffset` have no reader on the targeting path, `H` and `S` are read before they are
   written, and the seen-item slot is presentation.
3. **The writes,** in the native's order: the nearest-pickup slot, each scored pickup's `bHasPickupParticleOffset`, the
   shrink event, the seen-item slot, `H` with its highlight, then each event before its write to `S` (sections 6 and
   7).
4. **A `NotYet` after a write.** The only answers still to come once the writes start are those of the script the
   events run: the controller's `SawPickupable` and `ClearSeenPickupable` handlers, and the pickup's `BeginShrinking`.
   If one of them answers a `NotYet`, the call stops there. Its text starts "stopped after partial writes", its kind
   and record are kept, and the writes already made stay in place: nothing is rolled back. Any other error passes
   unchanged.

**The recipient, read early, refused late.** `WPlayerOwner` is read when the pass starts, with the owner and `HUDDef`,
as a plain read. A None value is refused (a named `NotYet`, not read: the native raises without a check) only when the
plan's path dispatches an event: a no-winner tick with `S` set (`ClearSeenPickupable`), or an in-range winner that is
not `S` (`SawPickupable`). Every other tick, with no pawn, suspended with `S` empty, a winner that is already `S`, an
out-of-range winner, a missing definition, or a failed 0.5 check, raises nothing and passes with a None recipient. The
refusal is decided in the plan, before any write.

**What a headless `look` answers, in order.** A driver of link 7 checks these in this order and stops at the first
that answers a `NotYet`, all of them in the plan and so before any write:
1. **The touched-slot prerequisite** (the fixture's own check, NATIVE_PICKUP.md section 4): Maya's controller holds the
   decoy as `CurrentTouchedPickupable`. #452's driver checks this first and refuses with an ordinary error when it
   does not hold. That is the fixture's check, not the pass's, and it stays first.
2. **No HUD in the world:** a named `NotYet` of kind layer absent, naming the player's HUD (its owner and its
   `HUDDef`). This is the pass's first answer while the world has no HUD.
3. **A HUD with no owner or no `HUDDef`:** a value: the pass ends (section 1, step 2). The look then sees nothing,
   with no event and no `NotYet`.
4. **No camera:** a named `NotYet` of kind layer absent, naming the view point and rotation. This is the answer once a
   HUD with an owner and a definition exists.

#452's driver (at a1bd92f2) goes from step 1 straight to step 4's camera answer, because its world has no HUD to ask
about. Both are layer-absent answers before any write, so the link's stop kind is the same. By this note's order the
strict answer while no HUD exists is step 2's, and an implementer of the pass (decision 2026-11-19-81's lane) should
answer the HUD first, or supply a HUD and answer the camera.

| Step | Off answer |
|---|---|
| The HUD (section 1) | the world has no HUD: a named `NotYet` (layer absent) naming the HUD, its owner and its `HUDDef`, until a host lane provides one; which `HUDDefinition` the player's HUD holds is open question 2 |
| No owner or no HUD definition | value: the pass ends; the nearest slot is emptied (the native empties it first; Off writes it after the plan, and here the plan is this check alone) |
| The view point `V` and rotation | a named `NotYet` (layer absent) naming the view (the camera, hub ruling) until a host lane supplies it; read before the pawn test |
| The events' recipient `WPlayerOwner` | value: a plain read when the pass starts. None is refused with a named `NotYet` (not read: the native raises without a check) only when the planned path raises `SawPickupable` or `ClearSeenPickupable`; decided in the plan, before any write |
| No pawn | value: no event, `H` and `S` unchanged; the nearest slot is not needed (no reader on the targeting path) |
| Suspended: the controller in its `Dead` state | value: no scoring, section 6 |
| Suspended: the pawn's query | value: no for a `WillowPlayerPawn`; a pawn of another class, or a driven vehicle, a named `NotYet` (not read) |
| Candidates, `bPickupable`, the class test | values: plain reads of `WillowGlobals.PickupList` and the flag |
| The range gate, `PartItemHover`, `PickupRange` | values: plain reads; strictly below the range |
| The 0.9 horizontal cone | value: the rule of section 3, including the 1e-8 cutoff (zero horizontal separation fails) |
| The particle offset | value: `PickupFlagOffset` turned by the pickup's rotation. The native sets `bHasPickupParticleOffset` as it scores; Off computes the offset in the plan and writes the flag with the other writes, after the bounds and the trace are decided |
| The bounding-box height | a named `NotYet` (layer absent) naming the pickup's component bounds, while the world has none |
| The score, the radius, the ranking | values: the formulas and cutoffs of section 4 (plain reads of `HoverWidthPickup` and `ItemHoverProjectionHeight`); a radius at most 0 scores 0 |
| The pawn test before the trace | value: the three flags |
| The trace | value: no trace within `PickupRange × 0.25` horizontally; otherwise a named `NotYet` (layer absent) naming collision |
| The definition, the 0.5 check, the range | values: section 7 |
| `SawPickupable` / `ClearSeenPickupable` | values: raised on `WPlayerOwner` as sections 6 and 7 say, each event **before** its write to `S`; the handlers are NATIVE_PICKUP.md's script. A `NotYet` from a handler comes after writes: "stopped after partial writes", kind kept, writes left in place |
| The shrink bookkeeping | value: nothing when no counted pickup's `LifeSpan` is above `PickupShrinkDuration`; otherwise a named `NotYet` (not read: the enable query and the HUD's limit are open question 3), decided in the plan |
| Presentation | value: nothing (nothing the rules read changes) |

## 11. Link 7 with the score resolved

Link 7's fixture: the aimed gun dropped 150 uu ahead of Maya, and a nearer decoy 45 uu to her side (NATIVE_PICKUP.md
section 4). Those placements are relative to her pawn. They give no camera geometry: the view point `V` and its
rotation are the camera's, which the headless world does not have. So the cone and the score below are conditional
on a supplied view.
- **With a supplied view at about Maya's location, its yaw toward the aimed gun:** the decoy is about 90 degrees off
  the view's yaw, a cosine near 0, so it fails the 0.9 cone and is never scored. The aimed gun passes the cone.
- **With any other supplied view** (a camera behind or above the pawn, or a yaw elsewhere), both cosines must be
  recomputed from that view with section 3's rule. The fixture's distances alone settle neither.
- **The aimed gun** is scored when it is `bPickupable`, within `PickupRange` (or the globals definition has a hover
  particle), and inside the cone. Its score is 1 when the view ray passes through its segment, and lower as the ray
  passes further off, down to 0 at the capture radius.
  - The segment's extent needs the gun's bounds height and `ItemHoverProjectionHeight`.
  - The radius needs `HoverWidthPickup`.
  - Being the only scored candidate, it wins with any positive score.
- **It is reported** (`SawPickupable` on Maya's controller) when it also passes the pawn test, the trace (when more
  than a quarter of `PickupRange` away horizontally), the definition read and the 0.5 view check, and lies within
  `PickupRange`.

## Open questions

1. **UNVERIFIED:** the suspend query of a pawn of another class and of a driven vehicle (section 2), and the HUD
   presentation step the pass calls with the winner, which writes nothing the targeting reads.
2. **UNVERIFIED:** which `HUDDefinition` the player's HUD holds, and so the `PickupRange` value, and whether the
   slice's globals definition has a hover particle (`PartItemHover`). Read since: NATIVE_LINK7_HUD_TARGETING.md
   section 3 (`HUDDef` is the globals definition's `HUDDefinition`, `PickupRange` 400 in the installed data).
3. **UNVERIFIED:** the shrink bookkeeping's enable query on the HUD and the limit the HUD holds (section 8). Read
   since: NATIVE_LINK7_HUD_TARGETING.md section 7 (not a split-screen player; `MaxNumPickups` by fit).
4. Resolved in review r1 of #461: the events go to `WillowHUD.WPlayerOwner`, which `WillowHUD.PostBeginPlay` sets to
   the owner cast to `WillowPlayerController`. **UNVERIFIED** in play, and the field's identity is a declared-order
   fit (medium confidence); the live check below records it.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Each check records, every tick it covers, the HUD's `CachedHoverPickup` (`H`), `CachedBestPickup` (`S`) and
`SavedLookAtInventory`, the controller's `CurrentSeenPickupable`, and each `SawPickupable` and `ClearSeenPickupable`
call with its receiver and argument, in call order.

- **The inputs.** Read the HUD's `HUDDef` and its `PickupRange`, and the globals definition's `HoverWidthPickup`,
  `ItemHoverProjectionHeight` and `PartItemHover`; read `WPlayerOwner` and `PlayerOwner` and confirm they are the same
  controller.
- **The cone cutoff, apart from the capture radius.** Drop one gun, stand at a distance where the capture radius
  covers a wide angle, and turn the yaw slowly with the pitch level so the ray stays on the segment. The event edge is
  then the cone's (a cosine of 0.9 from the horizontal direction to the gun's location `L`).
- **The capture-radius cutoff, apart from the cone.** Sweep **sideways**, not in pitch: a ray that leaves the
  segment past its top or bottom puts the closest point outside the segment, and then there is no `d` at all
  (section 4), so a pitch sweep measures the segment's end, not the radius. Stand far enough away that the capture
  radius's edge comes at a small yaw: more than about 2.1 × the capture radius horizontally from the gun, since the
  cone's 0.9 edge is about 25.8 degrees off. Keep the pitch so the ray crosses the gun's height between the segment's
  bottom and top, and turn the yaw slowly off the gun. At the edge, record the view and the gun's location and check
  from them both that the horizontal cosine is still above 0.9 and that both closest points lie inside their pieces
  (the segment between bottom and top, the ray within its length). The edge should then be `d` = `HoverWidthPickup ×
  0.1`, or × 1.5 once the gun is the hover pickup. Sweep out and back in to see the hover pickup's wider radius hold
  the gun longer than it took to gain it.
- **An exact tie, apart from the hover bonus.** Two guns of the same definition and rotation, placed as mirror images
  across the view ray: the view's yaw and pitch exactly 0, and the guns at the same forward distance and height, one
  at the view's sideways coordinate plus `a` and one at minus `a`, with `a` below the capture radius. Neither is yet
  the hover pickup. Record the inputs and recompute each gun's final single-precision ranking value (section 4) from
  them. The probe counts as a tie only when the two values are bit-identical; then the earlier entry of
  `WillowGlobals.PickupList` should win. Then make the later one the hover pickup first (aim at it alone) and repeat:
  the hover pickup should keep winning, by its 0.25 bonus and its wider radius and segment.
- **The trace exemption and allowed hits.** A wall between the view and a gun further than a quarter of `PickupRange`
  horizontally: not reported. The same wall with a gun within a quarter of `PickupRange`: still reported (no trace).
  A gun resting on a crate (its `Base`) whose top hides part of the line: still reported.
- **The event order around a replacement.** With `S` set to gun A, aim at gun B in range: record that
  `SawPickupable(B)` is called while `S` still reads A, then `S` and `CurrentSeenPickupable` read B.
- **The event order around a clear.** With `S` set, turn away from every pickup: record that `ClearSeenPickupable`
  is called while `S` still reads the old pickup, then `S` and `H` are empty.
- **Winner branches that leave state unchanged.** Aim at a gun just outside `PickupRange` after another was reported:
  no event, `S` unchanged, `H` the new gun. Aim at a pickup whose item has no definition, and at one that fails the
  0.5 check (a gun inside the horizontal cone but far below the view, with no hover particle): no event, `H` and `S`
  unchanged.
- **Two guns at different distances,** both hit by the ray: the nearer one is reported (the ranking divides by
  distance).
