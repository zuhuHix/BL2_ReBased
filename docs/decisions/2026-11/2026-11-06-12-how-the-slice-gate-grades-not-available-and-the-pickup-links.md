## 2026-11-06: how the slice gate grades NOT_AVAILABLE and the pickup links (slice-gate lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_GATE.md`. Host behaviour only; a PASS says the host did this in a scripted run with named fixtures, not that it matches the game.

- **NOT_AVAILABLE is for a missing capability or precondition, never a miss.** `enemy_attacks_maya` in range mode, `loot_drop_on_kill` with the gun roll forced and a pickup link whose ids show an attempt are FAILs, in host and checker.
- **"Test setup gap" is its own case:** no roll source or no generator manifest means the link did not test the host. It stays NOT_AVAILABLE, owned by slice-gate and counted on its own line. The earlier "no ow.RollGun source" with a real drop was this lane's driver not reading the source on that path.
- **The checker reads the pickup ids itself:** aimed drop, card and picked-up item must be one item with an accepted pickup, and the decoy a different item.

