## 2026-11-19: Rust ApplyExternalSlotEffectModifiers reads AttributeSlots as the declared static array

AI-assisted (Claude Code). Lane FIX-ATTRIBUTE-SLOTS. Number 49 was given by the hub. It fixes the slot read of
`WillowInventory.ApplyExternalSlotEffectModifiers` (decision 2026-11-19-03), which decision 2026-11-19-39 found
stopping link 8. Parsing does not change, and the rule is the same as before. Nothing here was checked in the running
game.

- **The fix** (`crates/ow-inventory/src/equip/natives.rs`, `attribute_slots`). The item's `AttributeSlots` is read as
  its class declares the property: a static array of slot structs, element by element in index order, as many as the
  declaration's `array_dim`. The installed `WillowInventory` declares `AttributeSlots[19]` (`class_layout.py` on the
  installed `Engine.upk`). The count comes from the declaration, not from a constant. The first version read the
  property as a dynamic array, which on the installed class failed with "AttributeSlots: expected an array, found
  Struct".
- **Refused as data errors:** a class that declares no `AttributeSlots`, or declares it as anything but slot
  structs (a dynamic array included), or with a negative dimension. Nothing else changes: the filter (activated,
  external, not a skill, an attribute), the count of `MaxSlotsActivated`, the stored value and the context order are
  as in decision 03.
- **Verified on invented data** (`ow-inventory` natives tests, 14 of 14):
  - New, `slot_package_tests.rs`: an invented `WillowGame.upk` declares `AttributeSlots` with a dimension of 3,
    through the package test kit (`ScriptToy`, `property_payload`). The loaded declaration carries 3. Every declared
    slot is read in index order, and the first and last are applied. A count above the dimension stops at it, and a
    count of 2 stops before the last slot.
  - New, `tests.rs`: on the hand-built world, an effect in the last of seven declared slots is read, and a class with a
    dynamic `AttributeSlots` is refused.
  - The two earlier slot tests now declare the installed shape (a static array) and write their slots element by
    element. Their assertions are unchanged.
- **Installed game, Off** (desktop, 2026-11-19, rebased on `origin/main` 0b879c9, which has #430; outputs under
  ignored `local/p2/fix-attribute-slots/`):
  - `--inventory-move --equip-run` on decision 03's 50 weapons: equip passes on 49 of 50. The 50th stops at a
    `NotYet`: the world's game info needs the map load. Before, all 50 stopped on the slot read. Unequip passes 50 of
    50. The spawn phase (`init`) stops on all 50 at `InventoryAttributeContextResolver`'s equipped-only flag
    (`NotYet`, NATIVE_ATTRIBUTES.md section 5), which this lane does not touch. The equip run runs every phase
    whatever the phase before it did, so the fixed read shows there.
  - `--slice-gate`, link 8 `equip_gun`: `notyet` at step `init`, on the same resolver, under both gradings (strict 11
    notyet, 3 error; past-errors 14 notyet). Since review r1 of #430 the link runs the spawn's `init` before the
    equip, and a `NotYet` stops it under both gradings, so the equip step is not reached and this fix does not
    change the row.
  - **Correction.** An earlier draft of this decision said link 8 is ok with #430's driver. That came from a
    throwaway build of #430's first version, whose link 8 skipped `init`. It no longer holds: the link now stops at
    the spawn first.
- **UNVERIFIED** in the running game: the slot rule itself (decision 03), and that the game reads the 19 slots in
  index order.
