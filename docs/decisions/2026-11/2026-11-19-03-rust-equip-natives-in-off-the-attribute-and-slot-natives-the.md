## 2026-11-19: Rust equip natives in Off: the attribute and slot natives, the equip run and the AllParts note

AI-assisted (Claude Code). Lane FIX-EQUIP-NATIVES (slice plan step 4, link 8; owners `ow-inventory` and `ow-attr`).
The lane implemented what the records already describe and wrote the analyst note for the one native no record
covers, `AllParts`. It made no change to parsing. Nothing here was checked in the running game, so every rule is
**UNVERIFIED** there.

- **The installed equip run** (new, Rust-only): `<package> --inventory-move --equip-run <weapons file> --cooked <dir>
  --output <file>` (`crates/ow-cli/src/cmd/inventory/equip_run.rs`). On a gameplay world in Off (refuse-all stub
  policy, the world's packs, `ow_combat::hit::Pack` and the inventory pack), it gives each weapon of a `--script-diff`
  list an owner (pawn, controller, inventory manager, all from class defaults) and runs four phases, each even when
  an earlier one stopped:
  - the installed `InitializeInternal` (the spawn);
  - the base values;
  - the installed `ApplyAllExternalAttributeEffects` (what `WeaponEquipping.BeginState` calls);
  - the installed `RemoveAllExternalAttributeEffects`.
  It prints each phase's stops with a weapon count. Two harness stand-ins, each labelled in the module and neither a
  rule:
  - the owner links take the place of the pickup and `AddInventory`, which are host side;
  - the controller's accuracy pool is made through the Off `CreatePool` native from Maya's class's
    `AccuracyPoolDefinition`. The game makes that pool in the player's own setup, which no equip script reaches. A
    `WorldInfo` from class defaults lets the pool native spawn its manager.
- **Stops on the installed game** (desktop, 2026-11-19; 50 weapons, one every 30 lines of a seed-1 `--item-card
  --weapons` roll of every weapon balance at six stages; outputs under ignored `local/p2/fix-equip-natives/`):
  - init: 50 of 50 stop at the unbound `WillowWeapon.ComputeValueOfParts`, before and after the lane. The record
    says its body was not resolved (NATIVE_WEAPON_RULES.md section 6), so it is left for the spawn lane.
  - equip: 50 of 50 stopped first on a port bug (`bOffHand holds Byte, not a bool`), then at the pool query's
    controller-only rule. With both fixed and the accuracy pool in place, 50 of 50 apply the weapon type's
    external effects and stop at the unbound `WillowWeapon.AllParts`.
  - unequip: on main (this harness built on the unchanged libraries, `origin/main` 86c28d0) 50 of 50 pass with
    nothing to remove, because the failed equip leaves no records. With the two fixes but without the new natives,
    50 of 50 stopped at the unbound `AttributeDefinitionBase.RemoveAttributeModifier`. After the lane, 50 of 50 pass
    and remove every record (3 or 4 per weapon, down to 0).
  - Each new native key equals the key the Rust loader derives from the installed packages (checked once with a
    temporary print, not kept).
- **Fixes on the equip path:**
  - `ow-attr`, `WeaponResourcePoolAttributeContextResolver`: the installed `Engine.Weapon` declares `bOffHand` as a
    byte, and its script only compares it with 0 (17 comparisons). The resolver now reads a nonzero byte as the off
    hand. A bool is still accepted.
  - `ow-attr` exports the record's controller rule as `context_resolvers::controller_of`, and `ow-world`'s resource
    pool query now uses it on any source. That query sits outside the lane's two crates, so the edit is flagged for the hub.
    Before, only a controller source was read, so a weapon source (the accuracy effects the equip passes with the
    weapon as the override) was `NotYet`. A source with no controller now has no pool (`Ok(None)`), as the resolver
    answers. Other owner resolver classes stay `NotYet`. The query is not gated on parity, so Reference changed too: a
    pawn source, or an actor with an `Instigator` (a held weapon), now gets its controller's pool or `Ok(None)` where it
    stopped with `NotYet`. That is closer to the C++ harness, whose pool lookup uses the controller's manager whatever
    the source, but no Reference test pins it yet (UNVERIFIED).
- **Natives bound in Off** (`ow_inventory::equip::natives`, registered by the inventory pack in Off only. The C++
  binds none of them, so Reference keeps the stub policy under R3):
  - `AttributeDefinitionBase.ResolveContext`, `AddAttributeModifier` and `RemoveAttributeModifier` call `ow-attr`
    (NATIVE_ATTRIBUTES.md section 1). `--script-diff` keeps its harness bindings for the script side, and in
    Reference for both sides.
  - `WillowInventoryDefinition.GetAttributeSlotMaxActivated` gives the stored count when it is 1 to 19, else 19.
    The records give that clamp for the number of slots considered. That this getter applies it is this lane's
    reading.
  - `WillowInventory.ApplyExternalSlotEffectModifiers` follows NATIVE_ATTRIBUTES.md section 9. The value it applies
    is the stored `ComputedModifierValue`, whose computation has not been read.
  The run cannot reach the last two yet, because the equip stops at `AllParts` first. Only the unit tests cover them.
- **Analyst: `AllParts`** (NATIVE_EQUIP_PARTS.md, new). It was read with a read-only Ghidra query, and the slot
  positions were checked against the package layout (`class_layout.py`, where `Core.Object` comes out at 60 bytes).
  The note says it walks eleven part slots in declared order, takes the name parts only with the flag, and skips
  empty slots. Off leaves it unbound: a fresh implementer builds it from the note. The lane's one Ghidra query also
  asked for `GetAttributeSlotMaxActivated` by mistake. That output (the native and its own callee) was deleted
  unread, and the native was implemented from the records only. This is a clean-room process exception the
  maintainer should know about: the lane both analysed `AllParts` and implemented the slot-max native whose Ghidra
  output it received. The code matches NATIVE_WEAPON_RULES "Attribute slots" and NATIVE_ATTRIBUTES section 9 word for
  word, and adds nothing beyond them.
- **Verified on invented data:**
  - `ow-inventory`: 46 tests, 6 of them new. They cover each native through a declared native function; Off binds
    all five keys and Reference binds none; the slot filter, the count, the override-first context and the
    appended records.
  - `ow-attr`: the resolver fixture now declares `bOffHand` as a byte, as the installed class does.
  - `ow-world`: the pool query finds a controller's pool from an actor whose instigator is that controller's pawn,
    and an object that is not an actor has no pool.
  - `ow-cli`: the equip run's summary.
- **Open:**
  - `AllParts`, for a fresh implementer from NATIVE_EQUIP_PARTS.md. After it come `ApplyExternalSlotEffectModifiers`
    and `GetAttributeSlotMaxActivated` on the installed run.
  - The spawn path: `ComputeValueOfParts`, `InitializeAttributeSlots` with its computed slot value, and
    `ApplyInternalSlotEffectModifiers`.
  - `WeaponResourcePoolAttributeContextResolver` on a controller source, which is unread. Real play does not reach
    it once the player has its accuracy pool.
  - Who creates the player's accuracy pool. It is part of the player's setup and no record reads it.
  - An `equip` step in `--combat-run`, which still needs the AllParts implementer. The Rust equip's part order
    against the note (it skips the name parts on the equip).
  - The pre-existing `--script-diff` line whose `BulletSpeed` NotYet appears on both sides.
