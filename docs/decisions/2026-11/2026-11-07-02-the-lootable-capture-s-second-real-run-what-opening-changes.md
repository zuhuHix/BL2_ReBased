## 2026-11-07: the lootable capture's second real run; what opening changes

AI-assisted (Claude). `tools/real_game/loot_capture.ps1` ran a second time, exit 1 as incomplete, saves identical afterwards. This answers the previous entry on which field moves. Results and ideas for a later run are in `docs/verification/VENDORS.md` section 9.

- **Verified, in this one run:** on the five lootable types that opened, `SimpleAnimState` and `RepSimpleAnimState` went from 4 to 7 and `bCanBeUsed` from true to false; on seven tries that did not open, none moved. `CachedUsableObject` was the object on every try that opened and empty on every other. One ammo pickup was collected by the use key.
- **`UNVERIFIED`:** the strong box's hinge and whether a walk-over collects, neither observed. Cash was not tested, because the capture mistook eridium for it, a known fault left in place. Pickups and prompts as stock, since other SDK mods were on.
