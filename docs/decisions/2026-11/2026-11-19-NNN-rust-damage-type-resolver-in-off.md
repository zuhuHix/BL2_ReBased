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
  entry's value. The note's "1 when in the list" is what the installed lists give: every entry there holds 1 (review r1
  of #584, item 4).
- **Read-only evaluation** (review r1 of #584, item 2): the chosen data is evaluated only when it is a constant,
  `BaseValueConstant` times `BaseValueScaleConstant`. Data with a `BaseValueAttribute` or an `InitializationDefinition`
  is `NotYet { NotPorted }` before it is touched, because the general initialiser could load a cold definition or
  reach a resolver that writes. The installed lists and fallbacks are constants. The shared selector of the
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
  - with several matching entries the first one is taken. That is `tools/weapon_recipe.py`'s order, and the installed
    lists give every entry the value 1, so the order does not change a slice result.
- **Preflight:** the class needs no mode-specific availability. `preflight_read_resident` (#573) passes it with nothing
  written.
- **Verified:** 9 synthetic tests (rule 9) cover:
  - the default type, the custom type and a part with no custom type;
  - no type, an empty list and the first match, with a fallback of 7 so that a hardcoded 0 fails;
  - only constants evaluated: a matched entry or a fallback with an attribute or a definition is refused, with nothing
    written;
  - Reference: both preflights, a value, a base value, the storage and a read all keep the unregistered refusal;
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
