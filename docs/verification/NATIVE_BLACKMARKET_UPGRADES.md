# Native: the black-market upgrade attribute value (2026-11-19)

AI-assisted (Claude Code). Lane ANALYST-BLACKMARKET-RESOLVER, analyst only (clean room, docs/LANE_BRIEF.md; decision
2026-11-19-63). The sources, in the order used:
- the records (NATIVE_ATTRIBUTES.md lists the resolver as not read);
- the installed packages, through `ow-package --def-dump` and `--exports`;
- the class script, through `research/script_disasm.py`;
- `Borderlands2.exe` locally, with the project's Ghidra tooling (read-only queries and the local read-only table reader,
  under the machine lock).

Raw output stays under ignored `local/analysis/`; this note is in our own words. No bl-sdk session was run.
**Every rule here is UNVERIFIED** until the live checks at the end confirm it. The lane that wrote this note does not
implement it: the rule stays `NotYet` in Off until a fresh implementer codes it from this note alone.

**Why it matters.** With `AttributeDefinition.GetValue` bound (decision 2026-11-19-58), slice-gate links 1, 3, 4, 5, 13
and 14 stop inside `maya:8` on `WillowGame.BlackMarketUpgradeAttributeValueResolver`, a value resolver `ow-attr` does
not implement.

## Summary

| Rule | Where it comes from | Confidence | Status |
|---|---|---|---|
| The resolver's value and base value are the same number | native (the value method answers the base method) | high | UNVERIFIED |
| The value: the context player controller's level for the resolver's upgrade, or 0 | native | high for the structure, medium for which controller class the context must be | UNVERIFIED |
| The upgrade's index: its position in the globals' black-market upgrade list, computed lazily and cached on the upgrade | native | high for the structure, medium for which globals list | UNVERIFIED |
| The controller's level read (`GetBlackMarketUpgrade`): 0 for index -1; otherwise grows the controller's list with zeros to cover the index, then returns the entry | native | high | UNVERIFIED |
| Where the levels live and how a save fills them | script (`LoadBlackMarketUpgrades`, the purchase functions) | high | UNVERIFIED |

## 1. What the installed data feeds it

- **The resolver** has one property, `AssociatedBlackMarketUpgrade`: a `BlackMarketUpgradeDefinition`. It declares no
  script function, so its value rule is native only.
- **The globals** (`GD_Globals.General.Globals`, a `GlobalsDefinition`) list nine upgrades in `BlackMarketUpgrades`:
  - six ammo upgrades: assault rifle, pistol, rocket launcher, shotgun, SMG and sniper rifle;
  - grenades;
  - the backpack;
  - the bank.
- **Each upgrade definition** holds:
  - `BalanceDefinition`, the item sold for the upgrade;
  - `CachedUpgradeIndex`, which is -1 in the package;
  - `UpgradeName`, `BehaviorProvider` and `ZippyFrameOverride`.
- **One installed use, inspected:**
  - Maya's assault-rifle ammo pool, `D_Resourcepools.AmmoPools.Ammo_Combat_Rifle_Pool`, names
    `GD_BlackMarket.Attributes.Att_BM_AssaultRifleAmmo` twice: as its `UpgradeLevelAttribute`, and as the
    `BaseValueAttribute` of its `MaxValueUpgrade`.
  - That attribute's context chain is one `PlayerControllerAttributeContextResolver`, and its value chain is one
    `BlackMarketUpgradeAttributeValueResolver`.
  - The resolver's `AssociatedBlackMarketUpgrade` is `GD_BlackMarket.Upgrades_Ammo.Upgrades_Ammo_AssaultRifle`.
  - The other upgrades' attributes were not enumerated; they are expected to follow the same shape (UNVERIFIED).

## 2. The resolver's value (and base value)

The value method answers exactly what the base-value method answers, so one rule covers both. The previous result in
the value chain is not used. Given the resolved context:

1. **The controller.** The context is kept only if it is an object of one `WillowGame` class. That class is found by
   name at first use. That it is `WillowPlayerController` is inferred: the level read in step 3 is the same native rule
   that `WillowPlayerController.GetBlackMarketUpgrade` reaches (medium). A None context, or one of another
   class, leaves no controller.
2. **The upgrade index** (section 3). With no `AssociatedBlackMarketUpgrade` the index is -1.
3. **The result.**
   - With a controller and an index other than -1: the controller's level for that index (section 4), turned from an
     int into a float.
   - Otherwise: **0.0**.

Nothing else is read. The pawn, the save object and the game info are not consulted.

## 3. The upgrade's index

- If the upgrade's `CachedUpgradeIndex` is -1, the index is computed first, then the field is returned.
- **The computation** fills the cache of every upgrade in the list at once:
  - It reaches the engine's globals object, and through it the globals definition (`GD_Globals.General.Globals` in
    the installed game: inferred from it being the one globals definition, medium).
  - It walks that definition's `BlackMarketUpgrades` list in order. Every entry that is not None gets its
    `CachedUpgradeIndex` set to its position: 0 for the first entry, and so on.
