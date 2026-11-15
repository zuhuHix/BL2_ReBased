## 2026-11-15: Rust item stats and cards: the oracle's reading and the instance through ow-attr

AI-assisted (Claude Code). Lane W4-STATS of the Rust port fills `crates/ow-items/src/{stats,cards}.rs` (with their
test files `stats/tests.rs` and `cards/tests.rs`) and the `--item-stats` / `--item-card` commands
(`crates/ow-cli/src/cmd/items_stats.rs`), architecture section 3.7. No parsing behaviour changes, and no file outside
the lane's own was edited. This file records the choices a later lane should be able to find.

- **Two modes, the mode a value** (rule R3).
  - `ParityMode::Reference` reproduces `tools/weapon_stats.evaluate` field for field (`stats::weapon_attributes`,
    `cards::weapon_card`), stand-ins included. Values come from the owners' tag streams through the roll's
    `RollData`. An attribute operand resolves only for `Weapon_Is_<Maker>`, the weapon level and attributes with a
    `ConstantAttributeValueResolver_*` subobject (`weapon_stats.resolver_constant`, looked up by the subobject's
    name, as the tool does, not through the attribute's chain); anything else leaves the value unresolved.
  - The stack fold is `ow_attr::stack::combine` in the `WeaponGen` form, so the rule has one implementation (R2).
    The accuracy pool's fold keeps an unrounded start, as `weapon_firing.accuracy_pool_of` does.
  - `ParityMode::Off`: `stats::instantiate_stats` makes the item's instance (the class of its kind:
    `WillowWeapon`, `WillowShield`, `WillowGrenadeMod`, `WillowClassMod`, `WillowArtifact`, `WillowUsableItem`;
    the definition's own `InventoryClass` is not read, UNVERIFIED) and writes `ExpLevel`.
    - A weapon's bases go in with `set_base` and every effect with `apply_effects` (the type's, the parts' in slot
      order, the activated internal slots', the name parts'), with the instance as the source, so its stats are its
      own attribute stacks.
    - An item's effects reach its owner (the shield pool), which is the world's object. Offline, each init is
      evaluated through `evaluate_init` with the instance as the source, and the owner's stacks are folded on the
      record's base (0 for the shield pool, UNVERIFIED).
- **Items** follow `tools/prepare_stock_shield.py` (script `WillowItem.InitializeInternal`; UNVERIFIED in game):
  - the definition's, the parts' and the name parts' `ExternalAttributeEffects`;
  - then the external slots that are not `bRunEffectsAsSkill`, at most `AttributeSlotMaxActivated` (class default
    19), with the name parts' upgrades counted.
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
- **The gestalt fields** of `weapon_recipe.roll` (gestalt mesh, fragments, material) live in `stats::assembly`. The
  registry's lookup keeps only the last element of a static array, so `stats::StaticArrays` reads
  `AdditionalGestaltModeSkeletalMeshNames` from the owner's tag stream with the registry's owner rule.
- **A live overlay** (`--overlay`, a `weapon_dump.py` capture through `ow_defs::overlay::Overlay`) patches the
  objects the stats read, as `live_overlay.LivePackage` does. The definitions that `attribute_value` follows inside
  the registry's lookup are read unpatched; a capture holds weapon parts, types and name parts only.
- **Shapes** (the 3.7 sketch refined):
  - `instantiate_stats(data, level, sys, cx, known) -> Instance`. It takes the attribute system and the item level,
    and `known` names objects the caller holds by path.
  - The Reference card is `weapon_card(stats, recipe, level, text) -> WeaponCard` (`to_pv` is the generator's JSON
    shape).
  - The Off-mode numbers of a weapon instance are `cards::instance_numbers`.
- **Verified on the installed game** (pc-hummer, 2026-11-15; outputs under ignored `local/p2/w4-stats/real/`). The
  comparison is a local script that imports `tools/weapon_gen_parity.py` (its requests, `python_gun`, `Diff` and
  identity check) and edits no tool. Python ran with `-X utf8` (decision 2026-11-15-21) and read packages with the
  Rust `ow-package` (decision 2026-11-15-17). The Rust side is `OW_PARITY=reference ow-package --item-card
  --requests`.
  - 249 weapon balances at stages 1, 8, 15, 30, 50, 80 with seeds 1 to 20: 29,880 guns.
  - The 64 pools' 7,680 CrtRand rolls (drops from Python's `loot_pools`) gave 6,033 pool guns.
  - Result: **3,460,004 fields compared, 0 mismatches** (floats bit for bit, int and float kinds checked), 0 Python
    errors (81 s; the Rust run 56 s).
  - The 2026-10-23 record counted 2,919,834 fields and 6,795 pool guns. The per-gun field count grew with the card
    rows added on 2026-11-06 (damage stack, refire rule, accuracy pool). Why today's pool rolls give 6,033 weapon
    drops was not traced.
  - The pool drop lists themselves are W4-POOLS' comparison, not this one.
  - `prepare_stock_shield.py`'s default card (level 8, seed 1) is reproduced by the Reference `--item-card --data`:
    the same three numbers as written (capacity, recharge rate, recharge delay).
- **Verified on invented data.**
  - The card cases of `tests/weapon_gen_test.py` are re-expressed on the same invented weapon (`stats/tests.rs`,
    `cards/tests.rs`): modifier stack and rounding, damage stack, burst rule, accuracy pool, rarity, price and text,
    currency cap, slot grade and clamps, missing base grade, slag remap, the card's damage-type rule.
  - `tests/prepare_stock_shield_test.py`'s shield is rebuilt as heap objects and its card reproduced at its level
    (2) through the real evaluator. The Up and Down bodies, every effect of an active slot, the name part's upgrade
    and the activation limit hold, and the two modes agree bit for bit.
  - An Off-mode weapon instance holds its stats on its own stacks.
- **Off mode on the installed game is not reached; `NotYet`, exit 3.**
  - A weapon's bases are written (they equal the Reference bases on the gun tried). Applying its effects then stops
    at the weapon's change notification, which needs the running world (W5-WORLD).
  - The shield's level attribute stops at `InventoryAttributeContextResolver`, whose equipped-only flag is not read.
- **Not done here (open).**
  - The D4 deletions. `tools/weapon_gen_manifest.py` is still imported by `tools/weapon_gen_firing.py` (ctest
    `weapon-gen-firing-synthetic`) and by `tools/weapon_gen_parity.py`. The pool cases of `tests/weapon_gen_test.py`
    wait for W4-POOLS. `tests/weapon_gen_parity_test.py` tests a tool that stays. So `src/weapon_gen.*`,
    `weapon_gen_cli.cpp` and these files stay for now.
  - The golden cards (69 with the live overlay): `local/realgame/cards/golden_cards.json` and the live capture are
    not on this PC.
  - The capped variant (`--stage-cap 50`): Reference applies no cap (W4-ROLL).
- **Not verified.** Every rule, in the running game. The comparison proves that the Rust card keeps to the Python
  oracle's rules, not that the rules are the game's.
