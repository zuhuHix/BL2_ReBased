## 2026-11-15: Rust item roll: two modes, the pool-level manufacturer and one generator per roll

AI-assisted (Claude Code). Lane W4-ROLL of the Rust port fills `crates/ow-items/src/{merge,candidates,pick,names,rarity,value,roll}.rs`
and the `--item-roll` command (`crates/ow-cli/src/cmd/items_roll.rs`), architecture section 3.7. No parsing behaviour
changes. This file records the choices a later lane should be able to find.

- **Edits outside the lane's own files** (review r1 of PR #319, each granted by the hub):
  - `crates/ow-items/src/error.rs`: `ItemError::Attr`, `Balance`, `BadData` and `WrongMode`. They are additive (the
    enum is `#[non_exhaustive]`), with `From<AttrError>` and `From<BalanceError>`, which keep a `NotYet` as `NotYet`,
    and `source()`.
  - `crates/ow-items/src/registry.rs`: a small public lookup, `TagLookup`, with `Props` and `Raises`. It is the
    registry's census and tag-stream reading under given schema lines, and `attribute_value` with attribute values
    passed in. Existing callers pass none, and the registry's output is unchanged: `item_registry_rust_test.py`
    passes, and the real `--item-registry` is byte-identical.
  - `tests/item_roll_rust_test.py`, `tests/rust_parity.d/roll.txt` and the `item-roll-rust` ctest entry. The ctest
    entry sets `OW_PARITY=reference`, as the manifest's `mode=reference` marker does.
  - The `ow-items` block of ARCHITECTURE.md section 3.7 and the `--item-roll` row of section 2.3.

- **One generator per roll.** `RollWorld` hands the roll's one `CrtRand(seed)` to every stream, the evaluator's
  included, as both oracles draw one stream per roll. It forwards `attribute_changed` only over a world that runs no
  script; over the Vm it answers `NotYet { NotPorted }`. The seeding is the tools'; the game's sequence is not
  reproduced (UNVERIFIED).
- **Two modes, the mode a value in `RollCx`.**
  - `ParityMode::Reference` reproduces `item_roll.roll` (shields, grenade mods, class mods, relics) and
    `weapon_recipe.roll` (weapons), stand-ins included, so the census and the weapon comparison can run on Rust
    output.
  - The Reference stand-ins (architecture section 6.1) are:
    - the manufacturer and grade drawn in the balance roll;
    - the uniform class-mod definition pick;
    - a part weight that needs runtime state taken as 1, and a window bound that needs it taken as no bound.
  - Weapons take the root-most manufacturer and weapon type, with no manufacturer draw, as `weapon_recipe.merge` does.
  - `ParityMode::Off` puts each choice where the records put it (UNVERIFIED in game):
    - The pool-level candidate expansion of NATIVE_LOOT.md: one candidate per manufacturer entry, at
      `min(stage, cap)` with `ow_balance::cap::item_stage_cap`, walking the base chain and stopping at the first step
      that yields one.
    - A balance rolled on its own draws among its candidates as a one-entry pool would, so it always draws once,
      even for a single candidate. That is a different `CrtRand` draw order from the oracle's; a test pins the
      extra draw.
    - Values that need runtime state go through the `ow-attr` evaluator: windows, weights, spawn modifiers, rarity
      terms and name expressions.
    - Class mods answer `NotYet { NeedsRealGame(6) }`. After W7 deletes Reference, the census no longer covers class
      mods until question 6 is answered.
- **Off mode's pick arithmetic** is the tools': CPython's compensated sum and a double-precision draw. It is
  UNVERIFIED as the game's: NATIVE_LOOT.md gives single-precision intervals for the pool pick, and `weapon_gen.cpp`
  sums plainly.
- **Question 7 (the grenade-mod per-element minimum).** 1,035 grenade-mod part entries bound their window by an
  attribute whose only value resolver is a constant resolver. Reference reads such a bound as no bound, as the tools do.
  Off mode evaluates it, which gives the constant, so the bound applies. The hub confirms this as the intended Off
  reading (review r1). It stays UNVERIFIED until question 7 is answered. A test shows the part dropping out below the bound in Off mode only.
- **Name parts in the rarity sum** is a reading, not a stand-in: the same in both modes, UNVERIFIED (question 6).
  Rarity also counts the two name parts for weapons; every weapon name part has rarity 0 (NATIVE_WEAPON_RULES.md
  section 12), so the weapon sum is the card's.
- **The two merge readings** (`merge.rs`): leaf-most (the registry's fields, `item_roll.py`) and root-most
  (`weapon_recipe.merge`) stay named functions. `reading_for` picks root-most for a weapon balance and leaf-most for
  every other kind. ITEM_REGISTRY.md section 4 counts where they differ: 12 class mod and 8 usable balances
  (manufacturers), 1 usable (definition). The synthetic test has a record of each shape, and the installed game's registry
  gives the same count (below).
- **Where the values come from.** The roll reads raw properties the registry does not hold: rarities, slot effects
  and upgrades, name parts, flags and globals. They come from each owner's tag stream through the registry's
  `TagLookup`, with `item_roll.py`'s schema lines (a superset of the registry's). That is the same reading as
  decision 2026-11-15-15. `RollData` (`merge.rs`) wraps it; `RollCx` holds it `pub(crate)`.
