AI-assisted (Claude Code).

# BL2_ReBased Rust port: architecture and lane plan (2026-11-14)

The design for porting the C++ core (`src/`, about 19,200 lines) and the Python runtime rules (`tools/`) into the Cargo
workspace, written so a hub can brief parallel lanes from it without the author. Inputs: `research_systems_map.md`
(this folder; cited as "the report", sections F1-F12, L0-L13), decision 2026-11-14-01, `AGENTS.md`,
`docs/LANE_BRIEF.md`, `docs/verification/GATES.md`, `ROADMAP.md`, `docs/OPENWILLOW_ENGINE_PLAN.md`, the six crate
skeletons and the C++ headers. Nothing here was run. Every game rule keeps the label its record gives it; porting a
rule changes nothing about what is known about it.

Spot checks made for this design (on main 74d7dda):

- The C++ core already has **three `pow` precisions**: `progression.cpp:248` calls `std::pow(float, float)` (single);
  `weapon_gen.cpp:562` calls `std::pow(double, double)` and rounds to f32 afterwards; the VM's `Object.**`
  (`natives_core.cpp:140`) takes doubles held at float precision and rounds the result. Section 5 deals with this.
- The VM's `Object.Rand` and `Object.FRand` (`natives_core.cpp:201-209`) each keep their own static LCG state (seeds
  12345 and 54321): two generators, not one.
- Float text in the C++ outputs uses **several formats**, none of which Rust's formatter produces: `setprecision(9)`
  (`package.cpp`, `assets.cpp`, `combat_run.cpp`), ostream default precision 6 (`cli.cpp:55` `--object-dump`,
  `vm.cpp:117` `describe()`), `%f` (`interp.cpp:256`), `%.5g` (`combat_hit.cpp:26`), `std::to_string`
  (`combat.cpp:150`), `%.1f`/`%g` (`weapon_gen.cpp:952/959`), `to_chars` shortest (`weapon_gen.cpp:1694`); the Python
  registry writes `json.dumps` floats (Python `repr`, `ensure_ascii=False`, `item_registry.py:836`). *(Corrected in
  revision 2; r1 said every output used `setprecision(9)`.)*
- The C++ tests drive the `ow-package` binary from Python (`tests/package_test.py $<TARGET_FILE:ow-package>` and 11
  more), and 82 Python tools take it with `--reader` (default `build/Release/ow-package.exe`). The binary's command
  line and output are the project's de facto interface, and the cheapest differential harness there is.
- `verify_packages.py` needs `--verify-decoded`, `--exports` and `--class-check --cooked --failures`; the class-body
  decoder lives in `vm.cpp`, so the Phase 0R reader gate needs reflection, not only the package tables.

**Status (2026-11-14):** the maintainer approved D1-D18 as recommended (section 11; D18, the deliberately
single-threaded simulation, on the same day after review r1). W0-SCAFFOLD is running on `feature/rust-w0-scaffold`;
what it still has to add or change after revision 3 is the list in section 15, **W0 changes needed**.

---

## Revision 2 (review r1, 2026-11-14)

An independent Opus-high review returned 19 findings and 8 minor items. Each was checked against the code before
being applied; none was rejected. Two were applied in a modified form (marked *modified*).

