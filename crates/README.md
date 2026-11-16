# The Rust crates

AI-assisted (Claude Code).

The Cargo workspace that ports the C++ core in `src/` and the Python runtime rules in `tools/` (decision
2026-11-14-01). The C++ code stays the reference until each crate matches it on the same synthetic fixtures and, for
the reader, on `tools/verify_packages.py`. The only third-party crates are the engine crates wgpu 30.0.1 and winit
0.30.13 and their transitive closure, declared by `ow-host` alone (`THIRD_PARTY.md`, "Engine crates: wgpu and winit",
maintainer approval 2026-11-16). Any other one needs its `THIRD_PARTY.md` entry first, which is the maintainer's
decision (AGENTS.md rule 6).

The scaffold (lane W0-SCAFFOLD) created every crate, every `Cargo.toml` and every module file in its final place.
Each module file names its owner lane in its first line; a lane fills its own files and does not edit a `lib.rs`, a
`Cargo.toml` or `Cargo.lock`. Until a module is ported it holds nothing, and anything that needs it returns the typed
`ow_core::NotYet` instead of a stand-in value.

## Crates

| Crate | Layer | Owns | Ports from |
|---|---|---|---|
| `ow-core` | foundation | `NotYet`, `ObjectPath` / `PackageName`, `DetMap` (key order), `OrderedMap` (insertion order), `ParityMode` (`OW_PARITY`), numerics, units, `%g` formatting, game randomness, the simulation clock | `weapon_gen.hpp` CrtRand, `weapon_recipe.f32`, the C++ float text |
| `ow-container` | L0 | LZO1X decoder, chunk container, its bounds checks | `src/container.*` (decoder written from `research/native_count.py`) |
| `ow-package` | L0 | `Reader`, header 832/46, name/import/export tables, `Package::load`, the tag walker, `PackageStore`, the shared synthetic-package builder (`testkit`, feature `test-support`) | `src/package.*` |
| `ow-object` | L1a | ids, `Value`, heap, class layouts, reflection, class bodies, typed values, class defaults | the loading half of `src/vm.cpp` |
| `ow-defs` | L1b | `DefStore`: object identity, lazy definitions, overlays | `PackageStore` use in `src/vm.cpp` |
| `ow-script` | L2a | bytecode decoder, opcode table, disassembler, package check | `src/script.*` |
| `ow-attr` | L3 | modifier stacks, the initialization-data evaluator, resolvers, expressions, attribute effects | `progression.cpp`, `weapon_gen.cpp` evaluator, the Python evaluators |
| `ow-balance` | L4 | playthrough / OP / player-count / stage types, XP curve and formulas, offline balance world | `progression.cpp`, `combat_xp.cpp`, `tools/slice_values.py` |
| `ow-items` | L6 | `DefinitionData`, registry, rolls, stats, cards, item pools | `weapon_gen.*`, `tools/item_*.py`, `weapon_*.py`, `loot_pools.py` |
| `ow-vm` | L2b | interpreter, native registry, stub policy, Core natives, census and sweep | the run half of `src/vm.cpp`, `interp.*`, `natives*.cpp`, `census.*` |
| `ow-world` | L5 | `World`, actors, iterators, timers, resource pools, level-up | `combat.cpp` pools, `mission_script.cpp` pools and level-up |
| `ow-inventory` | L7 | inventory natives, equip, pickups, backpack sort, menu navigation | `inventory_navigation.*` |
| `ow-behavior` | L8a | behavior providers, the kernel and its handler registry, the handlers that need only the world | `src/behavior.*` |
| `ow-combat` | L9 | firing maths, hit chain, damage, death, down state, respawn, loot on death, the damage-dealing and loot-spawn behavior handlers | `weapon_firing.*`, `combat*.cpp` |
| `ow-skills` | L8b (built above L9) | skills, skill trees, `SkillEffectManager`, action skill, Phaselock (above `ow-combat`: skills read the held weapon and deal damage; the Phaselock lift of an AI target runs through the installed script and the world's natives, so there is no edge to `ow-ai`) | `tools/skill_stats.py`, the `prepare_skill_*` rules, the skill notes |
| `ow-ai` | L10 | AI pawns, minds, awareness, targeting, melee, population | the notes and live records |
| `ow-mission` | L11 | mission tracker, script bridge, Kismet, dialog, movers, interactive objects, vendors, the Fire slice | `mission*.cpp`, `kismet.*`, `dialog.*`, `mover.*`, `slice.*` |
| `ow-save` | L12 | `PlayerSaveGame` model, load order, save triggers | nothing in `src/` |
| `ow-ui` | L13 | GFx bridge natives, the `MoviePlayer` trait | `tools/hud_overlay` rules (later) |
| `ow-assets` | host data | textures, TFC, PNG, static meshes, OBJ | `src/assets.*` |
| `ow-cli` | tools | the Rust `ow-package` binary: every C++ flag, one `cmd/` file per command family | `src/cli.cpp` |
| `ow-host` | host | the install check; later the engine route, the only unit/axis conversion, `HostRng`, the `MoviePlayer` implementation | none |

## Allowed dependency edges

A crate may depend on the crates listed and on anything they may depend on. Everything else is forbidden, and
`python tests/crate_edges_test.py` (registered in ctest as `py-crate_edges`) fails on an edge outside this table.
Every crate's `Cargo.toml` already declares its whole allowed closure, so no lane needs to edit one.

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

Forbidden on purpose: `ow-attr`, `ow-balance` and `ow-items` depending on `ow-vm` (offline rules stay offline);
`ow-package` depending on `ow-object` (the reader knows no reflection); `ow-behavior` depending on `ow-combat`,
`ow-skills` or `ow-items` (handlers from above register into the kernel's registry); anything depending on `ow-host`
or `ow-cli`.

The same test also refuses, in every Rust file:

- `mul_add` (no fused multiply-add);
- the float functions (powers, exponentials, logarithms, roots, trigonometry: `powf`, `exp`, `ln`, `log10`, `sqrt`,
  `sin`, `atan2`, `asinh` and the rest, as `x.f()`, `f64::f(x)` or `<f64>::f(x)`) outside `ow-core/src/num.rs`, which
  is their one choke point. The method form matches any method of these names, so rule code does not name its own
  methods `log`, `tan` and so on;
- the names `HashMap` and `HashSet` outside `ow-core/src/detmap.rs`: lookups use `ow_core::FixedHashMap` /
  `FixedHashSet` (a fixed hasher), and rule code never iterates a hash map;
- reading the parity mode (the identifiers `parity_mode`, `init_parity_mode`, `mode_from_env`, `ENV_VAR`, in calls,
  paths and `use` lines alike) outside `ow-core/src/parity.rs` and the binaries' `main.rs`, and the string
  `"OW_PARITY"` outside those files and `crates/*/tests/`, where a test sets it for the binary it runs. Libraries
  and command families take a `ParityMode` value: `main` reads it and passes it to `cmd::run`.

It also checks that every flag of `src/cli.cpp`'s usage text has an entry or option in `crates/ow-cli/src/args.rs`,
and that every flag `src/cli.cpp`'s `main()` dispatches on has a `cmd("--flag"` entry.

## The Rust `ow-package`

`cargo build -p ow-cli` builds `target/debug/ow-package.exe`. It takes the C++ binary's command lines, and every
command family is ported (lane W6-FIRE ported the last, decision 2026-11-16-38). Exit code 3, which the C++ binary
never uses, means the command met a `NotYet` (a rule, layer or native not ported or read yet). The parity
mode is read once at start-up from `OW_PARITY` (`reference` or unset); any other value prints
`ow-package: OW_PARITY must be "reference" or unset, ...` and exits 1.

The dispatch table is `crates/ow-cli/src/args.rs`; each family is one file under `crates/ow-cli/src/cmd/`, owned by
one lane. The command flags (architecture section 2.3, revision 2):

| File | Owner | Command flags |
|---|---|---|
| `cmd/tables.rs` | W1-PKG | `<package>` alone (the header summary), `--names`, `--imports`, `--exports`, `--census`, `--payload`, `--payload-file`, `--payloads`, `--verify-decoded` |
| `cmd/properties.rs` | W2-PROPS | `--properties`, `--properties-batch` |
| `cmd/resolve.rs` | W2-STORE | `--resolve` |
| `cmd/assets.rs` | W2-ASSETS | `--texture`, `--mesh`, `--terrain-records`, `--scene-records` |
| `cmd/script.rs` | W2-SCRIPT | `--script-check`, `--disasm` |
| `cmd/classes.rs` | W2-REFLECT | `--class-check`, `--interface-compare` |
| `cmd/objects.rs` | W3-DECODE | `--object-dump` |
| `cmd/defs.rs` | W3-DEFS | `--def-dump` (Rust-only) |
| `cmd/vm.rs` | W3-INTERP | `--run`, `--run-batch`, `--native` |
| `cmd/census.rs` | W4-VMTOOLS | `--native-census`, `--vm-sweep`, `--native-selftest` |
| `cmd/items.rs` | W4-REGISTRY | `--item-registry` (Rust-only) |
| `cmd/items_roll.rs` | W4-ROLL | `--item-roll` (Rust-only) |
| `cmd/items_stats.rs` | W4-STATS | `--item-stats`, `--item-card` (Rust-only) |
| `cmd/items_pools.rs` | W4-POOLS | `--item-pool` (Rust-only) |
| `cmd/behavior.rs` | W5-BEHAVIOR | `--behavior-dump`, `--behavior-run` |
| `cmd/kismet.rs` | W5-KISMET | `--kismet-census`, `--kismet-run` |
| `cmd/mover.rs` | W5-DIALOG | `--mover-event` (in the C++ dispatch, not in its usage text), `--mover-probe` |
| `cmd/inventory.rs` | W5-EQUIP | `--inventory-move` |
| `cmd/mission.rs` | W6-MISSION | `--mission-run` |
| `cmd/slice.rs` | W6-FIRE | `--slice-run` |
| `cmd/combat.rs` | W6-COMBAT | `--combat-run` |

The other flags of the usage text are options, parsed by the command that reads them, as in `src/cli.cpp`:
`--cooked` (most commands), `--property-offset` and `--array-schema` (`--properties`, `--properties-batch`, and
`--property-offset` for `--mesh`, `--texture`), `--output`, `--lod`, `--tfc`, `--mip`, `--all-mips` (`--mesh`,
`--texture`), `--failures` (`--script-check`, `--class-check`), `--class`, `--limit`, `--top` (`--vm-sweep`),
`--steps` (`--vm-sweep`, `--native-census`), `--no-static` (`--native-census`), `--remote`, `--mission`, `--op`,
`--originator`, `--tick` (`--kismet-run`), `--all` (`--object-dump`), `--self`, `--arg` (`--run`), `--native-args`
(`--native`). The Rust-only commands take the modifiers of architecture section 15: `--cooked` and `--overlay`
(all of them), `--stage`, `--seed`, `--count`, `--playthrough`, `--op-level` (`--item-roll`, `--item-pool`, and
`--item-stats` / `--item-card` without `--data`), `--data` (`--item-stats`, `--item-card`). Each `cmd/` module parses
its own trailing modifiers; the lists in `args.rs` are the usage record. Architecture section 2.3 lists some of these options
under another file (`--all` and `--top` under `cmd/census.rs`, `--class` and `--steps` under `cmd/vm.rs`,
`--mission` and `--tick` under `cmd/mission.rs`, `--cooked` under `cmd/resolve.rs`); where they differ, this list
says which command reads the option in `src/cli.cpp`.
