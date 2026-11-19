## 2026-11-19: Rust part iterator AllParts in Off, from the analyst note

AI-assisted (Claude Code). Lane FIX-ALLPARTS, the fresh implementer of `docs/verification/NATIVE_EQUIP_PARTS.md`
(written by lane FIX-EQUIP-NATIVES, decision 2026-11-19-03, PR #406). This lane coded from the note alone and opened no
analysis output. There was no real-game session, so the rule is **UNVERIFIED** in the running game. No dependency was
added, no bounds check was loosened and no existing `NotYet` guard was relaxed. The lane was
started on `fix/rust-equip-natives` and rebased onto main once #406 landed.

- **What was added** (`crates/ow-inventory/src/equip/natives.rs`, Off only, next to #406's natives):
  - `WillowWeapon.AllParts(DefinitionData, out Part, bIncludeNameParts)` and `WillowItem.AllParts(...)` are bound as one
    iterator, each under the native key the loader derives from its declaration. A package without the function binds
    nothing.
  - `all_parts` visits the definition data's eleven part slots by field name, in declared order (`WEAPON_ALL_PARTS`,
    `ITEM_ALL_PARTS`; their first nine are the equip's existing `WEAPON_PART_FIELDS` and `ITEM_PART_FIELDS`).
  - A slot holding None is skipped. The last two slots (the name parts) are visited only when the flag is set (any
    nonzero value); the material slot is visited either way.
  - Parts are not type-checked, and a part in two slots is visited twice. A slot holding a non-object is a typed error.
  - A loop that runs to its end leaves the out argument None. That is the VM's engine-iterator rule, and it is the
    note's.
- **Reference** is unbound: the C++ VM binds no `AllParts` (R3).
- **Left open, and why not `NotYet`.** The note leaves two cases unsettled: what the out argument holds after a `break`
  in the body, and whether a body that writes the definition data during the loop changes what is visited. The VM's
  iterator contract (`IteratorFn`) hands over the whole list when the loop starts and is not called again, so the
  native sees neither event and cannot answer `NotYet` for it. Here the VM keeps the last part after a `break`, and the
  list is fixed before the body runs. No installed caller does either (the note). Both stay UNVERIFIED.
- **Not changed:** `ow_inventory::equip`'s Rust equip still applies only the first nine slots. Per the note, the
  equip's flag is true, so the walk would include the prefix and title. That is a follow-up for that library, outside
  this lane (it is not the native).
- **Tests** (4, in `equip/natives/tests.rs`, on the equip fixture, which gained the two name-part fields):
  - the slot order, the skipped slots, the flag, a duplicate part, an all-None definition, and the first nine slots
    against the equip's lists;
  - the item's slots;
  - a non-object slot refused;
  - unbound under Reference.
- **Installed, before and after** (pc-hummer, Off; outputs under ignored `local/p2/fix-allparts/`).
  - The 50-weapon list is rebuilt by the recipe of decision 03: a seed-1 Reference `--item-card --weapons` roll of
    every weapon balance at six stages (1,494 rolls), every 30th line from the first, as weapon-file lines with each
    rolled part, prefix and title. 48 of the 50 have a prefix and all 50 a title. Decision 03's own list is on the
    desktop, so this one may differ from it.
  - `--inventory-move --equip-run`:
    - init: 50 of 50 stop at `WillowWeapon.ComputeValueOfParts`, before and after (the spawn lane's);
    - bases: 50 of 50 pass, before and after;
    - **equip, before:** 50 of 50 stop at the unbound `WillowWeapon.AllParts`;
    - **equip, after:** every weapon passes `AllParts`. 49 stop next, after the type's and parts' external effects,
      at `WillowInventory.ApplyExternalSlotEffectModifiers` (#406's native), reading `AttributeSlots`: "expected an
      array, found Struct". 1 stops at the game info query (`WorldInfo.Game` None);
    - records after equip: before, 3 or 4 per weapon (the type's effects); after, 3 to 7. 5 weapons gain records from
      their parts' `ExternalAttributeEffects`;
    - unequip: 50 of 50 pass and remove every record, before and after.
  - **The next stop's likely cause**, not this lane's to fix: `AttributeSlots` is probably a fixed-size array, which
    would fit 19 being the slot clamp. #406's native reads it as one value, which gives element 0, a struct. The
    declaration's size was not checked here, so this is UNVERIFIED.
  - `--slice-gate`: this base has no `--slice-gate`, so it was run on a throwaway merge of this base with main 8a5cd94,
    before and after this change. The rows are byte-identical: strict 0/5/9, past-errors 0/13/1. Link 8 stays
    `notyet`, because it has no headless driver (no run step joins a looted gun to a fire).
- **For the hub:**
  - `ApplyExternalSlotEffectModifiers`' `AttributeSlots` read is now every equip's stop;
  - the Rust equip's walk could include the name parts;
  - link 8 needs a driver before the gate can see the equip.
