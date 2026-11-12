## 2026-11-06: Generated guns follow the refire rule

AI-assisted (Claude). A playtest found a rolled Jakobs pistol firing 8 rounds on a held press where the slice pistol fires 1. The slice
pistol follows its type's `AutomaticBurstCount` of 1; generated guns had no firing data, so their burst count stayed 0, which means fire
while held. `AutomaticBurstCount` and `BurstInterval` are now type bases in `tools/weapon_stats.py` and `src/weapon_gen`, so they go through
the attribute stack (a part's PreAdd counts, from 0 where the type has no count), and `OpenWillowWeaponGen::ToItem` fills `Item.Firing` with them.

Manifest: no new record kind and no version change; each weapon type gains up to two `Base` rows. **Rerun `tools/weapon_gen_manifest.py`:** an
older manifest still loads, gives null burst values, and the host warns once and keeps the old fire-while-held behaviour. The local parity tool
reports no burst mismatch on all 249 balances, 2 seeds; it does report `gun.card.damage_stack`, a Python-only field unrelated to this change.
Regenerated recipes now say `burst_count` 0, not null, for types with no count; the host and checker read both the same.
A Dahl barrel's effect can evaluate to -1 when not aiming; the host clamps it to 0, fire while held. The burst Dahl guns gain while aiming (`ZoomWeaponAttributeEffects`) is not modelled for generated guns.

Verified: synthetic tests, parity, real rolls per type at the tool level. `UNVERIFIED`: a rolled gun's refire in the host and the running game
(a live read of one rolled Jakobs is planned after the host check). The rest of the firing block is still not resolved for generated guns.

