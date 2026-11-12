## 2026-11-04: range loot on death from the VM's death roll (range lane, PR3)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md) "Loot on death (PR3)".
- **New host behaviour, no parsing change.** `tools/prepare_range.py` kills each spawned pawn in the VM and keeps its death
  roll (pools and their evaluated chances per kind and level; they equal COMBAT_LOOT's table). The host rolls those chances in
  COMBAT_LOOT's order from a seeded stream. Weapon pools roll a gun with the weapon generator (`ow.RollGun`'s roll) and drop it
  through the inventory pickup's existing API. Money and ammo drop the range's own touch pickups. `ow.RangeLoot normal|always|off`.
- **Stand-ins (UNVERIFIED):** the money value per item (`round(4 x 1.13^level)`; the stock value is not decoded), the ammo rule
  (only below half the reserve, a magazine's worth), a pool pick that is not a gun drops nothing. Skins, eridium and health are
  not hosted.


