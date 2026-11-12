## 2026-11-03: vending machines in Sanctuary, buy and sell, with the shop price confirmed live (Vendors PR 1)

AI-assisted (Claude; implementation sub-agents on Haiku). Record: `docs/verification/VENDORS.md`. One read-only real-game
dump (`tools/real_game/vendors_capture.ps1`, level-8 Maya in Sanctuary, saves blocked and restored byte-identical).

- **Placement (package data).** Machines spawn at population opportunity points whose population is a
  `PopulationFactoryVendingMachine`; `tools/prepare_vendors.py` writes them with their kind (shop type), pose (rotation in
  degrees like the slice manifests) and the expanded `Inventory` / `FeaturedItem` loot pools to ignored
  `local/slice/vendors.json`. Pools in the map package that point at Startup objects are resolved there. Live, seven
  machines and the Black Market sit exactly on these points; an eighth (a Dr. Zed machine) is not found yet.
- **Price (behaviour, confirmed live 79/79).** Buy price = floor(value x (item level x 0.14 + 4.1)) in single precision, the
  same for the featured item; 0.14 and 4.1 are the live values of the two markup attributes, which have no value in the
  packages. Stock sizes (weapons 7 + featured) and the 20-minute reset were read live too.
- **Host.** `UOpenWillowVendorSubsystem` spawns placeholder machines from `-owvendors=`; the use key opens the nearest within
  350 uu; the page `tools/hud_overlay/vending.html` (stock movie as frame art, HTML lists) buys, sells and buys back through
  `OWVEND` lines. Guns only for now; the sale price (the item value), buy-back, the stock roll and the layout are UNVERIFIED.
- **Tests.** `node tests/vending_test.js`, `tests/prepare_vendors_test.py`, `tests/check_vendors_test.py` (synthetic), the UE
  automation test `OpenWillow.Vendor.Stock`, and the in-engine suite `tools/test_vendors.ps1` (`-owvendortest`) for the hub.

