## 2026-11-19: Read WillowInventory.GetInventoryDefinition as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INVENTORY-DEFINITION, analyst only (clean room). New record:
`docs/verification/NATIVE_INVENTORY_DEFINITION.md`.

- **Sources, in order:**
  - the records (none reads it);
  - the installed packages (`ow-package --exports` and `--payload`, `tools/ghidra/class_layout.py`);
  - the script (`research/script_disasm.py`);
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local read-only table reader, one
    headless run at a time under the machine lock.
- Raw output stays under ignored `local/analysis/invdef/`. No bl-sdk session was granted and no live evidence is
  recorded, so every rule is **UNVERIFIED**.
- **No code changes.** The native stays unbound in Off until a fresh implementer codes it from the note alone.

**What the note settles.**

- The declaration is `Engine.WillowInventory.GetInventoryDefinition()`: native, no parameters, a
  `WillowInventoryDefinition` result. No script override exists in the installed packages. `Engine.Weapon` extends
  `WillowInventory`, so weapons reach it too, as the drop script does (`WillowWeapon.DropFrom` through `Weapon.DropFrom`
  to `WillowInventory.DropFrom`).
- **The answer, by the object's engine class:**
  - a weapon (the `WillowWeapon` tree, four classes): its `DefinitionData.WeaponTypeDefinition`, as stored;
  - an item (the `WillowItem` tree, nine classes): its `DefinitionData.ItemDefinition`, as stored;
  - `WillowInventory` itself: None, after an engine log line.
- It never refuses and never writes.
- An Off answer list for an implementer, step by step, with a `NotYet` of kind `NotRead` for any class outside the two
  read trees.
- **It belongs with `ow_inventory::definition`'s three `Engine.WillowInventory` natives**, whose paths lane IMPL-PICKUP
  fixed. The module's reading of the definition field agrees with this one, but the native chooses the field by class,
  not by which field is set.

**Open (in the note).** A plain `Engine.Weapon` or another `WillowInventory` subclass (not read); whether
`DefinitionData` can still be unset when the drop calls it.
