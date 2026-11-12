## 2026-11-04: E takes the aimed pickup; no status page over an open shop (inventory lane)

AI-assisted (Claude). Records: `docs/verification/PICKUP_CARD.md`, `MAP_SCREEN.md`. Host behaviour change; UNVERIFIED in game.

- `PickupNearby` takes the pickup the world pickup card shows (`OpenWillowPickupCard::FindAimedPickup`: reach 220 uu, a 12-degree
  cone, line of sight) before the nearest-target rule for vending machines, doors and pickups (vendors #81). Decoy capture:
  the aimed gun is taken, the nearer un-aimed one stays. The nearest-pickup search uses the same sight test, so E takes nothing
  through a wall.
- The status pages (Inventory, Skills, Missions, Map) and the Walker's legacy inventory widget do not open while a vending shop
  is open: forced over a shop, closing the page left the shop without input or cursor.
- The quest's on-screen tracker text is not posted while a vending shop is open either (it already hid behind the status pages).

