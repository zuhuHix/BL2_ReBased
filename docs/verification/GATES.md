# Gates and the current baseline

Layout: the gate commands and the standing baselines come first, then the dated rows, newest first, then the Rust
host's gates and the tests outside ctest. For a number that moves with every lane (the workspace test count, the
slice gate) the current baseline is the newest row that reports it. Older rows are in
`docs/verification/gates-archive/`, and no gate needs them.

## Every change
- Build (Windows; CMake ships with VS Build Tools):
  `export PATH="/c/Program Files (x86)/Microsoft Visual Studio/18/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"`,
  then `cmake --build build --config Release --parallel`. A fresh configure pins the project's MSVC toolset:
  `cmake -S . -B build -T version=14.50`.
- `ctest --test-dir build -C Release --output-on-failure`
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`
- `cargo build --workspace` and `cargo test --workspace` (Rust toolchain pinned by `rust-toolchain.toml`).

## When the VM, missions or the slice change
- `python tools/verify_fire_event_order.py --self-test` and `python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe`
- Compare `--slice-run` output before and after against a copy of the previous build, and explain every difference.
- The rows' slice-gate figures are from the Rust `ow-package --slice-gate --cooked <dir>` (`docs/TOOLING.md`,
  "`ow-host`: the Rust host"). The rows report it as run on the installed game, under the machine lock.

- The slice gate's `-Shield` run was a UE5-host run and left the tree with it; its last numbers are history
  (`git show 08bcf3e:docs/verification/GATES.md`).

## When combat changes
- `python tools/verify_combat_pawn.py --self-test`, and `--vm <json>` on a fresh `--combat-run`.
- `python tools/verify_combat_pawn.py --live` checks the formulas against ignored live dumps when they exist.

## Records
- `python tools/decisions_index.py --check --baseline origin/main` after adding, editing or moving decisions (with the
  default baseline, `HEAD`, it passes trivially on committed work). Add `--at HEAD` to check the committed blobs.
- `python tools/leak_gate.py --range origin/main..HEAD` before a push, and `--text <file>` on a PR description
  (`docs/TOOLING.md`, "Leak gate").

## Standing baselines
The last result on record for each gate, with the row it comes from and what it ran on, as of the last time rows were
moved out (2026-11-26). A newer row below wins. Whoever moves rows out brings every line up to date first
(`gates-archive/README.md`), so that a gate's last result is never only in the archive.
- ctest: 173 of 173 passed (row PHASE-0R-READER-CHECK, 2026-11-25, pc-hummer runner, main a3b548d; in the archive).
- `verify_packages`: 9 packages, 3,339/3,339 class bodies exact with the C++ reader and with the Rust reader (the same
  row).
- `cargo test --workspace`: 4,064 passed / 0 failed / 59 ignored (row IMPL-LINK7-HUD-TARGETING, its review r2 of #810,
  2026-11-26, pc-hummer, branch fix/rust-link7-hud from main d118d2b; below). A lane's figure is its own branch's.
- `crate_edges_test.py`: OK (the same row). Its last recorded count is 19 tests (row FIX-BEHAVIOR-CONTEXT-OBJECT,
  2026-11-18, pc-hummer; in the archive).
- Installed Off strict `--slice-gate`: 8 ok / 6 not ok of 14, 8.500/14 = 60.7% (row IMPL-LINK7-HUD-TARGETING,
  2026-11-26, pc-hummer, release build under the machine lock, "on d118d2b" in the row's words; below). The row has
  each link's stop.
- Fire event order: the self-test passes and 33 of 33 rules hold on the installed packages with the C++ reader (row
  FIX-BEHAVIOR-CONTEXT-OBJECT, 2026-11-18, pc-hummer, base 2fcae040; in the archive).
- `verify_combat_pawn --vm`: 55 pass / 0 fail / 4 missing of 59 on the S-a to S-f run, and 97 pass / 0 fail /
  4 missing / 1 open of 102 with the loot run added (row "After lane Sg", 2026-10-09; in the archive). No later run
  is on record here or in the archive.
- The Rust host's gates: each baseline stands with its gate under "When the Rust host changes".

## Dated rows, newest first

A lane adds its row directly under the next line, above the newest row, in the form of the rows below.

Update this file in the commit that changes a number. Paste real output in reports; do not copy these numbers.

**IMPL-LINK7-LINE-CHECK (2026-11-26, pc-hummer, on fix/rust-link7-line-check, stacked on #816, on main c876a397):**
AI-assisted (Claude Code, Opus 5.5). NATIVE_LINK7_LINE_CHECK.md section 5 in Off (the hub's ruling in decision 2026-11-26-71): the HUD
targeting's line check from the view point with the HUD as the source, every primitive through the point-line filter,
the target and its `Base` kept, the bounds step with well-formed values. `cargo test --workspace -j 8 --no-fail-fast`
4,089 passed / 0 failed / 59 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate` (release build, under
the machine lock), base #816's head 056260f1 against this change: **9 ok / 14, X = 9.0/14 = 64.3%** (base 8 ok, 60.7%). Link 7
passes; link 6 still passes; every other row as on main. UNVERIFIED in game. Decision 2026-11-26-NNN.

