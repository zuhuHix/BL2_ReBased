## 2026-11-06: Sanctuary's lootables and pickups in the host (Vendors PR 3)

AI-assisted (Claude; implementation sub-agents on Haiku, data on Opus). Record: `docs/verification/VENDORS.md` section 7.
Package data only: nothing here has been seen in the running game, so the rules are UNVERIFIED.

- **Data.** `tools/prepare_interactives.py` writes 97 placed objects with their loot configurations, visuals, requirements and
  pickup data to ignored `local/slice/interactives.json`.
- **Host.** `UOpenWillowInteractiveSubsystem` (`-owinteractives=`) spawns the 95 supported objects; the golden and Bearer of Bad
  News chests are not spawned. The use key opens the nearest within 350 uu, in the walker's nearest-target rule. Guns and gear
  drop through the existing `AOpenWillowInventoryPickup`; `AOpenWillowLootPickup` handles cash, ammo, eridium and the
  health vial. Heals change current health only and are refused while down, as in PR #99.
- **Assets.** `tools/import_lootables.ps1` imported 21 of 36 assets. 15 clips were refused by the converter's mirror check: 2 and 3
  bone skeletons cannot determine the fitted map. Missing assets fall back to placeholders.
- **Tests.** UE automation `OpenWillow.Interactive.Loot`, `tests/check_interactives_test.py` and
  `tests/prepare_interactives_test.py` (synthetic), and the in-engine suite `tools/test_interactives.ps1`.

