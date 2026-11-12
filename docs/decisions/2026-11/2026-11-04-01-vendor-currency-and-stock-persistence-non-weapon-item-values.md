## 2026-11-04: vendor currency and stock persistence; non-weapon item values confirmed live (Vendors PR 2a)

AI-assisted (Claude; sub-agents on Haiku and Opus). Record: `docs/verification/VENDORS.md` sections 4 and 6.

- **Item value (package data, confirmed live 63/63).** Ammo, Health Now!, grenade mods, shields and class mods share the
  gun formula: value = trunc(part_total x 1.12 ^ item level x base cost), with each kind's `Att_BaseCost_*` constant and
  the grade's part total (1 for ammo and health). Checked against the read-only live dump of PR 1; the two DLC class mods
  are not described by the packages read and stay UNVERIFIED. `tools/prepare_vendors.py` writes an `item` block per
  non-weapon leaf (kind, ammo pool and amount, heal fraction, value terms); the heal and the ammo-on-purchase behaviour
  are UNVERIFIED. The host does not stock these kinds yet (PR 2b).
- **Persistence (host behaviour).** `-owcurrencysave=<file under local/>` keeps money, eridium and each machine's restock
  count and remaining timer; the shelf is re-rolled from the machine's seed and restock count with the bought items
  removed. Buy-back is not kept, and the timer does not run while the game is closed: both host choices, UNVERIFIED.
  `-owmoney` / `-owerid` override the save. `OpenWillowInventory.h` gains `GetEridium` (one line).
- **Tests.** UE automation `OpenWillow.Vendor.Persist` (synthetic), a second launch in `tools/test_vendors.ps1` that checks
  money and shelf survive a restart, `tests/prepare_vendors_test.py` (15) and `tests/check_vendors_test.py` (35).

