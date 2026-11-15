AI-assisted (Claude Code).

# BL2_ReBased Rust port: system map by dependency layer (research report, 2026-11-14)

For the architect who designs the port's layers. Written from the tracked records, code and tests on branch `rust-switch`
(= PR #276, HEAD 5a4935c). Nothing here was run against the game. No real-game session, Ghidra or REA analysis was
done for this report. Labels are copied from the records: **verified (how)** only where a record says how it was
confirmed in the running game; everything else is **UNVERIFIED**. Names, counts, short constants and formulas only; no
game data.

How to read the layer entries: **Game** = what the system is in BL2. **Known** = records and their status. **Code** =
what implements it today (C++ `src/`, Python `tools/`, tests). **Needs / Needed by** = dependencies. **Traps** = gaps,
duplicates, UE5-host shapes, and places that will have to change shape when a lower layer arrives.

---

## 0. Summary

### 0.1 Recommended bottom-up order (reasons in section 5)

| # | Layer | Gate that says it is done |
|---|---|---|
| L0 | Container + package tables (`ow-container`, `ow-package`) | C++ fixtures equal; `verify_packages.py --reader <rust>` equal |
| L1 | Reflection and definition store: class/property declarations, CDOs, one typed property decoder, object identity, cross-package resolution, **an overlay hook** | WeaponPartDefinition dump equals the C++ dump; class bodies 3,339/3,339 |
| L2 | Script VM core and native registry (stubs that log) | `vm_test.py` fixtures, Fire event-order check equal to C++ |
| L3 | **Attribute system** (one implementation): definitions, context/value resolvers, value/base/stack triples, notification, `AttributeEffect` apply/remove, expressions, the `AttributeInitializationData` evaluator | every current evaluator's fixtures reproduced by the one Rust evaluator |
| L4 | **Balance context**: player level and XP curve, region game stage, awesome level, playthrough index, OP level, stage cap, player count, champion/badass flags, playthrough `BalanceModifierDefinition`s | PT1 numbers confirmed in game reproduced; PT2/PT3 typed, refusing loudly where unread |
| L5 | Resource pools and pawn stat initialisation (health, shield, ammo, cooldown pools; level-up rebase) | 80 x 1.13^L and the level-up refill (confirmed live) reproduced |
| L6 | **Item system** for every kind: registry, balance merge, manufacturer/grade, parts, names, rarity, value, attribute slots, weapon stats and cards, non-weapon item stats, item pools, level cap, DLC | `weapon_gen_parity` (2.9 M fields) and `item_roll --census` (35,660 rolls) reproduced; golden cards 69/69 live overlay |
| L7 | Inventory and equip effects (backpack, slots, external effects, mission weapons, sort) | equip/unequip modifier round-trip fixtures |
| L8 | Behavior kernel + skills + action skill (Phaselock) | Fire event order; Phaselock duration 5 + 0.5 g; skill refresh fixtures |
| L9 | Combat: firing math, damage pipeline, elements, crit, shields, status, death, FFYL, respawn, kill XP | `verify_combat_pawn.py --vm` 97 PASS / 0 FAIL reproduced |
| L10 | AI and population (balanced pawns, dens, awareness, targeting, melee, leaps) | live-read rows of AI_AWARENESS_LIVE / AI_MELEE_TARGETING_LIVE |
| L11 | World logic: missions, Kismet, Matinee, dialog, use, interactive objects, vendors, stations | Fire mission end to end on the Rust VM |
| L12 | Saves (PlayerSaveGame fields, load order, item identity) | round-trip of synthetic saves; real-save read only after the Gibbed decision |
| L13 | UI (Scaleform/GFx bridge, Ruffle route) | rules read from the live movies reproduced |
| H | Host track, in parallel from L1: rendering, animation, collision and movement, particles, audio, video | Phase 0R benchmark, then Phase 1 |

### 0.2 The five biggest traps

1. **The attribute evaluator and the modifier stack exist five to seven times, each a different subset** (VM `interp.cpp`
   stack recompute; `progression.cpp` `AttributeEvaluator`; `weapon_gen.cpp` `Evaluator`/`CardEvaluator`;
   `tools/weapon_recipe.attribute_value`; `tools/prepare_stock_shield.Evaluator`; `tools/weapon_stats.py`;
   `tools/loot_pools.py`). Each refuses a different set of shapes, and some fake resolvers by name (`Weapon_Is_<Maker>`
   string match, "constant resolver's value"). Port one evaluator over the real resolver chains, below everything else.
2. **The balance context is a hand-filled struct, and playthrough/OP are barely modelled.** `vm::AttributeContext` has
   fixed fields (`playThroughCount = 1`, `level`, `numberOfPlayers = 1`, `champion = false`); the game's own index is
   0-based (`GetCurrentPlaythrough`, Normal = 0) while that field is a 1-based count (UNVERIFIED reading). UVHM stage
   rules and OP levels are "not read in detail"; stage cap 50 is opt-in; playthrough `BalanceModifierDefinition`
   effects and the AI attacker's extra level factor are assumed identity; `SLICE_LEVEL = 8` in
   `tools/prepare_stock_shield.py`; mission XP throws outside PT1/level < 50. Every item, enemy and XP number depends on
   this layer.
3. **The item system lives in three disconnected implementations.** Python tools (`item_registry.py`, `item_roll.py`,
   `weapon_recipe.py`, `weapon_stats.py`, `loot_pools.py`) are the only implementation for shields, grenade mods, class
   mods and relics, and they roll parts/names/rarity but evaluate no stats; the C++ `weapon_gen` reads a Python-written
   line manifest (base game only) instead of the package store; the VM's loot path (`combat_loot.cpp`) spawns stand-in
   inventories with no parts and no stats. The game has one path: pool -> balance -> definition data -> item.
4. **Cooked data is not what the running game evaluates.** 39 stat-relevant weapon objects differ live from the cooked
   packages; with cooked data 52-53/69 golden cards match, with the live overlay 69/69. The origin is UNVERIFIED (not the
   Micropatch hotfix service). The definition store needs an overlay layer decided before anything reads definitions
   (and the same layer later carries hotfixes and BLCMM text mods, Phase 6).
5. **Randomness and float precision are not unified, and bit parity rests on MSVC behaviour.** At least four generators:
   `CrtRand` (MSVC `rand()` LCG, Python + C++), the VM's `Object.Rand`/`FRand` (another LCG with static state, in
   `natives_core.cpp`), `combat_loot`'s `rand15` stand-in, and Python's own `random` in the slice tools. The game's
   sequence and seeding are unknown. The C++/Python parity is "floats bit for bit" only because it assumes no FMA and the
   same C runtime `pow`; a Rust port must pin this or the 2.9 M-field parity breaks.

---

## 1. Ground numbers (from the records)

| Fact | Value | Source |
|---|---|---|
| Package format | UE3 832 / licensee 46, LZO-compressed code packages | OPENWILLOW_ENGINE_PLAN.md section 0 |
| Packages read | 2,008 / 2,008 (base census); 2,010 scanned with DLC, 2 remaster texture packages invalid (bad magic) | ROADMAP, ITEM_REGISTRY.md |
| Serialized exports | 4,751,329 (copies, not unique assets) | ROADMAP Phase 0 |
| Script functions | 12,978 with bytecode; 12,968 decode structurally (10 known failures) | SCRIPT_BYTECODE_DISASM.md |
| Class export bodies | 3,339 / 3,339 exact (structural) | NATIVE_CLASS_SERIAL_LAYOUT.md, verify_packages |
| Natives | plan count 7,141 (35.5 % of 20,119 functions); executable tables 6,877 in 770 tables; 199 numbered script natives bind by name; counts not reconciled | ROADMAP Phase 2 |
| Native buckets (plan) | 286 builtins, 1,914 stock UE3, 512 Scaleform bridge, 609 online/save/DLC, 3,803 Gearbox-specific in 443 classes | OPENWILLOW_ENGINE_PLAN.md |
| Behavior classes | 302, 268 pure script | same |
| Maps | 82 persistent (37 base + 45 DLC), each split `_P/_Dynamic/_Combat/_Light/_FX/_Audio/_Skybox/_Px` | same |
| Item kinds | weapon 369 balances / 89 weapon types / 908 parts; shield 120 / 49 / 159; grenade mod 67 / 19 / 242; class mod 56 / 138 / 38; relic 71 / 48 / 166; usable 98 / 80 / 31; customization 993; mission item 160 / 270 | ITEM_REGISTRY.md |
| Names, makers | 876 weapon + 757 item name parts; 22 manufacturers; 17 gestalts | ITEM_REGISTRY.md |
| Pools | 2,014 item pools, 61 pool lists | WEAPON_PARTS_CATALOG.md |
| Other balances not read by the registry | `AIPawnBalanceDefinition` 985, `InteractiveObjectBalanceDefinition` 287, `GameBalanceDefinition` 26, `VehicleBalanceDefinition` 2 | ITEM_REGISTRY.md |
| Attribute properties | stack attributes 226 float, 24 int, 2 byte (+5 notifying); 208 "simple" definitions in `Startup.upk` | NATIVE_ATTRIBUTES.md |
| Native notes | about 50 `NATIVE_*.md` files; `NATIVE_INDEX.md` about 529 table rows | docs/verification |

---

## 2. Cross-cutting foundations (decide these first)

### F1. Object model and definition store

- **Game.** Everything gameplay reads is a definition object exported by a package (`*Definition`, `*BalanceDefinition`,
  `*PartListCollectionDefinition`, `AttributeInitializationDefinition`, `GD_*` objects in `Startup.upk`, area packages).
  Definitions reference each other across packages by import paths; area packages import groups of `Startup`
  (`D_Resources`, `D_Attributes`). Runtime objects (pawns, weapons, pools, skills) are instances whose class defaults
  come from `Default__<Class>`.
- **Known.** `PackageStore` (src/package.hpp) indexes the cooked tree lazily and resolves package-local negative imports
  to the root import's package, with a global fallback index. Records note: 4 paths have two classes in different
  packages; packages repeat export paths heavily (524 inventory balances and 265 weapon balances held by more than one
  package); the tools keep **the first owner in scan order** (ITEM_REGISTRY.md section 3), which picks the wrong class for
  one DLC reference (an `ItemPoolDefinition` first, a `MissionItemDefinition` later).
