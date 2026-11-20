## 2026-11-19: Rust DamageTypeAttributeValueResolver

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-DAMAGE-TYPE-RESOLVER, for slice link 8 (equip). The decision
number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** `WillowGame.DamageTypeAttributeValueResolver` is implemented in `ow-attr` by the new module
  `crates/ow-attr/src/value_resolvers/damage_type.rs` and registered with the other value resolvers. It answers in
  `ParityMode::Off` only. In Reference the preflights, a value, a base value and the storage each keep the refusal the
  class gave while it was unregistered ("value resolver class <path>", `NotYet { NotRead }`), as the black-market
  resolver does (review r1 of #584, item 3).
- **Source:** NATIVE_WEAPON_RULES.md section 5, "Name-part expressions": 1 when the weapon's damage type is in the
  resolver's list, else `ValueIfNotMatched` (0). The name-part outcome of that rule was confirmed live once
  (2026-10-23). The weapon's damage type follows NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576, accepted at
  3225f887). The field names (`DamageTypes`, `AssociatedDamageTypeName`, `ValueIfMatched`, `ValueIfNotMatched`) come
  from the installed class layout through `ow-package --def-dump`, a structural read. This lane read no native code or
  analysis output (clean room).
- **Behaviour:** the `ValueIfMatched` of the first `DamageTypes` entry whose `AssociatedDamageTypeName` is the object
  name of the weapon's damage type, else `ValueIfNotMatched`, ignoring the previous value. **Each list entry carries
  its own `ValueIfMatched`** (an `AttributeInitializationData`, by the installed class layout), so a match answers that
  entry's value (review r1 of #584, item 4). The synthetic tests give entries other values, 2 among them, to pin that
  per-entry selection.
- **What was inspected in the installed packages** (`ow-package --def-dump`, output under ignored `local/`): two
  resolvers only, `D_Attributes.WeaponDamageType.WeaponDamage_Is_Corrosive`'s (two entries) and
  `WeaponDamage_Is_Amp`'s (one entry), plus the Corrosive attribute definition. In both, every `ValueIfMatched` is the
  constant 1 and `ValueIfNotMatched` the constant 0, with no attribute or definition set. The other resolvers' lists
  were not inspected, so "every installed entry holds 1" is **not** established. The live name-part check of
  2026-10-23 (NATIVE_WEAPON_RULES.md section 5) confirms one title outcome, not the list values (review r2 of #584,
  item 4).
- **Read-only evaluation** (review r1 of #584, item 2): the chosen data is evaluated only when it is a constant,
  `BaseValueConstant` times `BaseValueScaleConstant`. Data with a `BaseValueAttribute` or an `InitializationDefinition`
  is `NotYet { NotPorted }` before it is touched, because the general initialiser could load a cold definition or
  reach a resolver that writes. The two installed resolvers inspected hold constants only. The shared selector of the
  manufacturer and weapon-type resolvers, which uses the general initialiser, is no longer used here.
- **The weapon's damage type (#576):**
  - no `DefinitionData.WeaponTypeDefinition` means no damage type, and the elemental part is then not read;
  - otherwise it is the type's `DefaultDamageTypeDefinition`;
  - that is replaced by the `DefinitionData.ElementalPartDefinition`'s `CustomDamageTypeDefinition` when both are set.
- **Only resident objects are read** (a load is a write):
  - the resolver is checked first, and its fields and the context's `DefinitionData` are read without a load (review
    r1 of #584, item 1: a direct value or base-value read can reach the resolver without the resident preflight);
  - an unloaded resolver, context, weapon type definition or elemental part is `NotYet { NotRead }` naming it, with
    #576's record;
  - one whose load failed is `BadData`;
  - the damage type handle is never loaded: only its object name is compared.

  `ow-combat`'s loading `weapon_damage_type` helper is not reused, as #576 notes.
- **Edge cases, settled the way the manufacturer resolver settles them:**
  - an empty list matches nothing;
  - a weapon with no damage type matches nothing, not even an entry named `None`;
  - a context with no `DefinitionData` (not an item) is `NotYet { NotRead }`;
  - names are compared by FName equality, as `tools/weapon_recipe.py` and the manufacturer resolver compare them;
  - with several matching entries the first one is taken. That is `tools/weapon_recipe.py`'s order. In the two lists
    inspected no name repeats;
  - the whole `DamageTypes` list is checked (an array of entries with a name each) on every path, a weapon with no
    damage type included, and `ValueIfNotMatched` is read only once no entry matched (review r2 of #584, item 3);
  - a slotless resource object keeps the refusal a loading read gave it, checked without a load: `NotYet
    { LayerAbsent("ow-defs") }` with no store, and `NotYet { NotRead }` for a stand-in the store leaves slotless. The
    check is now shared with `ensure_loaded` as `eval::slotless_refusal` (review r2 of #584, item 2).
- **Preflight:** the class's availability is mode-specific: in Off, `preflight_read_resident` (#573) passes it with
  nothing written; in Reference, both preflights refuse it with "value resolver class <path>" (`NotYet { NotRead }`),
  the unregistered class's refusal. `preflight_read_resident` now asks the mode availability **before** the residency
  check, so a cold or failed handle of such a class gets that refusal by its class name, not the unloaded-handle
  `NotRead` or `BadData` (review r2 of #584, item 1; a change to #573's preflight order).
- **Verified:** 12 synthetic tests (rule 9) cover:
  - the default type, the custom type and a part with no custom type;
  - no type, an empty list and the first match, with a fallback of 7 so that a hardcoded 0 fails;
  - only constants evaluated: a matched entry or a fallback with an attribute or a definition is refused, with nothing
    written;
  - Reference: both preflights, a value, a base value, the storage and a read all keep the unregistered refusal, and a
    cold store handle of a class registered to the resolver is refused by its class through the registry's dispatch,
    with nothing loaded;
  - slotless resolvers and item contexts, with and without a store;
  - a malformed list refused on both paths, and a malformed fallback read only when nothing matched;
  - a non-item context;
  - the resident preflight;
  - on an invented package in a synthetic store: a cold resolver, through both a value and a base value, and a
    fallback naming a cold definition, each refused with no load and no heap write; a cold type definition and a cold
    elemental part, both refused with nothing loaded; and a resident type's unloaded damage type handle, matched by
    name and left unloaded.
- **Probe (local, not pushed):** this change merged with #534 and #552 and run through the installed Off strict
  `--slice-gate` gives 1 ok / 4 notyet / 9 error. Link 8 passes `make` and the muzzle-flash evaluators, and stops at
  `init` on the unbound `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`. That is the expected
  next stop, now an error, where before it was a `NotYet`.
- **UNVERIFIED in game:** everything beyond the name-part outcome, including the first-match order, the FName
  comparison and the refusal of a non-item context.