- **A known difference in a rare case, kept.** `attribute_value` stops at the first formula term with no value.
  The tool evaluates all four terms first, so a later term that raises makes the tool raise where the port answers
  "no value". Both the registry and the roll have it. It is not changed, because the registry is byte-identical on the
  installed game.
- **Shapes (architecture section 3.7, hub decisions in review r1):**
  - `roll` returns `Result<RollResult, ItemError>`: an item, or `NotRollable` with the oracle's reason.
    `Rolled::definition_data()` gives the definition data.
  - `roll_candidate` rolls a candidate a pool already drew. It serves Off mode only; under Reference it answers
    `ItemError::WrongMode`.
  - `RollCx` carries the attribute system, the mode, the item's instance, a manufacturer the caller chose and the
    objects the caller holds by path.
  - Off mode with an instance: the caller runs `expand`, draws, instantiates the item, then calls `roll_candidate`
    with `with_item`. Without one, `roll` answers `NotYet` for what needs it.
- **Review r1 fixes.**
  - `GetExpLevelFromManufacturerData` asks each chain step in turn. A step that interpolates answers the word
    unchanged. A non-interpolating leaf over an interpolating base gives a base candidate the word as its level.
  - Float-to-integer conversions are checked: a rarity term or `ExpLevel` that is not finite or out of range is
    `BadData`, where Python's `math.trunc` raises.
  - The Off-mode check for a weight with no index looks only at the slots the roll draws.
  - The CLI answers `--overlay` with `NotYet` (exit 3) and reports a refused offline world.
- **Off-mode gaps that answer `NotYet` instead of a value.**
  - Name expressions need the item's instance as the source.
  - A part weight with no init data index.
  - A tagged `Operand2Usage` (its enum names are not read).
  - The `DamageTypeAttributeValueResolver` is not registered in `ow-attr`, so `WeaponDamage_Is_<Element>` answers
    `NotYet { NotRead }` there.
- **Off-mode gap that is not checked.** `GetInventoryDefinitionForManufacturerGrade`'s `InventoryClass` test is not
  applied: a tag stream does not carry the class default it needs.
- **Verified on the installed game** (pc-hummer, 2026-11-15, after W3-DEFS landed; `OW_PARITY=reference`; outputs
  under ignored `local/p2/w4-roll/real/`; the comparison is a local script that imports the tools and edits none):
  - `--item-roll --census` (39 s): 35,660 rolls (shield 14,380, grenade mod 6,300, class mod 6,600, relic 8,380)
    and 2,020 not rollable, the per-kind counts of ITEM_ROLL.md section 3.
  - `item_roll.check` on the Rust output: 0 failures in 35,660 rolls.
  - The same rolls equal `item_roll.roll` field for field: 35,660 of 35,660, and 2,020 of 2,020 not-rollable
    reasons.
  - `--item-roll --weapons` (39 s): the 249 weapon balances `Startup.upk` exports, at stages 1, 8, 15, 30, 50 and 80,
    seeds 1 to 20. They equal `weapon_recipe.roll` on 29,880 of 29,880 guns: parts with weights and candidates,
    prefix, title, name, manufacturer, game stage, weapon type and notes.
  - That last count holds with Python in UTF-8 mode (`python -X utf8`). Under the Windows default code page, 120
    guns (one balance) differ in the title's text and the name only. The chosen title part and priority are equal.
    The oracle decodes the Rust reader's UTF-8 output as cp1252 (`weapon_recipe.Package.run`, `text=True`), so it
    sees a non-ASCII character as two. This is found, not fixed: it affects every tool that reads strings through
    the Rust reader on Windows (decision 2026-11-15-17).
  - The records whose two readings differ (the root-first fields the registry writes): 12 class mod and 8 usable
    balances (manufacturers) and 1 usable balance (definition), as ITEM_REGISTRY.md section 4 counts.
  - The Rust `--item-registry` tree equals the tool's `registry.json` on the same install.
  - Off mode on the same balances (no oracle): 12,760 rolls complete and 1,900 are not rollable. 6,720 class mod
    rolls answer `NotYet` (question 6). 16,300 answer `NotYet { NotPorted }`, because their name expressions need
    the item's instance.
- **Verified on an invented install.**
  - `tests/item_roll_rust_test.py` is the tracked gate: its own invented install, the Rust `--item-roll --census`
    and `--weapons`, checked with `item_roll.check` and compared with `item_roll.roll` and `weapon_recipe.roll` on the
    C++ reader. The manifest line `roll.txt` and ctest `item-roll-rust` run it.
  - On an invented install (`roll.rs` tests, written out by the ignored `write_the_fixture_for_the_tools`), the
    oracles run with the Rust `ow-package` gave the same result as the Rust Reference roll:
    - `item_roll.roll`: 1,320 of 1,320 item rolls equal, field for field (parts, name parts, rarity level and band,
      manufacturer, grade, definition, level, hook removals, notes).
    - `item_roll.check`: 0 failures on those 1,320 rolls.
    - Not-rollable cases: 120 of 120 equal.
    - `weapon_recipe.roll`: 240 of 240 guns equal (parts with weights and candidates, prefix, title, name,
      manufacturer, notes).
  - Rust unit tests pin every rule above on synthetic data.
- **Not verified.**
  - Every rule, in the running game. The census proves that the Rust roll keeps to the oracles' rules, not that the
    rules are the game's.