- **Edge cases:**
  - The cache lives on the shared definition object, not on the player.
  - An upgrade listed twice ends with the position of its last appearance.
  - An upgrade not in the list keeps -1, and the walk runs again on every read (it changes nothing).
  - With no engine globals object, or no globals definition, the cache stays -1 and the result is 0.0.

## 4. The controller's level read: `WillowPlayerController.GetBlackMarketUpgrade(Index)`

The script native `GetBlackMarketUpgrade` and the resolver reach the same native rule.
- **Index -1:** the answer is 0, and nothing changes.
- **Any other index:**
  1. The controller's `BlackMarketUpgrades` (an int array) is first grown to at least index + 1 entries. The new
     entries are 0. This is what `VerifyBlackMarketUpgradesLength(Index)` does; it never shrinks the array.
  2. Then that entry is returned.
- **A read can write.** Reading the level of an upgrade at position 8 on a controller whose array is empty leaves the
  array nine entries long, all 0.
- **An index below -1** fails the engine's array bounds check (an assertion). The resolver never passes one, since
  cached positions are 0 or more, or -1.

`LookupBlackMarketUpgradeIndex(BalanceDef)` is a separate native, used by the purchase script, not by the resolver. It
walks the same globals list and answers the position of the first non-None upgrade whose `BalanceDefinition` is the
argument, or -1. It also answers -1 with no engine globals object or no globals definition. Note the difference from
section 3: for an upgrade listed twice, the lookup answers its first position, while the cache keeps its last.

## 5. Where the levels come from (script)

- **The class default** of `WillowPlayerController.BlackMarketUpgrades` is an empty array. So with no save loaded and
  nothing bought, every level reads 0, and the first read grows the array as in section 4.
- **`LoadBlackMarketUpgrades(SaveGame)`** empties the controller's array, then copies the save's
  `PlayerSaveGame.BlackMarketUpgrades` into it entry by entry, in order. `SaveBlackMarketUpgrades` is its counterpart;
  its body was not needed here.
- **A purchase** works as follows:
  1. `PurchaseBlackMarketUpgrade(BalanceDef)` finds the index with `LookupBlackMarketUpgradeIndex`, records the
     purchase with the analytics service, and calls the server function.
  2. `ServerPurchaseBlackMarketUpgrade` gives up if the index is -1 or the price is not above 0. Then, if the player's
     eridium on hand covers the price, it:
     - spends the eridium;
     - raises the level by one (`IncBlackMarketUpgrade`, native, not read here);
     - records the purchase;
     - tells the client, whose `ClientBlackMarketUpgradePurchased` raises its own copy when it is not the authority;
     - counts the stat and checks the achievement.
- The levels are per player controller, indexed by the upgrade's position in the globals list.

## 6. For an implementer, in the port's terms

- Register the resolver class in `ow-attr` as a value resolver whose value and base value are one rule.
- **Inputs:**
  - the context (a player controller, from the existing `PlayerControllerAttributeContextResolver`);
  - the resolver's `AssociatedBlackMarketUpgrade`;
  - that upgrade's `CachedUpgradeIndex`;
  - the engine globals definition's `BlackMarketUpgrades`;
  - the controller's `BlackMarketUpgrades`.
- **Effects:**
  - writing the cache onto the upgrade definitions;
  - growing the controller's array with zeros.
- An evaluation that may not write (a dry read) has to decide what to do with those two writes. The game makes both
  on every read, so a refusal is safer than a silent skip.
- **The default for the slice:** with no save and no purchase, the value is 0.0 for every upgrade, provided the globals
  definition is reachable. Maya's ammo maxima then take no black-market bonus.

## Not read

- Which `WillowGame` class the context must be. `WillowPlayerController` is inferred (section 2).
- `IncBlackMarketUpgrade`'s body, and the save function's body.
- `UpdateCachedUpgradeIndex`'s script entry point. Looking it up by name led to an unrelated shared native; the cache rule
  above was read from the upgrade definition's own native method.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- On a fresh character, read `PlayerController.BlackMarketUpgrades` (expected empty), then call
  `Att_BM_AssaultRifleAmmo.GetValue(PC)`. Expect 0.0, and the array grown to cover the assault-rifle upgrade's position.
- Read `CachedUpgradeIndex` on the nine upgrade definitions before and after that call. Expect -1 before, and their
  positions in `GD_Globals.General.Globals.BlackMarketUpgrades` after.
- On a character with one purchased upgrade, compare `GetBlackMarketUpgrade(i)` with the attribute's `GetValue(PC)` for
  that upgrade, and check that `GetValue` on a pawn source resolves through the controller to the same number.
- Call `GetValue` with a source that is not a player controller (an AI mind) and expect 0.0.
