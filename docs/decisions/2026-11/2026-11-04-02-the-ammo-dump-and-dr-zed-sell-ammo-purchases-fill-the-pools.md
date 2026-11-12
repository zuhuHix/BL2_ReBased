## 2026-11-04: the Ammo Dump and Dr. Zed sell; ammo purchases fill the pools (Vendors PR 2b)

AI-assisted (Claude; implementation sub-agent on Haiku). Record: `docs/verification/VENDORS.md` sections 4 and 6.

- **Stock (host behaviour, UNVERIFIED rules).** The Ammo Dump shows one row per ammo type, then grenade mods by weight,
  for 11 rows. Dr. Zed shows one row per health item, then shields by weight, for 8 rows. Both shelf sizes follow the live
  row counts. Rows come from the decoded item blocks (value confirmed live 63/63; the buy price uses the confirmed shop
  markup). They are seeded like the gun shelf, so the PR 2a save restores them; the manifest identity now covers the item
  blocks, so an older currency save is refused, not misapplied. Class mods are left out because their shop pools are
  empty in the packages.
- **Effects (UNVERIFIED in game).**
  - Ammo adds the item's rounds to the pool, capped at its maximum, and is refused with "Ammo full".
  - Health Now! heals 25 % of max health and is refused at full health.
  - Ammo and health rows stay on the shelf.
  - Shields and grenade mods enter the backpack as stand-in gear with no stats and rarity 1. They can be sold and bought
    back.
- **Tests.**
  - UE automation `OpenWillow.Vendor.Items`.
  - `node tests/vending_test.js` (26).
  - `tests/check_vendors_test.py` (ammo_buy, heal_buy, item_prices).
  - The `-owvendortest` sequence buys ammo at the Ammo Dump and a heal at Dr. Zed, after a test-only drop to half
    health.
