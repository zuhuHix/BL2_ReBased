## 2026-10-06: native notes on pickup, backpack, equip and the mission weapon (UNVERIFIED)

AI-assisted (Claude), analyst lane G12, Ghidra; own-words note `docs/verification/NATIVE_INVENTORY_EQUIP.md`, no listing
text. Pickup, backpack readying, quick-slot assignment and `WillowPawn.EquipItem` are script the VM already runs; the natives
are thin: backpack count and limit are stored numbers (limit default 12, `SetInventoryMaxSize` clamps to at least 12),
`GetWeaponReadyMax` is a stored value, new weapons take the first free quick slot 1..4 with 3 and 4 gated by the maximum,
`FindLeastValuableWeapon` picks which readied weapon yields; equip/unequip/pickup natives only raise behavior events. A
mission-balance weapon has required level 0, cannot be dropped, sold or saved, is refused from the backpack once its mission
is inactive and is taken back through the script event `RemoveMissionWeapons`. Save fix-up floor of weapon slots: 2, +1 per
completed plot mission 5 and 10, at most 4. Open: new-character `WeaponReadyMax`, SDU values, slot-effect natives, the
"seen pickup" decision. All UNVERIFIED in game.

