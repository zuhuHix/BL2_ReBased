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
- **The native:** it never refuses and never writes. The game logs a line only for the exact base.
- **Off's answers, kept separate from the native:** the thirteen read classes answer by the rule; the exact base
  answers None with no diagnostic; any other class (a plain `Engine.Weapon`, an unread descendant of either tree such
  as a DLC class, or another subclass) is a `NotYet` of kind `NotRead`. A missing `DefinitionData`, or a missing
  selected field, is a named `NotYet` of kind `NotRead`; a declared field that holds None answers None.
- **Review r1 of #457:** the Off answers above; how the monetary-value helper chooses (the first declared field, even
  when None) and why a change must keep it; and live checks for the populated and null references, the base with its
  log, other receivers, and the reference during the drop.
- **It belongs with `ow_inventory::definition`'s three `Engine.WillowInventory` natives**, whose paths lane IMPL-PICKUP
  fixed. The module's reading of the definition field agrees with this one, but the native chooses the field by class,
  not by which field is set.

**Open (in the note).** A plain `Engine.Weapon`, another `WillowInventory` subclass, or a `WillowWeapon` / `WillowItem` descendant beyond the thirteen read classes (not read); whether
`DefinitionData` can still be unset when the drop calls it.
