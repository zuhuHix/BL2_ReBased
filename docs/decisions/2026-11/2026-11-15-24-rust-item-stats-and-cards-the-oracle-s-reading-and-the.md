## 2026-11-15: Rust item stats and cards: the oracle's reading and the instance through ow-attr

AI-assisted (Claude Code). Lane W4-STATS of the Rust port fills `crates/ow-items/src/{stats,cards}.rs` (with their
test files `stats/tests.rs` and `cards/tests.rs`) and the `--item-stats` / `--item-card` commands
(`crates/ow-cli/src/cmd/items_stats.rs`), architecture section 3.7. No parsing behaviour changes. This file records the
choices a later lane should be able to find.

- **Edits outside the lane's own files** (review r1 of PR #323, each granted by the hub):
  - `crates/ow-attr/src/effects.rs`: `new_modifier` used a `Core.AttributeModifier` class the space only declared (a
    stack property of a store-built class names it before anything lays it out), so the allocation failed. It now
    uses the space's class only when it is laid out, and otherwise lays it out through the store. The test builds an
    invented Core package and a space that only declares the class.
  - `crates/ow-items/src/registry.rs`: `Props::statics(name)`, a read-only accessor for the static-array elements
    the registry already decodes. The registry's output is unchanged: `tests/item_registry_rust_test.py` passes (7
    tests).
  - `tests/item_stats_rust_test.py`, `tests/rust_parity.d/stats.txt` and the `item-stats-rust` ctest entry (with
    `OW_PARITY=reference`, as the manifest's `mode=reference` marker).
- **Two modes, the mode a value** (rule R3).
  - `ParityMode::Reference` reproduces `tools/weapon_stats.evaluate` field for field (`stats::weapon_attributes`,
    `cards::weapon_card`), stand-ins included. Values come from the owners' tag streams through the roll's
    `RollData`.
    - The bases are the tool's ten type fields (`TYPE_BASES`) with the class defaults it restates, and a spinning
      barrel's spin-up time. They are a subset of the script's bases (below): UNVERIFIED as the game's list, and
      enough for the card.
    - An attribute operand resolves only for `Weapon_Is_<Maker>`, the weapon level and attributes with a
      `ConstantAttributeValueResolver_*` subobject (`weapon_stats.resolver_constant`, looked up by the subobject's
      name, as the tool does, not through the attribute's chain); anything else leaves the value unresolved.
  - `ParityMode::Off` follows the script functions as read, with every value through `ow-attr`.
    - The instance: `stats::instantiate_stats` makes the item's instance (the class of its kind: `WillowWeapon`,
      `WillowShield`, `WillowGrenadeMod`, `WillowClassMod`, `WillowArtifact`, `WillowUsableItem`; the definition's
      own `InventoryClass` is not read, UNVERIFIED) and writes `ExpLevel`.
    - A weapon's stats are its own attribute stacks, read back with `value_of`.
    - An item's effects reach its owner (the shield pool), which is the world's object. Offline, each init is
      evaluated through `evaluate_init` with the instance as the source, and the owner's stacks are folded on the
      record's base (0 for the shield pool, UNVERIFIED).
- **Off-mode weapon bases, read from script** (`research/script_disasm.py` on `WillowGame.upk`, 2026-11-15; listings
  under ignored `local/analysis/`; UNVERIFIED in game). This replaces the first version's ten bases through
  `set_base`.
  - `CalculateWeaponBaseValues` makes 26 assignments. Each sets a weapon attribute's base with the attribute let
    (opcode 0x5F: the base companion is written, the value recomputed, no change notification).
    - The values come from the weapon type. Six are `EvaluateInitializationData` with the weapon as the source
      (`InstantHitDamage`, `InstantHitMomentum`, `MeleeDamage`, `BaseStatusEffectChanceModifier`,
      `StatusEffectDamage`, `ExtraShotDelay`). Sixteen copy a type field (`ShotCost`, `ClipSize`,
      `ProjectilesPerShot`, `AIAimError` into `AimError`, `Spread`, `PerShotAccuracyImpulse`, `Range` into
      `WeaponRange`, `FireRate` into `FireInterval`, `BurstInterval`, `AutomaticBurstCount`,
      `BurstShotAccuracyImpulseScale`, `ReloadTime`, `EquipTime`, `PutDownTime`, `ZoomedEndFOV`, `ZoomedRate`).
    - `InstantHitMomentum` is then multiplied by 100.
    - Only for a type with `bUseOverheatBehavior`: `OverheatRegenDelay`, `FireRegenDelay` and `RegenRate`.
  - `CalculatePartDependentWeaponBaseValues`, for the barrel, then the body, when set:
    - with `bIsSpinningEnabled`, the spin-up and spin-down durations (the barrel's into `BarrelSpin*`, the body's
      into `MagazineSpin*`);
    - with `bFlapsEnabled`, the flap expand and collapse durations;
    - each `EvaluateInitializationData` with the weapon as the source;
    - and the weapon's `BodyVariation` takes the body's `PartVariation` (a plain assignment).
    - That the flap block is not inside the spin block is this reading of the jump targets, UNVERIFIED.
  - A copied value enters the property's kind as the attribute store converts it (`ow_attr::stack::store_as`, a
    float into an int truncated); the VM's own conversion of the right side is `ow-vm`'s. They are taken as the same,
    UNVERIFIED.
  - `apply_weapon_effects` applies at most the type's `AttributeSlotMaxActivated` activated slots (class default 19,
    values outside 1..19 read as 19), the record's rule for weapons (NATIVE_WEAPON_RULES.md section 1). Python's
    `weapon_stats` applies no limit, so Reference applies none either. UNVERIFIED in game.