- **Code.** Two object models today:
  1. `vm::Runtime` (src/vm.hpp): `Object` = class pointer + **name-keyed property bag** (lower-cased names, no memory
     layout), `Class` with declared properties, functions, states, `ClassBody` (flags, within, default object, config
     name, interfaces), CDOs built on first use, `resource()` stand-ins for asset exports,
     `instantiateExport(package, index, prefix)` with a caller-supplied prefix (4 ordinary objects, 26 placed actors, 8
     components; "no offset scanning").
  2. `Package::properties` (src/package.cpp) prints tagged properties as JSON text, with array element types from an
     **explicit schema** (`tools/*-arrays.schema`, `--array-schema`) and a caller-supplied `--property-offset 4`. The
     Python rule tools all consume this text through subprocesses (`tools/weapon_recipe.py` `Package.run`).
  Also `vm::instantiateReference` falls back to looking up unresolved `Startup` group imports by path.
- **Needs:** L0. **Needed by:** everything.
- **Traps.**
  - Two property decoders with different typing: the schema-typed JSON printer (tools) and the VM's reflection-typed
    reader (arrays typed from the declaring class; "arrays inside struct defaults are not typed here (no reflection hook)
    and stay empty", src/vm.cpp). The Rust port should have one reflection-typed decoder and retire the array schemas.
  - The property-start prefix is supplied by the caller ("observed 4-byte prefix is UNVERIFIED as a rule", ROADMAP
    Phase 0 caveat). Rule 4: reuse the C++ values, do not re-derive.
  - Object identity: first-owner-in-scan-order is a tool convention, not a game rule. How the game resolves a path held
    by several packages (load order, which map is loaded) is not recorded.
  - Cooked vs runtime values (F7) must be decided here, not in the item layer.
  - Behavior providers carry an **untagged variable value block** after the tagged properties, layout FITTED and checked
    by exact consumption (BEHAVIOR_DATA_DECODE.md); the store must expose raw trailing bytes per export.
  - A name-keyed bag is simple, but attribute triples (F3), the 5E/5F opcodes and natives that read fields by offset in
    the original all need fast, typed access; the architect should decide bag vs typed slots once.

### F2. Script VM versus native Rust rules (where each rule runs)

- **Game.** 64.5 % of functions are UnrealScript; many "rules" are script that calls natives: `WillowWeapon.InitializeInternal`
  and `CalculateWeaponBaseValues` (weapon base stats), `AttributeEffect.ApplyAttributeEffects` (Engine.upk script, the loop
  every item and weapon effect goes through), `ResourcePool.CalculateBaseValues`, the save/load script
  (`GeneratePlayerSaveGame`), accept/turn-in (`AcceptMission`, `ServerCompleteMission`), the death drop
  (`DropLootOnDeath`), the action-skill start (`ServerStartActionSkill`). Natives hold the stat core (~120 natives, zero
  script), the mission tracker, the behavior kernel and the item pick.
- **Code today, three styles:**
  1. **VM-bound natives over the package store** (`src/combat*.cpp`, `mission*.cpp`, `progression.cpp`, `behavior.cpp`,
     `kismet.cpp`, `dialog.cpp`, `natives_core.cpp`): the installed script runs, and natives are registered on
     `vm::Runtime` by key `"Class.Name(type,...)"`. Unbound natives are logged stubs returning zero. Script can be
     overridden by host bindings (`overrideScript`).
  2. **Standalone maths a host calls** (`src/weapon_gen.*` from a manifest; `src/weapon_firing.*`, "plain maths a host
     calls", random numbers passed in; `src/inventory_navigation.*`).
  3. **Python rule tools** (`tools/weapon_recipe.py`, `weapon_stats.py`, `loot_pools.py`, `item_registry.py`,
     `item_roll.py`, `prepare_vendors.py`, `prepare_stock_shield.py`, `skill_stats.py`, `mission_closure.py`): decode
     with `ow-package` and pre-bake JSON for the host.
- **Traps.** The same rule exists in several styles (section 4). The decision of 2026-11-14 keeps Python "the language of
  tooling and analysis" and puts runtime code in Rust, so every rule that a running game needs (item roll, stats, loot,
  vendors) must move from style 3 into a Rust crate; Python can stay as the independent checker (the item roll already
  has one: `item_roll.check`). Running script where the game runs script (weapon base values, `ApplyAttributeEffects`)
  reduces rule count but needs L2 + L3 natives first.

### F3. Attributes and modifiers (the stat core)

Full model in NATIVE_ATTRIBUTES.md (every rule UNVERIFIED unless noted). In short:

| Piece | Rule (own words, from the notes) | Status |
|---|---|---|
| Storage | number lives on the context object as a triple: value, `<Name>BaseValue`, `<Name>ModifierStack` (array of `AttributeModifier`: type `MT_Scale` 0 / `MT_PreAdd` 1 / `MT_PostAdd` 2, float value); value always = base combined with stack; nothing lazy | UNVERIFIED |
| Stack formula | `(base + sum PreAdd) * ((1 + sum positive Scale) / (1 - sum non-positive Scale)) + sum PostAdd`, single precision, no clamp; int/byte attributes truncate toward zero | UNVERIFIED in game; reproduced on 69 golden cards (live overlay) |
| Add / remove | add refuses a duplicate modifier object; remove deletes every copy; notify only for properties with the notify flag | UNVERIFIED |
| Plain ("simple") attributes | add edits the stored value in place permanently (Scale multiplies raw, no `1 +`); remove is a successful no-op | UNVERIFIED |
| Notification | `ResourcePool` clamps `CurrentValue` to [Min, Max] (min test first); `Skill.Grade` sets `bForceRefreshModifiersNextTick`; other notify owners (weapon `ClipSize`, `WeaponReadyMax`, interactive `MaxHealth`, `ShopTimerRate`) unresolved | UNVERIFIED |
| Definitions | `ContextResolverChain` (source -> context object; first None ends; empty chain = None) + `ValueResolverChain` (fold, last wins); only `ObjectPropertyAttributeValueResolver` supports add/remove/set-base | UNVERIFIED |
| Context resolvers | 16+ classes (Controller, Pawn, Weapon, OffHandWeapon, PlayerController, PRI, NoContextNeeded -> globals, ResourcePool, WeaponResourcePool, GameInfo, BalancedActor, Inventory, EquippedInventory, StatusEffectChance, Designer, Skill); ~8 more not read | medium-high confidence, UNVERIFIED |
| Value resolvers | ObjectProperty, Constant, SimpleMath, Conditional, Global (slot 0 `ExperiencePointTestLevel`, slot 1 `BadassTokenTestRank`), PlayerSkill, ResourcePoolState, AttributeSlotEffect, Badass, Manufacturer, WeaponType; ~15 more not read (Random, Noun, Time, Currency, AmmoDropWeight, ClassDropWeight, DamageType, ...) | UNVERIFIED |
| `AttributeEffect.ApplyAttributeEffects` (script) | per entry: new modifier, value = evaluate `BaseModifierValue` **once at apply time**, resolve context (override first), add; record applied effects for removal | UNVERIFIED |
| Expressions | `AttributeExpressionData` comparisons in single precision; And/Or modes; empty list true | UNVERIFIED |
| Item attribute slots | 19 slots per item; grade = base grade (half up) + increases, max clamp then min clamp; value fit `BaseModifierValue + PerGradeUpgrade * grade` (the 2026-10-23 reading: `(grade - 1) * PerGradeUpgrade + BaseModifierValue`); `ComputedModifierValue` computation not read | UNVERIFIED (fit reproduces the observed cards) |
| VM opcodes | 0x5E attribute value property reference; 0x5F "let attribute" writes the base companion and recomputes, without notification | UNVERIFIED in game (NATIVE_BYTECODE_OPCODES.md) |
| Confirmed live | `WillowPlayerController.RecalculateAttributeInitializedState` rewrites the health pool's base and max at level-up, current health untouched (lane L1, 2026-10-07) | **verified (bl-sdk hook, two level-up captures)** |

- **Needs:** F1, L2 (5E/5F, notification virtuals). **Needed by:** item stats, pools, skills, damage, AI scaling, XP,
  vendors, UI cards. This is the layer that, if it arrives late, forces everything above to be rewritten.
- **Traps.** See section 4 (duplicates). Also: the global value slot is **mutable global state** (the XP curve sets slot
  0 to the level it evaluates, NATIVE_ATTRIBUTES section 6); a per-world context object, not a process global, keeps
  tests deterministic. Context-resolver chain order is not decodable from the packages ("object arrays are not decoded",
  NATIVE_ATTRIBUTES Open) — check whether the Rust reflection decoder (F1) closes this.

### F4. The `AttributeInitializationData` evaluator (every balance number)

- **Rule** (NATIVE_PROGRESSION.md section 1, NATIVE_ATTRIBUTES section 1, UNVERIFIED as a whole): base = constant, or the
  attribute's value when its context chain resolves (override context first); definition: `ValueFormula`
  `Multiplier x (Level ^ Power + Offset)` (offset **inside** the multiplier; power skipped when 1) or
  `ConditionalInitialization` (first entry whose expressions all hold, else default); combine by `BaseValueMode`
  (0 sets, 1 adds, 2 scales, 3 formula minus base); add `RandomVariance` if enabled; times `BaseValueScaleConstant`;
  then (definition set) min, max, `RoundingMode` (0 float, 1 round half up, 2 floor, 3 ceiling). Single precision.
- **Confirmed live through it:** level curve (levels 1-59), max health base `80 x 1.13^L` (levels 2, 8, 17, 70), Fire mission
  XP 395 at stage 8, skill points `max(0, L - 4)` (REALGAME_GROUND_TRUTH.md, 2026-10-02). XP curve
  `60 x (n ^ 2.8 + 7.33)`; required XP `R(n) = max(0, trunc(f(n)) - trunc(f(1)))`.
- **Trap.** Before 2026-10-02 the Python tools put the offset outside the multiplier; 33 of 228 formulas have an offset, 19
  of those a non-unit multiplier. Fixed in the tools, but it shows how a subset evaluator drifts.

### F5. Balance context: level, game stage, playthrough, OP, player count

| Input | Game rule (own words) | Status / source |
|---|---|---|
| Player level | `ExpLevel` on the PRI; `PlayerExperienceLevel` is a simple attribute reading it | UNVERIFIED reading; curve verified |
| Playthrough | `WillowGameReplicationInfo.CurrentPlaythrough`, **0-based** (Normal 0, TVHM 1, UVHM 2), read by `GetCurrentPlaythrough` without clamp or `PlaythroughOverride`; set by script `SetCurrentPlaythrough` | NATIVE_CONTROLLER_HELPERS.md, UNVERIFIED |
| Playthrough count attribute | `D_Attributes.Balance.PlayThroughCount`: the C++ evaluator feeds **1** on the first playthrough | "UNVERIFIED reading" (src/progression.hpp) |
| Region game stage | per region: table = region's `DlcExpansion` table or `GlobalsDefinition.RegionBalanceData[playthrough]`; stage = `clamp(playerLevel + boost, min, max)` with the largest completed `MissionOverrides` entry winning, else defaults; **stored on the player controller per region and playthrough the first time it is asked** (and saved as `RegionGameStages`); UVHM: player-level based, at least 50, OP-aware, "not read in detail" | NATIVE_PROGRESSION.md section 2, UNVERIFIED; Sanctuary PT1 7..9 (8..11 after WelcomeToSanctuary) |
| Mission level | mission's game stage from `GameStageRegion` unless `bGameStageLocked` | UNVERIFIED; XP amount at stage 8 verified |
| Item stage cap | `min(stage, cap)`: 50 in Normal and TVHM; UVHM with level-cap upgrades 80 plus OP capped at 10 | NATIVE_LOOT.md "Level cap", UNVERIFIED, third-playthrough branch "read at overview level" |
| Awesome level | carried beside game stage into pool rolls | UNVERIFIED |
| Playthrough modifiers | `WillowGlobals.PlayThroughBasedBalanceModifiers` (one `BalanceModifierDefinition` per playthrough): XP multipliers (UVHM by `MinEffectiveLevel`; level >= 50 earlier-playthrough modifier); `ApplyPlayThroughBasedPlayerAttributeEffects` / `UpdateSpawnedPlayerEnemyAIPawn` act only above playthrough 1 (exact test unconfirmed); AI attacker extra factor and table replacement only at index >= 2 | NATIVE_PROGRESSION, NATIVE_ATTRIBUTES section 7, NATIVE_COMBAT_GROUNDWORK 3.5, all UNVERIFIED |
| Per-playthrough data | `Init_EnemyExperience_PerPlaythrough` (10 PT1, 11 PT2, else 10), `Init_EnemyHealth_ByPlaythroughGlobal` (1 on PT1 and PT2), elemental effectiveness `..._ByPlaythrough` initialisers, `AIPawnBalanceDefinition.PlayThroughs` entries (index clamped to the array; custom lists replace defaults) | COMBAT_KILL_XP, NATIVE_DAMAGE_DEATH, NATIVE_LOOT, UNVERIFIED |
| Player count | `D_Attributes.GameProperties.NumberOfPlayers` (enemy gun damage 0.8 / 0.95 / 1.15 / 1.4 for 1-4 players; enemy health player boost; XP split) | UNVERIFIED reading; evaluator feeds 1 |
| Champion / badass | `Targetable_Is_Champion`; Badass rank resolver | UNVERIFIED; feeds false |

- **Code.** `vm::AttributeContext` (src/progression.hpp) is a fixed struct: `playThroughCount`, `level` (global slot),
  `replicationInfo`, `controller`, `resourcePool` and `playerPool` callbacks, `pawn`, `mind`, `numberOfPlayers = 1`,
  `champion = false`. `mission_script.cpp` throws "the playthrough experience multiplier is not implemented" for
  level >= 50 or playthrough != 1. Python tools take a `stage` and an optional `stage_cap`; UVHM not modelled.
- **Needs:** L3 evaluator, world state (PRI, GRI, mission records). **Needed by:** item rolls (stage, cap), enemy
  health/damage/XP, mission XP, vendor stock level, chest loot, region stage storage in saves.
- **Traps.** 0-based index vs 1-based count is a trap waiting to happen; model one `Playthrough` type with both views.
  The stored-at-first-query region stage makes the stage **stateful and saved**, not a pure function. OP levels appear only
  in the cap rule. Design the context as a provider over live world objects (the game's resolver chains) rather than a
  struct of fields; the struct is a stand-in for the resolvers.

### F6. Randomness

| Generator | Where | What |
|---|---|---|
| `CrtRand` (MSVC `rand()`: `state * 214013 + 2531011`, bits 16..30) | `tools/weapon_recipe.py`, `src/weapon_gen.hpp`, `loot_pools.py`, `item_roll.py`, `prepare_stock_shield.py`, `combat_loot.cpp` (`rand15`, `frand = rand15/32767`) | the project's choice because the game's pick uses `rand() / 32767`; **seeding is ours** |
| VM `Object.Rand(int)` / `Object.FRand()` | `src/natives_core.cpp` | a different LCG (`* 1664525 + 1013904223`) with **static** state seeded 12345 / 54321; "a real seed source belongs to the engine host" |
| Python `random` | `weapon_slice_gear.py`, `slice_npc_assets.py` | the slice's pre-rolled guns keep Python's generator on purpose |
| Evaluator random variance | `AttributeEvaluator::setRandom` | owner-supplied; "the slice never sets one" |

- **Game.** Part pick: `rand() / 32767` (NATIVE_WEAPON_RULES section 3); pool chance: `FRand() <= chance`
  (NATIVE_LOOT.md). The game's own sequence is **not reproduced**; seeding unknown (WEAPON_PARTS_CATALOG section 5).
- **Trap.** The census found that seeds 1..20 all draw near 0 first with the CRT generator (ITEM_ROLL.md section 3): seeds
  must be spread. A Rust design needs one RNG service with named streams (loot, AI, firing spread, variance), injectable
  for tests, and must keep `CrtRand` bit-exact for parity with the Python oracle.

### F7. Cooked data versus what the game runs (overlay)

- NATIVE_WEAPON_RULES.md section 7: 39 stat-relevant objects of 1,439 weapon parts, types and name parts differ live
  from `Startup.upk` (24 `WeaponAttributeEffects`, 7 `ClipSize`, 5 `ReloadTime`, ...); live values equal OpenBLCMM's dump on
  78/78 values; they exist 0.01 s after SDK load, before `Micropatch`; origin **UNVERIFIED** ("another package overriding the
  cooked objects, or older hotfix content", open). The `Micropatch` service held 23 entries (population weights, skills,
  a shield projectile, rare-enemy balances) that do not touch weapons.
- Golden cards: cooked 52-53/69, live overlay 69/69 main four stats (verified against real-game cards). Tools carry it
  as an input (`tools/weapon_card_audit.py --runtime-overlay`, `tools/real_game/live_overlay.py`).
- **Maintainer decision open** (named in that section). Architecturally: a definition store with a patch layer
  (ordered set of property overrides per object path) costs little now and serves hotfixes and BLCMM mods later.

### F8. Numbers: precision and rounding

- Game arithmetic is single precision almost everywhere; some products are "computed in extended precision, stored as
  float, then truncated" (mission XP); card printing scales and adds the half in single precision (33.349998 prints 33.4);
  float bases enter stacks as floats (`ReloadTime` 2.1 -> 2.0999999 decided a card).
- C++ and Python agree bit for bit (Python uses an `f32()` helper) **assuming no FMA and the same C runtime `pow`**
  (WEAPON_PARTS_CATALOG section 3). Rust's `f32::powf`/`f64::powf` go through LLVM; the port must test bitwise parity on
  the formulas with powers (`1.13^L`, `n^2.8`) and decide double-then-round vs float ops exactly as the C++ does.

### F9. Units and coordinates

- Unreal units (uu) and rotator units (65,536 per turn); `src/weapon_firing.hpp` works in rotator units
  (`RotatorUnitsPerDegree = 65536/360`). `tools/prepare_level.py` (`transform`) gave the UE5 host location unchanged
  (1 uu = 1 UE5 cm) and rotation degrees as `[Pitch, Yaw, Roll]` (SLICE_WORLD_PLACEMENT.md). Gravity was scaled to the
  installed ini's -500 (NATIVE_PLAYER_MOVEMENT.md). UE3 is Z-up (general UE3 knowledge, not a project record); Bevy and
  wgpu conventions differ, so one conversion boundary must be fixed in the host layer and nowhere else. Rules in
  `ow-game` should stay in uu and rotator units.

### F10. Time, ticks and latent work

- Behavior kernel: a latent behavior reruns after `max(wait, 1/60)` s (src/behavior.hpp, NATIVE_BEHAVIOR_POPULATION G2,
  UNVERIFIED); Kismet impulses use a stack (most recently queued first; NATIVE_MISSION_DISPATCH A4); actor timers
  (`SetTimer`, 41 call sites) per NATIVE_ENGINE_CORE.md; skill refresh per manager tick with `SkillEffectUpdateIterval`;
  status-effect damage clock (confirmed live, WEAPON_PROJECTILES.md); pool regeneration delays. The C++ `GotoState` is a
  name-only approximation and `Enable`/`Disable` are no-ops (NATIVE_ENGINE_CORE.md).
- One simulation clock (fixed or variable step, `TimeDilation`) has to be chosen before skills, pools and AI, and the
  deterministic test harness (F11) depends on it.

### F11. Determinism and tests

- Fixtures are synthetic only, built with `package()` / `tag()` (`tests/`). Parity tools exist and are the port's
  acceptance oracles: `tools/verify_packages.py` (reader), `tools/verify_scripts.py` (disasm vs Python),
  `tools/weapon_gen_parity.py` (C++ vs Python), `tools/item_roll.py --census` (roll vs independent checker),
  `tools/verify_fire_event_order.py` (33 rules), `tools/verify_combat_pawn.py --vm` (97 PASS / 0 FAIL / 4 MISSING / 1 OPEN),
  `tools/real_game/golden_card_compare.py` (69 cards).
- Counters: `vm::Runtime` call census and call trace, `stepLimit`. Keep them in the Rust VM; they are how the golden traces
  are compared.

### F12. Item identity and saves

- A saved item is its **definition data** (balance, type/definition, manufacturer, grade, the 9 weapon or 9 item part
  slots, prefix, title, level) plus quantity, equipped, mark; a weapon adds the quick slot; mission weapons are not saved;
  ammo is not a weapon field (pool data, UNVERIFIED) (NATIVE_SAVE_LOAD.md). A Rust item type should **be** that definition
  data from day one, so rolls, inventory, cards, saves and the UI share one identity. The on-disk serial packing is
  Gibbed territory (no THIRD_PARTY entry; maintainer decision open since 2026-10-22).

---

## 3. The layers

### L0. Install, container and package tables

- **Game.** `WillowGame/CookedPCConsole/*.upk` plus `DLC/<folder>` (Anemone, Aster, Orchid, Sage, Iris, Gladiolus,
  Lobelia, Lilac, Allium, Flax, Nasturtium, CommDay2013, Peony, LootChest2, PN2 and others); `.tfc` texture caches; LZO
  chunk container; name, import, export tables.
- **Known.** Phase 0 records; `PACKAGE_GLOBAL_INDEX.md`; verify_packages baseline: 9 code packages, counts and class bodies
  exact (GATES.md). Verified structurally (byte-for-byte against the Python reader), not a runtime claim.
- **Code.** `src/container.*` (vendored lzokay, MIT), `src/package.*` (Reader with `require()`/`limit`), `src/census.*`,
  `src/assets.*`; `research/native_count.py` (pure-Python LZO, the source the Rust decoder is written from);
  `crates/ow-container`, `crates/ow-package` skeletons; `ow-host::find_install` checks the install (4 Rust tests).
  Tests: `tests/package_test.py`, `properties_test.py`, `assets_test.py`, `global_resolve_test.py`.
- **Needs:** nothing. **Needed by:** everything.
- **Traps.** Sensitive files (rule 5: no loosened bounds). The census counts copies, not unique assets; first-owner rule
  (F1). Two invalid remaster packages must be skipped, not fixed.

### L1. Reflection and definition store

- **Game.** `Class`, `Struct`, `Enum`, `*Property` exports (including Gearbox `FloatAttributeProperty`,
  `IntAttributeProperty`, `ByteAttributeProperty` with value/base/stack companions), CDOs, tagged properties, interfaces.
- **Known.** NATIVE_CLASS_SERIAL_LAYOUT.md (class body field order read from native serialization, 3,339/3,339 structural,
  meaning UNVERIFIED); NATIVE_BYTECODE_OPCODES.md (property flags: stack attribute bit 31, plain bit 30, notify bit 29;
  `PropertyDecl.typeRef/2/3` carry stack and paired property); BEHAVIOR_DATA_DECODE.md (untagged variable block, fitted).
- **Code.** `vm::Runtime` loading (`findClass`, `structAt`, `declAt`, `defaultsOf`, `applyTaggedDefaults`,
  `checkClassBodies`, `implements`); `Package::properties` (schema JSON). Tests `vm_test.py`, `runtime_test.py`,
  `natives_test.py`.
- **Needs:** L0. **Needed by:** L2 and every rule (all definitions are read through it).
- **Traps.** F1. Tools that pre-bake JSON from `--properties` (recipes, registry, manifests) become unnecessary once
  Rust rules read the store directly; keep them as oracles.

### L2. UnrealScript VM and native dispatch

- **Game.** 12,978 bytecode functions; states, latent functions, timers, delegates, iterators (`AllActors`,
  `DynamicActors`), `out` params, struct copy semantics, the Gearbox hidden-local array and opcodes 0x4C-0x50, 0x5E/0x5F.
- **Known.** SCRIPT_BYTECODE_DISASM.md (12,968/12,978 structural), SCRIPT_VM_PROTOTYPE.md (synthetic tests, `--vm-sweep`;
  default-state return agreement does not pass the Phase 2 gate), NATIVE_ENGINE_CORE.md (timers, spawn, iterators, state
  machine; UNVERIFIED), NATIVE_BYTECODE_OPCODES.md. Fire event order: 45 events matched, 2 out of order, 4 extra against
  one live trace (verified as a comparison with lane L2's bl-sdk trace; ROADMAP Phase 4).
- **Code.** `src/script.*` (decoder), `src/vm.*`, `src/interp.cpp` (interpreter, incl. `letAttribute`), `src/natives.*`,
  `src/natives_core.cpp` (Core builtins; `GotoState` name-only; `Rand`/`FRand` stand-ins), `src/vm_sweep.cpp`;
  `research/script_disasm.py` (Python oracle). Tests `vm_test.py`, `script_disasm_test.py`, `natives_test.py`,
  `ui_trace_returns_test.py`.
- **Needs:** L1. **Needed by:** L3 (5E/5F, script `ApplyAttributeEffects`), L5 (pool script), L8-L12 (all script rules).
- **Traps.** Latent state code out of scope so far; `GotoState`/`Enable` approximations; the 6,877 vs 7,141 native count
  not reconciled and no stub table for all natives; step limit 5,000,000 per top-level call; performance ("a sloppy VM
  will not" run thousands of actors, plan section 4). Parameter evaluation context is UNVERIFIED (interp.cpp note).

### L3. Attribute system

See F3 and F4. **Code:** `src/interp.cpp` (stack recompute for 5F), `src/progression.*` (`AttributeEvaluator`, subset;
throws "unsupported ..." outside it), `src/combat.cpp` / `combat_hit.cpp` (pool and pawn attribute reads; "a property
that reads 0 with a base value of 0 is taken as uninitialised, i.e. the identity 1", stand-in), `src/weapon_gen.cpp`
(`Evaluator`, `CardEvaluator`), `tools/weapon_recipe.py` (`attribute_value`, `formula_value`; conditional ->
None), `tools/weapon_stats.py`, `tools/loot_pools.py`, `tools/prepare_stock_shield.py` (`Evaluator`),
`tools/skill_stats.py`. **Tests:** `weapon_balance_test.py`, `weapon_gen_test.py`, `skill_stats_test.py`,
`combat_test.py`, `card_rounding_cases.json`.
- **Needs:** L1, L2 (notification virtuals, typed let). **Needed by:** L4-L13.
- **Traps.** Section 4 lists the duplicates. The Rust layer should implement the resolver classes as the notes list them
  and refuse (error, not guess) for the ~23 unread resolver classes, so a later read adds a resolver without changing
  callers.

### L4. Balance context and progression

- **Game.** F5, plus the XP pipeline: mission XP (`span x pct x m`, truncated), kill XP
  (`max(10, ExperiencePointMultiplier x M x (ExpLevel + 4))`, M = 10 on PT1; level-difference scale; share; combat
  multiplier), `ExpEarn` / `ApplyExpPointsToExpLevel`, level-up (`OnExpLevelChange` -> `RecalculateAttributeInitializedState`
  -> pool rebase; health refill; a natural level-up adds a 30 s weapon-damage scale), skill points `max(0, L - 4)`.
- **Known.** NATIVE_PROGRESSION.md, NATIVE_LEVEL_UP_ATTRIBUTES.md, NATIVE_COMBAT_GROUNDWORK.md section 5,
  COMBAT_KILL_XP.md, REALGAME_GROUND_TRUTH.md. **Verified:** level curve 1-59, health base, Fire XP 395 at stage 8, skill
  points (2026-10-02); level-up refill and 30 s damage scale (lane L1, 2026-10-07); kill XP fitted on lower-level victims
  with d = 1-4 only (COMBAT_GOLDEN_TRACE.md "Not observed": equal level, higher victim, d >= 5, co-op, PT2+).
- **Code.** `src/progression.*` (`ExperienceCurve`), `src/combat_xp.cpp`, `src/mission_script.cpp`
  (`GetExperienceReward`), `tools/slice_values.py`. Tests `mission_script_test.py`, `combat_test.py`.
- **Needs:** L3, world objects (PRI, GRI, controller mission records). **Needed by:** L6 (stage), L9 (enemy level), L10
  (spawn stage), L11 (mission level), L12 (saved stages).
- **Traps.** F5. Region stage storage and `ResetGameStageForRegion` timing unread. Badass rank / tokens (account-wide
  progression) not covered by any record beyond the resolver row; "Save values leave the records" decisions
  (2026-11-12) mean Badass rank from the maintainer's save is deliberately not recorded.

### L5. Resource pools and pawn stat initialisation

- **Game.** `ResourcePoolManager` (16 slots), `ResourcePoolDefinition`s (health, shield, ammo per type, skill cooldown,
  vehicle boost...), `ResourcePool` (`MinValue`, `MaxValue`, `CurrentValue`, regeneration rates and delays, states
  Depleted/Filled/Regenerating, upgrades), `CalculateBaseValues` (script), pawn health glue (`SetHealthResourcePoolDefinition`,
  `GetHealth`, ...), shield pool via `SetShieldResourcePoolDefinition`; balanced AI pawns get starting values from their
  balance and `AIClass`.
- **Known.** NATIVE_COMBAT_GROUNDWORK.md sections 1-2 (slot order answered), NATIVE_LEVEL_UP_ATTRIBUTES.md sections 2-5,
  NATIVE_DAMAGE_DEATH.md section 4, COMBAT_DOWN_RESPAWN.md (ammo back on respawn: five pools by the injured definition's
  share, decision 2026-11-13). Shield numbers in the slice were **stand-ins** (UNVERIFIED; decision 2026-11-12 "default
  shield recharge becomes an invented stand-in").
- **Code.** `src/combat.cpp` (pool manager, pool natives for any pawn), `src/mission_script.cpp` (player pool, level-up),
  `src/combat_down.cpp`. Tests `combat_test.py`, `mission_script_test.py`, `check_shield_check_test.py`.
- **Needs:** L3, L4. **Needed by:** L7 (shield item effects), L8 (cooldown pool), L9 (damage to shield then health),
  L12 (pools in saves: `bSerializeInSaveGame`).
- **Traps.** The equipped shield item and its effects are what set the shield pool in the game; today the slice's shield
  is a stand-in fed by a pre-rolled card (`prepare_stock_shield.py`, level 8). Build pools before items, but expect the
  shield pool's values to come from L6/L7, not from a host constant.

### L6. Item system (registry, balances, parts, names, rarity, value, stats, pools)

**Game objects and how they connect.** `ItemPoolDefinition` (entries: balance or nested pool, probability as
`AttributeInitializationData`, quantity; `bAutoReadyItems`; min/max stage requirements) and `ItemPoolListDefinition`
-> `InventoryBalanceDefinition` (`WeaponBalanceDefinition`, `MissionWeaponBalanceDefinition`, `ClassModBalanceDefinition`,
`ItemBalanceDefinition`; `BaseDefinition` chain; `Manufacturers` with grades: stage window, `ExpLevel`, spawn weight
init, optional own definition; `bInterpolateExpLevel`; part list collection) -> `InventoryDefinition`
(`WeaponTypeDefinition`, `ShieldDefinition`, `GrenadeModDefinition`, `ClassModDefinition`, `CrossDLCClassModDefinition`,
`ArtifactDefinition`, `UsableItemDefinition`, `MissionItemDefinition`, ...) with `<Slot>Parts` lists, `BaseRarity`,
`AttributeSlotEffects`, `bAllowInventoryDefToModifyPartWeight` -> parts (`WeaponPartDefinition` and item part classes;
`Rarity`, `WeaponAttributeEffects`, `ExternalAttributeEffects`, `ZoomWeaponAttributeEffects`, `AttributeSlotUpgrades`,
monetary value modifiers, per-part name lists) -> `ManufacturerDefinition` -> name parts (`WeaponNamePartDefinition`,
`ItemNamePartDefinition`, with expressions) -> gestalt mesh and material. Weapon slots: Body, Grip, Barrel, Sight, Stock,
Elemental, Accessory1, Accessory2, Material. Item slots: Alpha to Theta, Material.

| Rule | Own-words rule | Status |
|---|---|---|
| Pool roll | each listed pool fires when `rand()/32767` < its probability; each Quantity draw walks eligible entries' shares; pool stage requirements gate it; variance clamp only after a caller's variance (interactive objects) | UNVERIFIED (NATIVE_LOOT, WEAPON_PARTS_CATALOG section 2) |
| Balance candidates | one candidate per manufacturer: first grade whose stage window holds the stage, weight interpolated across the window | UNVERIFIED |
| Stage cap | F5 | UNVERIFIED |
| Part pick | entry weight: flat 100 with no manufacturer list, else the manufacturer's or default weight; 0 outside the entry's stage window; floor 1e-8; weighted pick with `rand()/32767` | weights and pick shape **verified** for two common balances at stage 30 (400 game-rolled guns each, chi-square p 0.17-0.67) |
| Merge | definition lists then balance collections root-first; Selective/Complete replacement modes | weapons: runtime lists consistent (37 identical, 322 larger package chains); items: reused from weapons, UNVERIFIED |
| Relic hook | relic only: a part with slot upgrades none of which names one of the relic's `AttributeSlotEffects` slots weighs 0 | UNVERIFIED |
| Level | level = stage for interpolating balances, else grade `ExpLevel` | **verified** 1,600/1,600 game-rolled guns |
| Names | all name expressions must hold (`WeaponDamage_Is_<Element>` compares the damage type); best name part in slot order | **verified** 1,600/1,600 names; element title rule on one gun |
| Rarity | trunc `BaseRarity` + trunc part rarities (+ name parts for items, the reading the data supports); band from `GlobalsDefinition.RarityLevelColors`; mission items 500 | weapons UNVERIFIED in detail; items UNVERIFIED |
| Value | part total from the type's `BaseMonetaryValueModifier`, then parts, prefix, title (float steps), trunc, capped by currency (Credits 99,999,999) | sale value on 40/40 spawned guns **verified** |
| Weapon stats | bases from the weapon type (`CalculateWeaponBaseValues`, script), then type effects, part effects in slot order, internal attribute slots, name-part effects; stack formula | 366/484 live fields equal on cooked data, 392 with overlay; **92 mismatches untriaged**; golden cards 69/69 with overlay |
| Card display | `AttributePresentationDefinition` rounding; status chance through the damage type's mapped presentation (slag input clamped [0, 3.33]) | 69/69/68 of 69 golden cards (live overlay) |
| Class mod pools | filled at run time (decision 2026-11-13); which `ClassModDefinitions` entry is picked: not read (uniform stand-in) | UNVERIFIED |
| Non-weapon stats | shield capacity/recharge, grenade damage, class mod and relic bonuses: no general evaluator; the stock shield card is computed by `prepare_stock_shield.py` at level 8 | not built |

- **Known.** WEAPON_BALANCE_DECODE.md, NATIVE_WEAPON_RULES.md, WEAPON_PARTS_CATALOG.md, NATIVE_LOOT.md, NATIVE_ITEM_RULES.md,
  ITEM_REGISTRY.md, ITEM_ROLL.md, INVENTORY_CARD_STATS.md, PICKUP_CARD.md, RTSE_GUN_DATA_CROSSCHECK.md,
  WEAPON_PART_ASSETS.md (300/301 skins), REALGAME_GROUND_TRUTH.md ("Weapon cards", "Live weapon data").
- **Code.** Python: `item_registry.py` (schema `openwillow-item-registry/1`, every kind incl. DLC, byte-identical rebuilds),
  `item_roll.py` (+ independent `check`), `weapon_recipe.py` (roll, merge, `CrtRand`, attribute values), `weapon_stats.py`
  (evaluate, cards), `weapon_catalog.py`, `weapon_part_census.py`, `loot_pools.py`, `weapon_gen_manifest.py`,
  `weapon_card_audit.py`, `grenade_mod.py`, `prepare_stock_shield.py`, `prepare_vendors.py` (`item_parts`),
  `weapon_slice_gear.py`. C++: `src/weapon_gen.*`, `weapon_gen_cli.cpp` (`ow-weapongen`, base game only, from the line
  manifest: 249 balances, 652 parts, 748 name parts, 33 weapon types, 826 pools, 12 loot sources), `src/combat_loot.cpp`
  (VM pool roll with stand-in inventories). Tests: `item_registry_test.py`, `item_roll_test.py` (15 cases),
  `weapon_gen_test.py`, `weapon_gen_parity_test.py`, `weapon_balance_test.py`, `loot_pools_test.py`,
  `weapon_catalog_test.py`, `golden_card_compare_test.py`, `grenade_mod_test.py`, `prepare_stock_shield_test.py`.
- **Needs:** L1 (definitions), L3 (evaluator, stacks, slot effects), L4 (stage, cap, playthrough, player context for
  `AmmoDropWeight` and class drop weights), F6 (RNG), F7 (overlay). **Needed by:** L7, L9 (weapon stats drive firing and
  damage), L10 (enemy gear, `DefaultWeapon`), L11 (mission rewards, vendors, chests), L12 (saved definition data), L13
  (cards).
- **Traps.**
  - Three implementations (trap 3). The VM loot path's items have no parts or stats ("InitializeInventory is not run",
    combat_loot.cpp), so the VM drop and the generator never meet.
  - `weapon_recipe.merge` takes the root-most manufacturer/definition, the item tools the leaf-most; they differ on 12
    class mod and 8 usable balances (ITEM_REGISTRY section 4).
  - 1,036 stage windows need runtime state (1,035 grenade mod entries with a per-element minimum game stage read through a
    constant resolver): treated as no bound today; which reading the game uses is not settled.
  - The registry keeps only the leaf-most `Manufacturers` list, so the game's walk to a base balance with no covering
    grade is not modelled.
  - Weapon generator is base game only; DLC balances are in the registry but not rolled by C++.
  - The manufacturer is drawn in the item roll; the game draws it in the pool roll (one candidate per manufacturer).
  - Name-part `WeaponAttributeEffects` path is real but empty in this data.
  - The manifest is a UE5-era integration format (section 6).

### L7. Inventory, equip and pickup

- **Game.** `WillowInventoryManager` (backpack `InventorySlotMax_Misc`, `WeaponReadyMax`, quick slots), equip/unequip
  (`WillowItem.ApplyAllExternalAttributeEffects`: controller as source, item as override), mission weapons (`CanBeSaved`
  false), pickups (`WillowPickup`, `TouchedPickupable`), backpack sort and filter (native), marks (favorite/trash).
- **Known.** NATIVE_INVENTORY_EQUIP.md, NATIVE_INVENTORY_SORT.md (sort is native; only in-game evidence was the UI traces),
  NATIVE_USE_INTERACTION.md, INVENTORY_MOVIE_PROTOTYPE.md (equip and navigation via the VM `MoveDelta` on item-only rows),
  PICKUP_CARD.md. All UNVERIFIED except what those records list.
- **Code.** `src/inventory_navigation.*`, `tests/inventory_navigation_test.js` (JS twin), `tools/prepare_inventory_gear.py`,
  `tools/seed_inventory_demo.py`.
- **Needs:** L3, L5, L6. **Needed by:** L8 (equipped weapon is a context resolver target), L12, L13.
- **Traps.** Menu navigation exists in C++ and JS (overlay) at once. Equip effects must use the L3 apply/remove records,
  not a host stat copy.

### L8. Behavior kernel, skills, action skill

- **Game.** `BehaviorProviderDefinition` (sequences, events, behaviors, packed output links
  `index << 16 | length`, variable links, untagged variable block), `BehaviorKernel` threads (depth-first; signed id byte
  selects links; `bEnabled`, `MaxTriggerCount`, `ReTriggerDelay`; latent reruns), 268 pure-script `Behavior_*` classes.
  Skills: `SkillDefinition` (44 natives), `PlayerSkillTree` (`UpgradeSkill`, `SetSkillGrade`), `SkillEffectManager`,
  `Skill` (grade -> effect value, `AdjustModifiers` modes 0/1/2, per-frame refresh, `SkillGradeModifiers.*` attributes for
  class-mod bonuses), action skill (`StartActionSkill` -> `ServerStartActionSkill` refusals -> activation, duration,
  cooldown pool). Maya: Phaselock duration `Att_Phaselock_Duration` = 5 + 0.5 g with Suspension (designer attribute);
  trees Motion, Harmony, Cataclysm.
- **Known.** NATIVE_MISSION_DISPATCH.md (A1/A2 dispatch), NATIVE_BEHAVIOR_POPULATION.md, NATIVE_BEHAVIOR_CONTEXT.md,
  BEHAVIOR_DATA_DECODE.md, NATIVE_SKILLS.md, NATIVE_SKILL_BEHAVIOURS.md (formulas `PlayerSkillDamage = 8 x S^L`,
  `S = Att_UniversalBalanceScaler` 1.13; read from data), NATIVE_ACTION_SKILL.md, NATIVE_PHASELOCK_TARGETING.md,
  NATIVE_PHASELOCK_PRESENTATION.md, PHASELOCK_STOCK_DATA.md, SKILL_TREE_NAVIGATION.md. **Verified:** Phaselock bob
  (0.001 units RMS over 435 frames), cast gates (casts during a reload and aborts it; refused during a swap), bubble size
  rule (emitters read live). Skills live capture: three runs 2026-11-06 (NATIVE_SKILL_BEHAVIOURS "Skills live capture").
  Targeting rules themselves not compared with the game.
- **Code.** `src/behavior.*` (`BehaviorProvider`, handlers per behavior class, unhandled = error), `src/mission_script.cpp`
  (level-up behaviors stand-in), `tools/skill_stats.py`, `tools/prepare_skill_tree.py`, `tools/prepare_skill_effects.py`,
  `tools/prepare_action_skill.py`, `tools/prepare_phaselock_*.py`. Tests `behavior_test.py`, `skill_effects_test.py`,
  `skill_stats_test.py`, `skill_rules_test.js`, `skill_navigation_test.js`.
- **Needs:** L2, L3 (all skill effects are modifiers), L4 (level for `S^L`), L5 (cooldown pool), L7 (held weapon context
  for Accelerate-type effects). **Needed by:** L9 (skill damage, kill skills), L10 (AI uses behaviors), L11 (missions use
  behavior sequences).
- **Traps.** The behavior variable block layout is fitted. Skill effects on simple attributes are one-shot edits that are
  never withdrawn (NATIVE_ATTRIBUTES correction): a skill layer built on a host stat copy would get respec wrong. The
  Phaselock presentation scored 5.1-5.7/10 by a critic (not parity); the maintainer's feedback names bubble/effects,
  target anims and an arm-cast effect.

### L9. Combat: firing, damage, elements, shields, health, FFYL

- **Game.** Firing: `FiringModeDefinition` (patterns, bursts, spin-up), spread (yaw uniform across a disc, pitch on the
  chord), accuracy pool, recoil/kick, fire interval, ammo (clip, `ShotCost`, ammo pools), reload, projectiles (light
  bullets, rockets, air friction, splash with/without falloff), Tediore throw, status effects (chance roll, damage clock).
  Damage: `WillowDamagePipeline.AdjustDamage` stage chain (AI damage scale; damage-source modifiers incl. amplify;
  instigator per-element modifiers by `EDamageType` 1 Incendiary, 2 Shock, 3 Explosive, 4 Corrosive, 5 Impact, 7 Amp;
  level-difference tables; backstab/unsuspecting; hit region and crit multiplier `CurrentInstantHitCriticalHitBonus`;
  recipient damage-type reduction; intrinsic armor; shield then health), radius damage split over hit regions, elemental
  effectiveness by target type and playthrough, `DamageTypeDefinition`, `WillowDamageSource`, `OnTakeDamage` behavior event.
  Death (AI): `NotifyPawnDied`, drop loot, kill XP. Player: injured ("Fight For Your Life": down, bleed-out, second wind by
  kill), death, respawn (station choice, fee `GlobalsDefinition.TeleportCost`, ammo back by injured definition share).
  Allegiance `IsEnemy`/`IsFriendly`/`GetOpinion`.
- **Known.** NATIVE_WEAPON_FIRING.md, NATIVE_WEAPON_TRIGGER_AMMO.md, WEAPON_PROJECTILES.md (**verified live:** light bullet
  flight, air friction, rocket speed, per-pellet damage on 4 of 5 guns, splash with and without falloff, status damage
  clock, Tediore throw; **not measured:** spread distribution, proc rate; beams, ricochets not done),
  PROJECTILE_FIRING_PATTERNS_LIVE.md, NATIVE_DAMAGE_DEATH.md, NATIVE_COMBAT_GROUNDWORK.md, NATIVE_DOWN_TIMERS.md,
  COMBAT_HIT_CHAIN.md, COMBAT_DEATH.md, COMBAT_DOWN_RESPAWN.md (**verified:** respawn wait steps, six live respawns, two
  maps, one level; first-down bleed-out 12 s used in the host), COMBAT_MELEE.md, COMBAT_GOLDEN_TRACE.md (bullymong fight,
  Defense Boost factor), COMBAT_KILL_XP.md, COMBAT_LOOT.md (live 3-item money drop OPEN). Crit base value for Maya open
  (stored base 0; "must come from a modifier applied at start-up").
- **Code.** `src/weapon_firing.*` (standalone maths; `tests/weapon_firing_math_test.cpp`), `tools/weapon_firing.py`,
  `tools/weapon_gen_firing.py`, `tools/check_weapon_firing.py`; `src/combat.*`, `combat_hit.cpp`, `combat_death.cpp`,
  `combat_down.cpp`, `combat_xp.cpp`, `combat_loot.cpp`, `combat_run.cpp` (`ow-package --combat-run`, scripted steps
  S-a..S-g, live delays as stand-ins); `tools/verify_combat_pawn.py`. Tests `combat_test.py`, `weapon_firing_test.py`,
  `weapon_gen_firing_test.py`, `check_hud_down_test.py`.
- **Needs:** L3, L4 (levels, playthrough, player count), L5, L6 (weapon stats, damage type, firing mode from parts), L7
  (held weapon), L8 (skills, `OnTakeDamage`), host traces and collision (H). **Needed by:** L10, L11 (kill objectives),
  L13 (HUD, damage numbers).
- **Traps.** Firing maths exist in C++ (`weapon_firing`) and Python (`weapon_firing.py`, `weapon_gen_firing.py`). The VM
  melee blow is a guaranteed-hit fixture without AI. The AI attacker level factor is identity below PT index 2 (not
  coded for UVHM). Shield values were stand-ins. Combat ran only on the VM's synthetic run, never against host physics.

### L10. AI, enemies and population

- **Game.** `AIPawnBalanceDefinition` (985; playthrough entries with pool lists, `DefaultWeapon`), `WillowAIPawn`,
  `WillowMind`, `AIClassDefinition` (starting values; `OnTakeDamage`), GearboxFramework `AIComponent`, `GearboxMind`,
  `GearboxCoverStateManager`; population dens and `PopulationFactoryBalancedAIPawn` (`ApplyAttributeStartingValues`),
  `MissionPopulationAspect`; awareness (sight; proximity 768 for den bullymongs), threat over time, `ValidTarget`,
  target priority formula, melee, leaps and thrown rocks. Enemy health
  `Init_EnemyHealth_ByPlaythroughGlobal x Init_BaseEnemyHealth` (player-count boost x level formula), damage
  `Init_GlobalAIDamageBalanceMultiplier_Part1` = Part2 x (1 + 0.02 x level) (0.65 for one player PT1 non-champion), XP
  `Init_BaseEnemyExperience`.
- **Known.** NATIVE_AI_AWARENESS.md, NATIVE_AI_MELEE.md, NATIVE_AI_LEAP_THROW.md (UNVERIFIED, not built),
  AI_AWARENESS_LIVE.md and AI_MELEE_TARGETING_LIVE.md (**verified** for den-spawned bullymongs on one map: notice rules,
  `ValidTarget` six answers, threat gate, `TargetChanged`, priority formula to 5e-8 on 134 samples), COMBAT_RANGE.md
  (swing cycles of adult, baby, regular, slinger read live; psycho no live read), NATIVE_AMBIENT_NPC.md,
  SANCTUARY_AMBIENT_NPCS.md, creature zone tools.
- **Code.** No AI runs in C++; the UE5 host had a host brain (stand-in, retired). `tools/creature_zone.py`,
  `tools/prepare_range.py`, `tools/census_ambient_npcs.py`, `tools/slice_npc_assets.py`.
- **Needs:** L2 (script), L3/L4 (scaling by level, playthrough, players), L5 (pools), L6 (gear), L8 (behaviors), L9,
  H (navigation, traces, animation). **Needed by:** L11 (populations, kill objectives), the slice gate.
- **Traps.** Navmesh/pathing has no route yet (host). Enemy scaling uses the same evaluator and context as items; the
  `pawn`/`mind` fields of `AttributeContext` are ad hoc. UVHM enemy health regen and other playthrough effects not read.

### L11. World logic: missions, Kismet, Matinee, dialog, use, interactive objects, vendors

- **Game.** `MissionDefinition` (objective sets, objectives, rewards incl. `ExperienceRewardPercentage`, item reward pools,
  `GameStageRegion`), `MissionTracker` (81 natives; status Complete = 4; observers `IMission`, `BehaviorSequenceEnableByMission`),
  controller mission records per playthrough (`MissionPlaythroughs`), Kismet `SequenceOp` graphs (remote events, Matinee
  `InterpData`, populated events), dialog (`TriggerEvent`, talk acts, VO tags), use key (`IUsable`), `WillowInteractiveObject`
  (134 natives; doors, chests via `InteractiveObjectBalanceDefinition` 287, switches, vending machines), vendors (shop timer
  `ShopTimerRate`, item of the day, sell/buy value), fast-travel and respawn stations.
- **Known.** NATIVE_MISSION_DISPATCH.md, NATIVE_MISSION_SCRIPT_BRIDGE.md, NATIVE_OBJECTIVE_TRIGGERS.md,
  NATIVE_CONTROLLER_HELPERS.md, NATIVE_KISMET_MATINEE.md, NATIVE_DIALOG.md, NATIVE_DIALOG_GROUPS.md, NATIVE_USE_INTERACTION.md,
  NATIVE_MARCUS_USE_CHAIN.md, FIRE_MISSION_GOLDEN_TRACE.md, FIRE_DIALOG_CONTEXT.md, SANCTUARY_RPG_MISSION.md (host stand-ins:
  kickoff after acceptance, dialog outputs together, save-state fixture, lent mission weapon), SANCTUARY_MOVER_PROTOTYPE.md,
  VENDORS.md (confirmed-live / package / UNVERIFIED per rule), MISSION_SCREEN.md. Census over 133 missions: 3,420 behavior
  links in predicted id ranges (structural).
- **Code.** `src/mission.*`, `src/mission_script.*`, `src/kismet.*`, `src/dialog.*`, `src/mover.*`, `src/slice.*`
  (`FireMissionSlice`), `tools/mission_closure.py` (1,678 lines), `tools/prepare_vendors.py`, `check_vendors.py`,
  `prepare_interactives.py`, `check_interactives.py`, `prepare_mover.py`. Tests `kismet_test.py`, `mission_script_test.py`,
  `prepare_vendors_test.py`, `vending_test.js`, `prepare_interactives_test.py`, `lootable_tracks_test.py`.
- **Needs:** L2, L3/L4 (mission level, XP), L6 (rewards, vendor stock, chest loot), L8 (behavior sequences), L9/L10 (kill
  objectives, populations), H (Matinee playback, movers). **Needed by:** L12 (mission records), L13 (mission screen, log).
- **Traps.** Tracker rules not modelled: RequiredObjectivesComplete, Failed, repeatable, branching sets, the level-load
  replay, blocking sets (src/mission.hpp). The slice wires one mission (`FireMissionSlice`); the general tracker is Phase 5.
  Vendors are pre-baked by Python (`prepare_vendors.py`) for the UE5 host.

### L12. Saves

- **Game.** `PlayerSaveGame` (section F12 list), save trigger points, load order (`InitializeWorldMissionState`,
  `GrantMissionWeaponsToClientPlayer`, pre-load fixups, `ConditionalFixWeaponReadyMax`), `WillowSaveGameManager` storage
  boundary, DLC expansion data, unloadable items kept by name, region game stages per playthrough, visited teleporters,
  `LastVisitedTeleporter` as respawn start.
- **Known.** NATIVE_SAVE_LOAD.md (script read; no real save opened for that note), REALGAME_GROUND_TRUTH.md (save safety:
  backups, saving blocked). Decisions 2026-11-12 "Save values leave the checkers, fixtures and fee records / the records /
  the host and tools comments": values from the maintainer's save are not to appear in tracked files.
- **Code.** Real-game tools back up and block saves (`tools/real_game/`, `tests/block_saves_test.py`); no save reader or
  writer in `src/`. Gibbed is a behaviour reference only, no THIRD_PARTY entry.
- **Needs:** F12 (item identity), L4-L11 state. **Needed by:** Phase 5 gate ("with a real save file").
- **Traps.** Do not design save state before item identity and mission records are fixed. Health and shield are not saved
  (set to max on load); position is not saved.

### L13. UI (Scaleform)

- **Game.** Flash AS2 movies (GFx), 512 bridge natives (`GFxMoviePlayer`, `GFxObject`, `GearboxGFxMovie`,
  `WillowGFxMovie3D`, `QuestAcceptGFxMovie`), `WillowHUDGFxMovie` (98), `WillowUIInteraction`; menus are script (navigation
  and equip logic), backpack sort native.
- **Known.** NATIVE_GFX_BRIDGE.md, HUD_PARITY.md, HUD_MINIMAP.md, ACTION_SKILL_HUD.md (rules read from the live movie),
  MENUS_PARITY.md, INVENTORY_MOVIE_PROTOTYPE.md, INVENTORY_CARD_STATS.md, SKILL_TREE_NAVIGATION.md, MAP_SCREEN.md, CONTROLS.md.
- **Code.** `tools/hud_overlay/*.js` (Ruffle in a browser page, `serve.py`), `tools/extract_swfmovie.py`, `gfx_to_swf.py`,
  `gfx_compacted_font.py`, `prepare_inventory_movie.py`, `prepare_hud_*.py`, `prepare_mission_*.py`, many `tests/*_test.js`.
- **Needs:** L6/L7 (cards, inventory), L8 (skills), L11 (missions), L2 (UI script). **Needed by:** the slice (equip,
  skill point, accept/turn-in through pages).
- **Traps.** The overlay over a host window was a UE5-host arrangement; embedding Ruffle (Rust) natively is a benchmark
  question. Card rounding must come from L6, not a JS copy (`item_card.js` follows the evaluator today).

### H. Host-side systems (Rust host, engine route open)

| System | Game | Known / evidence | Status for Rust | Depends on |
|---|---|---|---|---|
| Rendering: static meshes, BSP, terrain, materials, lighting, sky, decals | UE3 material graphs (cel-shade edge, ink lines), lightmaps in `Lighting.tfc`, BSP polygons, terrain weightmaps | SANCTUARY_* records, COOKED_MATERIAL_VERIFICATION, MATERIAL_*, SKY_*, BSP_*, UV_WINDING_VERIFICATION, UMODEL_CROSSCHECK; host lighting were screenshot fits (UNVERIFIED) | not started; benchmark scene uses existing local exports | L0/L1, UModel exports |
| Animation | ~30 `AnimNode*` types, `SkeletalMeshComponent` (127 natives), physics assets | WEAPON_VISUALS, PHASELOCK presentation, prepare_character_anims/pose | not started | L1, UModel |
| Collision and movement | `Pawn`/`Controller`/`PlayerController` natives; walking, falling, jumping; gravity from ini (-500) | NATIVE_PLAYER_MOVEMENT.md, COLLISION_WALKING_VERIFICATION.md, collision tools/schemas | not started | L1 (collision data) |
| Traces and physics | `Trace`, `PrimitiveComponent` (47), ragdoll | Phase 3 list | not started | H collision |
| Particles | Cascade modules | read-only template reader `research/particle_system.py` (17,506 tables fit) | not started | L1 |
| Audio | Wwise `.pck`, decoded with vgmstream into `local/` (maintainer decision 2026-11-07) | SLICE_AUDIO_CHAIN.md, SLICE_AUDIO_LIVE.md, AUDIO census | not started | L1 |
| Video | Bink `.bik` | none | not started | - |
| Navigation | navmesh, `NavigationHandle` (44) | none | not started | collision |
| Input, camera | `PlayerInput`, Better UI Controls mod caveat | CONTROLS.md | not started | - |

---

## 4. Duplicated rules (the same rule implemented more than once)

| Rule | Implementations | Parity check today |
|---|---|---|
| Attribute stack recompute | `src/interp.cpp` (5F), `src/weapon_gen.cpp`, `tools/weapon_stats.py`, `src/combat_hit.cpp` reads | weapon_gen vs Python bitwise; VM path only on combat rows |
| `AttributeInitializationData` evaluator | `src/progression.cpp` (VM, context struct), `src/weapon_gen.cpp` `Evaluator` (manifest, `Weapon_Is_<Maker>` by name), `tools/weapon_recipe.attribute_value` (conditional -> None), `tools/prepare_stock_shield.Evaluator`, `tools/loot_pools.py`, `tools/skill_stats.py`, `tools/slice_values.py` | only weapon_gen vs weapon_recipe |
| Weapon roll and stats | `tools/weapon_recipe.py` + `weapon_stats.py`; `src/weapon_gen.cpp` | `weapon_gen_parity.py` 2,919,834 fields, 0 mismatches |
| Item part merge | `weapon_recipe.merge_slots`, `prepare_vendors.item_parts`, `item_registry` (reuses both), `prepare_stock_shield.candidates` | registry vs each, 0 different (ITEM_REGISTRY section 3) |
| Pool roll | `tools/loot_pools.py`, `src/weapon_gen.cpp` pools, `src/combat_loot.cpp` (VM) | Python vs C++ 7,680 pool rolls; VM path separate |
| Firing maths | `src/weapon_firing.*`, `tools/weapon_firing.py`, `tools/weapon_gen_firing.py` | `check_weapon_firing.py` (UE5 suite gone) |
| RNG | four generators (F6) | CrtRand Python = C++ |
| Inventory navigation | `src/inventory_navigation.*`, `tests/inventory_navigation_test.js` / overlay JS | JS test |
| Card rounding | `tools/weapon_stats.py`, `weapon_gen.cpp` `CardEvaluator`, overlay `item_card.js` | golden cards, `card_rounding_cases.json` |
| Script disassembly | `research/script_disasm.py`, `src/script.cpp` | `tools/verify_scripts.py` |
| Property decoding | `Package::properties` (schema JSON), VM reflection reader | verify_packages |

---

## 5. Shaped by the retired UE5 host

| Artefact | What it was for | What the Rust port should do |
|---|---|---|
| `local/weapons/gen/weapon_gen_manifest.txt` (line-based, `tools/weapon_gen_manifest.py`) | let the UE module link `weapon_gen` without a package reader | keep as a parity oracle only; Rust rolls read the definition store |
| `local/slice/*.json` gear, recipes with `firing` blocks, `stock_shield_gear.json` (level 8) | pre-rolled items converted to `FOpenWillowWeaponItem` (floats within 1e-4) | runtime rolls from L6 at the real stage |
| Editor jobs (`local/slice/editor_job.json`, ambient, Phaselock target), `tools/repo_paths.py resolve-job` | UE editor import | none; host imports UModel output itself |
| `tools/prepare_range.py`, `tools/check_interactives.py` reading `.uasset` names under `host/ue5/OpenWillow/Content` | role lookup from imported assets | read roles from packages or UModel listings |
| `tools/prepare_level.py` "host" transform (1 uu = 1 cm, degrees) | UE5 placement | one conversion at the Rust host boundary (F9) |
| Overlay pages (Ruffle in a browser over the game window) | HUD/menus over UE5 | benchmark question: embedded Ruffle |
| Slice prep tools (`prepare_slice_world`, `prepare_vendors`, `prepare_action_skill`, `prepare_phaselock_*`, `prepare_skill_tree`, `prepare_inventory_gear`, `prepare_interactives`, `prepare_mover`, `prepare_stock_shield`) | baked manifests for one map and one character | evaluate from definitions at run time; tools become checkers |
| Host stand-ins listed in records (start shield, host AI brain, respawn delays, kickoff timing) | make the slice loop run | each needs its game rule from L5/L9/L10/L11 |

## 6. Where current code will change shape when a lower layer arrives

| Place | Today | Changes when |
|---|---|---|
| `tools/prepare_stock_shield.py` `SLICE_LEVEL = 8`; slice gear at level 8 | constant level | L4 provides the region stage (Sanctuary PT1 7..9 / 8..11) |
| `vm::AttributeContext` fixed fields | ad hoc struct | L3 resolvers over live world objects; L4 context service |
| `mission_script.cpp` throws for PT != 1 or level >= 50 | honest refusal | L4 playthrough modifiers read |
| Stage cap 50 opt-in; UVHM, OP not modelled | partial | L4 UVHM rule read (needs bl-sdk or Ghidra) |
| Item roll draws the manufacturer itself | stand-in | L6 pool-level candidate expansion |
| Runtime-state stage windows treated as no bound | stand-in | L3 evaluates the attribute in context |
| `combat_loot.cpp` stand-in inventories | no parts, no stats | L6 in Rust, called by the VM natives |
| `combat_hit.cpp` "0 with base 0 means identity 1" | stand-in | L1/L3 apply class defaults and start-up modifiers properly |
| Shield pool numbers | invented stand-in | L6 shield stats + L7 equip effects |
| Crit multiplier | base not found | start-up modifier source read |
| `weapon_gen` `Weapon_Is_<Maker>` string match | name hack | `ManufacturerAttributeValueResolver` in L3 |
| VM `Rand`/`FRand` static LCG | stand-in | F6 RNG service |
| `GotoState` name-only, `Enable`/`Disable` no-ops | approximations | L2 state machine per NATIVE_ENGINE_CORE |
| Kill XP: PT1, single player, lower-level victims | fitted | L4 multipliers, co-op shares |
| Elemental effectiveness PT2 constants | data present, unused | L4 playthrough input |

---

## 7. Proposed order, with reasons

1. **L0 then L1** (already Phase 0R). Reason: every rule reads definitions. Put the overlay hook (F7) and object
   identity rule into L1 now; changing them after rules exist touches every caller.
2. **L2 VM core before game rules.** Reason: the game's own script computes weapon bases, applies attribute effects,
   rebases pools, accepts missions; running it removes rules we would otherwise re-implement, and the 5E/5F opcodes put
   the attribute system inside the VM.
3. **L3 attributes as one crate-level service, used by the VM natives and by Rust rules alike.** Reason: trap 1; every
   number above depends on it. Its tests are the union of today's evaluator fixtures.
4. **L4 balance context next, typed for all three playthroughs, OP and player count from day one**, implementing what is
   read (PT1) and returning a typed "not read" error elsewhere. Reason: the maintainer's concern; adding playthrough later
   would touch every evaluation call site.
5. **F6 RNG and F10 clock before L5-L9.** Reason: pools regenerate on time, skills refresh on ticks, loot and spread draw
   randoms; tests need both injectable.
6. **L5 pools, then L6 items (all kinds, DLC included), then L7 equip.** Reason: items need stage and attributes; equip
   needs items; shields feed pools through equip.
7. **L8 behavior kernel and skills, then L9 combat.** Reason: damage reads skill modifiers and fires `OnTakeDamage`
   behaviors; Phaselock is a skill on top of the kernel.
8. **L10 AI, L11 world logic, L12 saves, L13 UI** in slice order (Sanctuary, Maya, one mission), each only as far as the
   slice needs (AGENTS.md priority).
9. **Host track H in parallel from L1**: the Phase 0R benchmark needs only meshes, one animation, a capsule and a movie.
   Movement/collision rules (NATIVE_PLAYER_MOVEMENT) attach to the chosen engine.

Suggested crate split (for the architect to weigh): `ow-container`, `ow-package` (L0), `ow-defs` or part of `ow-vm`
(L1 store + overlay), `ow-vm` (L2), `ow-attr` (L3 + L4 context), `ow-items` (L6, L7 data), `ow-game` (L5, L8-L11 natives
and rules), `ow-save` (L12), `ow-host` (H, L13 bridge). Today's `ow-game` skeleton lumps L5-L11 together.

---

## 8. Questions only the real game can answer (bl-sdk sessions)

1. Playthrough values: `WillowGameReplicationInfo.CurrentPlaythrough` and the `PlayThroughCount` attribute in NVHM,
   TVHM and UVHM (needs characters in each mode; saves backed up).
2. UVHM region game stage and the stage cap with OP levels (item levels of drops at OP 0, 1, 8).
3. Origin of the cooked-vs-live weapon differences (which object or package carries the live values).
4. Triage of the 92 untriaged live weapon stat mismatches (read after initialisation, from the readied weapon).
5. `ComputedModifierValue` of item attribute slots (cards with clamped grades).
6. Non-weapon rolls: spawned shield, class mod and relic drops for slot frequencies, rarity levels, class mod definition
   pick, `GrenadeMod_FlameSpurt` rarity (NATIVE_ITEM_RULES "What a session should read").
7. The grenade-mod per-element minimum game stage: does a part outside it ever roll?
8. Maya's crit multiplier source (`CurrentInstantHitCriticalHitBonus` at spawn) and the modifier that sets it.
9. Spread distribution and status proc rate; beams and ricochets.
10. Typed-let 0x5F: does a base write notify? (`ClipSize` notification owner.)
11. Which classes override the attribute-changed notification (weapon `ClipSize`, `WeaponReadyMax`, `ShopTimerRate`).
12. Whether `ObjectPropertyAttributeValueResolver` finds inherited properties.
13. Enemy scaling in TVHM/UVHM: health, damage, XP of the same enemy at the same level; playthrough
    `BalanceModifierDefinition` effects on spawn.
14. When `ResetGameStageForRegion` is called; region stage after leaving and re-entering.
15. Kill XP outside the fitted range (equal level, higher victim, d >= 5, co-op).
16. Respawn station choice order; respawn without a station.
17. Leaps and thrown rocks; lone bullymong; cone boundary.
18. Which pools have `bSerializeInSaveGame` (ammo in saves).
19. Effects of the `Micropatch` entries (population weights, skills) on the slice.
20. The live 3-item money drop (COMBAT_LOOT OPEN).

## 9. Questions the maintainer must decide

1. **Cooked or live values** as the port's definition source, and whether an overlay layer (hotfixes, BLCMM text mods)
   is built into the store now (NATIVE_WEAPON_RULES section 7).
2. **Where item and loot rules run**: move them from Python into Rust runtime crates (Python kept as checker/oracle), as
   the 2026-11-14 language split implies, or keep Python as a generator for now.
3. **Script versus native**: run the game's own script for weapon base values, attribute effect application, pool
   rebase, save generation (needs VM + natives first), or re-implement those in Rust.
4. **Definitions at run time versus generated manifests** (the weapon_gen line manifest, slice JSON).
5. **RNG policy**: one seeded service with named streams; `CrtRand` kept for parity; no attempt to reproduce the game's
   sequence (unknown).
6. **Float policy**: bit-exact with the C++/Python oracles (pin `pow`, no FMA) or a stated tolerance.
7. **Playthrough scope now**: type all three playthroughs, OP and player count in L4 from the start (recommended) or
   PT1 only until after the Phase 4 gate.
8. **DLC in the item system** for the slice (registry has it; C++ generator does not).
9. **Gibbed THIRD_PARTY entry** for the save format (open since 2026-10-22); real save read/write is Phase 5.
10. **Crate boundaries** (section 7) and whether `ow-game` is split.
11. **Engine route** (Phase 0R, the benchmark), the coordinate conversion boundary, and the Ruffle/Scaleform route.
12. **Single player only for now**, keeping `NumberOfPlayers` as an input (co-op netcode is Phase 6).

---

## 10. Where to look (index)

| Topic | Records | Code |
|---|---|---|
| Reader, census | GATES.md, PACKAGE_GLOBAL_INDEX.md, EXTERNAL_TOOL_BENCHMARK.md | src/package.*, container.*, census.*, assets.*; tools/verify_packages.py |
| VM | SCRIPT_BYTECODE_DISASM.md, SCRIPT_VM_PROTOTYPE.md, NATIVE_BYTECODE_OPCODES.md, NATIVE_CLASS_SERIAL_LAYOUT.md, NATIVE_ENGINE_CORE.md | src/script.*, vm.*, interp.cpp, natives*.cpp, vm_sweep.cpp; research/script_disasm.py |
| Attributes, progression | NATIVE_ATTRIBUTES.md, NATIVE_PROGRESSION.md, NATIVE_LEVEL_UP_ATTRIBUTES.md | src/progression.*, interp.cpp; tools/weapon_recipe.py |
| Items | WEAPON_BALANCE_DECODE.md, NATIVE_WEAPON_RULES.md, WEAPON_PARTS_CATALOG.md, NATIVE_LOOT.md, NATIVE_ITEM_RULES.md, ITEM_REGISTRY.md, ITEM_ROLL.md | tools/item_registry.py, item_roll.py, weapon_recipe.py, weapon_stats.py, loot_pools.py; src/weapon_gen.* |
| Combat | NATIVE_DAMAGE_DEATH.md, NATIVE_COMBAT_GROUNDWORK.md, NATIVE_WEAPON_FIRING.md, NATIVE_WEAPON_TRIGGER_AMMO.md, WEAPON_PROJECTILES.md, COMBAT_*.md | src/combat*.cpp, weapon_firing.*; tools/verify_combat_pawn.py |
| Skills | NATIVE_SKILLS.md, NATIVE_SKILL_BEHAVIOURS.md, NATIVE_ACTION_SKILL.md, NATIVE_PHASELOCK_*.md, PHASELOCK_STOCK_DATA.md | src/behavior.*; tools/skill_stats.py |
| AI | NATIVE_AI_*.md, AI_*_LIVE.md, COMBAT_RANGE.md | (none in src) |
| Missions, world | NATIVE_MISSION_*.md, NATIVE_KISMET_MATINEE.md, NATIVE_DIALOG*.md, FIRE_*.md, SANCTUARY_RPG_MISSION.md, VENDORS.md | src/mission*.cpp, kismet.*, dialog.*, mover.*, slice.*; tools/mission_closure.py |
| Saves | NATIVE_SAVE_LOAD.md | (none in src) |
| UI | NATIVE_GFX_BRIDGE.md, HUD_*.md, MENUS_PARITY.md, INVENTORY_*.md | tools/hud_overlay/ |
| Real-game method | REALGAME_GROUND_TRUTH.md | tools/real_game/ |
| Lookups | `python tools/native_index_find.py <name>`, `python tools/decisions_index.py --grep WORD` (titles only) | |
