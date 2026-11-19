## 2026-11-19: Rust DamageTypeAttributeValueResolver

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-DAMAGE-TYPE-RESOLVER, for slice link 8 (equip). The decision
number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** `WillowGame.DamageTypeAttributeValueResolver` is implemented in `ow-attr` by the new module
  `crates/ow-attr/src/value_resolvers/damage_type.rs` and registered with the other value resolvers. It is registered
  in every mode, like the manufacturer and weapon-type resolvers it sits beside.
- **Source:** NATIVE_WEAPON_RULES.md section 5, "Name-part expressions": 1 when the weapon's damage type is in the
  resolver's list, else `ValueIfNotMatched` (0). The name-part outcome of that rule was confirmed live once
  (2026-10-23). The weapon's damage type follows NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576, accepted at
  3225f887). The field names (`DamageTypes`, `AssociatedDamageTypeName`, `ValueIfMatched`, `ValueIfNotMatched`) come
  from the installed class layout through `ow-package --def-dump`, a structural read. This lane read no native code or
  analysis output (clean room).
- **Behaviour:** the `ValueIfMatched` of the first `DamageTypes` entry whose `AssociatedDamageTypeName` is the object
  name of the weapon's damage type, else `ValueIfNotMatched`. Either is evaluated with the context as the source,
  ignoring the previous value, through the selector the manufacturer and weapon-type resolvers share.
- **The weapon's damage type (#576):**
  - no `DefinitionData.WeaponTypeDefinition` means no damage type, and the elemental part is then not read;
  - otherwise it is the type's `DefaultDamageTypeDefinition`;
  - that is replaced by the `DefinitionData.ElementalPartDefinition`'s `CustomDamageTypeDefinition` when both are set.
- **Only resident objects are read** (a load is a write):
  - an unloaded context, weapon type definition or elemental part is `NotYet { NotRead }` naming it, with #576's
    record;
  - one whose load failed is `BadData`;
  - the damage type handle is never loaded: only its object name is compared.

  `ow-combat`'s loading `weapon_damage_type` helper is not reused, as #576 notes.
- **Edge cases, settled the way the manufacturer resolver settles them:**
  - an empty list matches nothing;
  - a weapon with no damage type matches nothing, not even an entry named `None`;
  - a context with no `DefinitionData` (not an item) is `NotYet { NotRead }`;
  - names are compared by FName equality, as `tools/weapon_recipe.py` and the manufacturer resolver compare them;
  - with several matching entries the first one is taken. That is `tools/weapon_recipe.py`'s order, and the installed
    lists give every entry the value 1, so the order does not change a slice result.
- **Preflight:** the class needs no mode-specific availability. `preflight_read_resident` (#573) passes it with nothing
  written.
- **Verified:** 7 synthetic tests (rule 9) cover:
  - the default type, the custom type and a part with no custom type;
  - no type, an empty list and the first match;
  - a non-item context;
  - the resident preflight;
  - on an invented package in a synthetic store, a cold type definition and a cold elemental part, both refused with
    nothing loaded, and a resident type's unloaded damage type handle, matched by name and left unloaded.
- **Probe (local, not pushed):** this change merged with #534 and #552 and run through the installed Off strict
  `--slice-gate` gives 1 ok / 4 notyet / 9 error. Link 8 passes `make` and the muzzle-flash evaluators, and stops at
  `init` on the unbound `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`. That is the expected
  next stop, now an error, where before it was a `NotYet`.
- **UNVERIFIED in game:** everything beyond the name-part outcome, including the first-match order, the FName
  comparison and the refusal of a non-item context.