- **Shapes for architecture 3.7** (the hub writes the 3.7 text). W5-EQUIP's `override_script` of
  `CalculateWeaponBaseValues` binds `weapon_base_values`.
  - `instantiate_stats(data, level, sys, cx, known) -> Instance`: allocates the instance, writes `ExpLevel`, then
    calls `instantiate_onto`.
  - `instantiate_onto(object, data, sys, cx, known) -> Instance`: the same on an object the caller has (the world's
    weapon actor).
  - The three weapon steps it runs, each public:
    - `weapon_base_values(weapon, weapon_type, sys, cx)`;
    - `weapon_part_base_values(weapon, barrel, body, sys, cx)`;
    - `apply_weapon_effects(weapon, weapon_type, parts, names, sys, cx) -> ow_attr::api::Applied` (the records an
      unequip removes).
  - `known` names objects the caller holds by path; everything else comes from the world's store.
  - The Reference card is `weapon_card(stats, recipe, level, text) -> WeaponCard` (`to_pv` is the generator's JSON
    shape). The Off-mode numbers of a weapon instance are `cards::instance_numbers`.
- **One fold, one exception.** Every weapon attribute and the zoom fold through `Stacks::fold`
  (`ow_attr::stack::combine`, `WeaponGen` form). `Stacks::fold_raw` is the deliberate exception, for the accuracy
  pool only: `weapon_firing.accuracy_pool_of` starts its fold from a value it does not round to single precision (a
  decoded number or an init result), which `combine`'s `f32` base would round.
- **Items** follow `tools/prepare_stock_shield.py` (script `WillowItem.InitializeInternal`; UNVERIFIED in game).
  - The definition's, the parts' and the name parts' `ExternalAttributeEffects`.
  - Then the external slots that are not `bRunEffectsAsSkill`, at most `AttributeSlotMaxActivated` (class default
    19), with the name parts' upgrades counted.
  - The shield card refuses in Reference (`BadData`) a number that is not finite or not above 0, as the tool does.
    Off mode gives the number: what the game does with such a shield is not read.
  - Only the shield's three numbers have an oracle. The other kinds give their owner stacks, and their card numbers
    stay open (no presentation was read).
