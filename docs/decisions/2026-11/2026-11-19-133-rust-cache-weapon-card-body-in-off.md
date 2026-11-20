## 2026-11-19: Rust CacheWeaponCard's body in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-CACHE-WEAPON-CARD-BODY, implementer, for slice link 8 (equip), on top
of #506 (`fix/rust-cache-weapon-card`). `NNN` is a placeholder number, as on #506, until the maintainer rules on
three-digit numbers. The decisions check refuses it until then.

- **Clean room:** coded from `docs/verification/NATIVE_CACHE_WEAPON_CARD_BODY.md` (#491 at a419e35) and section 3 of
  `docs/verification/NATIVE_CACHE_WEAPON_CARD.md` alone. The implementer opened no native code and no Ghidra output.
  The body note lives on #491's branch, so the module's `RECORD` path resolves once #491 lands.
- **No live session.** Every rule stays **UNVERIFIED** in the running game. The body note's last section lists the
  bl-sdk checks.

- **New module** `ow-inventory` `equip::card`, registered from `manager::Pack`. It binds `CacheWeaponCard` in Off only,
  by its declaration's own native key; Reference binds nothing. It replaces #506's call-time `NotRead`, which leaves
  `equip::name_parts` together with that module's card preflight.
- **What it does** (module documentation, steps 1 to 7). It computes the whole card with no write:
  - the globals definition, through the Off getter's rule;
  - slot 0;
  - the presentation lookup (the DLC set's override list, then its default list, then the globals definition's
    two lists in the same order; a status-effect presentation replaced through its `MappingData` by damage type);
  - the two record maps;
  - the visits;
  - the order (priority at least 0.0001, largest first).

  It then writes, in this order: the stats cleared, slot 0's flag cleared when planned, the entries written, and
  replication on the authority (at most 5, after the slot entries).
- **What the note leaves unread is a named `NotYet { NotRead }`**, raised before any write:
  - an activated slot from 1 to 18 ("the card's slot count");
  - two appended entries of exactly equal priority ("the card order of equal-priority entries");
  - an appended entry with a record ("the presentation's displayed value"), so the section 3.3 value formula is
    not coded;
  - a visited definition that is neither a weapon type nor a weapon part.
- **The partial-writes mark** follows #506's rule: a `NotYet` while computing comes back unprefixed, and the caller
  that wrote (link 8's driver) marks it. Only a failure after the native's own first write is marked in the module.
- **The preflight** (`equip::card::preflight`, run by link 8's driver before `make`) does three things, each failure a
  `VmError::Script` naming the field, with nothing written:
  - NATIVE_WILLOW_GLOBALS.md section 6.2 steps 1 to 3;
  - the name-part schema;
  - the card schema.

  The schema loads each class that a checked field declares (`vm.find_class`, a code-package load, not a write)
  before reading that class's fields. The first installed run without that load refused
  `WillowGame.DownloadableItemSetDefinition declares no WeaponAttributePresentationOverride`: the class was only
  reserved by the reference. With the load, the installed schema holds. No synthetic fixture covers the
  reserved-class case; only the installed run does.
- **Link 8's world** (`equip_run::equip_world`) sets up NATIVE_WILLOW_GLOBALS.md section 6.1: a `WillowGlobals` from
  its class defaults, its `TheGlobalsDefinition` the installed `GD_Globals.General.Globals` (loaded there, the allowed
  preload), recorded as the world's globals. The unused `gate_world` is removed.
- **Synthetic tests** (`equip::card::tests`, 21 `#[test]`s, invented classes and values only):
  - the lookup order over the four lists, the first non-None entry, the status mapping (the type's damage type, the
    elemental part's, None, and no match clearing slot 0's flag);
  - slot 0's entry with its constraint, no entry when it is not included or not activated, and the slot-count
    refusal;
  - custom and damage-type presentations in order with the threshold, an increase on an existing entry, a card
    attribute with no record skipped;
  - the display refusal from either map and from a zero value, a below-threshold entry with a record left out, the
    tie and visit refusals, each with nothing written;
  - replication after the slot entry, the cap of 5, nothing on a non-authority, a weapon with no type writing nothing;
  - the globals' `LayerAbsent` and `NotRead`, the preflight holding, refusing globals of another class, an unset
    definition and two card schema failures;
  - binding by key in Off, and nothing bound in Reference.
- **Installed Off `--slice-gate`, strict, under the machine run lock**:
  - before (#506 at f66f82e, the r1 binary; 848ffc6 changes only `make`'s marking after a write): link 8 `notyet`,
    0 of 5 steps, stopped at `make` on the outline's `CacheWeaponCard` `NotRead`;
  - after: link 8 `notyet`, 1 of 5 steps (`make`), stopped at `init` on "stopped after partial writes:
    WillowWeapon.CacheWeaponCard: the card's slot count", so the installed weapon has an activated slot past 0;
  - the other 13 rows are identical in the two runs: 0 ok / 6 NotYet / 8 errors overall in both.
- **Review r1 of #521** (seven items; the hub's ruling: a preflight uses only non-loading lookups, class and code
  loading happens in explicit setup, and a definition load during computation is a write):
  1. **The preflight is read-only.** `equip::card::setup` (called by link 8's `equip_world` before any object is
     made) finds the card's root classes, then each class a checked field declares, with its parents, until the
     schema walk meets no reserved class. `card_schema` and `name_part_schema` find classes with
     `ClassTable::by_path` only: a class not loaded yet is a `VmError::Script` naming it. The globals definition's
     own class check still calls `Vm::find_class`, on a class the setup has loaded.
  2. **The slot count comes first.** It is checked right after the type field is read, before any definition is
     loaded. `cache_weapon_card` compares the definition store's settled handles (`DefStore::settled_handles`, new)
     and the heap's object count before and after the computation. A `NotYet` after a change, whether from a
     definition load or from attribute evaluation, is marked "stopped after partial writes".
  3. **`equip_world`** runs the setup first. The `WillowGlobals` allocation is its first write, and a refusal after
     it is marked.
  4. **A reserved subclass** is resolved in setup, with its parents, before `named()` reads its ancestry.
  5. **The base value** of each appended entry is read through `ow-attr` (`base_value_of`) in append order before
     anything is written, and its typed refusal passes through. B itself is not used, since V is 0 with no record.
  6. **First wins:** a key already bound keeps its handler.
  7. **`Role`** must be an enum byte that names `ROLE_Authority` (checked in the schema). The authority is resolved
     while computing, before the first card write.

  Six new synthetic tests (27 in `equip::card::tests`):
  - a plain-byte `Role`;
  - a base-value read that refuses;
  - the slot count before the type is read;
  - the load mark;
  - first wins;
  - a three-package store in which the weapon's `BalanceDefinition` names a reserved subclass in another package.
    The schema refuses it without loading it. The setup loads it with its parent, and the schema then stops at the
    DLC set's missing list.

  Installed gate after the fix: link 8 unchanged (1 of 5, stop `init` at the card's slot count), 0 ok / 6 NotYet / 8
  errors.
- **Review r2 of #521** (five items):
  1. **The write tracker** is #518's three monotonic counters, taken from #518 (origin/fix/rust-483-followup at
     211484a) unchanged so the stack folds: `Heap::committed_writes`, `GameRng::draws` and
     `DefStore::load_transitions`. `equip::card::load_mark` records them when the computation starts. A refusal is
     marked "stopped after partial writes" if and only if one of them moved, so a random draw now counts as a write.
     The r1 snapshot (`DefStore::settled_handles` and the heap's object count) is gone (item 3).
  2. **The preflight's globals check loads nothing.** `globals::globals_definition_of` takes the declared
     `GearboxGlobalsDefinition` and the `GlobalsDefinition` as classes already found. The preflight finds both with
     `by_path` (`defined_class`), and the setup finds them as card roots. `globals_definition` keeps its `find_class`
     lookups for its other callers.
  3. `settled_handles` is dropped (superseded by `load_transitions`).
  4. **The name-part ancestry.** After the card schema walk, the setup resolves each class the name-part schema's
     references declare: `DefinitionData`'s prefix and title parts, and `PrefixList` / `TitleList` on the type and the
     part. It resolves each with every parent, so `is_child_of` holds for a reserved subclass of
     `WeaponNamePartDefinition`.
  5. **`equip_world`** runs the card setup on the VM before the world is built and before any object is made. It
     then records the three counters and tracks from the first allocation, the `WorldInfo`'s default instantiation
     included.

  Three new synthetic tests (30 in `equip::card::tests`):
  - through the native's binding, an effect whose initialization definition has an enabled `RandomVariance` draws
    once, and then two equal-priority entries tie. The tie is marked once and the draw is counted, with no heap write
    and no load; without the draw the same tie is unmarked;
  - a reserved `GearboxGlobalsDefinition` is a preflight data error naming it, and it stays reserved;
  - a four-package store in which the type's `PrefixList` names a reserved subclass of the name part in another
    package. Before the setup it fails `is_child_of`; after the setup it holds.

  The `reserved` store fixtures gain a `GearboxFramework` package, since the setup now resolves that root.
  Installed gate after the fix: link 8 unchanged (1 of 5, stop `init` at the card's slot count), and the other 13
  rows are identical to r1. 0 ok / 6 NotYet / 8 errors.
- **Not done:** the slot count's source, the stable order of equal priorities, the display conversion and value
  formula, the visit of other definition classes, and any live check.