**IMPL-SPAWN-SETBASEVALUE (2026-11-26, pc-hummer, on fix/rust-spawn-setbasevalue, rebased onto main f80b68e, which has
#808 and #810):** AI-assisted (Claude Code, Opus 5.5). The combat world's set-up preload of the attribute starting
values' definitions, Off, in the note's three phases (#811 as landed): discovery (each spawn kind's archetype copied
once, logged, freed; its AI class chain loaded before its fields are read; a looping chain stops), the closure
`SetBaseValue`'s preflight walks (one walk shared with the preflight, `ow-attr` `walk_set_base_value`), and the loads
(logged; a failed one left). `cargo test --workspace` 4,090 passed / 0 failed / 59 ignored; `crate_edges_test.py` OK.
Installed Off strict `--slice-gate` (release build, under the machine lock, on 01035e1a): 8 ok / 6 not ok, 9.500/14 =
67.9% (main: 60.7%). The set-up made 4 copies and loaded 16 objects, none failed, each logged. **Links 3, 4, 5, 13 and
14 move past `SetBaseValue`, and both spawns now complete**: link 3 (`kill_enemy`) passes its spawn step (1/2) and
stops at its hit, link 4 (`enemy_attacks_maya`) passes its spawn step (1/2) and stops at its first swing, both at the
unbound `DamagePipeline.GetBarrelSourceTime()` (the note's section 5, first item). Link 5 is blocked by link 3; link 13
is blocked by link 4's stop in strict mode (0/4 steps; it does not reach the native itself), and link 14 by 13. Link 7
is at the line check's decoy `NotRead` (ruling 4); 1, 2, 6 and 8 to 12 ok, 8 ok as before. UNVERIFIED in game.
Decision 2026-11-26-72. Review of #815, r1: the walk stops at the depth bound and follows a definition once;
discovery reads each archetype through the store's own load (logged; its references stay handles), so no copy is made
or left; data errors are `VmError::Script`; `AttributeStartingValuesNew` must be an array. `cargo test -p ow-attr -p
ow-combat -p ow-cli` 892 passed / 0 failed / 12 ignored; `crate_edges_test.py` OK. Gate rerun on 867057a0: the same
rows and 9.500/14 = 67.9%; the set-up logged 4 archetypes and loaded 20 objects (the AI classes the copy used to load
unlogged now load in the walk, each logged), none failed. Review of #815, r2: each definition is checked once per call
with its nesting height, by the preload and the native preflight alike (the same answers, linear work); the archetype
is the named package's own copy (its handle, then `DefStore::load_export_of_file`); a slotless stand-in is skipped,
logged; a skipped `AttributeStartingValuesNew` tag is a data error. `cargo test -p ow-defs -p ow-attr -p ow-combat -p
ow-cli -p ow-world` 1,779 passed / 0 failed / 23 ignored; gate rerun on cdd6a70e: the same rows, 9.500/14 = 67.9%, 20 objects preloaded
from the named packages' copies, none failed.

**IMPL-FLOOR-TRACE-RULINGS (2026-11-26, pc-hummer, on fix/rust-floor-trace-rulings, on main f80b68e7):** AI-assisted
(Claude Code, Opus 5.5). NATIVE_FLOOR_TRACE_FLAGS.md section 6.1 in Off: the floor trace's primitive walk with the read
flags, the swept-box test (well-formed values only) and the `Owner` chains both ways; the touch trace's `Owner`-chain
correction; the `holds_no_geometry` guard kept. `cargo test --workspace -j 8 --no-fail-fast` 4,083 passed / 0 failed /
59 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate` (release build, under the machine lock), base
f80b68e7 against this head: both **8 ok / 14, X = 8.5/14 = 60.7%**, every row's stop the same. Links 6 and 7 have the
same steps. Links 3, 4, 5, 13 and 14 stay past the floor trace and stop at the spawn's `SetBaseValue`. UNVERIFIED in
game. Decision 2026-11-26-73.

**ANALYST-LINK7-LINE-CHECK (2026-11-26, pc-hummer, docs only, on docs/native-link7-line-check from main c1058de6,
rebased onto f80b68e7):**
AI-assisted (Claude Code, Opus 5.5). NATIVE_LINK7_LINE_CHECK.md reads the HUD targeting's line check: a point line
from the view to 25 units short of the pickup (correcting NATIVE_PICKUP_TARGETING.md's direction), flags 134357638
(movers, the BSP, world geometry, stop at the first hit, terrain, complex collision, the game bit; no pawns, volumes or
other actors), the HUD as source. With those flags the owner filters never test a pawn or a pickup (`WillowPickup`
keeps `Actor`'s body, `bWorldGeometry` false), so the decoy whose bounds meet the line is passed over by the game's
rule. The hub ruled (decision 2026-11-26-71) that section 5's rule replaces ruling 4's three-actor list under
`NoLevelLineCheck`. Review r1 of #814: the candidates include `Components` with each owner resolved, the real HUD as
source and the view point as start, well-formed bounds only, and the refusal order (an intersecting mover:
`HASH_ADMISSION`). No probe; no code changed, so no measured number moved. UNVERIFIED in game.

**IMPL-LINK7-HUD-TARGETING (2026-11-26, pc-hummer, on fix/rust-link7-hud from main d118d2b, which has the note
#806):** AI-assisted (Claude Code, Opus 5.5). The hub's four rulings on #806
as Off harness inputs (the harness HUD, the `AddPickup` registration, the eye-point view, the no-hit line check under
`NoLevel`) and the shrink step's rule, Off. `cargo test --workspace` 4,049 passed / 0 failed / 59 ignored;
`crate_edges_test.py` OK (rebuilt on d118d2b: `cargo test -p ow-inventory -p ow-cli -p ow-world` 1,645 passed / 0
failed / 15 ignored). Installed Off strict `--slice-gate` on d118d2b (release build, under the machine lock): 8 ok / 6
not ok, 8.500/14 = 60.7% (no change in count). **Link 7 passes `make` and `touch` and stops in `look` at the line check's
`NotRead`**: the decoy `WillowPickup`, whose collision is its `SkeletalMeshComponent`, has bounds by the
bounding-cylinder rule of (0, 45, 0) ± (118.5, 118.5, 32.7), which meets the line's box from (127.6, 0, 11.2) to (0, 0,
75), so ruling 4's geometric clearance does not clear it. Link 6 ok, 3/3 (the registration on). Links 3, 4, 5, 13 and
14 are at `AttributeInitializationDefinition.SetBaseValue` (as on main); 1, 2 and 8 to 12 are ok. **Begun play (ruling 2's
first form) was measured** and broke link 6: its `make` stopped at the spawn's "the base search after PostBeginPlay"
(7 ok / 14, 50.0%), so the registration fallback is used. Before the review rulings, with the line check answered no
hit outright, link 7 passed (9 ok / 14, 64.3%). UNVERIFIED in game. Decision 2026-11-26-66.
Review of #810, r2: `Vm::check_call` checks a script's decode without filling the decode cache, the globals
definition class is looked up read-only (no package opened, no reflection index), the pickup default and the
`LifeSpan`s are read only once shrinking is on, and a pawn `Location` missing an axis is `VmError::Script`. `cargo test
--workspace` 4,064 passed / 0 failed / 59 ignored; `crate_edges_test.py` OK. The gate was not rerun (lint and the
touched crates' tests, as the hub asked).

**IMPL-PLAYER-PAWN-FLAG (2026-11-26, pc-hummer, on fix/rust-player-pawn-on, on main c1058de6):** AI-assisted (Claude
Code, Opus 5.5). In Off, the slice's combat set-up turns on the player set-up's `Pawn` (logged), and #805's
nothing-touched stand-in also applies to a collision service with no geometry (its own line). `cargo test --workspace -j
8 --no-fail-fast` 4,051 passed / 0 failed / 59 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate`
(release build, under the machine lock), base c1058de6 against this head: both **8 ok / 14, X = 8.5/14 = 60.7%**, every
row the same. Links 1 and 6 pass, link 1 with the `Pawn` set. Links 3, 4, 5, 13 and 14 stop at the spawn's
`SetBaseValue`. UNVERIFIED in game. Decision 2026-11-26-70.

**IMPL-COMBAT-MUSIC-AND-AFTER (2026-11-26, pc-hummer, on fix/rust-combat-music, moved onto main 74b06d8, which has
#804):** AI-assisted (Claude Code, Opus 5.5). `ApplyGradeCustomizations`, `UpdatePlayThroughMaterial`,
`GetCombatMusicManager`'s lazy creation and `PlayerDamagedByEnemy`'s gates, with the threat model's Off no-op stand-in
(hub ruling on review of #801), Off. `cargo test -p ow-combat -p ow-cli` 572 passed / 0 failed / 10 ignored (the
workspace 4,023 / 0 / 59 before the move); `crate_edges_test.py` OK. Installed Off strict `--slice-gate` (release build,
under the machine lock): 8 ok / 6 not ok, 8.500/14 = 60.7%, as on main: links 3, 4, 5, 13 and 14 at the spawn's
`AttributeInitializationDefinition.SetBaseValue` ("a definition it names is not loaded"), so these natives are still
not reached; link 7 at the HUD's pickup targeting; 1, 2, 6 and 8 to 12 ok. UNVERIFIED in game. Decision
2026-11-26-65.
Review of #808, r1: the spawn customisations through a write-aware runner, both reference types checked before the
manager is made, the no-op gates first, the definition classes from the class table only. r2: the combat set-up sets
the globals' `TheGlobalsDefinition` when the route left it unset and preloads it and its `CombatMusicProperties`
(logged harness inputs, UNVERIFIED); the counted runner's preflight looks methods up read-only (cold code is NotRead)
and recomputes the partial-writes prefix from the counters; a manager class without a defined layout is NotRead;
`OnSpawnCustomizations` must be an array of objects.

**ANALYST-COMBAT-TAIL-LINKS-3-4 (2026-11-26, pc-hummer, docs only, on docs/native-links-3-4-tail from main
d118d2bb, rebased onto c1058de6):** AI-assisted (Claude Code, Opus 5.5). NATIVE_LINKS_3_4_TAIL.md reads links 3
and 4 past the spawn's
`SetBaseValue` (lane E's). A local probe (installed Off strict `--slice-gate` runs under the machine lock, lane E's
`fix/rust-combat-music` and `fix/rust-links-13-14` merged locally, each stop stood in, reverted and not committed)
gives both chains. Link 3: the pipeline getters (read, unbound), `IsFriendlyFire`'s interface value (a port gap),
the hit-region cap (`AddDamageToHitRegion`, read here: no cap on the slice), `GetDisplayNameAtGrade`,
`NotifyKilledEnemy` and `IsActionSkillRunning` (read here), then the combat world's unbound dialog globals getter.
Link 4: the getters and `IsPlayingAny`, the shield stage and the protection timer (read, not ported), the resistance
(#796's ruling), then `DoForceFeedbackWaveform`'s residency check before its gates (an order fix). No code changed, so
no measured number moved. UNVERIFIED in game.

**ANALYST-SPAWN-SETBASEVALUE (2026-11-26, pc-hummer, on docs/native-spawn-setbasevalue, on main d118d2bb):**
AI-assisted (Claude Code, Opus 5.5). Docs only: NATIVE_SPAWN_SETBASEVALUE.md, the index rows and decision 2026-11-26-68;
no code, so no test numbers change. The installed Off strict `--slice-gate` on #807's head 0a9cfdaf (the base of the
note): **8 ok / 14, X = 8.5/14 = 60.7%**. Links 3, 4, 5, 13 and 14 stop at the spawn's `SetBaseValue`, a starting
value's unloaded attribute. A local probe (temporary, reverted) under the machine lock, with IMPL-COMBAT-MUSIC's and
#805's commits laid over it and each later stop stubbed in turn (note section 6), reached **9 ok / 14, X = 10.75/14 =
76.8%**: link 4 passes, link 3 stops at the unbound `NotifyKilledEnemy`, and link 13 at the AI mind's
`AttackInjuredMeleeDamageModifier`. The predicted stop after the note's preload alone is `ApplyGradeCustomizations`
(section 8). UNVERIFIED in game.

**ANALYST-FLOOR-TRACE-FLAGS (2026-11-26, pc-hummer, docs only, on docs/native-floor-trace-flags from main
153ac950, rebased onto c1058de6):** AI-assisted (Claude Code, Opus 5.5). NATIVE_FLOOR_TRACE_FLAGS.md reads the
base search's floor trace:
the flags (8415: pawns, movers, the BSP, volumes, other actors, blocking only, world geometry, terrain; one result
added, no stop at the first hit), a box swept with the actor's collision cylinder (not a line), the actor as source.
It reads what it can hit (world geometry always; pawns, movers, volumes and other actors through the source's
blocked-by test) and the source's exclusion: its own primitives, its `Owner` chain and the actors it owns. That
confirms #798's R2 and corrects the walkers' "base chain" to the `Owner` chain (NATIVE_SPAWN_TRACE_FILTER.md,
NATIVE_TOUCH_PICKUP_TRACE.md, `trace_filter::chain_of`). Lane E's geometric clearance is right in order but too
narrow in region; a box test is proposed. No probe; no code changed, so no measured number moved. UNVERIFIED in game.

**IMPL-PAWN-INTERACTION-PROXY (2026-11-26, pc-hummer, on fix/rust-pawn-interaction-proxy from main 50114dc):**
AI-assisted (Claude Code, Opus 5.5). `SetCollisionSize` with the touch update's nothing-touched stand-in (hub ruling on
review of #802), `SetHardAttach` for four classes, and `SetBase`'s eighth class and identity case, Off. `cargo test
--workspace` 4,020 passed / 0 failed / 59 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate`
(release build, under the machine lock): 8 ok / 6 not ok, 8.500/14 = 60.7%, as on main. Links 3, 4, 5, 13 and 14 are
at "the base search's floor trace", link 7 at the HUD's pickup targeting, and 1, 2, 6 and 8 to 12 are ok. **Local probe**
(never pushed): this branch merged with #803 (fix/rust-links-13-14 @ dc0e9ef), with lane E's deferred
`PlayerPawnDuringSetUp` forced on in the combat set-up. It also gave 8 ok / 14, 60.7%, with **link 1 spawn_maya ok,
steps 2/2**. The stand-in's line was logged once on link 1, so the trade proxy's `SetCollisionSize`, `SetHardAttach`
and `SetBase` ran. Every other row was as above. UNVERIFIED in game. Decision 2026-11-26-62.

**ANALYST-LINK7-HUD-TARGETING (2026-11-26, pc-hummer, docs only, on docs/native-link7-hud-targeting, probed on main 21a668a7,
rebased onto eef4f83e):**
AI-assisted (Claude Code, Opus 5.5). NATIVE_LINK7_HUD_TARGETING.md finds the HUD's pickup targeting a pure query that
needs no Flash; its inputs are what a headless run lacks. The HUD's data come from script whose presentation steps
reach movie natives, the pickups never register because the pickup world has not begun play, and the view point
needs the camera layer. It reads the pass's shrink step (not split-screen, `MaxNumPickups` by fit). A local probe (an
installed Off strict `--slice-gate`, the inputs stood in, reverted and not committed) shows link 7's `look` passing
with the aimed gun reported; rerun on main eef4f83 (SetBase bound), link 7 passes all four steps under those
stand-ins. No code changed, so no measured number moved. UNVERIFIED in game.

**IMPL-COMBAT-WORLD-COLLISION (2026-11-26, pc-hummer, on fix/rust-combat-world-collision, stacked on #804 153ac950,
on main baa414fd):** AI-assisted (Claude Code, Opus 5.5). In Off, the slice's combat world without
`--map` gets three settings, each logged as a set-up harness input: link 9's `NoLevel` collision service, #798's
base-search setting, and the combat run's spawn positions (the n-th spawn at (1000 n, 0, 0); hub ruling). The base
search now clears another actor's collision cylinder by geometry (the 8-unit line wholly outside it), and keeps the
trace flags' `NotRead` for any other component. `cargo test --workspace -j 8 --no-fail-fast` 4,043 passed / 0 failed /
59 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate` (release build, under the machine lock) on
this head: **8 ok / 14, X = 8.5/14 = 60.7%**, main's score with #800. Links 3, 4, 5, 13 and 14 get past the floor trace (the spawns at 1000 and 2000, the
stand-in logged) and stop at the spawn's `AttributeInitializationDefinition.SetBaseValue` ("a definition it names is
not loaded"); link 7 at the pickup targeting's HUD pass. Before the rebase onto main (base 7b9daea9 against c5734abe):
7 ok both, the same rows but link 6, which #800 passes. UNVERIFIED in game. Decision 2026-11-26-64.

## Older rows

History; no gate needs it. The rows that stood below these until 2026-11-26 are in `docs/verification/gates-archive/`
(`grep -rn "<lane name>" docs/verification/gates-archive/` finds one). A row another document cites by name is there
too: `ROADMAP.md`, for one, cites FIX-FIRE-TO-HIT, PHASE-0R-READER-CHECK, SLICE-GATE-READING and "Ambient gate". Its
`README.md` says when and how rows move there; about ten stay here.

## When the Rust host changes

### Host (Rust, H1): proposed by lane H1, 2026-11-16
The host's first checks (decision `docs/decisions/2026-11/2026-11-16-44-engine-crates-wgpu-and-winit-in-ow-host-host-skeleton-h1.md`).
CI already runs most of them: the "Windows / Rust workspace" job runs the build, the tests and the refusal check, and
the Windows / MSVC LZO=ON job runs ctest, which includes `crate_edges_test.py` (`py-crate_edges`). The one new CI step
proposed here is `--headless-probe`, which waits for the maintainer's yes on a workflow change
(`.github/workflows/ci.yml` has not gained it; "CI legs" below says how the jobs split the suite).
- **CI, and every change to `ow-host`, `Cargo.lock` or `tests/crate_edges_test.py`:** `cargo build --workspace
  --locked`, `cargo test --workspace --locked` (the `ow-host` unit tests among them), `python tests/crate_edges_test.py`
  (also ctest `py-crate_edges`), and the existing refusal check (`ow-host --game <not an install>` must exit non-zero).
  Proposed, not yet in CI: `ow-host --headless-probe`: exit 0 prints the adapter; exit 4 means no adapter on that machine and counts as
  "not run", not as a failure; any other exit fails. With the engine crates, the build downloads them from crates.io
  (or restores them from the cargo cache); the CI job's comment says so since 2026-11-19.
- **A machine with a GPU and a screen** (the zuhu desktop, pc-hummer), after a change to the window, device or frame
  loop: a release build, then `ow-host --game <install> --frames 300`, and again with `--no-vsync`, timed. Pass is
  exit 0 with 300 frames presented. Record the adapter, the backend, the median frame time and the wall time. The
  vsync median should be close to the monitor's refresh interval; the `--no-vsync` median is the loop's throughput.
  A `--frames N` run fails by itself after 60 s plus 1 s per frame (`--time-limit S` sets another limit), also with
  the window minimised (runs below).
- **One run per machine:** every window run (`--window`, `--frames`) is a heavy run. Take the machine-wide lock first
  and release it after the process has exited: `. ./tools/ue_lock.ps1; Enter-OwUeLock -Owner <lane>; try { ... }
  finally { Exit-OwUeLock }`. `--headless-probe` and the install check open no window and need no lock. The
  maintainer works on the desktop's screen, so keep runs short.

**Baseline (2026-11-16, zuhu desktop, i9-9900K, RTX 4070, driver 621.01, release build, lane H1):** probe "NVIDIA
GeForce RTX 4070", Vulkan, exit 0. `--frames 300 --no-vsync`: 300 presented, 0 skipped, median 0.171 ms (min 0.164,
max 0.508) over 299 intervals, 642 ms wall. `--frames 300` with vsync: 300 presented, exit 0, median 252 ms. Dx12 gave
the same. The screen had been idle for about 18 hours, so the displays were very likely asleep; the vsync figure is
`UNVERIFIED` until it is repeated with the screen on (165 Hz monitor, 6.1 ms expected). `crate_edges_test.py` 19
tests OK.

**Minimised-window runs (2026-11-16, same desktop and build, after review r1 of PR #344, under the lock).** The
window was minimised and restored with `ShowWindow` on the window of the process the script had started. Wall times
are from process start to exit.
- `ow-host --game <install> --frames 300`: exit 0, 300 presented, 0 skipped, median 252.371 ms, 76.8 s wall.
- `ow-host --game <install> --frames 48`, minimised at 2.0 s, restored at 6.1 s: exit 0, 48 presented, 319 skipped,
  13.8 s wall.
- `ow-host --game <install> --frames 100000 --time-limit 15`, minimised at 2.0 s and kept minimised: exit 1 with
  "15 of 100000 frames presented within the 15 s limit", 15 presented, 1176 skipped, 15.8 s wall.
- In these runs the minimised window still got redraws (the skips), so the case of a minimised window that gets no
  redraw at all did not happen. That the deadline's own wake-up ends such a run is `UNVERIFIED`: the code covers it
  (the deadline is checked on every wake-up, and the loop always waits on it), but no run showed it.

### Host movement (Rust, H4): lane H4, 2026-11-16
Movement and collision (decision `docs/decisions/2026-11/2026-11-16-52-host-movement-and-collision-h4.md`).
- **Every change to `ow-host/src/movement*` or `ow-assets/src/collision.rs`:** the workspace build and tests above
  (the synthetic movement and body tests are among them).
- **On a machine with the game installed**, after a change to the mover, the sweep or the collision build: a release
  build, then `ow-host --game <install> --walk`. It opens no window, but it loads the whole scene (40-52 s on the
  desktop), so it is a heavy run: take the machine-wide lock first, as for window runs (`. ./tools/ue_lock.ps1;
  Enter-OwUeLock -Owner <lane>; try { ... } finally { Exit-OwUeLock }`). `--walk` does not take it itself (H1 has
  no Rust lock helper). Pass is exit 0 with `result: PASS`, and the collision counts unchanged unless the change
  explains them. A stall at a wall is reported by name and is not a failure; a stall with nothing in the way is.

**Baseline (2026-11-16, zuhu desktop, release build, lane H4):** placements 4,446; blocking 3,855, of them 3,124
with hulls; 299 meshes with a body (494 hulls); 4,219 brushes, 0 refused. Walk: PASS, 1,050 ticks, 3,562.2 uu
travelled, no start-solid tick; two named wall stalls. Segment 4's (a `Sanctuary_P` chain-link fence, taller than
the jump) is expected. Segment 5's, on `Sanctuary_Outer` `InterpActor_34`, is an **open question**, not settled
expected output: `prepare_level.py` drops that box's collision (decision 2026-09-23-03), and whether it blocks in
landed play is not observed (decision 2026-11-16-52). Workspace tests 1,505 passed, 0 failed, 17
ignored (on main 54880df plus this lane and its review r1); the walk run under the lock.

### Host play mode (Rust, H4-PLAY): lane H4-PLAY, 2026-11-16
Walking a map in the window (decision `docs/decisions/2026-11/2026-11-16-57-host-play-mode-walk-sanctuary-in-the-window-h4-play.md`).
- **Every change to `ow-host/src/play*`, the input or step code in `app.rs`, or `movement/walk.rs`:** the workspace build
  and tests (the `play` tests check one script's path against `walk::run` at three frame-time patterns).
- **On a machine with the game installed**, under the machine lock (every run below loads the whole scene), with a
  60 s script whose first nine segments are `--walk`'s (`local/p2/h4-play/walk60.txt` on the desktop):
  `ow-host --game <install> --walk`, then `--map Sanctuary --play-script <file>` (headless), then `--map Sanctuary
  --play --play-script <file> --size 1280x720 --screenshot <png> --no-vsync` (the window). Pass is exit 0 for all
  three, the headless run's first nine `play segment` digests equal to `--walk`'s, and the window run's twenty digests
  and end digest equal to the headless run's. Report the window's frame times and look at both screenshots. A
  run with vsync on a sleeping display shows about 252 ms frames (H1); its path must not change.

**Baseline (2026-11-16, zuhu desktop, RTX 4070, Vulkan, release build, lane H4-PLAY):** `--walk` digest
`fd6551c9598c8ec7` (1,050 ticks); headless 3,600 steps, end digest `d7b95ebfe14df774`; the window with vsync (display
asleep: median 252 ms) and with `--no-vsync` (87,516 frames in 60.3 s, median 0.680 ms, p95 0.711 ms) both gave that
digest and all twenty segment digests. After review r1 of PR #355: a scripted window run of exactly the `--walk` path
(17.5 s, `--no-vsync`, median 0.688 ms) gave `--walk`'s nine segment digests and its end digest `fd6551c9598c8ec7`;
workspace tests 1,607 passed, 0 failed, 21 ignored. Workspace tests 1,603 passed, 0 failed, 21 ignored (main 7f73940 plus this
lane; the installed-game runs were made on main e126214 plus this lane).

### Host held weapon (Rust, H4-WEAPON): lane H4-WEAPON, 2026-11-16
Firing in the play mode (decision `docs/decisions/2026-11/2026-11-16-61-host-held-weapon-fire-in-the-play-mode-h4-weapon.md`).
- **Every change to `ow-host/src/weapon*`, `overlay.rs`, or the play mode's trigger:** the workspace build and tests
  (the `weapon`, `overlay` and `play` tests).
- **On a machine with the game installed**, under the machine lock, with a firing script (`local/p2/h4-weapon/fire20.txt`
  on the desktop) and a weapon file under ignored `local/`: `--map Sanctuary --play-script <script>` without and with
  `--weapon <file>` (headless), then `--play --play-script <script> --weapon <file> --size 1280x720 --screenshot
  <png> --no-vsync` (the window). Pass is exit 0 for all three; the same path digest for all three (firing moves
  nothing); and the window's shot count and shot digest equal to the headless weapon run's. Look at the screenshots
  for the crosshair, the placeholder and, on a frame within 2 s of a hit, the impact marker.

**Baseline (2026-11-16, zuhu desktop, release build, lane H4-WEAPON, the Tediore pistol of the firing coverage
recipes):** weapon damage 34.24, fire interval 0.2100 s, range 16384 uu; path digest `3413832365d19721`; no weapon 2
shots, with the weapon 53 shots and shot digest `797e931e95ce7bb6`, headless and in the window (median frame
0.780 ms). Workspace tests 1,712 passed, 0 failed, 25 ignored (main e002de5 plus this lane with review r1 of PR #366; the installed-game
runs on main f5cd6a4 plus this lane).

### Host play mode on the loaded world (Rust, H-WORLD): lane H-WORLD, 2026-11-19
The play mode's world (decision `docs/decisions/2026-11/2026-11-19-01-host-play-mode-on-the-loaded-world-h-world.md`).
- **Every change to `ow-host/src/world*`, the play mode's world tick, or `anim/pawns.rs`' world placement:** the
  workspace build and tests (the `world`, `play` and `args` tests).
- **On a machine with the game installed**, under the machine lock, with H4-PLAY's 60 s script: `--map Sanctuary
  --play-script <file>` twice and once with `--no-world` (headless), then `--play --play-script <file> --size 1280x720
  --screenshot <png> --no-vsync --world-pawns <dir>...` (the window; the folders are UModel `-export -psk -notex
  -nostat` exports of Sanctuary's level packages under ignored `local/`). Pass is exit 0 for all four; the same path
  digest and segment digests for all four; the same world digest for both headless runs and the window. Report the
  world's load line, its halt line and how many pawns were drawn.

**Baseline (2026-11-19, zuhu desktop, RTX 4070, Vulkan, release, lane H-WORLD):** world Sanctuary_P (Off) 2,363
actors, 0 refused, 11 pawns; halted at tick 1 (`NotYet`: the host's `PHYS_Interpolating` physics for a placed
`Engine.InterpActor`), `TimeSeconds` 0.016667; path digest `d7b95ebfe14df774`, world digest `c233c6faac880f05`; the
window drew 1 of 11 pawns (median frame 1.724 ms).

**After H-PHYSICS-MODES (2026-11-19, same machine; decision `2026-11-19-22`):** the same path and world digests; still
halted at tick 1, now on the Matinee playback of a bound interpolating `WillowGame.WillowSpectatorPoint`. A change to
the physics step reruns the census (`world::tests::sanctuary_physics_census`, ignored, `OW_BL2_ROOT`) and reports its
first stop and outcome counts (today 57 pass, 23 at the world's `Base` guard, 66 at Matinee playback, 13 at the
walking, nav-mesh walking and rotating modes).

**After H-MATINEE (2026-11-19, same machine; decision `2026-11-19-53`):** the same path and world digests; still halted
at tick 1, now on the level-load event that starts the Matinee op binding a `WillowSpectatorPoint`. A change to the
Matinee scan or tracks reruns `world::tests::sanctuary_matinee_census` (ignored, `OW_BL2_ROOT`) and reports the ops by
start (today, with a complete graph: 20 at load, 53 from outside the graph, 8 with an unknown startup) and the movement tracks read (today 168 of 169,
every one ending within its op's `InterpLength`), with the physics census (today 57 pass, all unbound; no op is answered "not started at load").

The retired UE5 host's suites and their baselines are in `git show 08bcf3e:docs/verification/GATES.md`.

## Tests outside ctest
Since 2026-11-11 every `tests/*_test.py` that passes in a clean export (no `local/`, no build, no installed game, no network) under
a Python with no third-party packages, with none of its cases skipped, runs in ctest as `py-<name>` (the list in `tests/CMakeLists.txt`, which `CMakeLists.txt` includes). CI also runs
`level_test.py` and `viewer_test.py` directly (in the LZO=ON leg). The rest run only by hand (`python tests/<name>` or `node tests/<name>`); run those
that cover what you touched. Times are serial on the zuhu desktop, 2026-11-11. The list of what stays out, each with its reason, is `OW_OUTSIDE_CTEST` in `tests/CMakeLists.txt`;
`tests/realgame_registration_test.py` (ctest `py-realgame_registration`, since 2026-11-13) fails when a test of the real-game tools (one
that imports `tests/realgame_test_env.py` or names a `tools/real_game/` path) is neither registered nor in that list with a reason.

Since 2026-11-11 (lane ci-test-deps) CI also installs Node 26.4.0 and numpy 2.5.4 for tests only. The 18 `tests/*_test.js` run in
ctest as `js-<name>` (`OW_NODE_TESTS`) and the 9 Python tests that need numpy as `py-<name>` (`OW_NUMPY_PY_TESTS`: `ambient_transform`,
`creature_zone`, `listen_report_fit` (added by PR #221), `lootable_tracks`, `phaselock_target_attachments`, `prepare_phaselock_target`, `slice_npc_assets`, `weapon_reload_anims`,
`weapon_reload_library`). Each group is registered only when configure finds Node, or a `${Python3_EXECUTABLE}` that imports numpy;
otherwise configure prints which group it left out, and everything else still runs. CI configures with
`-DOW_REQUIRE_TEST_TOOLS=ON`, which turns a group left out into a configure error. CI's step "Case counts of the tests labelled
needs-*" prints each such entry's case count and fails the job if any of them skipped a case.

**CI legs (since 2026-11-19, decision `docs/decisions/2026-11/2026-11-19-100-ci-parallel-ctest-split-lzo-legs-cargo-cache-docs-only-skip.md`):**
ctest runs in parallel (`--parallel` at the runner's core count). The LZO=ON job runs every entry; the LZO=OFF job runs only the
entries labelled `cxx-binary` (`ctest -L cxx-binary`): those whose command runs `ow-package`, `ow-weapongen` or
`ow-weapon-firing-test`, found by `tests/CMakeLists.txt` from its own `add_test` calls. The `needs-*` case-count step checks the
`needs-*` entries of its job's share, so all of them in LZO=ON. CI creates the ignored `local/` folder before ctest, as a working
checkout has it. A pull request whose every changed path is under `docs/` (except `docs/TOOLING.md`, which
`real_game_migration_test.py` reads) or is a top-level `*.md` skips the three Windows jobs; the leak gate always runs. Locally,
`ctest -j 8` is the same suite as serial ctest.

Pillow (commit 2 of that lane, landed only with the maintainer's yes): CI installs `pillow==12.2.0`, and the 8 tests in
`OW_PILLOW_PY_TESTS` (`gfx_to_swf`, `prepare_map_screen`, `prepare_minimap`, `prepare_mission_log`, `prepare_mission_screen`,
`sanctuary_pillar`, `weapon_paint`, `weapon_part_census`) run as `py-<name>` where Pillow and numpy both import (`sanctuary_pillar`
needs both). `prepare_mission_log` skips one of its 4 cases where file symlinks need a privilege (this PC).

| Tests | Why not in ctest | Status on 2026-11-12 |
|---|---|---|
| `cash_capture` (236-237 s), `realgame_snapshot` (147-148 s), `skills_live_probe` (60-61 s) | over the budget: a file may add about 90 s and all additions together about 3 minutes (lane C, 2026-11-13). The three registered real-game files take that 3 minutes. `cash_capture` could be split (its 74 cases outside `DenDownReplacementTest` and `LoadWaitTest` take about 85 s), but that subset would not fit beside them, and a subset named by class would leave out any class added later | pass, also in the sanitised shell (2026-11-13) |
| `check_hud_minimap` (30 s), `provision_worktree` (36 s) | too slow for every PR (2026-11-11 times, not re-measured) | pass |

Notes (as of 2026-11-12; `lootables_editor` and `run_limit` left with the UE5 host on 2026-11-14): a test that skips cases there, or skips all of them, does not count as passing. Four registered tests skipped cases off Windows
(`check_overlay_data`, `lootables_editor`, `private_sync`, `run_limit`: junctions, PowerShell launchers); CI runs
on Windows, so they run there. `serve_paths` skips nothing: it makes a directory symlink, and on Windows falls back to a junction when that is refused. `private_sync` also skips its UNC alias case without an administrative share. `game_colours` and
`prepare_hud_strings` create `local/` in a clean checkout (the second leaves it empty); it is git-ignored and harmless. The slowest registered entry
that started processes then, `run_limit` (about 40 PowerShell runs), took 11.2 s on CI (PR #219, run 43062806521) and 15.4 s here, well under
its 120 s timeout. On that CI run (Python 3.12) the new entries took 66 s of 172 s with LZO on. `weapon_reload_library`
skips its junction case when `mklink /J` fails; on CI that skip fails the job.