- **Card rules** (each UNVERIFIED in game unless its record says otherwise): presentation rounding in single
  precision, the accuracy remap, the status-chance mapping with its input clamp (slag's `[0, 3.33]`), the sale value
  capped by currency, the level line, the text lines.
  - The red flavour colour is the caller's: the CLI reads `--colours <file>` (the ignored local colours file), and
    without it uses the neutral grey `tools/game_colours.py` uses, as the tools do.
  - The installed `.int` files override the cooked strings (`cards::Localization`, the tools' reading of UTF-16,
    universal newlines and Python's `splitlines`).
  - `html.unescape` is ported for numeric references and `amp`, `lt`, `gt`, `quot`, `apos`, `nbsp`. Other named
    references are kept as written. None was met on the installed cards (the comparison below is bit-equal).
- **The gestalt fields** of `weapon_recipe.roll` (gestalt mesh, fragments, material) live in `stats::assembly`; the
  static array `AdditionalGestaltModeSkeletalMeshNames` comes from the registry's `Props::statics`.
- **A live overlay** (`--overlay`, a `weapon_dump.py` capture through `ow_defs::overlay::Overlay`) patches the
  objects the stats read, as `live_overlay.LivePackage` does. The definitions that `attribute_value` follows inside
  the registry's lookup are read unpatched; a capture holds weapon parts, types and name parts only.
- **The batch command** writes one line per request. A request that is not a gun gives
  `{"line":<n>,"error":...}`, a stage cap gives `not_yet`, and an evaluation that fails gives `error` with the
  reason. So one bad request does not stop the batch.
- **Verified on the installed game** (pc-hummer, 2026-11-15; outputs under ignored `local/p2/w4-stats/`).
  - The comparison is a local script that imports `tools/weapon_gen_parity.py` (its requests, `python_gun`, `Diff`
    and identity check) and edits no tool. Python ran with `-X utf8` (decision 2026-11-15-21). The Rust side is
    `OW_PARITY=reference ow-package --item-card --requests`.
  - Requests: 249 weapon balances at stages 1, 8, 15, 30, 50, 80 with seeds 1 to 20 (29,880 guns), and the guns of
    the 64 pools' 7,680 CrtRand rolls (drops from Python's `loot_pools`): 6,033 pool guns.
  - Result: **3,460,004 fields compared, 0 mismatches** (floats bit for bit, int and float kinds checked), 0 Python
    errors. The same holds with Python on the Rust reader (decision 2026-11-15-17) and on the C++ reader.
  - **The field count.** It is 3,460,004 = 3,063,386 − 34,338 + 430,956.
    - 3,063,386 is the 2026-10-23 comparison's shape counted on today's rolls (a local script, `field_count.py`).
    - 34,338 are the pool identity and drop-list fields of that shape: 23,040 and 11,298. They are W4-POOLS'
      comparison, not this lane's.
    - 430,956 are the 12 card fields added since: 12 for each of the 35,913 guns. The damage stack (5) came on
      2026-11-02 in PR #59. The accuracy pool (3), the refire rule (count, interval, known: 3) and the projectile
      speed scale (1) came on 2026-11-06.
    - Each gun also carries the request's identity check (5 fields: balance, stage, seed, spawn stage, cap), which the
      2026-10-23 comparison already counted.
  - **The pool guns.** The records' 6,795 pool guns and 2,919,834 fields were counted before PR #57's second review
    changed the pool stage gate.
    - The gate change was "eligible only inside its positive bounds", and the clamp only with variance.
    - That review's commit (e1ee5bf6) records "6,033 pool guns, 3,063,386 fields". Those are the numbers this lane
      reproduces in that shape, with either reader.
    - WEAPON_PARTS_CATALOG.md, ROADMAP.md and decision 2026-10-23-01 still carry the pre-review numbers. Correcting
      them is not this lane's to do.
  - `prepare_stock_shield.py`'s default card (level 8, seed 1) is reproduced by the Reference `--item-card --data`:
    the same three numbers as written (capacity, recharge rate, recharge delay).
- **Verified on invented data.**
  - `tests/item_stats_rust_test.py` is the tracked gate. Its own invented install has two weapon balances (one a
    mission balance), every card rule, and a shield with a refused twin.
    - The Rust `--item-card --requests` equals `weapon_gen_parity.python_gun` on the C++ reader: 120 guns, 0
      mismatches.
    - The Rust shield card equals `prepare_stock_shield.stats` bit for bit, and both refuse the zero-rate shield.
    - In Off mode the test fails, saying so.
  - The card cases of `tests/weapon_gen_test.py` are re-expressed on the same invented weapon (`stats/tests.rs`,
    `cards/tests.rs`): modifier stack and rounding, damage stack, burst rule, accuracy pool, rarity, price and text,
    currency cap, slot grade and clamps, missing base grade, slag remap, the card's damage-type rule.
  - `tests/prepare_stock_shield_test.py`'s shield is rebuilt as heap objects and its card reproduced at its level
    (2) through the real evaluator. The Up and Down bodies, every effect of an active slot, the name part's upgrade
    and the activation limit hold, and the two modes agree bit for bit.
  - An Off-mode weapon instance holds its stats on its own stacks. The test covers the copied int base, the momentum
    scale, the overheat block, a body's spin-up, the body variation and the weapon slot limit.
- **Off mode on the installed game is not reached; `NotYet`, exit 3.**
  - A weapon's 26 bases and its part-dependent ones are written without a data error. The damage base equals the
    Reference base on the gun tried.
  - Applying the effects then stops at the weapon's change notification, which needs the running world (W5-WORLD).
  - The shield's level attribute stops at `InventoryAttributeContextResolver`, whose equipped-only flag is not read.
- **Not done here (open).**
  - The D4 deletions (deferred by the hub until W4-POOLS lands and `tools/weapon_gen_firing.py` no longer imports
    `tools/weapon_gen_manifest.py`).
  - The golden cards (69 with the live overlay): run on the desktop by the hub.
  - The capped variant (`--stage-cap 50`): Reference applies no cap (W4-ROLL).
- **Not verified.** Every rule, in the running game. The comparison proves that the Rust card keeps to the Python
  oracle's rules, not that the rules are the game's.
