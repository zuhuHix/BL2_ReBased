## 2026-11-19: Read the pickup path as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-PICKUP, analyst only, for slice links 6 (pick up the gun) and 7 (the aimed pickup wins over the nearest),
which have no headless driver. The new own-words note `docs/verification/NATIVE_PICKUP.md` and four `NATIVE_INDEX.md`
rows record, as behaviour only:

- The spawn side is NATIVE_LOOT_SPAWN.md's (`DropFrom` spawns the inventory's `WillowPickup`); the note adds the
  landing and rest rules.
- The take is script from end to end: the pickup's touch state and its validity rule, `GetCurrentPickupable`,
  `PickupSomething` (room, the full-backpack swap, the price), `PickupPickupable` (bulk pickups), `GiveTo`,
  `AddInventory` and the pickup's destroy.
- Link 7's choice is the player's per-tick native pickup targeting: a narrow view cone within the pickup radius,
  a score over distance with a hold bonus for the current one, and a line check. It raises `SawPickupable` and
  `ClearSeenPickupable`. The score itself was not read.
- New natives indexed: `GetInventoryPawn`, `WillowGlobals.RemovePickup`, `DroppedPickup.AddToNavigation` (empty in
  this build) and the targeting. The other natives the path reaches were already indexed.

Raw output stays under ignored `local/analysis/pickup/`; the Ghidra runs were read-only and took the machine lock.
Nothing is implemented: the path stays unported in Off until a fresh lane codes it from the note. Every rule is
UNVERIFIED in the running game; the note lists the bl-sdk checks and four open questions.
