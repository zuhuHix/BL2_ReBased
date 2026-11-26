## 2026-11-26: links 6 and 7: the pickup's affordability, the take's next stop, and what `look` needs from the HUD

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-LINK67-KEY-AND-LOOK, the analyst half of the clean room
(another lane implements the note), on main 78a88b72. Ghidra 12.1.4 headless, read-only, under the machine lock, and
`research/script_disasm.py`. Raw output is under ignored `local/analysis/link67/`. No bl-sdk session was granted.
**Every rule is UNVERIFIED in the running game.**

**What was read.** NATIVE_LINK67_KEY_AND_LOOK.md:
- `CanAffordToPickUpPickupable`: no pickupable or no cost is true. Golden keys (type 3) compare with the controller's
  `GetNumGoldenKeysAvailable`. Any other type compares with the cast `WillowPlayerReplicationInfo`'s
  `GetCurrencyOnHand`, and a failed cast is true. It writes nothing;
- `PayForPickupable`: `SpendGoldenKey` once per unit, or `AddCurrencyOnHand(type, -amount)`.

The pickupable's own cost query is a C++-only method with no script name. Its body was not located. The `WillowPickup`
answer from `bCostsToPickUp` / `CostsToPickUpType` / `CostsToPickUpAmount` stays the earlier reading's, by fit. The
slice's gun is a `WillowPickup` whose class default tags no cost field, so the answer is true.

**`look`.** The HUD's targeting pass reads no drawing state (neither `Canvas` nor `RenderDelta`), so its result is
independent of drawing. The game's HUD is spawned by `ClientSetHUD` at login. `PlayerOwner` and `WPlayerOwner` are set
in `PostBeginPlay`, beside presentation steps. Where `HUDDef` is set was not read. The proposed Off answer keeps the
HUD `LayerAbsent`. A harness HUD would only move the stop to the camera `LayerAbsent`; that is the hub's call.

**`key` after affordability.** The guard is a pickupable, no `DenyPickupAttempt`, and a pawn. Then the room check on
bound natives, `ServerPickupSomething`, and `PickupPickupable`, whose first unbound native is `UpdateAmmoCounts`.
That native's body is a controller table slot that was not read, so its Off answer is `NotRead`. Predicted: link 6
moves to `UpdateAmmoCounts`' `NotRead`, and link 7 stays at the HUD `LayerAbsent`.

No code changed.
