## 2026-11-19: Read the black-market upgrade resolver as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BLACKMARKET-RESOLVER, analyst only (clean room). New record:
`docs/verification/NATIVE_BLACKMARKET_UPGRADES.md`.

- **Sources, in order:**
  - the records, which listed `BlackMarketUpgradeAttributeValueResolver` as not read;
  - the installed packages (`ow-package --def-dump` and `--exports`);
  - the class script (`research/script_disasm.py`);
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local read-only table reader, one
    headless run at a time under the machine lock.
- Raw output stays under ignored `local/analysis/blackmarket/`. No bl-sdk session was run, so every rule is
  **UNVERIFIED**.
- **No code changes.** The resolver stays unimplemented (`NotYet` in Off) until a fresh implementer codes it from the
  note alone.

**What the note settles.**

- The resolver's value and base value are one rule.
- **The result:** with a player controller as the context and an upgrade index other than -1, the controller's level
  for that upgrade, as a float. Otherwise 0.0.
- **The upgrade index:** the upgrade's position in the engine globals definition's `BlackMarketUpgrades` list. It is
  computed lazily into `CachedUpgradeIndex` on every listed upgrade at once.
- **The level read** (`WillowPlayerController.GetBlackMarketUpgrade`): 0 for index -1. Otherwise the controller's
  `BlackMarketUpgrades` is grown with zeros to cover the index, and that entry is returned. A read can write.
- **Where the levels live:** the controller's `BlackMarketUpgrades`. Its class default is empty; it is filled from
  `PlayerSaveGame.BlackMarketUpgrades` by script and raised by a purchase.
- **The slice default:** with no save and no purchase, every black-market attribute reads 0.0.
- **The installed shape**, from one inspected attribute: `GD_BlackMarket.Attributes.Att_BM_AssaultRifleAmmo` has a
  player-controller context chain and this resolver, tied to the assault-rifle ammo upgrade. It feeds Maya's rifle ammo
  pool maximum.

**Review r1 of #451.** The note now says:
- when each write happens: the cache only on a cold read with the globals found, the array only when short;
- the Off order: preflight both effects, commit the cache, zero-fill, read;
- the named `NotYet`s for unsupported writes;
- missing globals (0.0) versus no engine-globals support (`LayerAbsent`), and that a warm cache skips the lookup;
- the absent context (the resolver not reached) versus a rejected one (the cold cache still computed);
- the unlisted upgrade's cold read.

The live checks now name their starting states and before/after fields, and mark the open verification gaps.

**Review r2 of #451.** On a cold lookup the Off preflight takes the effective index from the pending cache
assignments (last occurrence, or -1 when unlisted); it does not reread the field. When the growth that index needs is
unsupported, the answer is the array-growth `NotYet` with nothing written. The warm-cache check now stages a different
listed upgrade at -1 and expects it to stay -1, and the unlisted check says how the transient definition is bound to the
resolver.

**Open (in the note).**

- Which `WillowGame` class the context must be. `WillowPlayerController` is inferred.
- `IncBlackMarketUpgrade`'s body.
