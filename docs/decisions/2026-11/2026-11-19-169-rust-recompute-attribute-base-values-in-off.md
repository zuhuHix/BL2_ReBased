## 2026-11-19: Rust RecomputeAttributeBaseValues in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-RECOMPUTE-ATTRIBUTE-BASE-VALUES, for slice link 8 (equip). The decision
number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** `WillowWeapon.RecomputeAttributeBaseValues()` is bound in `ParityMode::Off` by the new module
  `crates/ow-inventory/src/equip/rebase.rs`, registered in the inventory pack. Reference is unchanged: the native
  stays unbound and the stub policy answers it.
- **Source:** NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md (PR #539 at 31d529e5, accepted), and only that note. This
  lane read no native code or analysis output (clean room).
- **Behaviour:** for every stack-attribute property of the weapon's runtime class, inherited ones included
  (`ClassTable::props` order), the modifier stack becomes empty and the base becomes the value's stored value, as a
  plain property write: no recompute, no notification, not through `ow-attr`'s set-base. Then
  `WeaponAttributeModifiers` becomes empty. The native reads no definition, evaluates nothing, draws nothing and loads
  nothing.
- **Preflight:** all checks are read-only and run before the first write. Every value is checked against its
  destination's declaration. A data shape the note does not cover is a `VmError::Script` naming the field: no
  `WeaponAttributeModifiers` array, unlinked companions, a non-array stack, a base whose numeric kind differs from the
  value's, or a static-array attribute. The native raises no `NotYet` (the note's section 4, item 6), so no
  partial-writes mark arises.
- **Verified:** 6 synthetic tests (rule 9):
  - the bake, including an inherited triple, an int above 2^24 kept exact, a byte attribute, an untouched plain field
    and a modifier object left alive;
  - idempotence;
  - no notification;
  - two preflight refusals that write nothing;
  - Reference binds nothing.
- **UNVERIFIED in game:** every rule, including the reach to inherited triples (the hub's ruling in the note) and the
  direction of the copy.
