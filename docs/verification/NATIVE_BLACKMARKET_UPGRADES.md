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
the value chain is not used.

**When the resolver is not reached at all.** If the attribute's context chain resolves to None, `GetValue` answers 0.0
before the value chain runs (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"). The resolver is not called, so its
cache and the controller are untouched.

**Given a resolved (non-None) context, in this order:**

1. **The controller check.** The context counts as a controller only if it is an object of one `WillowGame` class, or
   of a subclass of it (the check walks the object's class and its superclasses). The class is found by name at first
   use. That it is `WillowPlayerController` is inferred: the level read in step 3 is the same native rule that
   `WillowPlayerController.GetBlackMarketUpgrade` reaches (medium, UNVERIFIED). If the class cannot be found by name,
   the check lets every object through (high for the structure; a case the installed game should not meet).
2. **The upgrade index** (section 3). This step runs whether or not step 1 accepted the context. So a rejected context
   still computes, and may write, the cache when it is cold. With no `AssociatedBlackMarketUpgrade` the index is -1 and
   nothing is written.
3. **The result.**
   - With an accepted controller and an index other than -1: the controller's level for that index (section 4), turned
     from an int into a float. This is the only path that can grow the controller's array.
   - Otherwise (a rejected context, or index -1): **0.0**, and the controller is not touched.

Nothing else is read. The pawn, the save object and the game info are not consulted.

## 3. The upgrade's index

- **Warm cache.** If the upgrade's `CachedUpgradeIndex` is not -1, it is the index. Nothing else is read (no globals
  lookup) and nothing is written.
- **Cold cache** (-1). The index is computed first, then the field is read again:
  1. The engine's globals object is reached. Without one, the computation stops: nothing is written, the index stays
     -1 and the result is 0.0.
  2. Through it, the globals definition is reached (`GD_Globals.General.Globals` in the installed game: inferred from it
     being the one globals definition, medium). Without one, the same: nothing written, index -1, 0.0.
  3. That definition's `BlackMarketUpgrades` list is walked in order. Every entry that is not None gets its
     `CachedUpgradeIndex` set to its position: 0 for the first entry, and so on. This writes the cache of **every** listed
     upgrade, not only the one asked about.
- **Edge cases:**
  - The cache lives on the shared definition object, not on the player.
  - An upgrade listed twice ends with the position of its last appearance.
  - **An upgrade not in the list:** its own field stays -1. Every cold read still assigns the listed upgrades' fields
    (the same positions again), leaves the controller's array as it is, and answers 0.0. The field stays cold, so the
    next read walks the list again.

## 4. The controller's level read: `WillowPlayerController.GetBlackMarketUpgrade(Index)`

The script native `GetBlackMarketUpgrade` and the resolver reach the same native rule.
- **Index -1:** the answer is 0, and nothing changes.
- **Any other index:**
  1. The controller's `BlackMarketUpgrades` (an int array) is first grown to at least index + 1 entries. The new
     entries are 0. This is what `VerifyBlackMarketUpgradesLength(Index)` does; it never shrinks the array.
  2. Then that entry is returned.
- **When it writes.** Only when the index is not -1 and the controller's array is shorter than index + 1. Then the
  array is grown with zeros to exactly index + 1 entries. The existing entries keep their values. With an array long
  enough, nothing is written.
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
- **The two possible writes, and when each happens:**
  - **The cache:** only on a cold read (the asked upgrade's `CachedUpgradeIndex` is -1) where the globals object and
    the globals definition are both found. Then every non-None listed upgrade's `CachedUpgradeIndex` is assigned its
    position. This also happens when step 1 rejected the context.
  - **The controller's array:** only when the context is an accepted controller, the index is not -1, and the array is
    shorter than index + 1. Then it is zero-filled up to index + 1 entries.
- **The Off order (the hub's ruling in review r1 of #451):**
  1. Preflight both effects before either write: decide whether each write is needed (as above), and that the world
     can make it.
  2. Commit the cache assignments.
  3. Zero-fill the array.
  4. Read the entry.
- **Off answers, per case:**
  - The context chain resolves to None: 0.0 from `GetValue`, the resolver not called.
  - A context that is not a `WillowPlayerController` or a subclass: the cold-cache rule still applies (it may write the
    cache), then 0.0. The class rule is UNVERIFIED (section 2).
  - No `AssociatedBlackMarketUpgrade`: 0.0, nothing written.
  - Warm cache: no globals lookup; the level, or 0.0 for a rejected context.
  - Cold cache, and the world answers the globals lookup and finds no globals object or no globals definition: 0.0,
    nothing written.
  - Cold cache, and the world has no engine-globals support at all: `AttrWorld::globals()`'s `NotYet` of kind
    `LayerAbsent`, before any write. That is different from the case above, where the lookup is supported and finds
    nothing.
  - An unlisted upgrade: as section 3 (the listed fields assigned, 0.0).
  - A needed cache write the world cannot make (for example a read-only evaluation, or definitions it treats as
    immutable): a `NotYet` of kind `NotPorted` named "BlackMarketUpgradeAttributeValueResolver: the cold
    CachedUpgradeIndex assignment", before any write.
  - A needed array growth the world cannot make: a `NotYet` of kind `NotPorted` named
    "BlackMarketUpgradeAttributeValueResolver: zero-filling the controller's BlackMarketUpgrades", before any write.
  - A globals definition without a `BlackMarketUpgrades` array of objects, or a controller without a
    `BlackMarketUpgrades` array of ints: bad data.
- **The default for the slice:** with no save and no purchase, the value is 0.0 for every upgrade, provided the globals
  definition is reachable. Maya's ammo maxima then take no black-market bonus.

## Not read

- Which `WillowGame` class the context must be. `WillowPlayerController` is inferred (section 2).
- `IncBlackMarketUpgrade`'s body, and the save function's body.
- `UpdateCachedUpgradeIndex`'s script entry point. Looking it up by name led to an unrelated shared native; the cache rule
  above was read from the upgrade definition's own native method.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Each check names its starting state and the fields to read before and after. Read `CachedUpgradeIndex` on all nine
upgrade definitions and `PC.BlackMarketUpgrades` (length and entries) before and after every call.

1. **Cold cache, accepted controller, empty array.** Start a fresh character in a fresh process (all nine caches -1,
   the array empty). Call `Att_BM_AssaultRifleAmmo.GetValue(PC)`. Expect 0.0, all nine caches at their positions in
   `GD_Globals.General.Globals.BlackMarketUpgrades`, and the array grown with zeros to the assault-rifle position + 1.
2. **Warm cache.** Repeat the call. Expect the same value and no change to any cache or to the array. Whether the
   globals lookup was skipped can only be seen by hooking it; if the SDK cannot hook a native helper, this part is an
   open verification gap.
3. **Growth keeps entries.** On a character with a saved level 2 at the assault-rifle position and an array shorter
   than another upgrade's position, read that other upgrade's attribute. Expect the array grown with zeros, and the
   assault-rifle entry still 2.
4. **Value and base value at a nonzero level.** On the same character, compare `GetValue(PC)` and `GetBaseValue(PC)`
   for the assault-rifle attribute. Expect both 2.0.
5. **`GetBlackMarketUpgrade(-1)` called directly.** Expect 0 and the array unchanged.
6. **Absent context.** Call `GetValue(None)` in a cold-cache process. Expect 0.0 and all caches still -1.
7. **Rejected context.** Call `GetValue(<an AI mind>)` in a cold-cache process. Expect 0.0, the caches assigned, and
   the controller untouched. Whether the AI mind reaches the resolver at all depends on the attribute's
   player-controller context resolver. If that resolver answers None for it, this case collapses into case 6 and needs
   another non-None, non-controller context; that may be an open verification gap.
8. **Missing globals versus an unlisted upgrade.** Missing globals cannot be set up in a running game: an open
   verification gap. An unlisted upgrade needs a definition outside the list. The SDK may build one as a transient
   object; if not, an open verification gap. Expected: its field stays -1, the nine listed fields assigned, 0.0.
9. **Duplicate entries.** The installed list has none; testing it needs a modified globals list in memory. Expected:
   the duplicate's cache holds its last position, and `LookupBlackMarketUpgradeIndex` answers its first. If the SDK
   cannot edit the array in place, an open verification gap.