| # | Finding | What changed | Where |
|---|---|---|---|
| 1 | Timer queue contradicts the game's timers | W1-SIM keeps only `SimClock` and `TickGroup`; per-actor timers (the `Timers` array rules of NATIVE_ENGINE_CORE.md) move to W5-WORLD; TimeDilation application labelled UNVERIFIED | 3.1, 5.1 N9, W1-SIM, W5-WORLD |
| 2 | Natives cannot reach the attribute world, RNG or clock; borrow conflict | `Vm` owns `GameRng`, `SimClock` and `Rc<AttributeSystem>`; `Vm` implements `AttrWorld`; the attribute system draws through `AttrWorld::rng(CrtStream)` and natives through `vm.rng` (not an `EvalCx` field); world services (balance, engine queries) are VM services registered by `ow-world` | 3.5, 3.8, 3.9, W2-VMAPI, W2-ATTRAPI, W3-CORENAT |
| 3 | No-stand-in rule vs unchanged tests | `ow_core::parity::ParityMode` selected by the one env var `OW_PARITY` that `tools/rust_parity.py` sets; stand-in-pinning test lines run only in that mode; the pinned cases listed per lane; W7-RETIRE needs a Rust-native replacement test per stand-in | R3, 3.1, 6, lane entries, W7 |
| 4 | Partially compressed packages are supported | W1-LZO ports the partial table; `PartiallyCompressed` error removed | 3.2, 6, W1-LZO |
| 5 | Two stack-combine forms | `CombineForm { WeaponGen, Vm5F }` beside `PowForm`, same stop-and-report rule; real-game question 22 | 3.5, 5.3, 12, W1-ATTRMATH |
| 6 | Two float-to-int forms | `trunc_i64_wrap_i32` (VM natives) beside `trunc_i32` (attribute path), oracle named for each | 3.1, 5.1 N5 |
| 7 | Not every float output is `%.9g` | `ow-core::fmt` provides `%.Ng`, `%f`, `%.Nf`, ostream default, `to_string`, `to_chars` shortest, Python `repr` and Python JSON quoting | top, 3.1, 5.1 N6, W1-NUM |
| 8 | No Rust builder for synthetic packages | `ow-package` `testkit` behind feature `test-support` (W0 placeholder, W1-TESTKIT real) | 2.3, 3.2, W1-TESTKIT, W2 inputs |
| 9 | CLI flags without owners | W0 pre-creates every flag and `cmd/` file named in this document, incl. the Rust-only ones; `--census`, `--payload-file` to W1-PKG, `--scene-records` to W2-ASSETS | 2.3, W0, W1-PKG, W2-ASSETS |
| 10 | `Rc` makes the world `!Send` | *Recommendation (new D18):* keep it `!Send` deliberately (one sim thread, `Send` messages to the host); packages shared as `Arc<Package>`; recorded as a benchmark input | 3.3, 11 D18, 13 |
| 11 | Static-array elements not addressable | `PropAddr { prop, index }`; a layout reserves `array_dim` consecutive slots | 3.3, W1-OBJ |
| 12 | RNG types wrong | `GameRng::crt(CrtStream) -> &mut CrtRand` and `GameRng::script(ScriptStream) -> &mut dyn ScriptRng` (`rand_limit`, `frand`); VmLcg implements the C++ formulas; one RNG per roll | 3.1, 3.7, W1-RAND, W4-ROLL |
| 13 | `BTreeMap` breaks weighted picks | `ow_core::OrderedMap` (insertion order, a repeated key keeps its first position with the later value); `DetMap` only where key order is the oracle's | R6, 3.1, 5.1 N7 |
| 14 | Wave 1 is not flat | Landing order stated | 8 (wave 1 intro), 8.1 |
| 15 | Placeholders need constructors | `new_empty()` on each placeholder, kept by the real type | 2.3, W0 |
| 16 | Gates that never run the Rust binary | `script_disasm_test.py`, `ui_trace_returns_test.py`, `skill_effects_test.py`, `skill_stats_test.py` removed from Rust gates; re-expressed as Rust unit tests; `--disasm`/`--script-check` diffs against C++ on synthetic packages added | 6, W2-SCRIPT, W4-INTERP2, W6-SKILLS |
| 17 | Pool-roll path and quantity rounding | gate names `loot_pools.roll_crt`/`roll_pool_crt`; *modified:* `round_half_even` is its own `num` function (the oracle's quantity rounding, UNVERIFIED as a game rule), not a fifth variant of the game's 0-3 `RoundingMode` | 3.1, 6, W4-POOLS |
| 18 | Skills need inventory and combat | *modified:* skills move to a new crate `ow-skills` above `ow-combat` (22 crates); the behavior kernel's handler registry lets crates above register handlers for behavior classes that deal damage or spawn loot (unregistered: `NotYet`) | 2.1-2.3, 3.9, W5-BEHAVIOR, W6-SKILLS, D10 |
| 19 | Vocabulary too narrow | `LevelCapUpgrades(u8)`; `DefinitionData.game_stage: GameStage` | 3.6, 3.7 |
| minor | `PowForm` kept as a parameter; `black_box` in the pow table; choke points for exp/log/trig/sqrt; R7 covers stderr; `weapon_gen_test.py` is deleted, not passed; stale refs (W3-DEFS "6.1", 68 -> 82 tools, `tools/real_game/golden_card_compare.py`) | 5.2, 5.3, 3.1, R7, 6, W3-DEFS, 9 |

**W0 deltas** (for the hub to pass to the running scaffold lane, or a small follow-up scaffold PR if it has landed):
the `ow-skills` crate with its edges and files, and `ow-behavior` without the skill files (finding 18);
`ow-core/src/parity.rs` instead of `ow-cli/src/parity.rs` and `ow-core/src/ordmap.rs` (3, 13; already told);
`ow-package/src/testkit.rs` placeholder behind feature `test-support` (8; already told); every `cmd/` file and flag
listed in 2.3 (9; already told); `new_empty()` on placeholders (15; already told); no `TimerQueue` in the `sim.rs`
header (1; already told).

## Revision 3 (review r2, 2026-11-14)

A fresh Opus-high review of revision 2, checked against main 74d7dda and the W0 worktree, returned 12 findings ("ready
after these fixes, no redesign"). Each was checked against the code and the W0 branch; none was rejected.

| # | Finding | What changed | Where |
|---|---|---|---|
| 1 | A manifest line runs a whole test file; no xfail; pinned cases unowned | New wave-1 lane **W1-PINSPLIT** moves the pinned cases into their own files (`tests/vm_states_test.py`, `tests/combat_loot_standin_test.py`), still passing on the C++ binary; the harness gains `diff` lines (same arguments on both binaries, stdout and exit code compared), so a lane can gate single commands; "expected failures" dropped; W7-RETIRE owns and deletes the split files | 6, 6.1, W1-PINSPLIT, W1-HARNESS, W3-INTERP, W4-INTERP2, W7 |
| 2 | Two sources of the parity mode | Only binaries call `parity_mode()`; library crates take `ParityMode` as a value: `GameRng::from_world_seed(seed, mode)`, `Vm::new(.., mode)` | R3, 3.1, 3.8, section 15 |
| 3 | `AttrWorld` lends space and store twice | `AttrWorld::parts()` returns both in one split borrow (`WorldParts`) | 3.5, W2-ATTRAPI |
| 4 | Wave-1 lanes need types nobody provides | W0 places the final `GameStage`, `PackageId`, `ModifierId`; `combine` returns an `ow-attr`-local `Combined`, so W1-ATTRMATH does not wait for W1-OBJ | 3.2, 3.5, 3.6, wave-1 table, section 15 |
| 5 | `WorldHooks` frozen nowhere | `pub trait WorldHooks` in `ow-vm::api`, frozen by W2-VMAPI, held as `Rc<RefCell<dyn WorldHooks>>`; balance state is a separate service typed in `ow-world` | 3.8, 3.9, 7 |
| 6 | `ow-inventory` registers handlers without an edge | Loot-spawn behavior handlers move to `ow-combat::loot`; `ow-inventory` registers none (no edge change) | 3.9, 7, W5-EQUIP, W6-LOOT |
| 7 | 6.1 missing two `item_roll.py` stand-ins; class-mod pick misplaced | Added: manufacturer drawn in the balance roll (Reference only; Off draws at the pool level, as the game) and name parts in the rarity sum (an UNVERIFIED reading in both modes, not a stand-in); class-mod pick moved to W4-ROLL; coverage loss after W7 stated | 6.1, W4-ROLL, W4-POOLS |
| 8 | Stub policy per command unstated; citations off | `StubPolicy` per CLI command (3.8); logged zero-result stubs of the exploration commands are a declared policy, not a stand-in; citation 754-815; only `--vm-sweep` pins the VM LCGs | 3.8, 6.1, W3-CORENAT |
| 9 | D18 contradicted itself | D18 recorded as approved; `!Send` frozen in W1-OBJ | status, 11, 3.3 |
| 10 | CMake overlap, real-manifest entry, no release build in gates | W1-HARNESS owns its own lines in `tests/CMakeLists.txt` incl. the `rust-parity` entry over `tests/rust_parity.d/`; `cargo build --release -p ow-cli --locked` added to the standard gates | 8 intro, W1-HARNESS |
| 11 | Rust-only commands lack modifiers | Modifiers listed in the CLI table; `cmd/` modules parse their own trailing arguments (W0 already passes them) | 2.3, section 15 |
| 12 | Phaselock lift, `ow-skills` label, 8.1 arrow | The lift runs through script and world natives (no `ow-skills` -> `ow-ai` edge); label "L8b (built above L9)"; arrow W1-ATTRMATH -> W2-ATTRAPI | 2.1, 8.1, W6-SKILLS |

---

## 1. Design rules (apply to every lane)

| # | Rule | Why |
|---|---|---|
| R1 | **Concrete types point down, traits point up.** A crate uses the concrete types of the crates below it. A trait exists only where a lower crate must call back into a higher one (attribute notification into the VM world, context resolvers into the world, the movie player into the host). | Few abstractions for the maintainer to explain; every trait marks a real upward edge. |
| R2 | **One implementation per rule.** A rule that must work without a running world (item rolls, stats, cards, pools, the evaluator, XP and stage maths) lives in a library crate **below** the VM. A rule that only exists in a running world (missions, Kismet, AI decisions, menus) runs as the installed script on the VM, with natives in a native-pack crate **above** it. Where the game reaches an offline rule through script (`WillowWeapon.CalculateWeaponBaseValues`, `AttributeEffect.ApplyAttributeEffects`), the VM binds that script function to the Rust rule (`override_script`) and a test runs the installed script against the Rust rule as a differential oracle. | Removes trap 1 and trap 3 of the report: no second evaluator, no host stat copy. |
| R3 | **Typed "not yet", never a stand-in.** Every crate's error enum carries `NotYet(ow_core::NotYet)`. An unread resolver class, an unread playthrough branch, an unported native, a missing lower layer: each returns `NotYet` naming what is missing and the record that says so. No `unwrap_or(0)`, no default constants for unread data, no "0 with base 0 means 1" (`combat_hit.cpp`). A stand-in of the reference implementation (C++ or Python) that a parity test pins is reproduced only under `ow_core::parity::ParityMode::Reference`, which is selected by **one** environment variable, `OW_PARITY=reference`, read once at start-up **by the binary only** (`ow-cli`'s `main`, later `ow-host`'s): library crates never read the variable, they take the mode as a value (`GameRng::from_world_seed(seed, mode)`, `Vm::new(.., mode)`), so tests can build both modes in one process; `tools/rust_parity.py` sets it for the manifest lines marked `mode=reference` and for no others. Each lane lists the pinned test cases it meets (section 6.1). A stand-in is deleted in W7-RETIRE only after a Rust-native test of its replacement (the game rule, or `NotYet`) exists. | The maintainer's requirement: places where a lower layer is not finished are explicit. |
| R4 | **Final types from day one for the vocabulary everything shares**: `Playthrough` (0-based index with a 1-based count view), `OpLevel`, `PlayerCount`, `GameStage`, `ExpLevel`, the item `DefinitionData`, `ObjectPath`, `Value`, `ObjRef`, `Modifier`. Unimplemented branches return `NotYet`; the types do not change when the branch is read. | Adding UVHM, OP or co-op later touches rule bodies, not every call site. |
| R5 | **The definition store has the overlay and the identity rule from its first PR.** | Report F1, F7: changing them after rules read definitions touches every caller. |
| R6 | **Determinism is a type-level property.** Game randomness comes only from `ow_core::rand::GameRng` streams; host randomness is a different type that game crates cannot name. Game time comes only from `ow_core::sim::SimClock`. No `HashMap`/`HashSet` iteration in rule or output code: `ow_core::OrderedMap` where the oracle iterates in insertion order (weighted picks: `weapon_gen.cpp:744-760`, `weapon_recipe.pick`), `ow_core::DetMap` (BTreeMap) only where the oracle's order is key order (`std::map`, `sort_keys=True` JSON). | Report F6, F10, F11. |
| R7 | **The C++ CLI is the contract until C++ is deleted.** The Rust `ow-package` accepts the same flags and prints byte-identical stdout (JSON key order, each output's own float format, section 5.1 N6), the same exit codes, and the same stderr text where a test asserts it (e.g. `assets_test.py:163`, `'supports PF_G8 only'`), so the existing Python tests and the 82 tools that take `--reader` run against it unchanged. | The differential harness is free, and tools switch with a path change. |
| R8 | **No third-party crate in waves 0-5.** Everything (LZO, PNG stored-deflate, JSON writer, `%g` formatting, a type map) is our own code. Wanted crates are listed in section 11 as maintainer decisions. | Rule 6; nothing waits on a `THIRD_PARTY.md` entry. |
| R9 | **Units stay the game's** (uu, rotator units 65,536 per turn, Z-up, seconds as f32 `TimeSeconds`) in every crate except `ow-host`, which holds the only conversion to the engine's frame. | Report F9. |
| R10 | **Synthetic fixtures in tracked tests; real packages only in local runs**, whose outputs stay under `local/`. Golden numbers already in the records (curve, health base, XP 395, card counts) may appear in tests as constants. | Rules 1 and 9. |

---

## 2. Crate graph

The six skeletons become 22 crates (21 in revision 1; `ow-skills` added by review finding 18). `ow-game` is split by
layer (its doc comment moves to the new crates); the other five keep their names.

### 2.1 Crates and what each owns

| Crate | Layer | Owns | Ports from |
|---|---|---|---|
| `ow-core` | F | `num` (f32 policy, choke points for `pow`/`exp`/`log`/trig/`sqrt`, the two MSVC-faithful float-to-int forms, rounding), `units` (uu, rotator), `fmt` (every C++ and Python float format the outputs use, the JSON quoting of both), `rand` (CrtRand, VmLcg, named streams, `GameRng`; `HostRng` is in `ow-host`), `sim` (clock, tick groups), `parity` (`ParityMode`), `notyet`, `ident` (`ObjectPath`, `PackageName`), `DetMap`, `OrderedMap` | `weapon_gen.hpp` CrtRand, `weapon_recipe.f32`, the C++ float printers |
| `ow-container` | L0 | LZO1X decoder, fully and partially compressed containers (the partial chunk table of `container.cpp:38-57`), their bounds checks | `src/container.*`, written from `research/native_count.py` |
| `ow-package` | L0 | `Reader` (`require`/`limit` as `Result`), header 832/46, name/import/export tables, `Package::load`, the tagged-property **tag walker** and its serialized tree `PropValue` with an `ArrayTyper` hook, the schema typer (temporary), `PackageStore` (scan, lazy load, import resolution, global fallback), and `testkit` (feature `test-support`: the Rust counterpart of the `package()`/`tag()`/`container()` helpers in `tests/`, LZO containers made of literal runs) | `src/package.*`, the Python fixture helpers |
| `ow-object` | L1a | IDs, `Value`, `Heap` (arena), `NameTable`, `ClassTable` and class layouts, `PropertyDecl` with attribute companions, reflection loading from code packages, class-body decode, the reflection typer, `PropValue -> Value`, class default objects, `instantiate_export` with the C++ prefixes | `vm.cpp` loading half, `decodeClassBody`, `applyTaggedDefaults` |
| `ow-defs` | L1b | `DefStore`: object identity (`IdentityPolicy`, `LoadSet`), lazy definition loading into the heap, the overlay layers, typed property keys, raw trailing bytes per export, census iteration | `PackageStore` use in `vm.cpp`, `instantiateReference`, `tools/real_game/live_overlay.py` input |
| `ow-script` | L2a | Bytecode decoder, opcode table, disassembler text, package check | `src/script.*` |
| `ow-attr` | L3 | Modifier stacks, plain attributes, `AttributeInitializationData` evaluator, value and context resolvers (registry, `NotYet` for unread classes), expressions, `ApplyAttributeEffects` / remove records, notification hook | `progression.cpp`, `interp.cpp` 5F recompute, `weapon_gen.cpp` Evaluator, the Python evaluators |
| `ow-balance` | L4 | Playthrough / OP / player-count / stage types, XP curve, region game stage (stateful), item stage cap, playthrough modifiers, mission and kill XP formulas, the offline balance world | `progression.cpp` ExperienceCurve, `combat_xp.cpp`, `mission_script.cpp` GetExperienceReward, `tools/slice_values.py` |
| `ow-items` | L6 | `DefinitionData` (item identity), registry of every kind incl. DLC, balance merge, manufacturer candidates, part pick, names, rarity, value, level, stats and cards for every kind, item pools | `weapon_gen.*`, `tools/item_registry.py`, `item_roll.py`, `weapon_recipe.py`, `weapon_stats.py`, `loot_pools.py`, `prepare_stock_shield.py` |
| `ow-vm` | L2b | Interpreter, frames, states, latent code, delegates, iterators, native registry (`"Class.Name(types)"` keys), stub policy, Core natives, step limit, call census and trace, `override_script`, VM sweep and native census; **owns** the object space, the store, `GameRng`, `SimClock` and `Rc<AttributeSystem>`, and implements `AttrWorld` | `vm.cpp` run half, `interp.cpp`, `natives*.cpp`, `vm_sweep.cpp`, `census.*` |
| `ow-world` | L5 + stock natives | `World` (a `Vm` plus the native packs; `tick`), the world-hook service the VM's `AttrWorld` calls (globals, game info, engine queries, balance), actor spawn/destroy, iterators, per-actor timers (the record's `Timers` array), resource pools and the pool manager, pawn health glue, level-up rebase | `combat.cpp` pool half, `mission_script.cpp` pool and level-up parts |
| `ow-inventory` | L7 | Inventory manager natives, equip/unequip effects, pickups, backpack sort, menu navigation helper, item spawn natives calling `ow-items` | `inventory_navigation.*`, equip parts of the notes |
| `ow-behavior` | L8a | Behavior provider decode (fitted variable block), kernel, the handler registry (crates above register handlers for behavior classes they own), the handlers that need only the world | `src/behavior.*` |
| `ow-combat` | L9 | Firing maths, hit chain, damage pipeline natives, death, down/FFYL, respawn, kill-XP glue, loot on death, the scripted combat run, the damage-dealing behavior handlers | `weapon_firing.*`, `combat*.cpp`, `tools/weapon_firing.py` |
| `ow-skills` | L8b (built above L9) | Skills, skill trees, `SkillEffectManager`, action skill start and refusals, cooldown pool use, Phaselock rules (above `ow-combat`: skills read the held weapon and deal damage; the lift of an AI target runs through the installed script and the world's natives, so `ow-skills` needs no edge to its sibling `ow-ai`) | `tools/skill_stats.py`, `prepare_skill_*` rules, NATIVE_SKILLS / NATIVE_ACTION_SKILL / NATIVE_PHASELOCK_* |
| `ow-ai` | L10 | AI pawns, minds, awareness, targeting, melee, population factories | nothing in `src/` (notes and live records) |
| `ow-mission` | L11 | Mission tracker, mission-script bridge, Kismet, Matinee hooks, dialog, movers, interactive objects, vendors, the Fire slice | `mission*.cpp`, `kismet.*`, `dialog.*`, `mover.*`, `slice.*`, `tools/prepare_vendors.py` rules |
| `ow-save` | L12 | `PlayerSaveGame` model, load order, save triggers; the on-disk format only after decision D9 | nothing in `src/` |
| `ow-ui` | L13 | GFx bridge natives, the `MoviePlayer` trait the host implements, card text from `ow-items` | `tools/hud_overlay` rules (later) |
| `ow-assets` | H data | Textures (TFC, mips, PNG with stored deflate), static meshes (OBJ), later skeletal meshes, animations, level placement | `src/assets.*` |
| `ow-cli` | tools | The `ow-package` binary: every C++ flag and the Rust-only ones, one module per command family (it reads `ow_core::parity_mode()`) | `src/cli.cpp`, `src/combat_run.cpp` driver parts |
| `ow-host` | H | Install check (exists), the engine route after the pick, the only unit/axis conversion, `HostRng`, the `MoviePlayer` implementation | none |

### 2.2 Allowed dependency edges

Each crate may declare as a direct dependency the crates listed and anything they may depend on (the transitive
closure). Everything else is forbidden. The scaffold declares the whole allowed closure for each crate; an unused path
dependency costs nothing and means no lane edits a `Cargo.toml` to use a lower crate.

```
ow-core            (none)
ow-container       ow-core
ow-package         ow-container
ow-object          ow-package
ow-script          ow-package
ow-defs            ow-object
ow-attr            ow-defs
ow-balance         ow-attr
ow-items           ow-balance
ow-vm              ow-script, ow-defs, ow-attr          (ow-attr only for the 5E/5F stack recompute)
ow-world           ow-vm, ow-balance
ow-inventory       ow-world, ow-items
ow-behavior        ow-world
ow-combat          ow-inventory, ow-behavior
ow-skills          ow-combat
ow-ai              ow-combat
ow-mission         ow-ai, ow-skills, ow-combat, ow-inventory, ow-behavior
ow-save            ow-mission
ow-ui              ow-mission
ow-assets          ow-defs
ow-cli             everything except ow-host
ow-host            everything except ow-cli; engine crates (after the pick) only here
```

Forbidden on purpose: `ow-attr`, `ow-balance`, `ow-items` depending on `ow-vm` (offline rules stay offline);
`ow-package` depending on `ow-object` (the reader knows no reflection); anything depending on `ow-host` or `ow-cli`;
any engine or UI crate below `ow-host`; `ow-behavior` depending on `ow-combat`, `ow-skills` or `ow-items` (behavior
handlers from above are registered into the kernel's registry, R1). The scaffold lane (W0-SCAFFOLD) writes every edge into the crates'
`Cargo.toml` once, so no later lane edits a `Cargo.toml` or `Cargo.lock`, and `tests/crate_edges_test.py` fails on an
edge outside this table.

### 2.3 Module files (ownership unit)

The scaffold creates every file below with a `//!` header naming its owner lane and the `pub mod` lines in each
`lib.rs`, so lanes never edit a shared `lib.rs`. A lane may add files under its own module directory
(`src/<module>/...`).

| Crate | Files (owner lane) |
|---|---|
| `ow-core` | `lib.rs`, `notyet.rs`, `ident.rs`, `detmap.rs`, `ordmap.rs`, `parity.rs` (W0-SCAFFOLD); `num.rs`, `units.rs`, `fmt.rs` (W1-NUM); `rand.rs` (W1-RAND); `sim.rs` (W1-SIM) |
| `ow-container` | `lib.rs` (signature: W0; body: W1-LZO), `lzo.rs`, `chunks.rs` (W1-LZO) |
| `ow-package` | `reader.rs`, `header.rs`, `tables.rs`, `package.rs` (W1-PKG); `testkit.rs` behind feature `test-support` (placeholder: W0; real: W1-TESTKIT), its children `testkit/reflect.rs` (W2-REFLECT) and `testkit/script.rs` (W2-SCRIPT), declared as placeholders by W1-TESTKIT; `props.rs`, `schema.rs` (W2-PROPS); `store.rs` (W2-STORE) |
| `ow-object` | `ids.rs`, `value.rs`, `names.rs`, `heap.rs`, `layout.rs` (W1-OBJ); `reflect.rs`, `class_body.rs` (W2-REFLECT); `decode.rs`, `cdo.rs`, `prefix.rs` (W3-DECODE) |
| `ow-defs` | `store.rs`, `identity.rs`, `overlay.rs`, `keys.rs`, `tail.rs` (W3-DEFS) |
| `ow-script` | `ops.rs`, `function.rs`, `decode.rs`, `disasm.rs`, `check.rs` (W2-SCRIPT) |
| `ow-attr` | `math.rs`, `stack.rs` (W1-ATTRMATH); `api.rs`, `registry.rs` (W2-ATTRAPI; `registry.rs` only delegates to the files below and the W3 lanes do not edit it); `eval.rs`, `expr.rs`, `value_resolvers/` (W3-ATTREVAL); `context_resolvers/` (W3-ATTRCTX); `effects.rs`, `storage.rs` (W3-ATTREFFECTS). W2-ATTRAPI placed the `NotPorted` stubs in the W3 files; they stay the W3 owners' |
| `ow-balance` | `types.rs` (W1-BALTYPES); `curve.rs`, `stage.rs`, `cap.rs`, `playthrough.rs`, `xp.rs`, `offline.rs` (W3-BALANCE) |
| `ow-items` | `data.rs` (W1-ITEMDATA); `registry.rs` (W4-REGISTRY); `merge.rs`, `candidates.rs`, `pick.rs`, `names.rs`, `rarity.rs`, `value.rs`, `roll.rs` (W4-ROLL); `stats.rs`, `cards.rs` (W4-STATS); `pools.rs` (W4-POOLS) |
| `ow-vm` | `api.rs`, `services.rs`, `error.rs` (W2-VMAPI); `interp/` (W3-INTERP, then W4-INTERP2 adds `interp/aggregate.rs`, `interp/iter.rs`, `interp/delegate.rs`); `natives_core.rs` (W3-CORENAT); `census.rs`, `sweep.rs` (W4-VMTOOLS); `states.rs`, `latent.rs` (W5-STATES) |
| `ow-world` | `world.rs`, `query.rs`, `actors.rs`, `timers.rs` (W5-WORLD); `pools.rs`, `levelup.rs` (W5-POOLS) |
| `ow-inventory` | `manager.rs`, `equip.rs`, `spawn.rs` (W5-EQUIP); `sort.rs`, `navigation.rs` (W6, later) |
| `ow-behavior` | `provider.rs`, `kernel.rs`, `registry.rs`, `handlers/` (W5-BEHAVIOR) |
| `ow-skills` | `skills.rs`, `tree.rs`, `action_skill.rs`, `phaselock.rs` (W6-SKILLS) |
| `ow-combat` | `firing.rs` (W1-FIRING); `hit.rs`, `damage.rs`, `death.rs`, `down.rs`, `xp.rs`, `run.rs` (W6-COMBAT); `loot.rs` (W6-LOOT) |
| `ow-mission` | `kismet.rs` (W5-KISMET); `tracker.rs`, `bridge.rs` (W6-MISSION); `dialog.rs`, `mover.rs` (W5-DIALOG); `slice.rs` (W6-FIRE); `vendors.rs`, `interactive.rs` (W7) |
| `ow-assets` | `texture.rs`, `tfc.rs`, `png.rs`, `mesh.rs`, `obj.rs` (W2-ASSETS) |
| `ow-cli` | `main.rs`, `args.rs` (W0-SCAFFOLD); `json.rs` (W1-PKG); `cmd/<family>.rs`, one per command family, all created by W0 (table below) |

**CLI command files and flags** (W0 creates every file and wires every command flag below into `main.rs`, so no
later lane edits `main.rs`; each `cmd/` module receives the whole argument list and parses its own trailing modifiers,
so a lane adds a modifier inside its module only, and W0's `args.rs` lists them for the usage text; a flag that `src/cli.cpp`'s usage names and this table misses is mapped by W0 to the family whose
command reads it, and the mapping is written into `crates/README.md`):

| File | Owner | Flags |
|---|---|---|
| `cmd/tables.rs` | W1-PKG | `--names`, `--imports`, `--exports`, `--census`, `--payload`, `--payload-file`, `--payloads`, `--verify-decoded` |
| `cmd/properties.rs` | W2-PROPS | `--properties`, `--properties-batch`, `--property-offset`, `--array-schema` |
| `cmd/resolve.rs` | W2-STORE | `--resolve`, `--cooked` |
| `cmd/assets.rs` | W2-ASSETS | `--texture`, `--mesh`, `--tfc`, `--mip`, `--all-mips`, `--lod`, `--output`, `--terrain-records`, `--scene-records` (used by `tools/prepare_level.py`, `tools/collision_volumes.py`) |
| `cmd/script.rs` | W2-SCRIPT | `--script-check`, `--disasm`, `--failures` |
| `cmd/classes.rs` | W2-REFLECT | `--class-check`, `--interface-compare` |
| `cmd/objects.rs` | W3-DECODE | `--object-dump` |
| `cmd/defs.rs` | W3-DEFS | `--def-dump <object path>` with `--cooked <dir>`, `[--overlay <file>]` (Rust-only) |
| `cmd/vm.rs` | W3-INTERP | `--run`, `--run-batch`, `--native`, `--native-args`, `--self`, `--arg`, `--steps`, `--class` |
| `cmd/census.rs` | W4-VMTOOLS | `--native-census`, `--vm-sweep`, `--native-selftest`, `--no-static`, `--top`, `--limit`, `--all` |
| `cmd/items.rs` | W4-REGISTRY | `--item-registry` with `--cooked <dir>`, `[--overlay <file>]` (Rust-only) |
| `cmd/items_roll.rs` | W4-ROLL | `--item-roll <balance path>` with `--cooked <dir>`, `--stage <n>`, `--seed <n>`, `[--count <n>]`, `[--playthrough <index>]`, `[--op-level <n>]`, `[--overlay <file>]` (Rust-only) |
| `cmd/items_stats.rs` | W4-STATS | `--item-stats` and `--item-card`, each with `--cooked <dir>` and either `--data <definition-data json file>` or the `--item-roll` modifiers, `[--overlay <file>]` (Rust-only) |
| `cmd/items_pools.rs` | W4-POOLS | `--item-pool <pool path>` with `--cooked <dir>`, `--stage <n>`, `--seed <n>`, `[--count <n>]`, `[--playthrough <index>]`, `[--op-level <n>]`, `[--overlay <file>]` (Rust-only) |
| `cmd/behavior.rs` | W5-BEHAVIOR | `--behavior-dump`, `--behavior-run` |
| `cmd/kismet.rs` | W5-KISMET | `--kismet-census`, `--kismet-run`, `--remote`, `--originator`, `--op` |
| `cmd/mover.rs` | W5-DIALOG | `--mover-event`, `--mover-probe` |
| `cmd/inventory.rs` | W5-EQUIP | `--inventory-move` |
| `cmd/mission.rs` | W6-MISSION | `--mission`, `--mission-run`, `--tick` |
| `cmd/slice.rs` | W6-FIRE | `--slice-run` |
| `cmd/combat.rs` | W6-COMBAT | `--combat-run` |

**Placeholder types.** Where a contract names a type whose owner lane lands later or in parallel (`ow_package::PackageStore`
for W1 lanes, `ow_defs::DefStore` and `ow_attr::AttributeSystem` for W2-VMAPI and W2-ATTRAPI, the `testkit` builders),
W0-SCAFFOLD puts a public struct of that name in the owner's file, with a doc line saying it is a placeholder and a
`pub fn new_empty() -> Self` constructor. The owner lane replaces the body, **keeps the name and `new_empty()`** (an
empty store, an empty resolver registry), and any method a dependent calls before then returns
`NotYet { LayerAbsent }`. W2-VMAPI's tests call `Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off)`.

---

## 3. Public interfaces

Signatures are the contract's shape, written for this design; lanes may refine names and borrow details inside the
review bar of section 7. All enums that may grow are `#[non_exhaustive]`.

### 3.1 `ow-core`

```rust
// notyet.rs (W0)
pub struct NotYet { pub what: Cow<'static, str>, pub missing: Missing, pub record: &'static str }
pub enum Missing {
    NotRead,                 // the rule is not read from native code or script yet
    NotPorted,               // exists in C++ or Python, not yet in Rust
    LayerAbsent(&'static str),  // the lower layer this needs is not built
    NeedsRealGame(u8),       // blocked on real-game question N (architecture.md section 12)
}
// ident.rs (W0): case-insensitive like an FName, first spelling kept for printing.
pub struct ObjectPath(..);   pub struct PackageName(..);
impl ObjectPath { pub fn parse(s: &str) -> Result<Self, IdentError>; pub fn package(&self) -> &str; pub fn as_str(&self) -> &str; }

// detmap.rs, ordmap.rs (W0)
pub type DetMap<K, V> = std::collections::BTreeMap<K, V>;  // only where the oracle's order is key order
pub struct OrderedMap<K, V> { .. }   // insertion order; insert() of an existing key keeps its first position and takes
                                     // the later value (weapon_gen PickPart, weapon_recipe.pick, Python dicts)
// parity.rs (W0)
pub enum ParityMode { Off, Reference }   // Reference: reproduce the reference implementation's stand-ins (C++ or Python)
pub fn parity_mode() -> ParityMode;      // read once from OW_PARITY ("reference" or unset). Called only by binaries
                                         // (ow-cli, ow-host main); library crates take a ParityMode value (edge test bans other calls)

// num.rs (W1-NUM)
pub fn f32r(x: f64) -> f64;                       // round to single, as weapon_gen F32() / weapon_recipe.f32()
pub fn pow_f64(base: f64, exp: f64) -> f64;       // the one double pow: the C runtime pow on the reference target
pub fn pow_f32(base: f32, exp: f32) -> f32;       // the one single pow (powf), progression.cpp's form
pub fn exp_f64(x: f64) -> f64; pub fn ln_f64(x: f64) -> f64; pub fn sqrt_f64(x: f64) -> f64;   // the Core natives' forms
pub fn sin_f64(x: f64) -> f64; pub fn cos_f64(..); pub fn tan_f64(..); pub fn asin_f64(..); pub fn acos_f64(..);
pub fn atan_f64(..); pub fn atan2_f64(y: f64, x: f64) -> f64;     // natives_core.cpp:158-168: double C runtime, then makeFloat
pub fn trunc_i32(x: f32) -> i32;                  // attribute path (interp.cpp:561): i32::MIN for NaN and out of range
pub fn trunc_i32_f64(x: f64) -> i32;              // weapon_gen's integer attributes, same rule from a double
pub fn trunc_i64_wrap_i32(x: f64) -> i32;         // VM natives (natives_core.cpp:103,106,123,170-171): int64 truncation,
                                                  // then makeInt's wrap to 32 bits (vm.cpp:51); the out-of-range int64 case
                                                  // follows MSVC cvttsd2si (i64::MIN) before the wrap
pub enum RoundingMode { Float = 0, HalfUp = 1, Floor = 2, Ceiling = 3 }   // AttributeInitializationDefinition, the game's values
pub fn round_mode(x: f32, m: RoundingMode) -> f32;
pub fn round_half_even(x: f64) -> f64;            // Python round(): loot_pools' pool Quantity (the oracle's choice, UNVERIFIED as a game rule)
pub fn card_round(x: f32, digits: i32) -> f32;    // scale and add the half in single precision (33.349998 -> 33.4)
// units.rs (W1-NUM)
pub struct Rotator { pub pitch: i32, pub yaw: i32, pub roll: i32 }  // 65,536 units per turn
pub const ROTATOR_UNITS_PER_DEGREE: f64 = 65536.0 / 360.0;
pub struct Uu(pub f32);    // Unreal units; Z-up vectors are plain [f32; 3] named in uu
// fmt.rs (W1-NUM): one function per format an oracle prints; each output module names the one it mirrors.
pub fn fmt_g(x: f64, precision: usize) -> String; // printf "%.<p>g" and ostream setprecision(p), any p
pub fn fmt_ostream(x: f64) -> String;             // ostream default (precision 6): cli.cpp --object-dump, vm.cpp describe()
pub fn fmt_f(x: f64, decimals: usize) -> String;  // printf "%f" (6) and "%.<n>f": interp.cpp:256, weapon_gen.cpp:952
pub fn fmt_to_string_f32(x: f32) -> String;       // std::to_string(float) = "%f": combat.cpp:150
pub fn fmt_shortest_cpp(x: f64) -> String;        // std::to_chars shortest form: weapon_gen.cpp:1694
pub fn fmt_py_repr(x: f64) -> String;             // Python float repr, as json.dumps writes it: item_registry.py:836
pub fn json_quote(s: &str) -> String;             // the C++ quote() escapes
pub fn json_quote_py(s: &str) -> String;          // json.dumps(ensure_ascii=False) escapes

// rand.rs (W1-RAND)
pub trait Rand15 { fn rand15(&mut self) -> u16; }  // 0..=32767, what C rand() returns
pub struct CrtRand { pub state: u32 }              // state*214013+2531011, bits 16..30; new(seed) = srand(seed); impl Rand15
pub trait ScriptRng {                              // what Object.Rand(int) and Object.FRand() draw from
    fn rand_limit(&mut self, limit: i32) -> i32;   // 0 for limit <= 0, as the C++
    fn frand(&mut self) -> f32;
}
impl ScriptRng for CrtRand { .. }                  // rand15 % limit and rand15 / 32767: the public UE3 appRand convention,
                                                   // UNVERIFIED for this executable (not read from it)
pub struct VmLcg { pub state: u32 }                // the C++ VM stand-in, ParityMode::Reference only: state*1664525+1013904223;
impl ScriptRng for VmLcg { .. }                    // rand_limit = (state>>8) % limit, frand = (state>>8) / 2^24 (natives_core.cpp:201-209)
pub enum CrtStream { LootPool, ItemRoll, AttributeVariance, AiDecision, FiringSpread, StatusEffect, Population, Dialog }
pub enum ScriptStream { Rand, FRand }              // two streams, as the C++ keeps two separate states; Off-mode split is a project choice, UNVERIFIED
pub struct GameRng { .. }
impl GameRng {
    pub fn from_world_seed(seed: u64, mode: ParityMode) -> Self;   // streams seeded by splitmix64(seed, stream id) -> u32;
                                                   // `mode` selects VmLcg for the script streams; never read from the env
    pub fn crt(&mut self, s: CrtStream) -> &mut CrtRand;
    pub fn script(&mut self, s: ScriptStream) -> &mut dyn ScriptRng;   // CrtRand, or VmLcg(12345)/VmLcg(54321) under Reference
}
pub fn frand_f64(r: &mut dyn Rand15) -> f64;       // rand15 / 32767 in double: the listed-pool probability draw (weapon_gen.cpp:1676)
pub fn weighted_draw(r: &mut dyn Rand15, total: f64) -> f64;  // total * rand15 / 32767, left to right: every weighted pick (weapon_gen.cpp:757)
pub fn frand_f32(r: &mut dyn Rand15) -> f32;       // the same in single; each rule names which its oracle uses
// HostRng (cosmetic randomness: particles, sound variation) lives in ow-host, which no game crate may depend on.

// sim.rs (W1-SIM): the clock only. Per-actor timers are the game's Timers array rules (NATIVE_ENGINE_CORE.md,
// "Actor timers": own Count grown by TimerTimeDilation * delta, due when Count > Rate strictly, a loop fires
// floor(Count / Rate) times, pause, culling during the pass with the array length re-read, order from the actor tick),
// so they live on the actor in ow-world (W5-WORLD), not in a generic queue here.
pub struct SimClock { .. }                         // tick counter u64, time f64, TimeSeconds f32 accumulated as UE3 does
impl SimClock {
    pub fn advance(&mut self, real_dt: f32) -> Step;   // Step { dt, real_dt, tick } (#[non_exhaustive]); where CustomTimeDilation and the world's
                                                       // TimeDilation enter the actor's delta is not located in the record
                                                       // (NATIVE_ENGINE_CORE.md, Open): the dilated dt is UNVERIFIED
    pub fn time_seconds(&self) -> f32;
    pub fn set_time_dilation(&mut self, d: f32) -> Result<(), ClockError>;   // negative/NaN/inf refused, old value kept
}
pub enum ClockError { BadTimeDilation(f32) }   // #[non_exhaustive]
pub enum TickGroup { PreAsyncWork, DuringAsyncWork, PostAsyncWork, PostUpdateWork, EffectsUpdateWork }  // Core.upk Object.ETickingGroup values 0-4 (TG_MAX = 5)
```

### 3.2 `ow-container`, `ow-package`

```rust
// ow-container (signature fixed by W0)
pub fn decode_package(input: &[u8]) -> Result<Vec<u8>, ContainerError>;
pub enum ContainerError { Truncated { at: usize, need: usize }, BadMagic, BadChunk { index: usize, reason: &'static str },
                          Lzo { at: usize, reason: &'static str }, BadPartialTable { reason: &'static str },
                          TrailingPartialBytes, BadHeader { reason: &'static str },
                          InPartialChunk { index: usize, at: usize, error: Box<ContainerError> }, NotYet(NotYet) }
// decode_package handles both forms, as container.cpp does: a fully compressed container, and a package whose summary
// carries a partial compression table (codec 2; chunk bounds checked against the summary and each other; each chunk a
// nested container whose decoded size must match; no trailing bytes after the last chunk). tests/package_test.py:64-79.

// ow-package
pub struct Reader<'a> { bytes: &'a [u8], pos: usize, limit: usize }
impl Reader<'_> { pub fn require(&self, n: usize) -> Result<(), ReadError>; pub fn u32(&mut self) -> Result<u32, ReadError>;
                  pub fn i32(..); pub fn string(..); pub fn name(&mut self, count: i32) -> Result<(i32, i32), ReadError>;
                  pub fn reference(&mut self, imports: i32, exports: i32) -> Result<i32, ReadError>; pub fn with_limit(..) }
pub struct ObjectEntry { pub class: i32, pub super_: i32, pub outer: i32, pub archetype: i32, pub name: String,
                         pub size: i32, pub offset: i32, pub class_package: String, pub class_name: String }
pub struct Package { pub source: PathBuf, pub name: String /* the C++ file stem */, pub version: u16, pub licensee: u16,
                     pub data: Vec<u8>, pub names: Vec<String>, pub imports: Vec<ObjectEntry>, pub exports: Vec<ObjectEntry> }
impl Package { pub fn load(path: &Path) -> Result<Arc<Package>, PackageError>; pub fn from_bytes(..);
               pub fn from_decoded(source: PathBuf, name: String, data: Vec<u8>) -> Result<Package, PackageError>;
               pub fn package_name(&self) -> Result<PackageName, IdentError>;   // typed; W2-STORE keys identity on it
               pub fn path_of(&self, reference: i32) -> Result<String, PackageError>;
               pub fn find_export(&self, object_path: &str) -> Result<Option<i32>, PackageError>; }  // Err: cyclic chain
// As shipped by W1-PKG (decision 2026-11-15-03): name, from_decoded, package_name, find_export and Reader::name differ
// from revision 3 here, each to mirror the C++ reader.
pub type PackageId = u32;                          // index into PackageStore, stable for one store (placed by W0, final)
// Package and PackageStore are Send + Sync (Arc<Package>), so threaded tools can share package bytes (D18).

// testkit.rs (feature "test-support"; placeholder W0, real W1-TESTKIT): synthetic packages for Rust tests, rule 9.
pub struct PackageBuilder { .. }   // header 832/46, names, imports, exports, payloads, the same layout package_test.py writes
impl PackageBuilder { pub fn tag(&mut self, name: &str, kind: &str, value: &[u8]) -> Vec<u8>; .. }  // tags need the name
                                   // table: the tests/ tag() helpers' byte form; tag_detail, tag_full, typed tags too
pub fn container(data: &[u8], block: usize) -> Vec<u8>;        // a valid LZO container made of literal runs only
pub fn partial(builder: &mut PackageBuilder) -> Vec<u8>;       // build_partial(): the package_test.py:64-79 shape
// testkit/reflect.rs (placeholder; W2-REFLECT): property, struct, class and enum exports, class_body().
// testkit/script.rs (placeholder; W2-SCRIPT): function exports and the Asm bytecode writer.

// props.rs (W2-PROPS): the serialized tree, references still package-relative.
pub enum PropValue { Int(i32), Float(f32), Bool(bool), Byte(u8), Enum(String), Name(String), Str(String),
                     Object(i32), Struct { ty: String, fields: Vec<PropEntry> }, Array(Vec<PropValue>),
                     Raw(Vec<u8>) /* a value the C++ walker leaves undecoded (unknown tag type, untyped array): kept, never guessed */,
                     Delegate { object: i32, function: String } /* reserved for W3-DECODE; never produced: delegate tags are Raw */ }
pub struct PropEntry { pub name: String, pub index: i32, pub value: PropValue,
                       pub tag: Option<Tag> /* None for a fixed-layout struct field (Vector, Box, ...) */ }
pub struct Tag { pub ty: String, pub type_name: Option<String>, pub offset: usize, pub size: i32, pub array: Option<ArrayTag> }
pub struct ArrayTag { pub count: i32, pub element: Option<ElementType> }
pub enum ElementType { Int, Float, Name, Str, Object, Byte, Struct(String),
                       Other(String) /* a schema text the C++ accepts: an empty array decodes, an element fails */ }
pub trait ArrayTyper { fn element(&self, owner_class: &str, property: &str) -> Option<ElementType>; }
                                                   // owner_class: the export's class object name (`Class` for class 0),
                                                   // the struct name inside a struct; SchemaTyper ignores it
pub fn property_range(pkg: &Package, export: i32, start: usize) -> Result<(usize, usize), PropError>;
pub fn read_tagged(pkg: &Package, export: i32, start: usize, typer: &dyn ArrayTyper)
    -> Result<TaggedProps, PropError>;            // TaggedProps { entries, end: usize }  (end = start of any untagged tail,
                                                   // from the export's payload start)
pub enum PropError { Package(PackageError), InStruct { ty: String, error: Box<PropError> } /* cause via source() */,
                     /* the walker's eleven C++ texts */ .. }
pub struct SchemaTyper { .. }                      // the --array-schema file; retired by W3-DECODE's gate
// As shipped by W2-PROPS (decision 2026-11-15-04): PropError, PropEntry.tag, ElementType::Other and the owner text
// differ from revision 3 here, each to carry what the C++ walker prints.

// store.rs (W2-STORE)
pub struct PackageStore { .. }
impl PackageStore { pub fn new(cooked_root: PathBuf) -> Self; pub fn scan(&mut self) -> Result<(), StoreError>;
    pub fn load(&mut self, name: &PackageName) -> Result<PackageId, StoreError>; pub fn get(&self, id: PackageId) -> &Package;
    pub fn resolve(&mut self, from: PackageId, reference: i32) -> Result<Option<(PackageId, i32)>, StoreError>;
    pub fn resolve_path(&mut self, package: &PackageName, path: &str) -> Result<Option<(PackageId, i32)>, StoreError>;
    pub fn scan_order(&self) -> &[PathBuf]; }
```

### 3.3 `ow-object` (contract W1-OBJ; the most important freeze)

Decision D14 below: properties live in **slots by class layout**, not a name-keyed bag. A class's layout is its super
chain's slots followed by its own declarations, so a `PropId` from the declaring class is the same slot in every
subclass. Names are interned once; the interpreter resolves `InstanceVariable` operands to slots at link time. This
is our own layout, not the original's memory offsets (rule 4 is not touched: no native offset is used).

A static array (`array_dim > 1`) takes `array_dim` consecutive slots; heap access is by `PropAddr { prop, index }`, with
`index < array_dim` checked (an overlay patch `Prop[3].Field` and the interpreter's static-array element both resolve to
one). Shared values use `Rc`, so `ObjectSpace`, `Vm` and `World` are deliberately `!Send` (D18): the simulation lives on
one thread and talks to the host through `Send` message types; tools that want threads build one space per thread over
shared `Arc<Package>` bytes. A compile-time assertion in `ow-object` stops the build if `Value`, `ClassTable`, `Heap` or
`ObjectSpace` becomes `Send`.

*Shipped shapes (W1-OBJ, PR #282, review r2; replaces the revision 3 sketch).* The module path is part of each name
(`lib.rs` re-exports nothing): `ids::`, `value::`, `names::`, `layout::`, `heap::`. Every enum below that may grow is
`#[non_exhaustive]`, and so are `PropertyDecl` and `Object` (built with their constructors).

```rust
// ids.rs
pub struct ObjRef { index: u32, generation: u32 }          // made only by the heap; .index(), .generation()
pub struct ClassId(u32); pub struct StructId(u32); pub struct EnumId(u32); pub struct FuncId(u32);  // ::new(raw), .raw(), .index()
pub struct NameId { index: u32, number: u32 }              // FName: base entry + instance number (0 = none, n = "_<n-1>");
                                                           // made only by NameTable (one id per text); .index(), .raw_index(),
                                                           // .number(); NameId::NONE
pub struct PropId { pub slot: u32, pub declared_in: ClassId }     // first slot of the property
pub struct PropAddr { pub prop: PropId, pub index: u32 }         // index 0 for a scalar; < array_dim, checked;
                                                                 // PropAddr::scalar(p), PropAddr::element(p, i), From<PropId>

// value.rs
pub enum Value { None, Int(i32) /* wrapping arithmetic */, Float(f32), Bool(bool), Byte(u8), Str(Rc<str>) /* UTF-8 */,
                 Name(NameId), Object(Option<ObjRef>), Class(Option<ClassId>), Interface(Option<ObjRef>),
                 Struct(Rc<StructValue>), Array(Rc<Vec<Value>>), Delegate(Option<DelegateValue>) }   // Rc::make_mut = copy on write
pub struct StructValue { pub ty: StructId, pub fields: Vec<Value> }   // StructLayout entry order; static-array fields expanded
pub struct DelegateValue { pub object: Option<ObjRef>, pub function: NameId }
impl Value { pub fn string(&str); pub fn array(Vec<Value>); pub fn structure(StructId, Vec<Value>); pub fn kind_name(&self);
             pub fn elements(&self); pub fn array_mut(&mut self); pub fn as_struct(&self); pub fn struct_mut(&mut self);
             pub fn truth(&self, names: &NameTable) -> bool /* C++ Value::truth */ }
pub fn same_value(a: &Value, b: &Value) -> bool;   // UnrealScript ==, the C++ sameValue with the documented differences:
                                                   // mixed kinds symmetric; a null class equals None, Object(None) and
                                                   // Interface(None) in both orders; delegates compare object and function

// names.rs: case-insensitive (ASCII fold), first spelling kept, None = entry 0, trailing "_<n>" split into the number
impl NameTable { pub fn new() -> Self; pub fn intern(&mut self, s: &str) -> NameId;
                 pub fn intern_number(&mut self, base: &str, number: u32) -> NameId;   // goes through the text: one id per text
                 pub fn with_number(&mut self, n: NameId, number: u32) -> NameId;      // n's base text, another number
                 pub fn find(&self, s: &str) -> Option<NameId>; pub fn text(&self, n: NameId) -> Cow<'_, str>;
                 pub fn base_text(&self, n: NameId) -> &str; pub fn get(&self, n: NameId) -> Option<Cow<'_, str>>;
                 pub fn len(&self) -> usize; pub fn is_empty(&self) -> bool; }

// layout.rs
pub enum PropKind { Int, Float, Bool, Byte { enum_: Option<EnumId> }, Str, Name, Object { class: ClassId },
                    Class { meta: ClassId }, Interface { class: ClassId }, Struct(StructId), Array(Box<PropKind>),
                    Delegate, FloatAttribute, IntAttribute, ByteAttribute { enum_: Option<EnumId> } }   // .kind_name(), .is_attribute()
pub struct AttrCompanions { pub value: PropId, pub base: PropId, pub stack: PropId }   // NATIVE_BYTECODE_OPCODES typeRef/2/3
pub struct PropertyDecl { pub name: NameId, pub kind: PropKind, pub flags: u64, pub array_dim: i32,
                          pub attr: Option<AttrCompanions>, pub source: Option<(PackageId, i32)> }
impl PropertyDecl { pub fn new(name, kind); with_flags; with_array_dim; with_attr; with_source;
                    pub fn is_stack_attribute(&self) /* bit 63 */; is_plain_attribute /* bit 62 */; notifies /* bit 61 */;
                    is_param; is_out; is_optional; is_return; }   // consts OPTIONAL, PARAM, OUT, RETURN, STACK_ATTRIBUTE,
                                                                   // PLAIN_ATTRIBUTE, NOTIFY
pub enum LayoutError { BadPath { path, error: IdentError }, AlreadyDefined { path }, UnknownClass(ClassId), UnknownStruct(StructId),
                       UnknownEnum(EnumId), NotDefined(ClassId), SuperNotDefined { class, super_ }, UnknownSuperStruct { owner, super_ },
                       StructNotDefined(StructId), BadDefault { owner, reason }, DuplicateName { owner, name },
                       BadArrayDim { owner, name, dim }, TooManySlots { owner }, NotInClass { class, prop }, NotAProperty(PropId),
                       IndexOutOfRange { prop, index, dim }, BadAttribute { owner, name, reason } }   // Display + Error

pub struct ClassLayout { .. }   // path() -> &ObjectPath, super_class(), is_defined(), first_slot(), slot_count(),
                                // own_decls() -> impl Iterator<Item = (u32, &PropertyDecl)>
pub struct StructLayout { .. }  // path(), super_struct(), slot_count() (entries), fields() -> DoubleEndedIterator<(u32, &PropertyDecl)>,
                                // field(name) -> Option<(u32, &PropertyDecl)>, entry_field(entry) -> Option<(u32, &PropertyDecl)>,
                                // zero() -> &Value (no defaults), own_defaults() -> &[(u32, Value)]
pub struct EnumLayout { .. }    // path(), values() -> &[NameId], index_of(name) -> Option<usize>

pub struct ClassTable { .. }
impl ClassTable {
    pub fn new() -> Self;
    pub fn declare(&mut self, path: &str) -> Result<ClassId, LayoutError>;        // reserve an id: mutual references
    pub fn define(&mut self, path: &str, super_: Option<ClassId>, props: Vec<PropertyDecl>) -> Result<ClassId, LayoutError>;
    pub fn link_attribute(&mut self, c: ClassId, value: NameId, base: NameId, stack: NameId) -> Result<AttrCompanions, LayoutError>;
    pub fn declare_struct(&mut self, path: &str) -> Result<StructId, LayoutError>; // structs holding arrays of themselves
    pub fn define_struct(&mut self, path: &str, super_: Option<StructId>, fields: Vec<PropertyDecl>) -> Result<StructId, LayoutError>;
    pub fn define_enum(&mut self, path: &str, values: Vec<NameId>) -> Result<EnumId, LayoutError>;
    // Struct defaults are set through ObjectSpace (reference-checked); the table's own setters are crate-private.
    pub fn struct_defaults(&self, s: StructId) -> Result<Value, LayoutError>;   // computed per call: zero, inner defaults, own entries
    pub fn zero_value(&self, kind: &PropKind) -> Value;                         // no struct defaults
    pub fn default_value(&self, kind: &PropKind) -> Result<Value, LayoutError>; // the C++ Runtime::zeroValue
    pub fn zero_slots(&self, c: ClassId) -> Result<Vec<Value>, LayoutError>;
    pub fn default_slots(&self, c: ClassId) -> Result<Vec<Value>, LayoutError>; // what Heap::alloc starts from
    pub fn layout(&self, c: ClassId) -> &ClassLayout;  pub fn get_layout(&self, c: ClassId) -> Option<&ClassLayout>;
    pub fn struct_layout(&self, s: StructId) -> Option<&StructLayout>;  // None while only declared
    pub fn enum_layout(&self, e: EnumId) -> Option<&EnumLayout>;
    pub fn is_child_of(&self, c: ClassId, base: ClassId) -> bool;   // false for an id the table did not make
    pub fn prop(&self, c: ClassId, name: NameId) -> Option<PropId>;  pub fn decl(&self, p: PropId) -> Option<&PropertyDecl>;
    pub fn resolve(&self, c: ClassId, a: PropAddr) -> Result<usize, LayoutError>;   // the checked slot
    pub fn props(&self, c: ClassId) -> Result<Vec<(PropId, &PropertyDecl)>, LayoutError>;
    pub fn fits(&self, kind: &PropKind, v: &Value) -> bool;          // shape only, references not followed
    pub fn by_path(&self, path: &str) -> Option<ClassId>;  struct_by_path;  enum_by_path;  len;  is_empty;
}
// Order of definition: super class before subclass; super struct and every by-value struct before what holds it
// (a by-value struct cycle cannot be built); a reference or an array element needs only a declaration.

// heap.rs
pub enum Origin { Runtime, Export { package: PackageId, index: i32 }, Default { class: ClassId },
                  Resource { package: PackageId, index: i32 } }   // Origin::is_instance: Runtime, Export
pub struct Object { pub class: ClassId, pub name: NameId, pub outer: Option<ObjRef>, pub slots: Vec<Value>,
                    pub state: Option<NameId>, pub destroyed: bool, pub origin: Origin }   // Object::new(class, name, outer, slots, origin)
pub enum Step { Element(u32), Field(u32) }   // into an array element / a struct entry
pub enum HeapError { Stale(ObjRef), Layout(LayoutError), WrongKind { expected, found }, WrongClass { expected, found },
                     WrongStruct { expected, found }, WrongFieldCount { ty, expected, found }, SlotMissing { object, slot },
                     BadPath { at, reason }, FreedTwice(ObjRef) }   // Display + Error; From<LayoutError>
pub struct Heap { .. }
impl Heap {
    pub fn new() -> Self;  pub fn len(&self) -> usize /* not freed, destroyed included */;  pub fn is_empty(&self) -> bool;
    pub fn alloc(&mut self, classes: &ClassTable, class: ClassId, name: NameId, outer: Option<ObjRef>, origin: Origin)
        -> Result<ObjRef, HeapError>;   // a defined class; an outer that is not freed; default_slots, each checked as a
                                        // write (a stale or wrong-class struct default is refused, never copied)
    pub fn get(&self, o: ObjRef) -> Result<&Object, HeapError>;  pub fn get_mut(&mut self, o: ObjRef) -> Result<&mut Object, HeapError>;
    pub fn contains(&self, o: ObjRef) -> bool;
    pub fn read(&self, classes: &ClassTable, o: ObjRef, a: PropAddr) -> Result<&Value, HeapError>;
    pub fn write(&mut self, classes: &ClassTable, o: ObjRef, a: PropAddr, v: Value) -> Result<(), HeapError>;   // kind- and index-checked
    pub fn write_in(&mut self, classes: &ClassTable, o: ObjRef, a: PropAddr, path: &[Step], v: Value) -> Result<(), HeapError>;
                                                                        // Arr[i] = x, Location.X = 1: checks only the new value
    pub fn slot_mut(&mut self, classes: &ClassTable, o: ObjRef, a: PropAddr) -> Result<&mut Value, HeapError>;  // address checked
    pub fn check_value(&self, classes: &ClassTable, kind: &PropKind, v: &Value) -> Result<(), HeapError>;
    pub fn destroy(&mut self, o: ObjRef) -> Result<(), HeapError>;      // pending kill: readable, skipped by live()
    pub fn free(&mut self, o: ObjRef) -> Result<Object, HeapError>;     // the GC step in the heap: nulls every reference to o
                                                                        // held by objects (VM frames are the VM's), bumps the
                                                                        // generation; does NOT reach struct defaults
    pub fn free_many(&mut self, objects: &[ObjRef]) -> Result<Vec<Object>, HeapError>;   // one heap walk for the set;
                                                                        // stale or repeated handle: refused, nothing changed
    pub fn live<'a>(&'a self, classes: &'a ClassTable, base: ClassId) -> impl Iterator<Item = ObjRef> + 'a;
                                                                        // instances only, creation order, not destroyed
}
pub struct ObjectSpace { pub heap: Heap, pub classes: ClassTable, pub names: NameTable }   // what offline tools and the VM share
impl ObjectSpace {
    pub fn new() -> Self;
    alloc(class, name: &str, outer, origin); read(o, a); write(o, a, v); write_in(o, a, path, v); slot_mut(o, a);
    prop(class, name: &str); live(base);   // the Heap calls with this space's tables
    pub fn free(&mut self, o: ObjRef) -> Result<Object, HeapError>;                         // Heap::free + clears o out of
    pub fn free_many(&mut self, objects: &[ObjRef]) -> Result<Vec<Object>, HeapError>;      // every struct default
    pub fn set_struct_default(&mut self, s: StructId, entry: u32, v: Value) -> Result<(), HeapError>;   // one entry (one tag),
    pub fn set_struct_defaults(&mut self, s: StructId, v: Value) -> Result<(), HeapError>;              // or all; check_value first
}
// reflect.rs (W2-REFLECT), decode.rs (W3-DECODE)
pub fn load_code_packages(store: &mut PackageStore, names: &mut NameTable, classes: &mut ClassTable) -> Result<CodeIndex, ReflectError>;
pub fn decode_class_body(pkg: &Package, export: i32) -> Result<ClassBody, ReflectError>;   // ends exactly at the serial size
pub struct ReflectionTyper<'a> { .. }  impl ArrayTyper for ReflectionTyper<'_> { .. }
pub fn to_value(tree: &PropValue, decl: &PropertyDecl, cx: &mut ValueCx) -> Result<Value, DecodeError>;
pub enum PropertyPrefix { Ordinary = 4, PlacedActor = 26, Component = 8 }   // copied from the C++ (UNVERIFIED as a rule, ROADMAP Phase 0)
```

Arena semantics differ from the C++ `shared_ptr` bag in one place: an unreferenced object stays alive until `free`.
`destroy` is UE3's pending kill; `free` is the garbage-collection step and clears the heap's references to the object
in the same call, as UE3's collector clears references when it removes an object (when that happens after
`Destroy()` in BL2 is UNVERIFIED). Each `free` walks the whole heap once; `free_many` clears a set in one walk, which is
what a world should use for a tick's dead objects. Struct defaults live in the class table, which the heap cannot
reach, so the world frees through `ObjectSpace::free` / `free_many`, which also clear them; `alloc` refuses a default
left stale by a bare `Heap::free`. Freeing an outer leaves its inner objects with no outer (UE3 keeps or collects
inners with their outer: UNVERIFIED which applies). For actors (what the iterators walk) this matches UE3, where actors
live until destroyed; who calls `free` and when is for the world and VM lanes. Strings are UTF-8 while the game's
`FString` is UTF-16: which unit `Len`/`Mid` count in is W2-VMAPI's / W3-CORENAT's decision (UNVERIFIED). Decision file
2026-11-15-02 records these differences.

### 3.4 `ow-defs` (W3-DEFS)

```rust
pub struct LoadSet { pub packages: Vec<PackageName> }          // the game's loaded set, in load order
pub enum IdentityPolicy { FirstInScanOrder /* the tools' convention; parity; UNVERIFIED as a game rule */, Loaded(LoadSet) }
pub struct Overlay { pub name: String, pub kind: OverlayKind, pub patches: Vec<Patch> }
pub enum OverlayKind { LiveCapture, Hotfix, TextMod, Test }
pub struct Patch { pub object: ObjectPath, pub property: String /* "Prop[3].Field" */, pub value: String /* text form, typed by reflection */ }
pub struct DefStore { .. }
impl DefStore {
    pub fn new(store: PackageStore, policy: IdentityPolicy) -> Self;
    pub fn find(&mut self, w: &mut ObjectSpace, path: &ObjectPath) -> Result<Option<ObjRef>, DefError>;   // lazy load + overlays
    pub fn resolve_ref(&mut self, w: &mut ObjectSpace, from: PackageId, reference: i32) -> Result<Option<ObjRef>, DefError>;
    pub fn owners(&self, path: &ObjectPath) -> Vec<(PackageName, i32)>;      // every holder, for audits
    pub fn push_overlay(&mut self, w: &mut ObjectSpace, o: Overlay) -> Result<OverlayId, DefError>;  // also patches loaded objects
    pub fn origin(&self, obj: ObjRef, prop: PropId) -> ValueOrigin;          // Cooked | Overlay(id)
    pub fn raw_tail(&self, obj: ObjRef) -> Option<&[u8]>;                    // untagged bytes after the tags (behavior providers)
    pub fn exports_of_class(&mut self, class_path: &str) -> Result<Vec<ObjectPath>, DefError>;  // scan order, for censuses
    pub fn key(&mut self, w: &ObjectSpace, class_path: &str, prop: &str) -> Result<PropId, DefError>; // typed key, cached
}
```

### 3.5 `ow-attr`

```rust
// math.rs, stack.rs (W1-ATTRMATH): pure, f32, no objects.
pub enum ModifierType { Scale = 0, PreAdd = 1, PostAdd = 2 }
pub struct ModifierId(pub u64);                     // placed by W0, final: the AttributeModifier identity (the caller maps objects to ids)
pub struct Modifier { pub kind: ModifierType, pub value: f32, pub id: ModifierId }
pub enum Combined { Float(f32), Int(i32), Byte(u8) } // ow-attr's own result type, so this file needs nothing from ow-object
pub fn combine(base: f32, mods: &[Modifier], form: CombineForm, kind: NumKind) -> Combined;   // no clamp, stack order
pub enum CombineForm {      // the two oracles disagree in precision; section 5.3 settles which the game uses
    Vm5F,      // interp.cpp:546-561: sums and result all f32, (base+ΣPre) * ((1+Σup)/(1-Σdown)) + ΣPost (Σdown <= 0);
               // int/byte: trunc_i32 (i32::MIN out of range), a byte keeps the low 8 bits
    WeaponGen, // weapon_gen.cpp:871-875 + 1018-1030: each sum rounded to f32 per add (Σdown kept as Σ|down|), factor
               // (1+Σup)/(1+Σ|down|) in double, one f32 rounding at the end; int: trunc(factor*(trunc(base)+ΣPre)+ΣPost) in double
}
pub enum NumKind { Float, Int, Byte }
pub struct Stack(Vec<Modifier>);  impl Stack { pub fn add(&mut self, m: Modifier) -> AddOutcome /* refuses a duplicate id */;
                                               pub fn remove_all(&mut self, id: ModifierId) -> usize; }
pub fn plain_apply(stored: f32, m: &Modifier) -> f32;  // plain attribute: Scale multiplies raw (no "1 +"); remove is a no-op
pub enum BaseValueMode { Sets = 0, Adds = 1, Scales = 2, FormulaMinusBase = 3 }
pub fn formula(multiplier: f32, level: f32, power: f32, offset: f32, p: PowForm) -> f32;  // M * (L^P + O), power skipped at 1
pub enum PowForm { Single /* progression.cpp */, DoubleThenSingle /* weapon_gen.cpp, VM ** */ }   // see section 5.3
// Both forms stay parameters of the frozen API even after 5.3 settles the game's form (callers then pass the settled one).
pub fn combine_base(mode: BaseValueMode, base: f32, def_value: f32) -> f32;

// api.rs, registry.rs (W2-ATTRAPI). Shipped shapes (W2-ATTRAPI, review r1; replaces the revision 3 sketch): module path
// ow_attr::api for the traits and data, ow_attr::registry for AttributeSystem and let_base.
// State that outlives one evaluation is the WORLD's (the global slots: one process-wide pair in the game; the
// generators), so a nested evaluation started by attribute_changed (EvalCx::new(world).with_forms(forms)) is the
// same context. The only per-call state is EvalCx::forms, which attribute_changed receives.
pub trait AttrWorld {
    fn parts(&mut self) -> WorldParts<'_>;            // one split borrow: both halves at once (DefStore lookups need both)
    fn globals(&mut self) -> Result<ObjRef, AttrError>;   // NoContextNeeded target; never None (NATIVE_ATTRIBUTES 5,
                                                      // progression.cpp:84): a world without one -> NotYet { LayerAbsent }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError>;   // Ok(None) only in a world modelling no game info
                                                      // (offline tool, unit test); the VM world: Some or an error
    fn engine_query(&mut self, q: EngineQuery, source: ObjRef) -> Result<Option<ObjRef>, AttrError>;  // default: NotYet { LayerAbsent("ow-world") }
    fn global_slots(&mut self) -> &mut GlobalSlots;   // GlobalAttributeValueResolver's slots, owned by the world
    fn attribute_changed(&mut self, owner: ObjRef, value_prop: PropId, forms: NumForms) -> Result<(), AttrError>;  // notify-flag properties only
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError>;   // none: NotYet (progression.cpp:209 refuses)
}
// Adapter rule: an adapter that overrides rng() (a roll's one generator) must not hand attribute_changed to a world that
// runs script with its own generators; it runs the notification against itself, or runs no script.
#[non_exhaustive] pub struct WorldParts<'a> { pub space: &'a mut ObjectSpace, pub defs: Option<&'a mut DefStore> }
impl WorldParts<'_> { pub fn new(space, defs) -> Self; pub fn defs(&mut self) -> Result<&mut DefStore, AttrError> /* None: NotYet { LayerAbsent("ow-defs") } */ }
#[non_exhaustive] pub enum EngineQuery { ResourcePool { resource: ObjRef }, EquippedInventory { location: u8 },
    Inventory { definition: ObjRef, equipped_only: bool }, PlayerController, AttributeContextSource }  // W3-ATTRCTX adds more
// Implementations: ow_vm::Vm (the running world; attribute_changed dispatches the class's notification, which may run
// script, because the Vm is borrowed whole), ow_balance::OfflineWorld (tools and tests), and ow_items::RollWorld, an
// adapter that forwards everything to an inner world except rng() and attribute_changed: rng() returns the roll's one
// CrtRand for every stream, and attribute_changed forwards only when the inner world runs no script (OfflineWorld);
// over a world that may run script (the Vm) it answers NotYet { NotPorted }, so a nested evaluation can never draw from
// the inner world's generators (the adapter rule above).
#[non_exhaustive] #[derive(Clone, PartialEq)]   // not Eq: BalanceError and VmError carry Attr(AttrError)
pub enum AttrError { NotYet(NotYet), Heap(HeapError), Layout(LayoutError), BadData { reason: Cow<'static, str> },
                     DuplicateResolver { class: String }, World(String) /* the world's own error as text */ }  // W3-DEFS adds Def(DefError)
pub struct GlobalSlots { pub experience_point_test_level: f32 /* slot 0 */, pub badass_token_test_rank: f32 /* slot 1 */ }
                                    // get(i): 0.0 beyond slot 1; set(i): NotYet { NotRead } beyond slot 1
pub struct NumForms { pub pow: PowForm, pub combine: CombineForm }   // defaults DoubleThenSingle, WeaponGen (section 5.3)
pub struct EvalCx<'a> { pub world: &'a mut dyn AttrWorld, pub forms: NumForms }   // no RNG or slots field: both are the world's
impl<'a> EvalCx<'a> { pub fn new(world: &'a mut dyn AttrWorld) -> Self; with_forms(..) }
// Resolvers get the calling system, so a resolver can resolve another definition's chain with the same cx.
pub trait ContextResolver { fn resolve(&self, sys: &AttributeSystem, resolver: ObjRef, attr: ObjRef, source: Option<ObjRef>, cx: &mut EvalCx) -> Result<Option<ObjRef>, AttrError>; }
pub trait ValueResolver {
    fn value(&self, sys: &AttributeSystem, resolver: ObjRef, context: ObjRef, previous: f32, cx: &mut EvalCx) -> Result<f32, AttrError>;   // previous: the chain's fold
    fn base_value(..same..) -> Result<f32, AttrError> { self.value(..) }                                     // class default
    fn storage(&self, sys: &AttributeSystem, resolver: ObjRef, context: ObjRef, cx: &mut EvalCx) -> Result<Option<Storage>, AttrError> { Ok(None) } // writable: ObjectProperty only
}
#[non_exhaustive] pub enum Storage { Stack { owner: ObjRef, companions: AttrCompanions, kind: NumKind }, Plain { owner: ObjRef, prop: PropId, kind: NumKind } }
pub struct InitData { pub base_value_constant: f32, pub base_value_attribute: Option<ObjRef>, pub initialization_definition: Option<ObjRef>,
                      pub base_value_scale_constant: f32 }   // AttributeInitializationData; InitData::constant(v)
pub struct ExprData { pub attribute_operand1: Option<ObjRef>, pub comparison_operator: Comparison, pub operand2_usage: Operand2Usage,
                      pub attribute_operand2: Option<ObjRef>, pub constant_operand2: f32 }   // AttributeExpressionData
pub enum Comparison { EqualTo = 0, .., GreaterThanOrEqual = 5 }  pub enum Operand2Usage { PreferAttribute = 0, Multiply = 1 }
pub enum ExprMode { And = 0, Or = 1 }
#[non_exhaustive] pub struct EffectData { pub attribute_to_modify: Option<ObjRef>, pub modifier_type: ModifierType, pub base_modifier_value: InitData }  // ::new
pub struct AppliedEffect { pub context: ObjRef, pub attribute: ObjRef, pub modifier: ObjRef }   pub struct Applied(pub Vec<AppliedEffect>);
pub struct AttrValue { pub context: ObjRef, pub value: f32 }
pub fn modifier_id_of(modifier: ObjRef) -> ModifierId;   // generation << 32 | index: the AttributeModifier object's identity
// Add and remove take the AttributeModifier OBJECT (the stack holds object references; its Type and Value are read at
// each recompute) and answer an outcome, not a count: a plain attribute's remove answers true and changes nothing
// (NATIVE_ATTRIBUTES section 3), which a count of 0 would read as false. .succeeded() is the native's bool.
// Chain rule (NATIVE_ATTRIBUTES section 1): the value resolvers are asked in order while the previous one succeeded; the
// outcome is the LAST resolver asked; nothing is rolled back, so a failing outcome means the chain failed, not that no
// storage changed. Implementations add no rollback. (With one resolver a failure leaves its storage as it was.)
#[non_exhaustive] pub enum AddModifierOutcome { Added, Duplicate, AppliedInPlace /* plain attribute */, Refused(Refusal) }
#[non_exhaustive] pub enum RemoveModifierOutcome { Removed { count: usize }, NotPresent, PlainNoOp /* true */, Refused(Refusal) }
#[non_exhaustive] pub enum Refusal { NoContext, NoValueResolvers, NullResolver, NotWritable, NotAnAttribute }
pub struct AttributeSystem { .. }   // immutable registry keyed by the EXACT resolver class path (a subclass is not served by
                                    // its parent's entry); unknown class -> NotYet { NotRead }
                                    // held as Rc<AttributeSystem>; a caller clones the Rc, then lends the world:
                                    //   let attrs = vm.attrs.clone(); attrs.evaluate_init(&d, src, None, &mut EvalCx::new(&mut vm))?;
                                    // (no borrow of vm.attrs is live while vm is lent, so no E0502)
// Every entry point below is a one-line delegate into its owner's file (section 2.3): eval.rs, expr.rs (W3-ATTREVAL),
// effects.rs, storage.rs (W3-ATTREFFECTS); standard() calls value_resolvers::register (W3-ATTREVAL) and
// context_resolvers::register (W3-ATTRCTX). W2-ATTRAPI placed those bodies as NotYet { NotPorted } stubs; the W3 lanes
// fill their own files and do not edit registry.rs.
impl AttributeSystem {
    pub fn new_empty() -> Self;                                                       // the placeholder's constructor, kept
    pub fn standard() -> Result<Self, AttrError>;                                     // every resolver class the notes read
    pub fn with_value_resolver(self, class_path: &str, r: impl ValueResolver + 'static) -> Result<Self, AttrError>;   // twice: refused
    pub fn with_context_resolver(self, class_path: &str, r: impl ContextResolver + 'static) -> Result<Self, AttrError>;
    pub fn value_resolver(&self, space: &ObjectSpace, resolver: ObjRef) -> Result<&dyn ValueResolver, AttrError>;
    pub fn context_resolver(&self, space: &ObjectSpace, resolver: ObjRef) -> Result<&dyn ContextResolver, AttrError>;
    // `override_source` is the script's OptionalOverrideContextSource, tried first. A script "Object ContextSource" may
    // be None, so sources are Option.
    // eval.rs (W3-ATTREVAL)
    pub fn resolve_context(&self, attr: ObjRef, source: Option<ObjRef>, cx: &mut EvalCx) -> Result<Option<ObjRef>, AttrError>;
    pub fn resolve_contexts(&self, attr: ObjRef, source: Option<ObjRef>, cx: &mut EvalCx) -> Result<Vec<ObjRef>, AttrError>;   // ResolveContexts
    pub fn value_of(&self, attr: ObjRef, source: Option<ObjRef>, override_source: Option<ObjRef>, cx: &mut EvalCx) -> Result<Option<AttrValue>, AttrError>; // None: chain did not resolve
    pub fn base_value_of(..same..) -> Result<Option<AttrValue>, AttrError>;
    pub fn value_from_context(&self, attr: ObjRef, context: Option<ObjRef>, cx: &mut EvalCx) -> Result<f32, AttrError>;       // GetValueFromContext
    pub fn base_value_from_context(..same..) -> Result<f32, AttrError>;                                                       // GetBaseValueFromContext
    pub fn evaluate_init(&self, data: &InitData, source: Option<ObjRef>, override_source: Option<ObjRef>, cx: &mut EvalCx) -> Result<f32, AttrError>;
    // expr.rs (W3-ATTREVAL)
    pub fn expressions_hold(&self, exprs: &[ExprData], mode: ExprMode, source: Option<ObjRef>, override_source: Option<ObjRef>, cx: &mut EvalCx) -> Result<bool, AttrError>;
    // effects.rs (W3-ATTREFFECTS)
    pub fn add_modifier(&self, attr: ObjRef, context: Option<ObjRef>, modifier: ObjRef, suppress_notify: bool, cx: &mut EvalCx) -> Result<AddModifierOutcome, AttrError>;
    pub fn remove_modifier(&self, attr: ObjRef, context: Option<ObjRef>, modifier: ObjRef, suppress_notify: bool, cx: &mut EvalCx) -> Result<RemoveModifierOutcome, AttrError>;
    pub fn set_base(&self, attr: ObjRef, source: Option<ObjRef>, v: f32, suppress_notify: bool, cx: &mut EvalCx) -> Result<bool, AttrError>; // the native's bool; stops at the first failure, no rollback
    pub fn apply_effects(&self, effects: &[EffectData], source: Option<ObjRef>, override_source: Option<ObjRef>, cx: &mut EvalCx) -> Result<Applied, AttrError>;
    pub fn remove_effects(&self, applied: Applied, cx: &mut EvalCx) -> Result<(), AttrError>;
    // storage.rs (W3-ATTREFFECTS): the by-name Object natives, no definition, no chains
    pub fn add_modifier_by_name(&self, object: ObjRef, modifier: ObjRef, attribute_name: NameId, suppress_notify: bool, cx: &mut EvalCx) -> Result<AddModifierOutcome, AttrError>;
    pub fn remove_modifier_by_name(..same..) -> Result<RemoveModifierOutcome, AttrError>;
    pub fn attribute_value_by_name(&self, object: ObjRef, attribute_name: NameId, cx: &mut EvalCx) -> Result<f32, AttrError>;  // 0.0: not an attribute
}
// storage.rs (W3-ATTREFFECTS). 0x5F: the VM converts the right side with its own assignment rule first (interp.cpp:511-522,
// ow-vm's W3-INTERP; ow-attr keeps no copy of it); let_base takes the converted Value, stores it into the base companion
// as is (an Int stays exact, interp.cpp:593) and recomputes the value in `form`. A Value whose kind differs from the
// base's is refused with AttrError::BadData, nothing written: a deliberate refusal (the C++ stores it silently). No change
// notification (NATIVE_BYTECODE_OPCODES opcode 5F, high confidence; NATIVE_LEVEL_UP_ATTRIBUTES's pool
// rebase depends on it; interp.cpp:566-569). The NATIVE_ATTRIBUTES "Open" line on the typed let predates that reading.
pub fn let_base(space: &mut ObjectSpace, owner: ObjRef, c: AttrCompanions, rhs: Value, form: CombineForm) -> Result<(), AttrError>; // UNVERIFIED in the game
```

### 3.6 `ow-balance`

```rust
// types.rs (W1-BALTYPES)
pub struct Playthrough(u8);  // 0 Normal, 1 TVHM, 2 UVHM: WillowGameReplicationInfo.CurrentPlaythrough (NATIVE_CONTROLLER_HELPERS, UNVERIFIED)
impl Playthrough { pub const NORMAL: Self; pub const TVHM: Self; pub const UVHM: Self;
                   pub fn from_index(i: i32) -> Result<Self, BalanceError>; pub fn index(self) -> u8;
                   pub fn count(self) -> u8 /* index + 1: the PlayThroughCount reading, UNVERIFIED (question 1) */ }
pub struct OpLevel(u8);        // 0..=10 (NATIVE_LOOT cap reading, UNVERIFIED); nonzero only in UVHM
pub struct PlayerCount(u8);    // 1..=4; single player = 1 (D12)
pub struct GameStage(pub i32);   // placed by W0, final (W1-ITEMDATA uses it before W1-BALTYPES lands)
pub struct ExpLevel(i32); pub struct AwesomeLevel(i32);
pub enum LevelCapPack { First, Second, Third }  // record order; increments 11 / 11 / 8 are data for W3-BALANCE
impl LevelCapPack { pub const ALL: [LevelCapPack; 3]; pub fn bit(self) -> u8 /* 0x1, 0x2, 0x4: this crate's encoding */ }
pub struct LevelCapUpgrades(u8); // set of licensed level-cap packs; input to the player level cap in every playthrough
impl LevelCapUpgrades { pub const NONE: Self; pub const ALL: Self;
                        pub fn from_bits(bits: u32) -> Result<Self, BalanceError> /* unknown bits refused */;
                        pub fn bits(self) -> u8; pub fn with(self, pack: LevelCapPack) -> Self;
                        pub fn contains(self, pack: LevelCapPack) -> bool; pub fn is_empty(self) -> bool }
#[non_exhaustive]
pub struct BalanceSnapshot { pub playthrough: Playthrough, pub op: OpLevel, pub players: PlayerCount,
                             pub player_level: ExpLevel, pub stage: GameStage, pub awesome: AwesomeLevel,
                             pub level_cap_packs: LevelCapUpgrades }
impl BalanceSnapshot { pub fn new(/* the seven fields in order */) -> Result<Self, BalanceError> /* validates */;
                       pub fn validate(&self) -> Result<(), BalanceError> /* nonzero OP only in UVHM, UNVERIFIED */ }
#[non_exhaustive]  // Clone + PartialEq, not Eq (so Attr(AttrError) need not be Eq)
pub enum BalanceError { NotYet(NotYet), OutOfRange { what: &'static str, value: i64 },
                        OpLevelOutsideUvhm { op: u8, playthrough: Playthrough }, UnknownLevelCapPackBits { bits: u32 },
                        Attr(AttrError) /* added once ow-attr has its error type */ }

// W3-BALANCE
pub fn experience_required(sys: &AttributeSystem, cx: &mut EvalCx, curve: ObjRef, level: ExpLevel) -> Result<i64, BalanceError>;
pub fn region_game_stage(..., region: ObjRef, pt: Playthrough, level: ExpLevel) -> Result<GameStage, BalanceError>; // stored on first query, where the game stores it
pub fn item_stage_cap(pt: Playthrough, op: OpLevel, upgrades: LevelCapUpgrades) -> Result<GameStage, BalanceError>;
pub fn playthrough_modifier(pt: Playthrough, ...) -> Result<Option<ObjRef>, BalanceError>;  // PlayThroughBasedBalanceModifiers entry
pub fn mission_xp(..) -> Result<i32, BalanceError>; pub fn kill_xp(..) -> Result<i32, BalanceError>;
pub struct OfflineWorld { .. }   // builds GRI, PRI and controller objects in an ObjectSpace from a BalanceSnapshot, and implements AttrWorld,
                                 // so offline tools evaluate through the same resolver chains as the running game
```

### 3.7 `ow-items`

```rust
// data.rs (W1-ITEMDATA): the identity rolls, inventory, cards, saves and UI share (report F12).
pub enum ItemKind { Weapon, Shield, GrenadeMod, ClassMod, Relic, Usable, Customization, MissionItem, MissionWeapon }
pub enum WeaponSlot { Body, Grip, Barrel, Sight, Stock, Elemental, Accessory1, Accessory2, Material }
pub enum ItemSlot { Alpha, Beta, Gamma, Delta, Epsilon, Zeta, Eta, Theta, Material }
pub struct DefinitionData { pub kind: ItemKind, pub balance: Option<ObjectPath>, pub definition: ObjectPath, pub manufacturer: Option<ObjectPath>,
                            pub manufacturer_grade: i32, pub parts: [Option<ObjectPath>; 9], pub prefix: Option<ObjectPath>,
                            pub title: Option<ObjectPath>, pub game_stage: GameStage, /* the remaining fields exactly as the package
                            declares WeaponDefinitionData / ItemDefinitionData: names read with the C++ tool, not invented */ }
pub struct InventoryInstance { pub data: DefinitionData, pub quantity: i32, pub equipped: bool, pub mark: Mark, pub quick_slot: Option<u8> }

// W4 lanes
pub struct Registry { .. }  pub fn build_registry(defs: &mut DefStore, space: &mut ObjectSpace) -> Result<Registry, ItemError>;
// One RNG per roll, as the oracle draws one CrtRand(seed) stream for the whole roll (item_roll.py:22, 281): the roll's
// world is a RollWorld whose rng() returns that one generator for every stream, including the evaluator's variance.
// It forwards everything else to `inner` except attribute_changed, which it forwards only when `inner_runs_script` is
// false (an OfflineWorld); over a world that may run script (the Vm) it answers NotYet { NotPorted }, because the
// nested evaluation would draw from the inner world's generators (the adapter rule of 3.5).
pub struct RollWorld<'a> { pub inner: &'a mut dyn AttrWorld, pub rng: &'a mut CrtRand, pub inner_runs_script: bool }   // impl AttrWorld
pub struct RollCx<'a> { pub balance: &'a BalanceSnapshot, pub eval: EvalCx<'a> }        // eval.world is a RollWorld
pub fn roll(reg: &Registry, balance: &ObjectPath, cx: &mut RollCx) -> Result<DefinitionData, ItemError>;
pub fn instantiate_stats(data: &DefinitionData, cx: &mut EvalCx) -> Result<ObjRef, ItemError>;  // the weapon/item object the stats live on
pub fn card(stats_object: ObjRef, cx: &mut EvalCx) -> Result<Card, ItemError>;                 // AttributePresentationDefinition rounding
pub fn roll_pool(pool: &ObjectPath, cx: &mut RollCx) -> Result<Vec<DefinitionData>, ItemError>;   // the oracle: loot_pools.roll_pool_crt
```

The stats of an item **are attributes on an instance object** in every case: offline, `instantiate_stats` makes a
detached object of the item's class in the offline `ObjectSpace`; in the world it is the weapon actor. One path.

### 3.8 `ow-vm` (contract W2-VMAPI)

*Shipped shapes (W2-VMAPI, PR #295, review r1; replaces the revision 3 sketch).* Module paths `ow_vm::api`,
`ow_vm::error`, `ow_vm::services`. Every enum and struct below that may grow is `#[non_exhaustive]` (`StubPolicy`,
`Limits`, `CallCensus`, `FunctionDecl`, `IteratorResult`, `Arg`, `CallResult`, `NativeCall`, `VmError`). `Vm` itself
implements `ow_attr::api::AttrWorld` (no separate view type), and is `!Send` by a compile-time check (D18).

```rust
pub struct Vm { pub space: ObjectSpace, pub defs: DefStore, pub attrs: Rc<AttributeSystem>, pub rng: GameRng,
                pub global_slots: GlobalSlots /* the world owns the process-wide pair */, pub clock: SimClock,
                pub parity: ParityMode, pub log: Vec<String>, pub limits: Limits, pub stub_policy: StubPolicy,
                pub census: Option<CallCensus>, pub trace: Option<Vec<String>>, .. /* functions, natives, iterators,
                notification overrides, services, per-class name counters, step and depth counters: private */ }
pub struct Limits { pub step_limit: u64 /* 5,000,000 per top-level call */, pub max_call_depth: u32 /* 200 bytecode frames,
                    counted in the Vm so it carries through natives, checked before a frame is made */ }
pub enum StubPolicy { LogAndZero /* census, sweep, exploration (Phase 2 harness) */,
                      Refuse { allow: BTreeSet<String> } /* gameplay runs and gates: a stub site is VmError::UnboundNative */ }
impl StubPolicy { pub fn refuse_all() -> Self; pub fn allows_stub(&self, key: &str) -> bool; }
pub struct CallCensus { pub script_calls, pub native_calls, pub stub_calls, pub none_contexts: DetMap<String, u64> }
pub struct FunctionDecl { pub path: ObjectPath, pub owner: Option<ClassId>, pub native_key: Option<String> /* "Class.Name(types)" */,
                          pub is_static: bool, pub params: Vec<PropertyDecl>, pub result: Option<PropertyDecl> }
impl FunctionDecl { pub fn script(path); pub fn native(path, key); with_owner; with_static; with_params; with_result;
                    is_native; display_key }
pub type NativeFn = Rc<dyn Fn(&mut NativeCall<'_>) -> Result<Value, VmError>>;
pub type IteratorFn = Rc<dyn Fn(&mut NativeCall<'_>) -> Result<IteratorResult, VmError>>;
pub struct IteratorResult { pub out_param: usize, pub items: Vec<Value> }   // ::new
pub type AttrNotifyFn = Rc<dyn Fn(&mut Vm, ObjRef, PropId, NumForms) -> Result<(), AttrError>>;   // a class's change notification
pub struct NativeCall<'a> { pub vm: &'a mut Vm, pub function: FuncId, pub this: Option<ObjRef>, pub args: Vec<Arg> }   // ::new
pub struct Arg { pub value: Value, pub out: bool, pub supplied: bool }   // ::new; out values copied back after the native returns
pub struct CallResult { pub ret: Value, pub outs: Vec<Value> }
impl NativeCall<'_> { pub fn arg(&self, i: usize) -> Result<&Value, VmError>; pub fn set_out(&mut self, i: usize, v: Value) -> Result<(), VmError>;
                      pub fn has(&self, i: usize) -> bool;   // arg/set_out: BadArgument past the params; set_out refuses a non-out param
                      pub fn call_script(&mut self, f: FuncId, this: Option<ObjRef>, args: Vec<Value>) -> Result<CallResult, VmError>; }
impl Vm {
    pub fn new(space: ObjectSpace, defs: DefStore, attrs: Rc<AttributeSystem>, mode: ParityMode) -> Self;
        // rng = from_world_seed(0, mode), clock at 0, slots 0, stub_policy = Refuse with nothing allowed; the binary passes parity_mode()
    pub fn with_world_seed(self, seed: u64) -> Self;   // reseeds in the VM's own mode (no Off VM on Reference streams)
    pub fn with_stub_policy(self, p: StubPolicy) -> Self;
    pub fn find_class(&mut self, path: &str) -> Result<ClassId, VmError>;     // known classes; else NotYet { LayerAbsent } until loading
    pub fn declare_function(&mut self, d: FunctionDecl) -> Result<FuncId, VmError>;  pub fn function(&self, f: FuncId) -> Option<&FunctionDecl>;
    pub fn find_function(&mut self, path: &str) -> Result<FuncId, VmError>;   // declared functions; else NotYet { LayerAbsent }
    pub fn instantiate(&mut self, c: ClassId, name: &str) -> Result<ObjRef, VmError>;   // "": Off <Class>_<n>, per-class counter
                                                   // (UE3's form, UNVERIFIED numbering); Reference <Class>_0 every time (6.1)
    // instantiate_export(pkg, index, PropertyPrefix): absent until W3-DECODE's ow_object::prefix exists
    pub fn call(&mut self, f: FuncId, this: Option<ObjRef>, args: Vec<Value>) -> Result<CallResult, VmError>;
        // top-level: fresh step budget; from inside a native: shares budget and depth (C++ callFromNative); refused
        // while any service is borrowed; script function without override: NotYet { NotPorted } until W3-INTERP
    pub fn register_native(&mut self, key: &str, f: NativeFn) -> Result<(), VmError>;  pub fn has_native(&self, key: &str) -> bool;
    pub fn register_iterator(&mut self, key: &str, f: IteratorFn) -> Result<(), VmError>;  pub fn iterator_for(&self, f: FuncId) -> Option<IteratorFn>;
    pub fn override_script(&mut self, path: &str, f: NativeFn) -> Result<(), VmError>;
    pub fn register_attribute_notify(&mut self, class_path: &str, f: AttrNotifyFn) -> Result<(), VmError>;
    pub fn service<T: 'static>(&self) -> Option<Rc<RefCell<T>>>;  pub fn add_service<T: 'static>(&mut self, t: T) -> Result<Rc<RefCell<T>>, VmError>;
    pub fn set_world_hooks(&mut self, h: Rc<RefCell<dyn WorldHooks>>) -> Result<(), VmError>;  pub fn world_hooks(&self) -> Option<Rc<RefCell<dyn WorldHooks>>>;
    pub(crate) fn tick(&mut self) -> Result<(), VmError>;                      // one step (the interpreter's per-expression seam)
    pub(crate) fn stub_site(&mut self, key: &str, text: String) -> Result<(), VmError>;   // every stub site consults the policy
}
pub trait NativePack { fn register(&self, vm: &mut Vm) -> Result<(), VmError>; }   // every crate above the VM exports one
pub enum VmError { Script { function: String, message: String }, StepLimit, UnboundNative(String), NotYet(NotYet),
                   Heap(HeapError), Attr(AttrError), Decode(String), Registration { key: String, reason: &'static str },
                   BadArgument { function: String, index: usize, reason: &'static str }, ServiceBorrowed(&'static str) }
                   // Def(DefError) pending W3-DEFS (additive); From<NotYet | HeapError | AttrError>
impl From<VmError> for AttrError { .. }   // NotYet, Heap and Attr pass through unchanged; the rest as AttrError::World(text)
```

```rust
// The one upward hook the VM's AttrWorld implementation calls; frozen by W2-VMAPI, implemented by ow-world.
pub trait WorldHooks {
    fn globals(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError>;     // the running game always has one
    fn game_info(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError>;   // likewise (progression.cpp:84,87)
    fn engine_query(&mut self, q: EngineQuery, source: ObjRef, w: WorldParts<'_>) -> Result<Option<ObjRef>, AttrError>;
}
// Held as Rc<RefCell<dyn WorldHooks>> in its own slot of the service map (set_world_hooks): Vm clones the Rc out,
// try-borrows it (a hook already borrowed is AttrError::World, not a panic) and passes its own parts. Without hooks,
// globals, game_info and engine_query are NotYet { LayerAbsent("ow-world") }. Balance state (region stages,
// playthrough) is not part of this trait: it is a separate service whose type lives in ow-world (it names ow-balance
// types, which ow-vm may not). AttrWorld::rng is the VM's GameRng C-rand streams; global_slots is Vm::global_slots;
// attribute_changed runs the nearest registered AttrNotifyFn in the owner's class chain (none: nothing, UNVERIFIED).
```

**Stub policy per command.** A logged zero-result stub is a declared policy of the exploration commands (the Phase 2
harness), not a stand-in; the gameplay commands refuse an unbound native.

| Command | `StubPolicy` | Pinned by |
|---|---|---|
| `--native`, `--native-args`, `--native-selftest` | `LogAndZero` | `natives_test.py:16-20` (status `unimplemented`) |
| `--run`, `--run-batch` | `LogAndZero` | `vm_test.py:818` (an iterator native with nothing bound) |
| `--native-census`, `--vm-sweep` | `LogAndZero` | `vm_test.py:886-889` (a stub counted once) |
| `--behavior-run`, `--kismet-run`, `--mission-run`, `--slice-run`, `--combat-run`, `--inventory-move`, `--mover-event`, every `ow-host` world | `Refuse { allow }` (the allow list, each entry with its reason, lives with the command) | the gameplay gates of section 6 |

**Stub sites.** Every stub site consults `StubPolicy::allows_stub` with a stated key, logs the C++ text when allowed
and is `UnboundNative(key)` under `Refuse` otherwise: an unbound native (key `Class.Name(types)`, `UNIMPLEMENTED <key>`,
`interp.cpp:692`) in `Vm::call`, and the interpreter's five through `Vm::stub_site`: a function name not found
(`<Class>.<Name>`, `interp.cpp:634`), a native index with no function (`native index <n>`, `:663`), an unhandled
primitive cast (`primitive cast 0x<hh>`, `:217`), `DynArraySort` (`:467`), and a `foreach` over an iterator native with
nothing bound (the iterator's key, `:866`). The table with the logged texts is on `StubPolicy` in `api.rs`.

Services are `Rc<RefCell<T>>`, one per type, kept in registration order (a `TypeId` index for lookups only); a native
borrows one only for a short scope and never across `call_script`. That rule is in the contract's doc comment and is
the review bar's first check; the VM enforces it: any call while a service is borrowed is `VmError::ServiceBorrowed`,
naming the first borrowed service in registration order.

### 3.9 Native packs, world and host

- **Everything a native needs is reachable from `&mut Vm`** (review finding 2): the object space, the store, the
  attribute system (`Rc`), `GameRng`, `SimClock` and the parity mode are `Vm` fields; state of crates above the VM is a
  VM service. `ow-world` registers the `WorldHooks` service of 3.8 (globals, game info, engine queries) and, as a
  separate service typed in `ow-world`, the balance state; `Vm`'s `AttrWorld` implementation calls the hooks; without it those methods return
  `NotYet { LayerAbsent("ow-world") }`, which is how W3-CORENAT and the W3 attribute lanes run before wave 5.
- `ow-world::World { vm: Vm }` is a thin owner: `World::new` registers the packs, `World::tick(dt)` advances
  `vm.clock` and runs actor ticks by `TickGroup` (per-actor timers inside each actor's tick, as the record orders
  them), then latent code. Every native-pack crate exports `pub struct Pack; impl NativePack for Pack`. `ow-cli` and
  `ow-host` assemble the packs they need.
- The behavior kernel (`ow-behavior`) keeps a handler registry keyed by behavior class; `ow-combat` (damage and
  loot-spawn behaviors, the latter in `loot.rs`), `ow-skills` and `ow-mission` register handlers for the classes they
  own from their `NativePack::register`. `ow-inventory` registers none (it has no edge to `ow-behavior`). An unregistered class reached at run time is
  `NotYet { NotPorted }` and stops that branch, as `behavior.cpp` reports unhandled classes today.
- `ow-ui::MoviePlayer` (trait: load a movie, set and get variables, invoke, advance) is implemented in `ow-host`
  (Ruffle if the benchmark supports it). `ow-ui` holds the bridge natives and calls only the trait.
- `ow-host` reads only `World` and `ow-assets` outputs. The uu/rotator to engine-frame conversion is one module there.

---

## 4. The "not yet" convention in practice

| Situation | What the code does | Example |
|---|---|---|
| Rule not read from native code or script | `Err(NotYet { missing: NotRead, record: "NATIVE_ATTRIBUTES.md section 5" })` | an `AmmoDropWeight`-style resolver class not in the notes |
| Rule exists in C++/Python, lane not landed | `NotYet { missing: NotPorted }` | `ow-cli` command family not ported (exit code 3, distinct from the C++ codes) |
| Lower layer missing | `NotYet { missing: LayerAbsent("ow-items stats") }` | the shield pool asked for its capacity before W4-STATS |
| Blocked on a real-game question | `NotYet { missing: NeedsRealGame(N) }` | UVHM region stage (question 2) |
| Read but not confirmed in the game | runs, doc comment `Status: UNVERIFIED (record, section)` | the stack formula |
| Confirmed live | runs, doc comment `Status: verified (capture, matching number)` | XP curve levels 1-59 |
| Unbound native in a gameplay run | `VmError::UnboundNative(key)` under `StubPolicy::Refuse` | a Kismet op with no binding during the Fire run |

Tests assert the `NotYet` cases as well as the working ones, so a lane that fills a branch flips a test on purpose.

---

## 5. Determinism and numerics

### 5.1 What is required

| # | Requirement | How it is enforced |
|---|---|---|
| N1 | Reference platform for bit-parity gates is `x86_64-pc-windows-msvc` (desktop, hummer, CI). Results on the Linux laptop are reported, not used for parity. | Gate commands name the target; W1-NUM's pow table test runs everywhere and fails loudly on a platform whose `pow` differs. |
| N2 | No fused multiply-add. Rust never contracts `a*b+c` by itself; `mul_add` is banned in all crates. | `tests/crate_edges_test.py` greps `mul_add`. |
| N3 | Every power and every transcendental (`pow`, `exp`, `ln`, `sqrt`, `sin`/`cos`/`tan`, `asin`/`acos`/`atan`/`atan2`) goes through its `ow_core::num` choke point; the std methods (`powf`, `powi`, `exp`, `ln`, `sqrt`, `sin`, ...) are banned outside `num.rs`. | the same grep. On the msvc target `f64::powf` lowers to the C runtime `pow` the C++ and Python oracles call; the choke point is where a portable implementation would go if one is ever needed, without touching callers. |
| N4 | Attribute values are `f32` (`Value::Float(f32)`). For one `+ - * /` of two f32 values, f32 arithmetic equals "double then round" (f64 has more than 2x24+2 bits), so storing f32 matches the C++ "double kept at float precision". Compound expressions and powers follow the oracle's rounding points exactly; any `f64` in rule code carries a comment naming the oracle step it mirrors. | review bar; W1-NUM documents the rule with examples. |
| N5 | Float-to-int conversion never uses `as` (Rust saturates). Two forms, each named for its oracle: `trunc_i32` (attribute path, `interp.cpp:561`: `i32::MIN` for NaN and out of range; also weapon_gen's integer attributes via `trunc_i32_f64`) and `trunc_i64_wrap_i32` (VM natives: `int64_t(double)` then `makeInt`'s wrap to 32 bits, `vm.cpp:51`, `natives_core.cpp:103,106,123,170-171`). Script ints are `i32` with wrapping ops. | grep for `as i32`/`as i64` on floats in rule crates is a review item; W1-NUM adds a test of the edge cases. |
| N6 | Output text: each output mirrors the format its oracle prints (the list at the top: `%.9g`, ostream default 6, `%f`, `%.5g`, `std::to_string`, `%.1f`/`%g`, `to_chars` shortest, Python `repr`), through the matching `ow_core::fmt` function; JSON keys in the oracle's order (`DetMap` for `std::map` and `sort_keys=True`); C++ `quote()` or Python `json.dumps(ensure_ascii=False)` escapes; stderr text where a test asserts it. | the CLI parity harness compares bytes. |
| N7 | No `HashMap`/`HashSet` iteration in rule or output code. `OrderedMap` where the oracle iterates in insertion order (weighted picks, Python dicts), `DetMap` (BTreeMap) only where the oracle's order is key order, else `Vec`. `HashMap` only for lookups, with `BuildHasherDefault<DefaultHasher>` (fixed keys). | grep bans `std::collections::HashMap::new`/`HashSet::new` (fixed-hasher constructors allowed); review. |
| N8 | Game randomness only through `Vm::rng` (`GameRng` streams), `AttrWorld::rng`, or a roll's one `CrtRand`; parity tools pass `CrtRand::new(seed)` with the oracle's exact seed per roll. Cosmetic randomness uses `HostRng` and never draws from a game stream, so the frame rate cannot change a gameplay result. | `HostRng` lives in `ow-host`, which no game crate may depend on (edge test). |
| N9 | Game time only from `Vm::clock` (`SimClock`); `TimeSeconds` is accumulated as f32 the way UE3 does; a run records its dt sequence so it can be replayed. Where the world's `TimeDilation` and an actor's `CustomTimeDilation` enter an actor's delta is not located (NATIVE_ENGINE_CORE.md, Open): UNVERIFIED. | W1-SIM test: identical dt sequences give identical clocks; W5-WORLD replay test. |

### 5.2 How it is tested

1. **Pow table** (W1-NUM): about 300 invented (base, exponent) pairs plus the record constants (1.13^L and n^2.8 for
   L, n in 1..=80, 10^d for d in 0..=6), each with the expected bit pattern of `pow` (double) and `powf` (single),
   generated on the zuhu desktop by the C++ build and by Python `math.pow` and stored in the Rust test as hex. Both
   generators must agree before the table is committed. The test passes every input through `std::hint::black_box`
   so LLVM cannot fold a constant `pow` at compile time (which would test the compiler's folding, not the runtime);
   the same table form covers `exp`, `ln` and the trig choke points on invented inputs.
2. **Bitwise differential runs** compare `to_bits()`, never a tolerance: Rust vs C++ CLI on synthetic packages (every
   lane), Rust vs Python on real packages (items: the 2,919,834-field parity), Rust vs recorded live rows (golden cards,
   XP curve, health base).
3. **Replay determinism**: every scripted run (`--combat-run`, `--slice-run`, `--mission-run`) is executed twice in
   one test with the same seed and dt sequence; the call traces must be identical.
4. **Seed spreading**: W1-RAND tests that stream seeds from world seeds 1..=20 do not all draw near 0 first (the
   census finding of ITEM_ROLL.md section 3).

### 5.3 The three `pow` forms and the two stack-combine forms

`progression.cpp` (single `powf`), `weapon_gen.cpp` (double `pow`, rounded after) and the VM's `**` (double, rounded)
disagree in form; so do the two stack combines (`CombineForm::WeaponGen`, `weapon_gen.cpp:871-875` with per-add f32
sums at 1018-1030, versus `CombineForm::Vm5F`, `interp.cpp:546-561`, all f32). The game's own forms are unknown. The
frozen API keeps both as parameters (`PowForm`, `CombineForm`); the one evaluator and the one stack owner choose per
call only until each is settled, and keep the parameter afterwards (callers then pass the settled form):

- Defaults `DoubleThenSingle` and `WeaponGen` (the forms behind the 2.9 M parity fields and 69/69 golden cards).
- The XP-curve and health-base fixtures (verified live: levels 1-59; 80 x 1.13^L at levels 2, 8, 17, 70) and the VM's
  5F fixtures (`vm_test.py`) must pass under the defaults too. If every fixture set passes under the defaults, the
  defaults are the settled forms; if a set passes only under the other form, the lane stops and reports: that is a
  finding for the maintainer and real-game questions 21 (pow) and 22 (stack combine), not a lane decision.

The same holds for **two half-up roundings** of RoundingMode 1 (`ATTRROUNDING_IntRound`; review r1 of W1-NUM). The
attribute-initialization path (`weapon_gen.cpp:568` `Round()`, `weapon_recipe.ROUNDING`) rounds `floor(x + 0.5)` in
double: `ow_core::num::round_mode(x, RoundingMode::HalfUp)`, so `-2.5` gives `-2`. `progression.cpp:311`
`restricted()` (XP curve, health base) calls `std::round`, halves away from zero: `ow_core::num::round_half_away_f32`,
so `-2.5` gives `-3` and `-0.3` gives `-0.0`. They differ only on negative halves. Each port calls the form its oracle
uses (W1-ATTRMATH's `restricted()` calls `round_half_away_f32`); which form the game uses for each rule is UNVERIFIED.

---

## 6. Parity strategy per layer

"Freeze" = the C++/Python copy stops being developed: no new feature, no new consumer, edits only to keep it
building; it stays as the differential reference. "Delete" = removed from the tree. Since every C++ file above the
reader includes `package.hpp`, C++ is frozen layer by layer and **deleted in one lane** (W7-RETIRE) once the Rust CLI
passes every ctest entry the C++ CLI passes. From wave 1 on, no new rule is written in `src/` (decision D17).

| Crate | Oracle | Done when (gate) | Freeze / delete |
|---|---|---|---|
| `ow-container` | `research/native_count.py` decoder; C++ `--verify-decoded`; `tests/package_test.py --compressed` | Rust unit fixtures (re-expressed from `package_test.py`'s `container()` and its partial-compression case, lines 64-79: a valid partial table accepted, the six bad-table offsets and trailing bytes refused) pass; `package_test.py {bin} --compressed` passes; every package under `CookedPCConsole` and the DLC folders decompresses to the C++ bytes (SHA-256 list under `local/`), the 2 known-bad remaster packages fail with the same error kind | freeze `src/container.*` and lzokay at the gate; delete in W7 |
| `ow-package` | C++ CLI; `tests/package_test.py`, `properties_test.py`, `global_resolve_test.py` with the Rust binary; `verify_packages.py --reader` | `verify_packages.py --reader target/release/ow-package.exe`: 9 packages, decoded bytes, counts, export fields equal, 3,339/3,339 class bodies exact (needs W2-REFLECT); `--properties` byte-identical to C++ on every export of the 9 code packages and `Startup.upk` (local diff, count reported) | freeze `src/package.*`; tools' `--reader` default switches to the Rust binary (W3-SWITCH); delete in W7 |
| `ow-script` | `tools/verify_scripts.py` (C++ and Python vs the Rust binary), `research/script_disasm.py`; C++ `--disasm`, `--script-check`. (`tests/script_disasm_test.py` tests only the Python decoder: its cases are re-expressed as Rust unit tests, it is not a Rust gate.) | Rust unit tests from `script_disasm_test.py`'s cases on `testkit` packages; `--disasm` and `--script-check` byte-identical to C++ on those synthetic packages; 12,968/12,978 decode on the real packages with the same 10 failures; `--disasm` text identical to C++ for every function of the 9 packages | freeze `src/script.*`; `research/script_disasm.py` stays (independent oracle) |
| `ow-object` | C++ `--class-check`, `--interface-compare`, `--object-dump`; `vm_test.py`, `runtime_test.py` loading cases | 3,339/3,339; `--object-dump` of every class default object of the 9 packages identical to C++; reflection typer = schema typer on every entry of `tools/*-arrays.schema` over the census (then the schemas and `SchemaTyper` are deleted) | freeze `vm.cpp` loading half |
| `ow-defs` | C++ `--properties`/`--object-dump`; `tools/item_registry.py` owner choices | the Phase 0R step 6 check: a `WeaponPartDefinition` dump equals the C++ dump; `owners()` + `FirstInScanOrder` picks the same owner as the tools for every registry path (the one DLC mismatch listed, not hidden); overlay unit tests; live-capture overlay reproduces the 39 changed objects from the local capture | n/a (new) |
| `ow-vm` | `vm_test.py`, `natives_test.py`, `runtime_test.py` with the Rust binary; C++ `--vm-sweep`, `--native-census`. (`ui_trace_returns_test.py` tests a Python tool on fake data: re-expressed as Rust unit tests, not a Rust gate.) | the three tests pass on the Rust binary, the stand-in-pinning cases of 6.1 under `OW_PARITY=reference`; `--vm-sweep` JSON identical to C++ on the 9 packages under `OW_PARITY=reference` (`Rand`/`FRand` on VmLcg) | freeze `vm.cpp`, `interp.cpp`, `natives*.cpp` |
| `ow-attr` | union of evaluator fixtures: `card_rounding_cases.json`, `weapon_balance_test.py`, the cases of `weapon_gen_test.py` (which drives `ow-weapongen`, so its cases are re-expressed; the file is deleted with the C++ generator, not passed), the cases of `skill_stats_test.py`, `prepare_stock_shield_test.py`, evaluator rows of `mission_script_test.py` and `combat_test.py` | every fixture of every current evaluator reproduced by the one Rust evaluator (re-expressed as Rust tests on synthetic packages); shapes outside the read set return `NotYet`, with a test each | freeze `progression.*`; the Python evaluators frozen with their tools (section 9) |
| `ow-balance` | REALGAME_GROUND_TRUTH rows; `mission_script_test.py` | XP curve 1-59, health base at 2/8/17/70, Fire XP 395 at stage 8, skill points `max(0, L-4)`, Sanctuary PT1 stage 7..9 (8..11 after WelcomeToSanctuary) reproduced; PT2/PT3 branches typed and `NotYet` where unread | `tools/slice_values.py` deleted at the gate |
| `ow-items` | `item_registry.py` JSON; `item_roll.py --census` + `item_roll.check`; `weapon_gen_parity.py`; `loot_pools.py` (its CrtRand path, `roll_crt`/`roll_pool_crt`, not the `random.Random` one); `tools/real_game/golden_card_compare.py` | registry JSON identical (schema `openwillow-item-registry/1`, Python float and escape format); 35,660 census rolls pass `item_roll.check` (runtime-state weights and windows as 6.1 says); 2,919,834 fields equal to Python bit for bit; 7,680 pool rolls equal to `roll_pool_crt`; 69/69 golden cards with the live overlay (52-53 without, reported) | `weapon_gen.*`, `weapon_gen_cli.cpp`, the manifest tool and the tests that drive only `ow-weapongen` (`weapon_gen_test.py`, `weapon_gen_parity_test.py`) deleted at the gate, their cases re-expressed first; Python per section 9 |
| `ow-world`, `ow-combat` | `combat_test.py`, `verify_combat_pawn.py --vm` | `combat_test.py` on the Rust binary; `--vm` 97 PASS / 0 FAIL / 4 MISSING / 1 OPEN reproduced (loot rows against the live rows, not the C++ stand-in inventories) | freeze `combat*.cpp` |
| `ow-behavior` | `behavior_test.py` with the Rust binary | the test passes on the Rust binary; Fire event order (below) | freeze `behavior.*` |
| `ow-skills` | the cases of `skill_effects_test.py` and `skill_stats_test.py` (Python tools on fake packages: re-expressed as Rust unit tests on `testkit` packages); NATIVE_SKILL_BEHAVIOURS live captures | the re-expressed cases pass; Phaselock duration `5 + 0.5 g`; the 2026-11-06 skills live capture rows reproduced where they are numbers | `tools/skill_stats.py` deleted at the gate |
| `ow-mission` | `kismet_test.py`, `mission_script_test.py`, `verify_fire_event_order.py` | 33/33 rules with the Rust reader; `--slice-run` output identical to C++ (every difference explained in the handoff) | freeze `mission*`, `kismet*`, `dialog*`, `mover*`, `slice*` |
| `ow-combat::firing` | `tests/weapon_firing_math_test.cpp`, `tools/weapon_firing.py`, `weapon_gen_firing.py` | every C++ math test re-expressed in Rust, bit-equal | freeze `weapon_firing.*`; `tools/weapon_firing.py` deleted once nothing calls it |
| `ow-assets` | `tests/assets_test.py` with the Rust binary; C++ `--texture`/`--mesh` | PNG and OBJ bytes identical to C++ on a local sample of 200 textures and 200 meshes (list under `local/`) | freeze `assets.*` |

The differential harness: `tools/rust_parity.py --bin target/release/ow-package.exe` runs every line of
`tests/rust_parity.d/*.txt` (one file per lane, so no conflicts; a line is a test command with `{bin}` for the binary)
and prints pass/fail per line. A line ending in `mode=reference` runs with `OW_PARITY=reference`; every other line runs
with the variable unset (the harness clears it). A line starting `diff ` holds `ow-package` arguments: the harness runs
them on the C++ binary (`--cxx build/Release/ow-package.exe`) and the Rust binary and compares stdout and exit code
(and stderr when the line ends in `stderr`); this is how a lane gates single commands before a whole test file can
pass. There is no expected-failure marker: a line is in the manifest when it passes. A lane's gate includes its lines
passing.

### 6.1 Stand-ins the existing tests pin

These test cases assert a reference implementation's stand-in. They run against the Rust binary only in
`ParityMode::Reference` (manifest lines marked `mode=reference`). Because a manifest line runs a whole file, W1-PINSPLIT
moves the pinned cases of `vm_test.py` and `combat_test.py` into files of their own (both still pass on the C++
binary in ctest), so the rest of each file runs in Off mode. The lane that meets each stand-in lists the exact cases
in its handoff and its manifest, and writes a Rust-native test of the replacement (the game rule, or the `NotYet` it
returns) in the same PR. W7-RETIRE owns the split files and deletes each only when its replacement tests exist.

| Stand-in | Pinned by | Lane | Replacement test |
|---|---|---|---|
| `GotoState` name-only, an undeclared state only setting the name, a `Label` logged and not run (`label Begin not run`), the 8-level nesting bound ("Loop abandoned") | `tests/vm_test.py:754-815`, moved by W1-PINSPLIT to `tests/vm_states_test.py` | W3-CORENAT (reproduces), W5-STATES (replaces) | the record's state machine (NATIVE_ENGINE_CORE.md) on synthetic classes |
| `Object.Rand`/`FRand` static LCGs (12345 / 54321) | `--vm-sweep` only (no `vm_test.py`/`natives_test.py` case prints a random value) | W3-CORENAT (reproduces), W4-VMTOOLS (sweep gate) | streams of `GameRng` (CrtRand, UNVERIFIED convention) |
| 21 stand-in inventories from 7 `Pool_Coins` calls, no parts or stats | `tests/combat_test.py:934`, moved by W1-PINSPLIT to `tests/combat_loot_standin_test.py` | W6-LOOT | pool roll through `ow-items`, items with definition data |
| A part weight that needs runtime state taken as 1; a window bound that needs runtime state taken as no bound | `tools/item_roll.py:34-35, 317` (census and `check`) | W4-ROLL | evaluated through `ow-attr` in context; `NotYet` where the context is missing |
| Manufacturer and grade drawn in the balance roll (the game draws them in the pool roll, one candidate per manufacturer) | `tools/item_roll.py:25-26` (step 1), `weapon_recipe.roll` | W4-ROLL | Off mode draws at the pool level, which changes the CrtRand draw order, so the census and the `weapon_recipe.roll` equality run only as `mode=reference`; Off mode has Rust-native tests of the pool-level expansion |
| Uniform class-mod definition pick | `tools/item_roll.py:29-31` (step 2), decision 2026-11-13 | W4-ROLL | `NotYet { NeedsRealGame(6) }` until read. **After W7 deletes Reference, class-mod rolls return `NotYet` and `item_roll.check` no longer covers class mods**, until question 6 is answered |
| Name parts counted in the rarity sum (the tool labels it STAND-IN; NATIVE_ITEM_RULES.md section 2 says the data supports it) | `tools/item_roll.py:47, 82` (`NAME_PARTS_IN_RARITY`) | W4-ROLL | not a Reference stand-in: Rust uses the same reading in both modes, labelled UNVERIFIED (a reading the record supports), with its own test; question 6 settles it |
| "A property reading 0 with base 0 is the identity 1" | `combat_hit.cpp` (the lane greps `combat_test.py` for rows that depend on it) | W6-COMBAT | class defaults and start-up modifiers; crit base `NotYet { NeedsRealGame(8) }` |
| `prepare_stock_shield.py` at `SLICE_LEVEL = 8` | `prepare_stock_shield_test.py` | W4-STATS | non-weapon stats at the region stage |
| Every unnamed object named `<Class>_0` (`vm.cpp:1083`; several per class from `combat.cpp:325`, `combat_hit.cpp:173`, `combat_loot.cpp:435`, `EX_New` `interp.cpp:472`) | outputs that print unnamed object names; `vm_test.py:937-938` (first object `Foo_0`) holds in both modes | W2-VMAPI (reproduces in `Vm::instantiate`), W7-RETIRE (deletes) | Off mode's per-class `<Class>_<n>` (UE3 `MakeUniqueObjectName` form, numbering UNVERIFIED), `ow-vm` `instantiate_names` |

---

## 7. Interfaces frozen first (contracts)

| Contract | Owner lane | Frozen when | Dependents |
|---|---|---|---|
| Crate list, edges, module files, CLI dispatch table | W0-SCAFFOLD | W0 lands | all |
| `NotYet`, `ObjectPath`, `DetMap`, `OrderedMap`, `ParityMode` and `OW_PARITY` (read by binaries only) | W0-SCAFFOLD | W0 lands | all |
| The small final types `GameStage`, `PackageId`, `ModifierId` | W0-SCAFFOLD (owners keep them) | W0 lands | W1-ITEMDATA, W1-OBJ, W1-ATTRMATH |
| Placeholder types with `new_empty()` (2.3) | W0-SCAFFOLD | W0 lands | W1, W2 lanes |
| `ow_container::decode_package` signature | W0-SCAFFOLD | W0 lands | W1-PKG |
| `ow-core::num` / `fmt` (choke points, `f32r`, both truncation forms, rounding, every output format) | W1-NUM | its PR lands | every rule and CLI lane |
| `ow-core::rand` (`Rand15`, `ScriptRng`, `CrtRand`, `VmLcg`, `CrtStream`, `ScriptStream`, `GameRng`) | W1-RAND | its PR lands | items, VM natives, combat, AI |
| `ow-core::sim` (`SimClock`, `TickGroup`) | W1-SIM | its PR lands | VM, world, behavior, pools |
| `ow-package::testkit` builders (`testkit.rs`) | W1-TESTKIT | its PR lands | every W2+ lane with synthetic packages |
| `testkit/reflect.rs`, `testkit/script.rs` helpers | W2-REFLECT, W2-SCRIPT | each owner's PR lands | W3+ lanes with reflection exports or script bodies |
| `ow-object` IDs, `PropAddr`, `Value` (`Rc`, `!Send`: D18), `Heap`, `ClassTable`, `PropertyDecl`, `ObjectSpace` | W1-OBJ | its PR lands | reflect, decode, defs, attr, VM |
| `ow-package` `Package`, `Reader`, `PropValue`, `ArrayTyper` | W1-PKG, W2-PROPS | their PRs land | script, reflect, decode, assets |
| `ow-attr` stack math, `Modifier`, `PowForm`, `CombineForm` | W1-ATTRMATH | its PR lands | attr lanes, VM 5F |
| `ow-attr` `AttrWorld` (incl. `rng`, `parts`/`WorldParts`), `EvalCx`, resolver traits, `Rc<AttributeSystem>` signatures | W2-ATTRAPI | its PR lands | attr lanes, balance, items, world |
| `ow-vm` `Vm` (owning rng, clock, attrs; mode passed in), `AttrWorld for Vm`, `WorldHooks`, `NativeCall`, `NativeFn`, `NativePack`, `StubPolicy` and its per-command table, services | W2-VMAPI | its PR lands | interp, core natives, every native pack |
| `ow-balance` types (incl. `LevelCapUpgrades`) | W1-BALTYPES | its PR lands | balance, items, world, combat, AI, save |
| `ow-items::DefinitionData` (`game_stage: GameStage`) | W1-ITEMDATA | its PR lands | items, inventory, save, UI |
| `ow-defs` `DefStore`, `LoadSet`, `IdentityPolicy`, `Overlay` | W3-DEFS | its PR lands | attr lanes, items, VM |
| `ow-behavior` handler registry | W5-BEHAVIOR | its PR lands | combat (damage and loot spawn), skills, mission |
| The C++ CLI's flags, stdout formats, exit codes and asserted stderr text | (the C++ code) | now | every `cmd/` module |

**Review bar for changing a frozen contract:**

- *Additive* (a new variant on a `#[non_exhaustive]` enum, a new method, a new optional field): one reviewer, the PR
  compiles the whole workspace, the doc comment says why.
- *Breaking* (a changed signature, type or meaning): a decision file (`docs/decisions/`), this document's section
  updated in the same PR, every dependent crate fixed in the same PR, two reviews (the hub's reviewer pair: an Opus
  high review and a Luna max review, or Opus 5.5 medium if Luna is out), and the hub batches it so no lane in flight
  is rebased onto it mid-work without notice.
- A lane that finds a contract wrong does not change it in its own PR: it writes the case in its handoff, the hub opens
  a contract PR.

---

## 8. The order: waves of lanes

Waves are **dependency tiers**, not calendar steps: a lane starts as soon as every lane in its "Inputs" has landed.
Sizes: S under ~500 changed lines, M 500-1,000, L 1,000-1,500 (anything bigger is split). "Game" means the
installed packages on disk (zuhu desktop or pc-hummer); **no lane needs a real-game session**. Models follow the brief
(Opus 5.5 medium by default, high for the tricky ones); the hub's `models.md` wins where it says otherwise. Reviews
follow the hub's reviewer policy.

Every lane's done-when also includes the standard gates (`cargo build --workspace --locked`, `cargo build --release
-p ow-cli --locked` (so the manifest lines run the binary of this tree, not a stale one), `cargo test --workspace
--locked`, ctest and `verify_packages.py` unchanged against the C++ build, `tests/crate_edges_test.py`,
`python tools/decisions_index.py --check --baseline origin/main`), a decision file when parsing behaviour changes,
and `local/p2/<lane>/handoff.md` (section 14).

### Wave 0 (one lane, lands before wave 1; under a day)

**W0-SCAFFOLD: the workspace in its final shape** · S-M · game: no · Opus 5.5 medium
- Goal: create the 22 crates of section 2.1 (delete `crates/ow-game`, moving its doc), every `Cargo.toml` with the
  section 2.2 edges, every module file of section 2.3 with its owner-lane header, `lib.rs` files with the `pub mod`
  lines, `Cargo.lock` regenerated; `ow-core` `notyet.rs`, `ident.rs`, `detmap.rs`, `ordmap.rs` and `parity.rs`
  (`OW_PARITY`) complete; the `decode_package` signature returning `NotYet`; the `test-support` feature on
  `ow-package` with the `testkit.rs` placeholder; `ow-cli` `main.rs` dispatching every flag of section 2.3's CLI table
  (all of `src/cli.cpp`'s usage plus the Rust-only flags) to its `cmd/` module, each returning exit code 3 "not ported
  to Rust yet"; `crates/README.md` (crate table, edges, flag map);
  `tests/crate_edges_test.py` (stdlib only: `tomllib`) checking edges and the banned patterns of section 5.1, registered
  in `tests/CMakeLists.txt` (`OW_STANDALONE_PY_TESTS`) only with the maintainer's yes (D15; without it the test is run
  by hand in every lane; approved 2026-11-14, so it is registered); the placeholder types of section 2.3 with
  `new_empty()`.
- Owns: `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, every `crates/*/src/lib.rs`, the module stub files,
  `crates/ow-core/src/{notyet,ident,detmap,ordmap,parity}.rs`, `crates/ow-cli/src/{main,args}.rs`,
  `crates/ow-cli/src/cmd/*.rs` stubs, `crates/README.md`, `tests/crate_edges_test.py`, one line in
  `tests/CMakeLists.txt`.
- Not yet: no logic in any module beyond the five `ow-core` files.
- **W0 delta** (revision 2): the deltas listed in the revision section; if W0 has landed before receiving them, the hub
  runs a small W0b scaffold PR with only those deltas before W5-BEHAVIOR and W6-SKILLS start (the `ow-skills` move
  touches no wave 1-4 lane).
- Done when: workspace builds and tests; the edge test passes and fails on a deliberately bad edge (test case);
  `ow-package --names x` prints the not-ported message with exit 3.
- Warn (sensitive files): `Cargo.toml`. The new ctest entry was approved with D15 (2026-11-14).

### Wave 1 (start together after W0; 12 lanes plus the harness)

All thirteen start when W0 lands (W0 includes the section 15 changes). They do not all **land** together (review finding 14):

| Order | Lanes | Why |
|---|---|---|
| 1st (no wave-1 input) | W1-NUM, W1-RAND, W1-SIM, W1-LZO, W1-OBJ, W1-BALTYPES, W1-ITEMDATA, W1-TESTKIT, W1-HARNESS, W1-PINSPLIT | independent (`GameStage`, `PackageId`, `ModifierId` come from W0) |
| 2nd (after W1-NUM) | W1-ATTRMATH, W1-FIRING (the pow and trig choke points; W1-ATTRMATH returns its own `Combined`, so it does not wait for W1-OBJ); W1-PKG (`fmt`, `json_quote`) | they may be written before W1-NUM lands, against its 3.1 signatures, and rebase once it does |
| final step of W1-PKG | its `--compressed` and real-package checks run after W1-LZO lands | `decode_package` |

W1-NUM is the lane to start first and review first.

**W1-NUM: numerics, units, text** · M-L · game: no · Opus 5.5 high
- Goal: `ow-core::num`, `units`, `fmt` as in 3.1, with the section 5 rules written in the module docs.
- Owns: `crates/ow-core/src/{num,units,fmt}.rs`.
- Inputs: W0. Uses: `weapon_gen.cpp` F32 and FormulaValue, `weapon_recipe.f32`, the C++ float printers listed at the
  top, `item_registry.py:836` (`json.dumps`), `natives_core.cpp:103-171` (casts, transcendentals), `weapon_stats.py`
  card rounding, `tests/card_rounding_cases.json`, `loot_pools.py` (`round()`).
- Not yet: no attribute formula (that is W1-ATTRMATH), no portable `pow` implementation.
- Tests / done when: the pow and transcendental tables of 5.2 (generated on the desktop by both oracles, which agree;
  inputs through `black_box`); every `fmt` function equals its oracle on a stored table of about 500 invented floats
  each (subnormals, powers of ten, halfway cases, negative zero; produced by the C++ build and by Python, which
  agree where both print the format); `json_quote` and `json_quote_py` on invented strings with control and
  non-ASCII characters; `trunc_i32`, `trunc_i32_f64` and `trunc_i64_wrap_i32` edge cases (NaN, infinities, 2^31,
  2^63); `round_half_even` on .5 cases; `card_round` reproduces every case in `card_rounding_cases.json`.

**W1-RAND: one RNG service** · S · game: no · Opus 5.5 medium
- Goal: `ow-core::rand` as in 3.1.
- Owns: `crates/ow-core/src/rand.rs`.
- Inputs: W0. Uses: `weapon_gen.cpp` CrtRand, `weapon_recipe.CrtRand`, `natives_core.cpp` Rand/FRand.
- Not yet: no game sequence reproduction (unknown); no use sites.
- Tests / done when: CrtRand equals the Python and C++ sequences for 1,000 draws from 20 seeds; VmLcg's `rand_limit`
  and `frand` equal the C++ statics (`(state >> 8) % limit`, `(state >> 8) / 2^24`, seeds 12345 and 54321, two
  states); `GameRng::script` returns VmLcg only under `ParityMode::Reference`; stream seeds spread (5.2 item 4);
  streams are independent (drawing one does not move another).

**W1-SIM: simulation clock** · S · game: no · Opus 5.5 medium
- Goal: `ow-core::sim` as in 3.1: `SimClock` and `TickGroup` only.
- Owns: `crates/ow-core/src/sim.rs`.
- Inputs: W0. Uses: NATIVE_ENGINE_CORE.md (tick order, Open on dilation), decision D13.
- Not yet: **no timers** (per-actor timers follow the record's `Timers` array rules and belong to W5-WORLD), no actor
  ticking, no latent script (VM), no Kismet impulse stack (mission).
- Tests / done when: f32 `TimeSeconds` accumulation equals a hand-computed f32 sum; `TimeDilation` scales the step
  (doc: where it enters an actor's delta is UNVERIFIED); replay of a recorded dt list is identical; tick counter.

**W1-LZO: the container** · M · game: yes (final step) · Opus 5.5 medium
- Goal: LZO1X decoder and chunk container written from `research/native_count.py` (no C translated), every size and
  offset checked before use, errors never clamps.
- Owns: `crates/ow-container/src/{lib,lzo,chunks}.rs` (the container has no CLI flag of its own).
- Inputs: W0.
- Also: the **partially compressed** form, as `container.cpp:38-57` reads it (summary walk to the compression table,
  codec 2, chunk bounds against the summary and each other, nested containers with matching decoded sizes, the
  trailing-byte check). The r1 entry said to refuse it; that was wrong (`package_test.py:64-79` expects success).
- Tests / done when: synthetic containers re-expressed from `package_test.py`'s `container()` (valid, truncated, bad
  chunk sizes, bad LZO streams, padded blocks) and its partial case (valid; the six bad-table offsets; trailing bytes)
  give the same accept/refuse as C++; a local run decompresses every package of the
  install and matches C++ byte for byte (hash list under `local/`), the 2 bad remaster packages refused.
- Warn: `crates/ow-container/`.

**W1-PKG: package reader core** · M · game: yes (final step) · Opus 5.5 medium
- Goal: `Reader`, header, name/import/export tables, `Package::load` (through `decode_package`; uncompressed
  synthetic packages work before W1-LZO lands), and the CLI family of `cmd/tables.rs`: `--names`, `--imports`,
  `--exports`, `--census`, `--payload`, `--payload-file`, `--payloads`, `--verify-decoded`.
- Owns: `crates/ow-package/src/{reader,header,tables,package}.rs`, `crates/ow-cli/src/cmd/tables.rs`,
  `crates/ow-cli/src/json.rs`, `tests/rust_parity.d/pkg.txt`.
- Inputs: W0 (W1-NUM's `fmt_g`/`json_quote` when it lands; until then the module returns `NotYet` for float output).
- Not yet: tagged properties, store, class bodies.
- Tests / done when: `tests/package_test.py {bin}` passes on the Rust binary (and `--compressed` once W1-LZO is in);
  `verify_packages.py`'s decoded-bytes, counts and export-field checks pass (the class-body step waits for W2-REFLECT).
- Warn: `crates/ow-package/`.

**W1-OBJ: object model contract** · M · game: no · Opus 5.5 high
- Goal: `ow-object` `ids`, `value`, `names`, `heap`, `layout` as in 3.3, complete and tested with synthetic class
  tables (no package loading). The doc comment states the slot-layout decision (D14), the arena difference, static
  arrays as `array_dim` consecutive slots addressed by `PropAddr`, and the deliberate `!Send` (D18).
- Owns: `crates/ow-object/src/{ids,value,names,heap,layout}.rs`.
- Inputs: W0.
- Not yet: reflection from packages, decoding, CDOs, any VM behaviour.
- Tests / done when: layouts with super chains (a parent's `PropId` valid in a child, refused on an unrelated class);
  copy-on-write of structs and arrays; stale handles refused; static-array elements read and written by index,
  an index at or past `array_dim` refused; `live()` in creation order and skipping destroyed;
  case-insensitive names keeping the first spelling; `Value` equality as UnrealScript `==` per kind (from
  `vm.cpp` `sameValue`).

**W1-ATTRMATH: attribute arithmetic** · S-M · game: no · Opus 5.5 high
- Goal: `ow-attr::math` and `stack` as in 3.5: the stack formula in both `CombineForm`s, duplicate refusal,
  remove-all, plain attributes, int/byte truncation per form, the formula with both `PowForm`s, base-value modes,
  range restriction and rounding.
- Owns: `crates/ow-attr/src/{math,stack}.rs`.
- Inputs: W0 (W1-NUM's pow choke points when they land; the lane may start with the formula tests and wire `pow` last).
- Not yet: resolvers, definitions, objects, notification.
- Tests / done when: the stack and formula cases from `weapon_gen_test.py`, `weapon_balance_test.py` and
  `card_rounding_cases.json` bit-equal under `CombineForm::WeaponGen`, and `interp.cpp`'s 5F cases in `vm_test.py`
  bit-equal under `CombineForm::Vm5F`, all re-expressed as numbers; a test that shows where the two forms differ on
  invented inputs (so 5.3's comparison has a ready case); the report's F4 offset-inside-multiplier case.

**W1-BALTYPES: balance vocabulary** · S · game: no · Opus 5.5 medium
- Goal: `ow-balance::types` as in 3.6, with every range and every UNVERIFIED reading in doc comments.
- Owns: `crates/ow-balance/src/types.rs`.
- Inputs: W0.
- Not yet: any rule (curve, stage, cap): those are W3-BALANCE.
- Tests / done when: `Playthrough::from_index(0).count() == 1`; out-of-range indices refused; `OpLevel` refused
  above 10 and nonzero outside UVHM (as a typed error, labelled UNVERIFIED); `LevelCapUpgrades` a set of the three level-cap
  packs (increments 11, 11, 8 in the records), unknown bits refused with a typed error (independent licensing UNVERIFIED).

**W1-ITEMDATA: item identity** · S · game: yes (read the struct declarations with the C++ tool) · Opus 5.5 medium
- Goal: `ow-items::data` as in 3.7. The field list mirrors `WeaponDefinitionData` and `ItemDefinitionData` as the
  installed `WillowGame.upk` declares them (read with `build/Release/ow-package.exe --object-dump`/`--properties`;
  only field names and types go into the code).
- Owns: `crates/ow-items/src/data.rs`.
- Inputs: W0.
- Not yet: rolls, registry, stats, the save serial packing (D9).
- Tests / done when: slot enums round-trip by index in the game's slot order; a synthetic `DefinitionData` converts to
  and from the struct field order the declaration gives.

**W1-FIRING: firing maths** · M · game: no · Opus 5.5 medium
- Goal: port `src/weapon_firing.*` to `ow-combat::firing`: same functions, f64 as the C++, random numbers passed in,
  rotator units.
- Owns: `crates/ow-combat/src/firing.rs`.
- Inputs: W0 (W1-NUM `pow_f64` for `fireIntervalChanceFactor`).
- Not yet: weapon state, ammo pools, projectiles in a world, status rolls against a world.
- Tests / done when: every case of `tests/weapon_firing_math_test.cpp` re-expressed, bit-equal; the
  `tools/weapon_firing.py` cases that test the same functions reproduce.

**W1-TESTKIT: synthetic packages in Rust** · M · game: no · Opus 5.5 medium
- Goal: `ow-package::testkit` (feature `test-support`) as in 3.2: the Rust counterparts of the `package()`, `tag()`
  and `container()` helpers in `tests/` (`package_test.py`, `properties_test.py`, `runtime_test.py`,
  `global_resolve_test.py`, `assets_test.py`), LZO containers of literal runs, the partial-compression shape. It
  writes bytes only; it does not use the reader, so it runs beside W1-PKG.
- Owns: `crates/ow-package/src/testkit.rs` and `testkit/tests.rs`; declares the placeholder children `testkit/reflect.rs`
  (W2-REFLECT) and `testkit/script.rs` (W2-SCRIPT), not their contents.
- Inputs: W0.
- Not yet: any reading or checking (W1-PKG's tests use it).
- Tests / done when: for each Python helper, the same invented inputs give byte-identical output to the Python helper
  (a Rust test compares against byte strings the Python helpers produced, stored in the test), and
  `build/Release/ow-package.exe` reads the testkit packages.

**W1-HARNESS: the Rust parity runner** · S · game: no · Opus 5.5 medium (or the hub)
- Goal: `tools/rust_parity.py` and the `tests/rust_parity.d/` convention of section 6: `mode=reference` lines (set
  `OW_PARITY=reference` for those, clear it for the rest) and `diff` lines (same arguments on the C++ and the Rust
  binary, stdout and exit code compared, stderr on request); a CI step that runs `cargo build --release -p ow-cli
  --locked` and then the real manifest (approved with D15).
- Owns: `tools/rust_parity.py`, `tests/rust_parity_test.py`, `tests/rust_parity.d/README.md`, the CI step, and its
  own lines in `tests/CMakeLists.txt`: `py-rust_parity` (the self-test) and `rust-parity` (the real manifest over
  `tests/rust_parity.d/`, registered when `target/release/ow-package.exe` exists at configure time, else a configure
  message, as the Node and numpy groups do). `tests/CMakeLists.txt` is line-owned: W0 owns the `crate_edges` line,
  W1-HARNESS these, W1-PINSPLIT its two lines; the hub lands them in that order.
- Inputs: W0.
- Tests / done when: synthetic manifests with a passing, a failing, a `mode=reference` and a `diff` line (against two
  tiny stand-in executables) give the right summary; both ctest entries registered.

**W1-PINSPLIT: pinned stand-in cases into their own files** · S · game: no · Opus 5.5 medium
- Goal: move the cases that pin a reference stand-in out of the test files the Rust binary must pass in Off mode:
  the states block of `tests/vm_test.py` (754-815: `GotoState` events, undeclared state, nesting bound, `label Begin not
  run`) into `tests/vm_states_test.py`, and the stand-in-inventory check of `tests/combat_test.py:934` into
  `tests/combat_loot_standin_test.py` (re-running only the setup that check needs). Both new files and the shortened
  originals pass on the C++ binary; each new file gets its ctest line beside the original's.
- Owns: `tests/vm_test.py` and `tests/combat_test.py` (the moved blocks only), `tests/vm_states_test.py`,
  `tests/combat_loot_standin_test.py`, their two lines in `tests/CMakeLists.txt`. After it lands, W7-RETIRE owns the two
  new files.
- Inputs: W0. Does not touch Rust.
- Done when: ctest passes with the same total of assertions as before (count in the handoff), no assertion lost or
  changed.

### Wave 2

**W2-PROPS: tagged properties** · M · game: yes · Opus 5.5 medium
- Goal: the tag walker, `PropValue`, `ArrayTyper`, `SchemaTyper`, `read_tagged` with the untagged tail position, and
  the CLI `--properties`, `--properties-batch`, `--array-schema`, `--property-offset`.
- Owns: `crates/ow-package/src/{props,schema}.rs`, `crates/ow-cli/src/cmd/properties.rs`,
  `tests/rust_parity.d/props.txt`.
- Inputs: W1-PKG, W1-NUM, W1-TESTKIT.
- Not yet: reflection typing (W3-DECODE), conversion to `Value`.
- Done when: `tests/properties_test.py {bin}` passes; `--properties` byte-identical to C++ on every export of the 9
  code packages and `Startup.upk` with the existing schemas (count in the handoff).

**W2-STORE: package store** · M · game: yes · Opus 5.5 medium
- Goal: `PackageStore` with scan order, lazy load, import resolution and the global fallback index as in C++
  (`buildGlobalIndex`, roots, misses), and `--resolve`.
- Owns: `crates/ow-package/src/store.rs`, `crates/ow-cli/src/cmd/resolve.rs`, `tests/rust_parity.d/store.txt`.
- Inputs: W1-PKG, W1-TESTKIT.
- Not yet: object identity policy (W3-DEFS); the store reports every candidate, it does not choose among packages
  beyond what C++ does.
- Done when: `tests/global_resolve_test.py {bin}` passes; `--resolve` of 10,000 references sampled from the census
  equals C++ (local).

**W2-SCRIPT: bytecode decoder** · M-L · game: yes · Opus 5.5 medium
- Goal: port `src/script.*`: `readFunction`, `decode`, `format`/`disassemble`, `checkPackage`; CLI `--script-check`,
  `--disasm`.
- Owns: `crates/ow-script/src/*`, `crates/ow-cli/src/cmd/script.rs`, `tests/rust_parity.d/script.txt`,
  `crates/ow-package/src/testkit/script.rs`.
- Inputs: W1-PKG, W1-TESTKIT.
- Not yet: execution.
- Done when: the cases of `tests/script_disasm_test.py` (which tests only the Python decoder) re-expressed as Rust
  unit tests on `testkit` packages; `--disasm` and `--script-check` byte-identical to C++ on those synthetic packages;
  `tools/verify_scripts.py` with the Rust binary on the real packages: 12,968/12,978 with the same 10 failures;
  `--disasm` identical to C++ for every function (local).

**W2-REFLECT: reflection and class bodies** · M-L · game: yes · Opus 5.5 high
- Goal: load Class/Struct/Enum/Property exports into `ClassTable` layouts (including the attribute companions and the
  byte-attribute shift), `decode_class_body`, interface tables; CLI `--class-check`, `--interface-compare`.
- Owns: `crates/ow-object/src/{reflect,class_body}.rs`, `crates/ow-package/src/testkit/reflect.rs`, `crates/ow-cli/src/cmd/classes.rs`,
  `tests/rust_parity.d/reflect.txt`.
- Inputs: W1-PKG, W1-OBJ, W1-TESTKIT, W2-STORE (cross-package super classes).
- Not yet: tagged default values, CDOs.
- Done when: **the Phase 0R reader gate**: `verify_packages.py --reader target/release/ow-package.exe` passes in full
  (9 packages, 3,339/3,339 class bodies exact); `--interface-compare` identical to C++.

**W2-VMAPI: VM contract** · S-M · game: no · Opus 5.5 high
- Goal: `ow-vm` `api.rs`, `services.rs`, `error.rs` as in 3.8, with a minimal `Vm` that owns the space, the store,
  `Rc<AttributeSystem>`, `GameRng`, `SimClock` and the parity mode, can register natives, hold services and call a
  native directly (no interpreter), `StubPolicy`, and `impl AttrWorld for Vm` (globals, game info and engine queries
  through the `WorldHooks` service, `NotYet { LayerAbsent("ow-world") }` without it; `rng()` from `Vm::rng`).
- Owns: `crates/ow-vm/src/{api,services,error}.rs`.
- Inputs: W1-OBJ, W1-RAND, W1-SIM, W2-ATTRAPI's trait signatures (the two lanes agree the `AttrWorld` text first:
  W2-ATTRAPI lands it, W2-VMAPI implements it; the hub sequences the two PRs).
- Not yet: interpreter, script calls (they return `NotYet { NotPorted }`).
- Done when: unit tests build `Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off)`, register
  a native, call it with out-args, draw from `vm.rng` inside it, use a service, call an `AttributeSystem` method with
  `EvalCx::new(&mut vm)` from inside a native (the clone-the-Rc pattern compiles), and get `UnboundNative` under
  `Refuse`.

**W2-ATTRAPI: attribute system contract** · S-M · game: no · Opus 5.5 high
- Goal: `ow-attr` `api.rs`, `registry.rs` as in 3.5: `AttrWorld` (with `rng`), `EvalCx` (no RNG field), resolver
  traits, `AttributeSystem` (immutable, shared as `Rc`, `new_empty()` kept) with every public method returning
  `NotYet { NotPorted }` until W3 fills it, the registry returning `NotYet { NotRead }` for an unknown resolver class,
  and a test `AttrWorld` over a synthetic `ObjectSpace`.
- Owns: `crates/ow-attr/src/{api,registry}.rs`; in review r1 it also placed the `NotYet { NotPorted }` stubs in
  `eval.rs`, `expr.rs`, `effects.rs`, `storage.rs`, `value_resolvers/mod.rs` and `context_resolvers/mod.rs`, which
  stay their W3 owners' files (each `AttributeSystem` method is a one-line delegate into them).
- Inputs: W1-OBJ, W1-ATTRMATH, W1-RAND. (Takes `DefStore` by type; until W3-DEFS lands the test world uses a
  hand-filled space and `parts().defs` is `None`, which the methods turn into `NotYet`.)
- Done when: the test world compiles against every trait; an unknown resolver class gives the typed error.

**W2-ASSETS: textures and meshes (host track)** · M · game: yes · Opus 5.5 medium
- Goal: port `src/assets.*` into `ow-assets`: TFC reads, mips, PNG writer (stored deflate, own CRC/Adler as C++
  does), static mesh LODs, OBJ writer; the CLI family of `cmd/assets.rs` incl. `--terrain-records` and
  `--scene-records` (used by `tools/prepare_level.py`, `tools/collision_volumes.py`); stderr texts the tests assert
  (`'supports PF_G8 only'`, `assets_test.py:163`).
- Owns: `crates/ow-assets/src/*`, `crates/ow-cli/src/cmd/assets.rs`, `tests/rust_parity.d/assets.txt`.
- Inputs: W1-PKG, W1-TESTKIT, W2-STORE. (Uses the C++ `--property-offset` values.)
- Not yet: skeletal meshes, animation, materials, levels (after the engine pick).
- Done when: `tests/assets_test.py {bin}` passes; the local 200+200 sample is byte-identical.

### Wave 3

**W3-DECODE: typed values and class defaults** · L · game: yes · Opus 5.5 high
- Goal: `ReflectionTyper`, `to_value`, class default objects (`applyTaggedDefaults` semantics incl. inherited
  defaults), `instantiate_export` with `PropertyPrefix` (copied from C++), struct defaults typed through reflection
  (C++ leaves arrays inside struct defaults empty: this lane decodes them and reports the difference), CLI
  `--object-dump`.
- Owns: `crates/ow-object/src/{decode,cdo,prefix}.rs`, `crates/ow-cli/src/cmd/objects.rs`,
  `tests/rust_parity.d/decode.txt`.
- Inputs: W2-PROPS, W2-REFLECT.
- Not yet: the identity policy and overlays (W3-DEFS).
- Done when: `--object-dump` equals C++ on every CDO of the 9 packages except the struct-default arrays C++ leaves
  empty (each such difference listed); the reflection typer equals the schema typer on every schema entry over the
  census, after which the schemas and `SchemaTyper` are deleted in the same PR; report whether context-resolver chain
  order now decodes (report F3 Open).

**W3-DEFS: definition store** · L · game: yes · Opus 5.5 high
- Goal: `DefStore` as in 3.4: `FirstInScanOrder` and `Loaded(LoadSet)`, `owners()`, lazy loading through W3-DECODE,
  overlays (parse the file `tools/real_game/live_overlay.py` writes, plus a plain test format), `origin()`,
  `raw_tail()`, `key()`, `exports_of_class()`.
- Owns: `crates/ow-defs/src/*`, `crates/ow-cli/src/cmd/defs.rs` (a `--def-dump <path> [--overlay <file>]` flag,
  new and Rust-only), `tests/rust_parity.d/defs.txt`.
- Inputs: W2-STORE, W3-DECODE.
- Not yet: any rule reading definitions.
- Done when: the `ow-defs` row of the section 6 table; default overlay set is empty (D1, approved). Static-array
  patches (`Prop[3].Field`) address `PropAddr`.

**W3-SWITCH: tools use the Rust reader** · S · game: yes · Opus 5.5 medium
- Goal: move the `--reader` default of the Python tools to the Rust binary (one helper in `tools/repo_paths.py`,
  callers use it), per tool: a tool moves when every flag it calls is ported; the rest keep the C++ default and are
  listed with the flag they wait for (a VM or combat flag moves in a later small switch lane). Freeze the C++ reader
  (header comment).
- Owns: the `--reader` default lines in `tools/*.py`, `tools/repo_paths.py`.
- Inputs: W2-PROPS, W2-STORE, W2-REFLECT, W3-DECODE, W2-ASSETS (for the mesh and texture tools).
- Done when: every registered `py-*` ctest entry passes, with the moved tools on the Rust binary.

**W3-INTERP: interpreter core** · L · game: yes (sweep) · Opus 5.5 high
- Goal: `ow-vm::interp`: frames, locals, params and out params, return, jumps, `switch`, `goto`/labels, context and
  class context, casts, constants, virtual/final/global/super calls, native dispatch (incl. numbered natives), the
  Gearbox hidden locals 0x4C-0x50, 0x5E read and 0x5F through `ow_attr::let_base`, step limit, census and trace hooks;
  CLI `--run`, `--run-batch`, `--native`, `--native-args`, `--self`, `--arg`, `--steps`.
- Owns: `crates/ow-vm/src/interp/*` (except the three W4-INTERP2 files, created as `NotYet` stubs here),
  `crates/ow-cli/src/cmd/vm.rs`, `tests/rust_parity.d/vm.txt`.
- Granted edits in W2-VMAPI's `crates/ow-vm/src/api.rs` (hub decision, W2-VMAPI reviews r1 and r2): the body of
  `Vm::run_frame` (one bytecode frame; `Vm::run_bytecode` around it keeps the depth bookkeeping, so the counter unwinds
  on every path), function and class loading behind `Vm::find_function` / `Vm::find_class`, calls of the
  `pub(crate)` seams `Vm::tick` and `Vm::stub_site`, and, once frames exist, the C++ `<- caller` trace in the
  depth-limit message (`interp.cpp:711-713`). Any other change to `api.rs` or `error.rs` is a contract change
  (section 7). The 0x5F typed let converts the right-hand side with the VM's assignment rule (`interp.cpp:511-522`)
  before `let_base`, which refuses another kind.
- Inputs: W2-SCRIPT, W2-VMAPI, W3-DECODE, W3-DEFS, W1-ATTRMATH.
- Not yet: struct/array element ops, `foreach`, delegates, interfaces (W4-INTERP2), states and latent code (W5-STATES).
- Done when: Rust unit tests on `testkit` packages for every feature this lane owns, re-expressed from the
  `vm_test.py` and `runtime_test.py` cases that use only these features; `diff` manifest lines (`--run`, `--run-batch`,
  `--native` on those packages) equal to C++. The whole-file lines (`vm_test.py`, `runtime_test.py`) are W4-INTERP2's
  gate; `tests/vm_states_test.py` is a `mode=reference` line from W3-CORENAT on.

**W3-CORENAT: Core natives** · M · game: no · Opus 5.5 medium
- Goal: port `natives_core.cpp` onto the W2-VMAPI registry: operators, math (through the `ow_core::num` choke points),
  strings, names, object natives; float-to-int with `trunc_i64_wrap_i32` where the C++ goes `int64_t(double)`;
  `Rand`/`FRand` on `vm.rng.script(ScriptStream::Rand / FRand)` (VmLcg under `ParityMode::Reference`, CrtRand
  otherwise); `GotoState` keeps the C++ name-only approximation only under `ParityMode::Reference`, else
  `NotYet { NotPorted }` until W5-STATES. `GameRng` is a `Vm` field, so this lane needs nothing from wave 5.
- Owns: `crates/ow-vm/src/natives_core.rs`, `tests/rust_parity.d/corenat.txt`.
- Inputs: W2-VMAPI, W1-NUM, W1-RAND.
- Stand-ins met (6.1): `tests/vm_states_test.py` (the split states block, as a `mode=reference` line) and the VM LCGs
  (pinned only by `--vm-sweep`, W4-VMTOOLS's gate), each with its Rust-native replacement test (`NotYet` for
  `GotoState`, the CrtRand script streams).
- Stub policy: the exploration commands log and return zero (3.8 table); `natives_test.py:16-20` asserts it and is not
  a stand-in.
- Done when: every native has a direct unit test from the C++ expectations; `natives_test.py {bin}` passes once
  W3-INTERP is in.

**W3-ATTREVAL: the one evaluator** · L · game: yes (real-data parity) · Opus 5.5 high
- Goal: `AttributeSystem::evaluate_init`, `value_of`, `expressions_hold` and the value resolvers the notes read
  (ObjectProperty incl. the inherited-property question, Constant, SimpleMath, Conditional, Global slots 0/1,
  PlayerSkill, ResourcePoolState, AttributeSlotEffect, Badass, Manufacturer, WeaponType); random variance through
  `AttrWorld::rng(CrtStream::AttributeVariance)`; every other resolver class `NotYet { NotRead }`.
- Owns: `crates/ow-attr/src/{eval,expr}.rs`, `crates/ow-attr/src/value_resolvers/*`. The bodies of `resolve_context`,
  `resolve_contexts`, `value_of`, `base_value_of`, `value_from_context`, `base_value_from_context` and `evaluate_init`
  are `eval.rs`'s stubs, `expressions_hold`'s is `expr.rs`'s, and the registrations go in `value_resolvers::register`;
  `registry.rs` (W2-ATTRAPI) only delegates and is not edited. Context resolvers, NoContextNeeded included, are
  W3-ATTRCTX's to register; until that lane lands, this lane's parity tests register NoContextNeeded in their own test
  world.
- Inputs: W2-ATTRAPI, W3-DEFS.
- Not yet: context resolvers beyond NoContextNeeded and a fixed test context (W3-ATTRCTX); effects (W3-ATTREFFECTS).
- Done when: the `ow-attr` gate of section 6 for the evaluator fixtures; section 5.3 settled or reported for both
  `PowForm` and `CombineForm`; the `Weapon_Is_<Maker>` name match replaced by the Manufacturer resolver with the
  weapon_gen cases still passing.

**W3-ATTRCTX: context resolvers** · M · game: no · Opus 5.5 medium
- Goal: the 16 context resolver classes the notes read (Controller, Pawn, Weapon, OffHandWeapon, PlayerController, PRI,
  NoContextNeeded, ResourcePool, WeaponResourcePool, GameInfo, BalancedActor, Inventory, EquippedInventory,
  StatusEffectChance, Designer, Skill) over `AttrWorld`, engine-side lookups through `engine_query`; the ~8 unread
  `NotYet`.
- Owns: `crates/ow-attr/src/context_resolvers/*`. The registrations go in `context_resolvers::register`, which
  `AttributeSystem::standard` calls; `registry.rs` (W2-ATTRAPI) is not edited. Every context resolver class,
  NoContextNeeded included, is registered there by this lane.
- Inputs: W2-ATTRAPI, W3-DEFS.
- Done when: each resolver has a synthetic-world test (chain ends at the first None; empty chain is None; override
  context first) and the `pawn`/`mind`/`controller` cases of `progression.hpp` reproduce.

**W3-ATTREFFECTS: modifiers on objects, effects, notification** · M · game: no · Opus 5.5 high
- Goal: `add_modifier`, `remove_modifier`, `set_base`, `apply_effects` (value evaluated once at apply time, override
  context first, applied records kept), `remove_effects`, plain-attribute permanence, notification only for notify-flag
  properties through `AttrWorld::attribute_changed`.
- Owns: `crates/ow-attr/src/{effects,storage}.rs`. The bodies of `add_modifier`, `remove_modifier`, `set_base`,
  `apply_effects`, `remove_effects` are `effects.rs`'s stubs; the by-name natives (`add_modifier_by_name`,
  `remove_modifier_by_name`, `attribute_value_by_name`) and `let_base` are `storage.rs`'s; `registry.rs` (W2-ATTRAPI)
  only delegates and is not edited. The chain outcome rule of 3.5 holds: no rollback.
- Inputs: W2-ATTRAPI, W3-DEFS, W1-ATTRMATH.
- Done when: the effect cases of `skill_effects_test.py` and `weapon_gen_test.py` (part effects in slot order),
  re-expressed as Rust tests (both files test other binaries or Python tools), and the `ResourcePool` clamp rule
  reproduce in synthetic tests; respec of a plain attribute keeps the edit (NATIVE_ATTRIBUTES
  correction) with its own test.

**W3-BALANCE: balance service** · M · game: yes · Opus 5.5 high
- Goal: section 3.6 rules: XP curve, region game stage (stored where the game stores it, first query wins), item stage
  cap, the player max level (`GetMaxExpLevel`: 50 + the increment of each licensed level-cap pack, clamped to 50 + the
  sum of all; NATIVE_PROGRESSION.md, "Level cap"), playthrough modifiers, mission and kill XP, skill points, `OfflineWorld`.
- Owns: `crates/ow-balance/src/{curve,stage,cap,playthrough,xp,offline}.rs`.
- Inputs: W1-BALTYPES, W3-ATTREVAL, W3-ATTRCTX.
- Not yet: UVHM region stage (`NeedsRealGame(2)`), playthrough modifier effects above index 0 where unread, kill XP
  outside PT1 single player (UNVERIFIED or `NotYet` per record), Badass rank.
- Done when: the `ow-balance` gate of section 6.

### Wave 4

**W4-INTERP2: aggregates, iteration, delegates** · M-L · game: yes (sweep) · Opus 5.5 high
- Goal: struct and dynamic-array ops (element, length, insert, remove, add, find, sort, struct compare), `foreach`
  over iterator natives and dynamic arrays, delegates, interface context and casts.
- Owns: `crates/ow-vm/src/interp/{aggregate,iter,delegate}.rs`.
- Inputs: W3-INTERP.
- Done when: all of `vm_test.py` (after W1-PINSPLIT), `runtime_test.py`, `natives_test.py` pass on the Rust binary in
  Off mode; `tests/vm_states_test.py` passes as a `mode=reference` line; the cases of `ui_trace_returns_test.py` (a Python tool on fake data) re-expressed as Rust
  unit tests of the scalar trace replay.

**W4-VMTOOLS: census and sweep** · M · game: yes · Opus 5.5 medium
- Goal: port `census.*` and `vm_sweep.cpp`; CLI `--native-census`, `--vm-sweep`, `--native-selftest`.
- Owns: `crates/ow-vm/src/{census,sweep}.rs`, `crates/ow-cli/src/cmd/census.rs`.
- Inputs: W4-INTERP2, W3-CORENAT.
- Done when: `--vm-sweep` JSON identical to C++ on the 9 packages under `OW_PARITY=reference` (the `ow-vm` gate);
  then freeze the C++ VM files.

**W4-REGISTRY: item registry** · M · game: yes · Opus 5.5 medium
- Goal: the registry of every kind incl. DLC from `DefStore`, emitting the `openwillow-item-registry/1` JSON
  (`--item-registry`, Rust-only flag).
- Owns: `crates/ow-items/src/registry.rs`, `crates/ow-cli/src/cmd/items.rs` (registry part; W4-ROLL/STATS/POOLS add
  their subcommands in their own `cmd/items_<name>.rs` files).
- Inputs: W1-ITEMDATA, W3-DEFS.
- Done when: JSON byte-identical to `tools/item_registry.py` on the installed game; `tests/item_registry_test.py`
  re-expressed in Rust on synthetic packages.

**W4-ROLL: item roll** · L · game: yes · Opus 5.5 high
- Goal: merge (both the root-most and leaf-most readings kept as named functions, the 12+8 differing balances in a
  test, one chosen per record), manufacturer candidates at the pool level (the game's place, report L6 trap), part pick
  (`frand` form per the oracle), relic hook, names with expressions (`WeaponDamage_Is_<Element>` through the
  evaluator), rarity, value, level; CLI `--item-roll`.
- Owns: `crates/ow-items/src/{merge,candidates,pick,names,rarity,value,roll}.rs`, `crates/ow-cli/src/cmd/items_roll.rs`.
- Inputs: W4-REGISTRY, W3-ATTREVAL, W3-BALANCE, W1-RAND.
- Not yet: stats, pools.
- Roll API: one `CrtRand(seed)` per roll through `RollWorld` (3.7), as the oracle; part picks iterate an
  `OrderedMap` (a repeated part keeps its first position with the later weight).
- Stand-ins met (6.1), reproduced only under `mode=reference` for the census and `weapon_recipe.roll` comparisons,
  each with Rust-native tests of the Off-mode rule: runtime-state weights as 1 and window bounds as none
  (`item_roll.py:34-35, 317`); the manufacturer drawn in the balance roll (`item_roll.py:25-26`; Off mode expands one
  candidate per manufacturer at the pool level, a different CrtRand draw order); the uniform class-mod definition pick
  (`item_roll.py:29-31`; Off mode `NotYet`). Name parts in the rarity sum (`item_roll.py:47, 82`) is a reading, not a
  stand-in: the same in both modes, UNVERIFIED.
- Done when: 35,660 census rolls pass `item_roll.check` (`tools/item_roll.py` pointed at Rust output, `mode=reference`);
  weapon rolls equal `weapon_recipe.roll` for the parity seeds (`mode=reference`); the Off-mode pool-level expansion and
  the class-mod `NotYet` have their own tests; stage windows that need runtime state evaluated through the
  evaluator, the grenade-mod per-element minimum labelled per question 7.

**W4-STATS: stats and cards** · L · game: yes · Opus 5.5 high
- Goal: `instantiate_stats` (bases from the weapon type, type effects, part effects in slot order, internal slot
  effects, name-part effects, all through `ow-attr`), non-weapon item stats (shield, grenade mod, class mod, relic,
  usable) the same way, `card` (presentation rounding, status chance mapping, slag clamp); bind
  `WillowWeapon.CalculateWeaponBaseValues` through `override_script` is W5-EQUIP's job, not this lane's.
- Owns: `crates/ow-items/src/{stats,cards}.rs`, `crates/ow-cli/src/cmd/items_stats.rs`.
- Inputs: W4-ROLL (data), W3-ATTREVAL, W3-ATTRCTX, W3-ATTREFFECTS, W3-BALANCE.
- Done when: 2,919,834 fields bit-equal to Python (`weapon_gen_parity.py` given the Rust output); golden cards 69/69
  with the live overlay (`tools/real_game/golden_card_compare.py`); the `prepare_stock_shield_test.py` card
  reproduced at its level through the real evaluator; then `src/weapon_gen.*`, `weapon_gen_cli.cpp`,
  `tools/weapon_gen_manifest.py`, `tests/weapon_gen_test.py` and `tests/weapon_gen_parity_test.py` deleted (D4), their
  cases already re-expressed in Rust (W1-ATTRMATH, W3-ATTREFFECTS, this lane).

**W4-POOLS: item pools** · M · game: yes · Opus 5.5 medium
- Goal: pool roll (listed-pool chance, quantity draws over eligible shares, stage requirements, nested pools,
  `bAutoReadyItems`, caller variance clamp), the quantity rounded with `round_half_even` as the oracle does
  (UNVERIFIED as a game rule), class mod pools filled at run time (decision 2026-11-13; the definition pick inside a
  class-mod roll is W4-ROLL's, 6.1), the pool-level manufacturer candidates W4-ROLL defines; CLI `--item-pool`.
- Owns: `crates/ow-items/src/pools.rs`, `crates/ow-cli/src/cmd/items_pools.rs`.
- Inputs: W4-ROLL, W3-BALANCE.
- Done when: 7,680 pool rolls equal `loot_pools.py`'s CrtRand path (`roll_crt` / `roll_pool_crt`; the
  `random.Random` path `roll` / `roll_pool` is not an oracle).

### Wave 5 (world and first native packs)

| Lane | Goal | Owns | Inputs | Size / game / model |
|---|---|---|---|---|
| W5-WORLD | `World { vm }`, the `WorldHooks` service behind `Vm`'s `AttrWorld` (globals, game info, engine queries, balance state), actor spawn/destroy, `AllActors`/`DynamicActors` iterators, tick groups, **per-actor timers from NATIVE_ENGINE_CORE.md "Actor timers"** (the `Timers` array on the actor: replace-not-add on (name, object), `Count` grown by `TimerTimeDilation * delta` unless paused, due when `Count > Rate` strictly, a loop fires `floor(Count / Rate)` times, culling during the pass with the length re-read, pause, `GetTimerCount`/`GetTimerRate`, the order from the actor tick; dilation of the delta UNVERIFIED), `EvaluateInitializationData` native | `ow-world/src/{world,query,actors,timers}.rs` | W4-INTERP2, W3-CORENAT, W3-BALANCE, W1-SIM | L / yes / Opus high |
| W5-STATES | States, `GotoState`/`Enable`/`Disable` per NATIVE_ENGINE_CORE, latent functions with the 1/60 floor, label jumps | `ow-vm/src/{states,latent}.rs` | W4-INTERP2, W1-SIM | M-L / yes / Opus high |
| W5-POOLS | Resource pools, pool manager natives, pawn health glue, level-up rebase (verified live: base and max rewritten, current untouched) | `ow-world/src/{pools,levelup}.rs` | W5-WORLD, W3-ATTREFFECTS | M / yes / Opus medium |
| W5-BEHAVIOR | Provider decode (fitted variable block via `raw_tail`), kernel dispatch rules A1/A2, the handler registry (packs above register the classes they own; unregistered is `NotYet`), the world-only handlers; `--behavior-dump`, `--behavior-run` | `ow-behavior/src/{provider,kernel,registry,handlers/}`, `ow-cli/src/cmd/behavior.rs` | W5-WORLD | L / yes / Opus high |
| W5-KISMET | Kismet sequence ops, impulse stack, logic ops; `--kismet-census`, `--kismet-run` | `ow-mission/src/kismet.rs`, `ow-cli/src/cmd/kismet.rs` | W5-WORLD | M / yes / Opus medium |
| W5-DIALOG | Dialog and movers; `--mover-event`, `--mover-probe` | `ow-mission/src/{dialog,mover}.rs`, `ow-cli/src/cmd/mover.rs` | W5-WORLD | M / yes / Opus medium |
| W5-EQUIP | Inventory manager, equip/unequip through `apply_effects`/`remove_effects` (controller source, item override), item spawn natives calling `ow-items` (no behavior handlers: loot-spawn behaviors are `ow-combat::loot`'s), `override_script` of `CalculateWeaponBaseValues` and `ApplyAttributeEffects` with the differential test (installed script vs Rust rule on 50 weapons); `--inventory-move` | `ow-inventory/src/{manager,equip,spawn}.rs`, `ow-cli/src/cmd/inventory.rs` | W5-WORLD, W4-STATS | L / yes / Opus high |

### Wave 6 (slice systems)

| Lane | Goal | Owns | Inputs | Size / game / model |
|---|---|---|---|---|
| W6-MISSION | Mission tracker (what C++ has; unmodelled rules `NotYet`), script bridge, controller mission records per playthrough; `--mission`, `--mission-run` | `ow-mission/src/{tracker,bridge}.rs`, `ow-cli/src/cmd/mission.rs` | W5-BEHAVIOR, W5-KISMET, W5-POOLS, W3-BALANCE | L / yes / Opus high |
| W6-FIRE | The Fire slice (`FireMissionSlice`), `--slice-run`; Fire event order 33/33 | `ow-mission/src/slice.rs`, `ow-cli/src/cmd/slice.rs` | W6-MISSION, W5-DIALOG, W5-STATES | M / yes / Opus high |
| W6-COMBAT | Hit chain, damage pipeline natives, death, down/FFYL, respawn, kill-XP glue, the damage-dealing behavior handlers (registered into the kernel), the scripted run S-a..S-f; `--combat-run`; the 6.1 "0 with base 0" rows as `mode=reference` only | `ow-combat/src/{hit,damage,death,down,xp,run}.rs`, `ow-cli/src/cmd/combat.rs` | W5-POOLS, W5-EQUIP, W5-BEHAVIOR, W1-FIRING | L (split hit+damage / death+down+xp if over) / yes / Opus high |
| W6-LOOT | Loot on death through `ow-items` pools (no stand-in inventories; `tests/combat_loot_standin_test.py` as a `mode=reference` line, replaced by a Rust-native test), the loot-spawn behavior handlers registered into the kernel, S-g rows | `ow-combat/src/loot.rs` | W6-COMBAT, W4-POOLS | M / yes / Opus medium |
| W6-SKILLS | Skills, skill trees, action skill start and refusals, cooldown pool, Phaselock duration `5 + 0.5 g` (the lift of an AI target through the installed script and world natives, no `ow-ai` edge), skill behavior handlers; the cases of `skill_effects_test.py` and `skill_stats_test.py` re-expressed as Rust tests | `ow-skills/src/{skills,tree,action_skill,phaselock}.rs` | W5-BEHAVIOR, W5-EQUIP, W5-POOLS, W6-COMBAT (damage, held weapon) | L / yes / Opus high |

### Wave 7 and the host track

- **W7-AI** (several lanes, split by the AI notes: awareness and targeting first, then melee, then leaps and throws):
  `ow-ai`, from NATIVE_AI_*.md and the live records; needs W6-COMBAT.
- **W7-WORLDLOGIC**: vendors, interactive objects, stations (`ow-mission/src/{vendors,interactive}.rs`); needs
  W4-POOLS, W6-MISSION.
- **W7-SAVE**: `ow-save` model and load order over `DefinitionData`; the on-disk format only after D9.
  Items from DLC that is not installed are kept by the save as opaque `SerialNumber` records with their slot fields
  (weapons: `QuickSlot`, `Mark`; items: `Quantity`, `bEquipped`, `Mark`) in
  `PlayerSaveGame.UnloadableDlcWeaponSaveGameData` / `UnloadableDlcItemSaveGameData`, and cannot be an
  `InventoryInstance`; the save model needs its own carrier for them.
- **W7-UI**: `ow-ui` bridge natives and the `MoviePlayer` trait; needs the engine pick for the implementation.
- **W7-RETIRE**: delete `src/`, `third_party/lzokay`, the C++ targets and the ctest entries that only test the C++
  binary, once the Rust binary passes every one of them (`tools/rust_parity.py` over the full list); a sensitive-file
  change (`CMakeLists.txt`), with the maintainer's yes. It owns `tests/vm_states_test.py` and
  `tests/combat_loot_standin_test.py` from W1-PINSPLIT on. Two conditions: every stand-in of 6.1 has its Rust-native
  replacement test before the `mode=reference` line that pins it, and `ParityMode::Reference` with it, is deleted;
  the tests that drive only `ow-weapongen` (`weapon_gen_test.py`, `weapon_gen_parity_test.py`) are already gone
  (deleted at W4-STATS, not passed).
- **Host track** (after the maintainer's engine pick, the benchmark lanes under `local/bench/` untouched): H1 the
  engine crate in the tree (its `THIRD_PARTY.md` entry first), H2 scene loading from `ow-defs`/`ow-assets` (Sanctuary),
  H3 the unit/axis boundary module, H4 movement and collision per NATIVE_PLAYER_MOVEMENT, H5 the `MoviePlayer`
  implementation. `W2-ASSETS` is the only host-side lane before the pick.

### 8.1 Critical path and parallelism

```
W0 ─┬─ W1-NUM ─┬─ W1-PKG (+ W1-TESTKIT, W1-LZO) ─┬─ W2-PROPS ─┐
    │          ├─ W1-ATTRMATH ─► W2-ATTRAPI       ├─ W2-STORE ─┼─ W2-REFLECT ── W3-DECODE ── W3-DEFS ─┬─ W3-ATTREVAL ─┬─ W3-BALANCE ── W4-ROLL ── W4-STATS ── W5-EQUIP ─┐
    │          │                                  └─ W2-SCRIPT ┘                                       ├─ W3-ATTRCTX  ─┘                                                ├─ W6-* (skills after W6-COMBAT)
    ├─ W1-OBJ, W1-RAND, W1-SIM ── W2-ATTRAPI ── W2-VMAPI ──────────────────────────────────────────────┴─ W3-INTERP ── W4-INTERP2 ── W5-WORLD ─────────────────────────┘
    │          └─ W1-FIRING (leaf until W6-COMBAT)
    └─ W1-BALTYPES, W1-ITEMDATA, W1-HARNESS, W1-PINSPLIT (leaves until wave 3-4)
```

Peak parallelism: 13 in wave 1 (landing in the order of the wave 1 table), 7 in wave 2, 9 in wave 3 (decode/defs, switch, interp, corenat, three attr lanes,
balance), 6 in wave 4. The longest chain is reader -> reflection -> decode -> defs -> evaluator -> roll -> stats ->
equip; it is also the chain the maintainer's requirement is about, which is why every contract on it is frozen first.

---

## 9. Python rule tools

| Family | Tools | Recommendation | Wave |
|---|---|---|---|
| Item registry and roll | `item_registry.py`, `item_roll.py` (+ `check`) | **Port** (W4-REGISTRY, W4-ROLL). Freeze both at the Rust gate; delete `item_registry.py` and the roll half of `item_roll.py` once nothing reads their output. **Keep `item_roll.check` permanently** as the independent checker (it was written apart from the roll). | 4 |
| Weapon recipe and generator | `weapon_recipe.py`, `weapon_gen_manifest.py`, `weapon_gen_parity.py`, C++ `weapon_gen.*` | **Port** into `ow-items` reading the store (W4-ROLL, W4-STATS). Delete the C++ generator and the manifest tool at the W4-STATS gate (D4). Keep `weapon_recipe.py` frozen as the offline oracle until Rust = Python on the 2.9 M fields **and** 69/69 golden cards, then delete it; the golden cards (real game) remain the oracle. `weapon_gen_parity.py` is rewritten to compare Rust against the frozen Python, then deleted with it. | 4 |
| Stock shield | `prepare_stock_shield.py` (`SLICE_LEVEL = 8`) | **Retire** (UE5-era slice prep). Its evaluator subset is replaced by W4-STATS non-weapon stats at the real stage; its test cases move into Rust. Delete at the W4-STATS gate. | 4 |
| Weapon stats and cards | `weapon_stats.py`, `weapon_card_audit.py`, `tools/real_game/golden_card_compare.py` | **Port** `weapon_stats.py` (W4-STATS), freeze, delete with `weapon_recipe.py`. **Keep** `tools/real_game/golden_card_compare.py` and `weapon_card_audit.py` as tools (they compare against real-game rows; they consume output, they hold no rule). | 4 |
| Loot pools | `loot_pools.py` | **Port** (W4-POOLS; the oracle is its CrtRand path, `roll_crt`/`roll_pool_crt`), freeze as oracle, delete when the VM loot path (W6-LOOT) passes the live rows. | 4, delete 6 |
| Progression | `slice_values.py`, `skill_stats.py`, `mission_closure.py` XP parts | **Port** the rules (W3-BALANCE; `skill_stats.py` into W6-SKILLS). Delete `slice_values.py` at the W3 gate and `skill_stats.py` at W6-SKILLS. **Leave** `mission_closure.py` (analysis: which objects a mission needs), except any XP rule in it, which must call the Rust CLI instead of re-implementing. | 3, 6 |
| Firing | `weapon_firing.py`, `weapon_gen_firing.py`, `check_weapon_firing.py` | **Port** (W1-FIRING), delete the two rule tools when nothing calls them. | 1 |
| Vendors | `prepare_vendors.py` (`item_parts`) | **Port** the rules (W7-WORLDLOGIC); the prep tool goes with the UE5-era slice prep. | 7 |
| Disassembly, reader oracles | `research/script_disasm.py`, `research/native_count.py`, `verify_packages.py`, `verify_scripts.py` | **Leave** as permanent independent oracles (structural, no gameplay rule). | - |

Rule for all: from wave 1 on, no new rule is added to a Python rule tool; a newly read rule goes into Rust only.

---

## 10. Contracts the brief calls "frozen first", by owner (summary for the hub)

W0-SCAFFOLD (graph, `NotYet`, ident, maps, `ParityMode`, CLI table, placeholders) -> W1-NUM, W1-RAND, W1-SIM,
W1-OBJ, W1-ATTRMATH, W1-BALTYPES, W1-ITEMDATA, W1-TESTKIT (vocabulary) -> W1-PKG + W2-PROPS (package types) ->
W2-ATTRAPI then W2-VMAPI (upward traits; the VM implements `AttrWorld`) -> W3-DEFS (store) -> W5-BEHAVIOR (handler
registry). Section 7 has the review bar.

---

## 11. Maintainer decisions

The report's 12, then five more, then one from review r1. **D1-D18 were approved by the maintainer as recommended
on 2026-11-14** (D18 after review r1); the "alternative" column is kept as the record of what was weighed.

| # | Question | Recommendation | Under the recommendation | Under the alternative |
|---|---|---|---|---|
| D1 | Cooked or live values; overlay now? | **Overlay layer now, cooked as the default runtime source, the live-capture overlay used by the card and stat gates** until question 3 finds the origin. | W3-DEFS builds overlays; `ow-host` loads none by default; gates name the overlay file under `local/`. | *Live by default*: `ow-host` loads the player's own capture from `local/` (each machine needs one; rule 11 holds). *No overlay*: the layer is still built (hotfixes, text mods later); the card gate accepts 52-53/69 and records why. |
| D2 | Where item and loot rules run | **Rust runtime crates; Python frozen as oracle, then deleted; `item_roll.check` kept.** | Section 9. | *Python generator for now*: wave 4 still ports (the host cannot run Python); only the deletion dates move. |
| D3 | Script versus native | **Offline-needed rules in Rust below the VM, the VM binds the game's script entry to them (`override_script`), the installed script kept as a differential test; world-only rules run as script.** | R2; W5-EQUIP's differential test. | *Script everywhere*: `ow-items` stats would need a VM, so items move above `ow-vm` and offline cards run a VM session; the edge table changes (`ow-items` -> `ow-vm`). *Native everywhere*: missions and Kismet would be re-implemented; not recommended (more rules, less fidelity). |
| D4 | Definitions at run time or generated manifests | **Run time, from `DefStore`.** Manifests and slice JSON become oracles, then go. | W4-STATS deletes the manifest path. | *Manifests kept*: an extra loader in `ow-items` reads the manifest; two input paths for one rule, which this design avoids. |
| D5 | RNG policy | **One `GameRng` with named CrtRand streams seeded from a world seed; parity tools pass the oracle's raw seeds; no attempt at the game's sequence; `HostRng` separate.** | W1-RAND. | *A different generator for FRand*: only the `ScriptStream::FRand` generator changes, behind the same `ScriptRng` trait. |
| D6 | Float policy | **Bit-exact with the oracles on `x86_64-pc-windows-msvc`; `pow` through the choke point; no tolerances in gates.** | Section 5. | *Stated tolerance*: the harness compares within epsilon; the design is the same but loses drift detection. |
| D7 | Playthrough scope now | **Type all three playthroughs, OP and player count from W1-BALTYPES; implement PT1; `NotYet` elsewhere.** | Section 3.6. | *PT1 only until Phase 4*: identical types, more `NotYet`; nothing else changes (that is the point of R4). |
| D8 | DLC in the item system | **Yes**: the registry and the roll cover DLC balances (the Rust roll reads the store, so it costs nothing extra). | W4 lanes include DLC in their census. | *No*: the registry still indexes DLC (identity); rolls are limited by a `LoadSet` without DLC packages. |
| D9 | Gibbed `THIRD_PARTY.md` entry for the save format | **Decide before W7-SAVE**; until then `DefinitionData` mirrors the game's struct (what a save stores) and nothing reads or writes a real save. | No lane before W7 is affected. | *No Gibbed*: the serial packing comes from Ghidra analysis of the game's save code (clean room) in W7. |
| D10 | Crate boundaries, split `ow-game` | **The crates of section 2**, `ow-game` split by layer (21 when approved; 22 after review finding 18 moved skills into `ow-skills` above `ow-combat`, a refinement inside the approved split). | W0-SCAFFOLD (+ the W0 delta). | *Fewer crates*: merge `ow-ai`, `ow-save`, `ow-ui` into `ow-mission` until they have code; the edge table keeps the same order. |
| D11 | Engine route, coordinate boundary, Ruffle | **Engine by benchmark (yours). Coordinates: one module in `ow-host`, everything below in uu/rotator units, Z-up. UI: `ow-ui` bridge behind `MoviePlayer`, implemented by the host.** | Section 3.9. | Any engine pick changes only `ow-host` and the host track. |
| D12 | Single player only for now | **Yes, `PlayerCount(1)`, with `NumberOfPlayers` resolving through its data chain to it.** | W1-BALTYPES, W3-BALANCE. | *Co-op scoped now*: only `PlayerCount` values and the XP share rule change; netcode stays Phase 6. |
| D13 | Simulation step (added) | **Fixed 1/60 s simulation step by default, dt an input; golden traces replay their recorded dt.** | W1-SIM, W5-WORLD. | *Variable step*: `World::tick(dt)` with the frame's dt; same API, determinism tests use recorded dt lists. |
| D14 | Object model (added) | **Slot layouts in an arena heap** (section 3.3). | W1-OBJ. | *Name-keyed bag as in C++*: the same API (`PropId` becomes a name key), slower; no caller changes. |
| D15 | Gate and CI changes (added; gate/workflow PRs need your yes) | **Yes** to `tests/crate_edges_test.py` in ctest, `tools/rust_parity.py` in ctest, and a CI step building the Rust release binary for it. | W0, W1-HARNESS. | *No*: lanes run the checks by hand and paste output; nothing in the design changes, the checks are just not enforced. |
| D16 | Object identity rule (added) | **`LoadSet` API now; `FirstInScanOrder` for offline tools (parity, UNVERIFIED), `Loaded(Startup + map + DLC)` in the world; the one known DLC mismatch listed.** | W3-DEFS. | *First owner everywhere*: one policy; the DLC reference keeps resolving to the wrong class until the rule is read. |
| D17 | C++ freeze (added) | **From wave 1, no new rule in `src/`; each layer frozen at its gate; `src/` deleted in W7-RETIRE.** | Section 6. | *Keep developing C++*: every rule fixed twice until W7; not recommended. |
| D18 | Threading of the simulation (review r1; **approved 2026-11-14**) | **`!Send` on purpose** (frozen in W1-OBJ; changing it later is a breaking contract change under the section 7 bar): `Value` and services use `Rc`, the simulation (`Vm`/`World`) is created and run on one thread, the host talks to it through `Send` message types (commands in, snapshots out); `Package`/`PackageStore` are `Arc` and `Send + Sync`, so threaded tools run one space per thread over shared package bytes. Record it as a Phase 0R benchmark input: Bevy can hold it as a non-send resource or run it on its own thread; the own wgpu engine owns its loop; Fyrox scripts need `Send`, so Fyrox would host the simulation on a side thread. | W1-OBJ, W2-VMAPI as written. | *`Send` now*: `Arc` in `Value`, `Arc<dyn Fn + Send + Sync>` natives, services as owned `Box<dyn Any + Send>` taken out and put back around each use (no `RefCell` across threads); about one atomic per value clone and a more involved service API; not chosen. |

Third-party crates the design would want later (none needed before the host track; each is your decision with a
`THIRD_PARTY.md` entry): the chosen engine and its dependencies (host track); `ruffle_core` if the benchmark supports
embedding (H5); possibly `libm` (MIT/Apache) if the Linux laptop must give bit-parity results (W1-NUM would report
the need); a PNG/deflate crate only if the stored-deflate writer becomes a bottleneck.

---

## 12. Real-game questions and the lanes they block

The report's 20 bl-sdk questions plus two (21 added in revision 1, 22 in revision 2). None blocks a lane from landing: a lane implements what is read, labels it
UNVERIFIED, and returns `NotYet { NeedsRealGame(N) }` where nothing is read. "Blocks the gate" means the lane's gate
counts the case as an open item, not a failure. None blocks the Phase 4 slice (PT1, Sanctuary, Maya) except 8, 16 and
20, which the slice touches.

| # | Question | Lane affected | Until answered |
|---|---|---|---|
| 1 | Playthrough values in NVHM/TVHM/UVHM | W1-BALTYPES, W3-BALANCE | `count = index + 1` UNVERIFIED |
| 2 | UVHM region stage and the cap with OP | W3-BALANCE | UVHM stage `NotYet`; cap rule as read, UNVERIFIED |
| 3 | Origin of the live weapon values | W3-DEFS (D1 default) | layer built, cooked default |
| 4 | The 92 untriaged live stat mismatches | W4-STATS | listed as an open item in its gate |
| 5 | `ComputedModifierValue` of slots | W4-STATS | the 2026-10-23 fit, UNVERIFIED |
| 6 | Non-weapon roll frequencies, class-mod pick | W4-ROLL, W4-POOLS | uniform pick labelled stand-in + `NotYet` flag |
| 7 | Grenade-mod per-element minimum stage | W4-ROLL | current reading, labelled |
| 8 | Maya's crit multiplier source | W6-COMBAT | crit base `NotYet { NeedsRealGame(8) }` (no 0-means-1) |
| 9 | Proc rate, beams, ricochets (the spread distribution is confirmed live, 2026-11-03: `WEAPON_PROJECTILES.md` section 10) | W1-FIRING, W6-COMBAT | spread as confirmed; the rest maths as read, UNVERIFIED; beams/ricochets `NotYet` |
| 10 | Does a 0x5F base write notify? | W3-INTERP, W3-ATTREFFECTS | no notification, UNVERIFIED |
| 11 | Classes overriding the attribute notification | W3-ATTREFFECTS, W5-WORLD | known owners handled; others counted in the census, no effect |
| 12 | Does ObjectProperty find inherited properties? | W3-ATTREVAL | inherited lookup (slot layouts make it natural), UNVERIFIED |
| 13 | Enemy scaling TVHM/UVHM | W3-BALANCE, W7-AI | `NotYet` above PT index 0 where unread |
| 14 | `ResetGameStageForRegion` timing | W3-BALANCE | never reset, UNVERIFIED |
| 15 | Kill XP outside the fitted range | W3-BALANCE | formula applied, UNVERIFIED outside d = 1-4 |
| 16 | Respawn station choice, no station | W6-COMBAT | the six observed cases; no-station `NotYet` |
| 17 | Leaps, thrown rocks, lone bullymong, cone edge | W7-AI | `NotYet` |
| 18 | Which pools serialize in saves | W7-SAVE | none assumed |
| 19 | Micropatch effects on the slice | W3-DEFS (hotfix overlay kind) | not loaded |
| 20 | The live 3-item money drop | W6-LOOT | the row stays OPEN |
| 21 | (added) Bit patterns of live values computed with a power (`1.13^L` health base, XP curve points) | W3-ATTREVAL (section 5.3) | `DoubleThenSingle` default; a conflict is reported, not guessed |
| 22 | (added, review r1) Bit patterns of live stack attributes with several modifiers of each kind (a readied weapon's `InstantHitDamage`, `ClipSize` with a negative scale, a pool's `MaxValue`), to settle `CombineForm` | W1-ATTRMATH, W3-ATTREVAL, W3-ATTREFFECTS (section 5.3) | `WeaponGen` default; `Vm5F` kept for the 5F fixtures; a conflict is reported, not guessed |

---

## 13. Risks

| Risk | Containment |
|---|---|
| A frozen contract (`ow-object`, `ow-vm` API, `AttrWorld`) proves wrong after dependents exist | Contract lanes run at Opus high with compile-tested stubs; the section 7 review bar; `#[non_exhaustive]` enums; lanes report contract problems instead of patching them. |
| Bit parity breaks on `pow` or on an operation order | W1-NUM's pow table; one evaluator tested against the union of fixtures; section 5.3 stop-and-report rule; reference platform fixed. |
| CLI output drifts from C++ (float text, key order, escapes, exit codes, asserted stderr) | one `ow-core::fmt` function per oracle format (N6); `DetMap`/`OrderedMap` by the oracle's order; byte comparison in the harness; the 82 tools that take `--reader` switch per tool in W3-SWITCH after a grep of the flags they call. |
| Parallel lanes collide on shared files (`Cargo.toml`, `Cargo.lock`, `lib.rs`, CLI `main.rs`, parity list) | W0 creates all of them once; one module file per owner; `tests/rust_parity.d/` one file per lane; CLI commands one module each. |
| Reference stand-ins carried into Rust (VM `Rand`, `GotoState`, "0 with base 0 means 1", stand-in loot inventories, the uniform class-mod pick, runtime-state weights as 1) | R3: each is `NotYet` or lives behind `ParityMode::Reference`, selected only by `OW_PARITY` from the harness; listed in 6.1 and the lane handoff; deleted in W7-RETIRE after its Rust-native replacement test exists; the loot gate is the live rows, not C++ equality. |
| The simulation is `!Send` (D18) and the engine pick needs `Send` | Recorded as a benchmark input before the pick; the host boundary is message types, so only `ow-host` changes if the simulation moves to a side thread; switching to `Arc` is a contract change with the section 7 bar. |
| W0 is running from revision 1 | The W0 deltas are listed in the revision section; the hub forwards them or runs a W0b PR; the only crate-level delta (`ow-skills`) touches no lane before wave 5. |
| Two live copies during the transition | Section 6 freeze rule, D17, section 9 deletion points; W3-SWITCH moves the tools to the Rust reader. |
| VM performance (thousands of actors) | Slot layouts and link-time operand resolution from the start; W4-VMTOOLS records sweep time next to the C++ time in its handoff; a regression over 2x is an open item. |
| Rust borrow complexity makes code hard for the maintainer to explain | Arena handles instead of references in public APIs; `Rc<RefCell>` only for VM services with the no-borrow-across-script rule; R1 keeps traits rare. |
| Game-derived data leaking into tracked tests | Synthetic fixtures only; real-package comparisons print counts and write lists and hashes under `local/`; the pow table holds invented inputs and record constants only; the leak gate on every PR. |
| Linux laptop results differ | Reference target named in every gate; pow test fails loudly elsewhere; `libm` decision only if needed. |
| Real-package gates need the two machines with BL2 | Lanes marked "game: yes" are scheduled on the zuhu desktop or pc-hummer; their synthetic parts run anywhere, so a lane can be written on any machine and gated on one of the two. |
| The identity rule picks the wrong owner in the world | `LoadSet` API from W3-DEFS; `owners()` audit; the known DLC case is a test. |
| Engine pick arrives late or changes | Nothing below `ow-host` depends on it; `W2-ASSETS` is engine-agnostic; the host track starts after the pick. |
| Scope creep beyond the slice | Waves 5-6 cover only what Sanctuary, Maya and the Fire mission need (AGENTS.md priority); W7 lanes are breadth and need the maintainer's go-ahead before the Phase 4 gate. |

---

## 14. Text for every lane brief

Paste this block into each lane brief, then the lane's entry from section 8.

> You are a lane of the BL2_ReBased Rust port. Read `docs/LANE_BRIEF.md` and `docs/verification/GATES.md` first,
> then `local/hub/rust-port/architecture.md` sections 1, 3 (your crate), 5, 7 and your lane entry in section 8.
>
> **Tools, in this order:** bl-sdk live reads and hooks of the running game first (the RTSE way,
> `tools/real_game/`), REA (the `rea` MCP server) for decoding, Ghidra (`tools/ghidra/run.ps1`) for
> `Borderlands2.exe`. This lane runs no real-game session unless this brief says so; work from the recorded evidence
> and label what stays unconfirmed `UNVERIFIED`.
>
> **Process safety:** never kill a process by image name (`taskkill /IM`, `Stop-Process -Name`, `pkill`, `killall`);
> stop only a PID your own run started, after checking its command line; never delete a lock you do not hold. Do not
> launch Unreal Editor or the game. Leave `local/bench/` alone.
>
> **Parity mode:** reference stand-ins exist only under `OW_PARITY=reference`, which only `tools/rust_parity.py`
> sets, for manifest lines marked `mode=reference`; only the binaries read it (`parity_mode()`), library code takes a
> `ParityMode` value. Never read or set that variable anywhere else, never add a
> stand-in outside `ParityMode::Reference`, and list each pinned test case you meet (architecture.md 6.1) with the
> Rust-native test of its replacement.
>
> **Rules:** AGENTS.md rules 1-11. No game-derived payload in tracked files (real-package outputs go under `local/`);
> offsets come from the C++ code that recovered them, never re-derived; no loosened bounds check; synthetic fixtures
> only; no third-party crate. Edit only the files your entry owns; if you need a change elsewhere (a contract, a shared
> file), stop and write it in your handoff. Unread rules return `NotYet`, never a guessed value.
>
> **Finish:** rebase on `origin/main`, run the gates in your entry plus `cargo build --workspace --locked`,
> `cargo test --workspace --locked`, `python tests/crate_edges_test.py`, the ctest and `verify_packages.py` commands of
> GATES.md, and `python tools/decisions_index.py --check --baseline origin/main`. Do not commit or push. Write
> `local/p2/<lane>/handoff.md`: changed tracked paths, gate output with numbers, what is verified and how and what is
> UNVERIFIED, every C++ stand-in you met and what you did with it, contract problems found, a proposed commit message,
> the path of your new decision file (dated, opening "AI-assisted (Claude)") if parsing behaviour changed, and open
> items. Name any sensitive file you touched (`Cargo.toml`, `crates/ow-container/`, `crates/ow-package/`) with what
> changed, why, and what is verified.

---

## 15. W0 changes needed (revision 3, for the running W0 lane)

Checked against `feature/rust-w0-scaffold` on 2026-11-14 (it already has the 22 crates, `ow-skills`, `ordmap.rs`,
`parity.rs`, the `testkit.rs` placeholder, the CLI table and the placeholders with `new_empty()`). What it still has to
add or change; nothing else in revision 3 touches W0's files.

1. **Small final types** (the owner lanes keep them unchanged and add methods):
   - `crates/ow-balance/src/types.rs`: `pub struct GameStage(pub i32);` with `#[derive(Debug, Clone, Copy, PartialEq,
     Eq, PartialOrd, Ord, Hash)]`, re-exported at the crate root. Owner afterwards: W1-BALTYPES.
   - `crates/ow-package/src/package.rs`: `pub type PackageId = u32;`, re-exported at the crate root. Owner afterwards:
     W1-PKG.
   - `crates/ow-attr/src/stack.rs`: `pub struct ModifierId(pub u64);` with the same derives, re-exported at the crate
     root. Owner afterwards: W1-ATTRMATH.
2. **Parity mode read by binaries only:**
   - `crates/ow-core/src/parity.rs` doc: `parity_mode()` is called only by binaries (`ow-cli/src/main.rs`, later
     `ow-host/src/main.rs`); library crates take a `ParityMode` value (`GameRng::from_world_seed(seed, mode)`,
     `Vm::new(.., mode)`). Keep `mode_from_value` public so tests build both modes in one process.
   - `tests/crate_edges_test.py` `BANNED`: a call of `parity_mode(` and any use of `ENV_VAR` / the string `"OW_PARITY"`
     outside `crates/ow-core/src/parity.rs`, `crates/ow-cli/src/main.rs` and `crates/ow-host/src/main.rs`, with a test
     case that a library file calling it fails.
3. **Rust-only command modifiers** in `crates/ow-cli/src/args.rs` `options` (usage text only; the `cmd/` modules parse
   them): `--item-registry`: `--cooked <dir>`, `[--overlay <file>]`; `--item-roll <balance path>` and
   `--item-pool <pool path>`: `--cooked <dir>`, `--stage <n>`, `--seed <n>`, `[--count <n>]`, `[--playthrough <index>]`,
   `[--op-level <n>]`, `[--overlay <file>]`; `--item-stats`, `--item-card`: `--cooked <dir>`, `--data <file>` or the
   `--item-roll` modifiers, `[--overlay <file>]`; `--def-dump <object path>`: `--cooked <dir>`, `[--overlay <file>]`.
   (`--op-level`, not `--op`, which is a Kismet option.) The `cmd/` module doc headers say the module parses its own
   trailing modifiers.
4. **Doc headers** (no code): `crates/ow-combat/src/loot.rs`: "loot on death and the loot-spawn behavior handlers
   (registered into the `ow-behavior` kernel)"; `crates/ow-inventory/src/spawn.rs`: "item spawn natives; registers no
   behavior handler (those are `ow-combat::loot`'s)"; `crates/ow-vm/src/api.rs`: names the `WorldHooks` trait W2-VMAPI
   freezes; `crates/README.md`: `ow-skills` layer "L8b (built above L9)", and the Phaselock lift runs through script and
   world natives (no edge to `ow-ai`).
5. **No edge change.** `ow-inventory` keeps no edge to `ow-behavior` (finding 6 resolved by moving the handlers).
6. **`tests/CMakeLists.txt`:** W0 keeps only its `crate_edges` entry; W1-HARNESS and W1-PINSPLIT add their own lines.
