## 2026-11-19: Bind WillowInventory.GetInventoryDefinition in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-INVENTORY-DEFINITION, implementer only (clean room): coded from
`docs/verification/NATIVE_INVENTORY_DEFINITION.md` (decision 2026-11-19-71, final as accepted in #457 at 2d169ba) alone. The
implementer did not analyse the native.

- **What changed.** `ow_inventory::definition` binds a fourth native in `ParityMode::Off`,
  `Engine.WillowInventory.GetInventoryDefinition` (`GET_INVENTORY_DEFINITION`), through the module's existing
  `register`. The `Engine.` path is the note's, and it is kept apart from the three older constants, which lane
  IMPL-PICKUP (#452) moves to the same prefix.
- **The Off answers, by the inventory's exact class (note section 3):**
  - one of the four read weapon classes: `DefinitionData.WeaponTypeDefinition`, as stored (None included);
  - one of the nine read item classes: `DefinitionData.ItemDefinition`, as stored;
  - exactly `Engine.WillowInventory`: None, with no diagnostic;
  - any other class, including a descendant beyond the thirteen (a DLC class, say) and a plain `Engine.Weapon`: a
    `NotYet` of kind `NotRead` naming the class;
  - a read class without `DefinitionData`, or whose `DefinitionData` declares no selected field: a named `NotYet` of
    kind `NotRead`, with the note's wording.
- **The class paths.** The note names the eleven classes after `WillowWeapon` and `WillowItem` without a package. The
  module reads all thirteen as `WillowGame.*` (UNVERIFIED). A class found under another path answers the "not read"
  `NotYet`, so a wrong guess stops the run and never answers silently.
- **It never writes.** There is therefore no partial-write case for the hub's "stopped after partial writes" ruling to
  cover. A layout that is declared but malformed (a `DefinitionData` that is not a struct, or a selected field
  that is not an object reference) stays a data error, not a `NotYet`, as elsewhere in the module (note section 3,
  step 5).
- **`ComputeMonetaryValue` is unchanged.** It still takes the first field its `DefinitionData` declares (note
  section 4); the new native has its own class rule and shares no helper with it.
- **Reference stays unbound.** `src/` has no stand-in.

**Verified.** Synthetic tests (`definition/get_definition_tests.rs`, invented classes) cover each step, the malformed
layouts, the class rule against a struct that declares both fields, and Reference. The slice gate's links 6 and 7 are measured on installed
Sanctuary (in the PR). **Not verified in the running game:** every rule (the note's live checks are still to be done).
