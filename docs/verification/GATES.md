# Gates and the current baseline

Update this file in the commit that changes a number. Paste real output in reports; do not copy these numbers.

**IMPL-PAWN-AI-INTERFACE (2026-11-19, desktop, on main bb46e11):** AI-assisted (Claude Code, Claude Opus 5.5).
`Pawn`'s `QueryInterface` rule for `IAIInterface` in Off (a per-class rule registry in `ow-vm`, `ow-combat`
`pawn_interface`); the open answers are named `NotYet`s; Reference unchanged. `cargo build --workspace` clean; `cargo
test --workspace` 2,099 passed / 0 failed / 45 ignored; ctest at `-j 2` 173/173, 0 failed, 538.78 s; verify_packages 9
packages, all class bodies exact; `crate_edges_test.py` 19 OK. Installed, Off: `--slice-gate` strict 0/14/0,
byte-identical before and after (no row reaches `Behavior_AIHold`; link 11 stops at `accept`, `GetDataStoreClient`);
`--mission-run` Fire unchanged. **Rebased onto main 74c7b0a** (`ow-vm` `api.rs` kept both sides): `cargo build
--workspace` clean; `cargo test -p ow-vm -p ow-combat -p ow-ai -p ow-mission -p ow-cli` 753 passed / 0 failed / 7
ignored. **Review r1 of #456:** `cargo test --workspace` 2,167 passed / 0 failed / 46 ignored; clippy flags nothing in
the changed files; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict 0/14/0; no row names a QueryInterface
rule (links 10 to 12 at `accept`, `GetDataStoreClient`; the combat links at `maya:8`). Decision 2026-11-19-62; logs
under ignored `local/p2/pawn-ai/`.
**IMPL-BLACKMARKET-RESOLVER (2026-11-19, pc-hummer, on main 74c7b0ac with #451's note stacked):** AI-assisted
(Claude Code, Opus 5.5). `WillowGame.BlackMarketUpgradeAttributeValueResolver` in Off from NATIVE_BLACKMARKET_UPGRADES.md
alone. `cargo build --workspace` clean; `cargo test --workspace` 2,169 passed / 0 failed / 46 ignored (14 resolver tests);
`crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict: 0/14/0, byte-identical before and after on the same base. Links 1, 3, 4, 5, 13 and 14 stop at
`maya:8` on the stock `OnPlayerClassChange`'s unbound `WillowPlayerController.InitPlayerSkillTree` (#444), which runs
before the ammo maxima, so the resolver is not reached on main. No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-72; logs under ignored `local/p2/bm-resolver/`.
**IMPL-INVENTORY-CONTEXT (2026-11-19, desktop, rebased onto main 9e53789):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,128 passed / 0 failed / 45 ignored (6 new in `ow-attr`, 3 in
`ow-inventory`). `crate_edges_test.py` OK. ctest `-j 2` 173/173, 388.70 s. verify_packages 9 packages, all class bodies exact.
Installed Off, main and this branch each built: `--slice-gate` strict 0/14/0 before, 0/13/1 after (link 8 moves from the
resolver's `NotYet` at `init` to the unbound native `WillowInventory.InitializeAttributeSlots(bool)`; the other 13 rows identical).
No host or real-game run; the rules stay `UNVERIFIED` in game. Decision 2026-11-19-67.
Review r1 of #455 (same day, desktop): `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,130 passed / 0 failed /
45 ignored (2 new `ItemActors` script tests in `ow-inventory`). `crate_edges_test.py` OK. ctest `-j 2` 173/173, 444.60 s. verify_packages
9 packages, all class bodies exact. Installed Off `--slice-gate` strict 0/13/1, unchanged (link 8 still stops at `InitializeAttributeSlots`).

**IMPL-SETTINGS-FILL (2026-11-19, pc-hummer, rebased onto main 7556c254, which has #448 and #450):** AI-assisted (Claude Code, Opus
5.5). The online game settings entry filled at store creation, `SetIntProperty`, `GetCurrPlaythrough` and `GetDLCManager`
(None) in Off, from NATIVE_SETTINGS_FILL.md. `cargo build --workspace --locked -j 4` clean; `cargo test --workspace
--locked -j 4` 2,178 passed / 0 failed / 46 ignored after the rebase (5 new data store tests in `ow-world`); `crate_edges_test.py` 19 OK.
Installed Off `--slice-gate` strict (release build and run under the machine lock): 0 ok / 14 NotYet / 0 errors; links
10 to 12 pass 3 of 4 driver steps and stop in `accept` at `GetActiveMissionNumber` ("stopped after partial writes"),
past the settings entry. ctest and verify_packages not run (no C++ change). No real-game run; every rule stays
`UNVERIFIED` in game. Decision 2026-11-19-80; logs under ignored `local/p2/settings-fill/`.

**FIX-ATTR-GETVALUE-NATIVE (2026-11-19, pc-hummer, review r1 folded in, rebased onto main 7e80d7b6):** AI-assisted
(Claude Code, Opus 5.5). `AttributeDefinition.GetValue` bound as a script native in Off on `ow-attr`'s shared steps
(preflight, context, out write, value chain). After the rebase: `cargo test -p ow-combat -p ow-attr -p ow-inventory
--locked -j 4` 445 passed / 0 failed / 3 ignored (5 `GetValue` tests); clippy flags nothing in the changed files. Before
review r1, on main b985dcbf: `cargo test --workspace` 2,044 / 0 / 45 and `crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict on this branch: 0/14/0, with links 1, 3, 4, 5, 13 and 14 at the player controller's starting-values
`NotYet` (#444 is not on main yet, so the native is not reached). On a local unpushed merge with
`fix/rust-class-starting-values` before review r1, the six links moved to the library's `NotYet` on
`WillowGame.BlackMarketUpgradeAttributeValueResolver`, still inside `maya:8`. No real-game run; `UNVERIFIED` in game.
Decision 2026-11-19-58; logs under ignored `local/p2/fix-attr-getvalue/`.
**FIX-CLASS-STARTING-VALUES (2026-11-19, pc-hummer, rebased onto main 7de9171a, which has #429, #431 and #434):**
AI-assisted (Claude Code, Opus 5.5). `ApplyCharacterClassStartingValues` (the `Controller` base and the
`WillowPlayerController` override) in Off from NATIVE_CLASS_STARTING_VALUES.md. `cargo build --workspace --locked -j 4`
clean; `cargo test --workspace --locked -j 4` 2,053 passed / 0 failed / 45 ignored (3 new, 1 updated in `ow-combat`);
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict: links 1, 3, 4, 5, 13 and 14 move from the override's
NotYet to an error at `maya:8`, the unbound script native `AttributeDefinition.GetValue(object,object,object)`; rows 10
to 12 stop at `UIInteraction.GetDataStoreClient` (`accept`); 0 ok / 8 NotYet / 6 errors. **Review r1:** `cargo test -p
ow-combat -p ow-world` 273 passed / 0 failed / 7 ignored, ow-cli 89 passed (combat digests unchanged); clippy flags
nothing in the changed files; installed Off `--slice-gate` 0/14/0, the six links now at the `InitPlayerSkillTree`
`NotYet { NotPorted }` before any delete. **Review r2:** ow-combat + ow-world 295 / 0 / 7, ow-cli 89 passed; after rebasing onto main c7f08814 (#442
landed), ow-combat + ow-world 301 / 0 / 7. ctest
and verify_packages not run (no C++ change). No host or real-game run; every rule stays `UNVERIFIED` in game. Decision
2026-11-19-57; logs under ignored `local/p2/fix-class-starting-values/`.

**FIX-PLAYER-CLASS-B (2026-11-19, desktop, on main df99b25):** AI-assisted (Claude Code, Claude Opus 5.5). The pawn
archetype preload (a harness input) and the player controller's starting values NotYet in Off; review r1 of #426 folded
in. `cargo build --workspace` clean; `cargo test --workspace` 2,017 passed / 0 failed / 45 ignored; ctest at `-j 2`
173/173, 0 failed, 535.32 s; verify_packages 9 packages, all class bodies exact. Installed, Off: `--combat-run maya:8`
and slice-gate links 1, 3, 4, 5, 13 and 14 move from `FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren` to
`WillowPlayerController.ApplyCharacterClassStartingValues`'s NotYet (the unnamed script call of NATIVE_COMBAT_GROUNDWORK.md
1.10 step 2); both harness inputs are in the run's log; links 10 to 12 unchanged.

**FIX-ACTIVATE-EVENT (2026-11-19, pc-hummer, rebased onto main f64aeba9; installed runs on main cb01e96f):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,021 passed / 0 failed / 45 ignored after the rebase
(3 new in `ow-ai`, 76/76 there). Release CLI build clean. ctest `-j 4` 173/173 passed, 113.12 s (C++ unchanged; existing
build). `crate_edges_test.py` 19 OK; Fire checker self-test passes and, under Reference with this branch's binary, 33 of 33
rules hold. Installed Off: `--slice-gate` strict 0/11/3 before, 0/14/0 after (link 10 moves from the `ActivateEvent` error
at `use` to the `GetDataStoreClient` `NotYet` at `accept`); `--slice-run` and `--mission-run` Fire unchanged. No host or
real-game run; the rule stays `UNVERIFIED` in game. Decision 2026-11-19-42; logs under ignored
`local/p2/fix-activate-event/`. **Reviews r1 to r3:** ow-ai 84/84 after r3 (10 ActivateEvent tests); r2: ow-cli 82/82;
`cargo test --workspace` 2,027 passed / 0 failed / 45 ignored; installed Off `--slice-gate` strict still 0/14/0, rows 10
to 12 unchanged at `GetDataStoreClient` (`accept`).

**FIX-PLAYER-CLASS (2026-11-19, desktop, on main e59bff0):** AI-assisted (Claude Code, Claude Opus 5.5).
`GBXDefinition.GetDefinition` and `LoadPlayerPawnDataAsync` in Off; the `maya` step's class select in Off.
`cargo build --workspace` clean; `cargo test --workspace` 2,010 passed / 0 failed / 45 ignored; ctest at `-j 2`
173/173, 0 failed, 357.40 s; verify_packages 9 packages, all class bodies exact. Installed, Off: `--combat-run
maya:8` and slice-gate links 1, 3, 4, 5, 13 and 14 move from `FindAlreadyLoadedObject` with an empty path to
`FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren` (held by a package, not loaded); links 10 to 12
unchanged (`AIComponent.ActivateEvent`).

**FIX-POOL-SPAWN-B (2026-11-19, desktop, on main 79036dd):** AI-assisted (Claude Code, Claude Opus 5.5). The three
`WillowInventory` natives bound in Off; `GetItemTossVelocity` NotYet in Off; Reference unbound. `cargo build
--workspace` clean; `cargo test --workspace` 1,972 passed / 0 failed / 43 ignored (7 new `ow-inventory`, 7 new
`ow-combat` tests). C++ Release build clean; ctest at `-j 2` 173/173, 0 failed, 472.69 s; verify_packages 9 packages,
all class bodies exact. Installed, Off: `--slice-gate` 14 rows identical before and after (links 1, 3, 4, 5, 13 and 14
stop at the unbound `WillowGlobals.GetPlayerPawnDataManager()`); `--combat-run` `player:8 spawn:baby:6` stops the
spawn at `TargetableAttributeValueResolver`, before and after. No installed Off run reaches the death roll.
**TEST-FOLLOWUPS (2026-11-19, desktop, on main 0a23f13):** AI-assisted (Claude Code, Claude Opus 5.5). Tests only:
two review follow-ups (#407 pack order, #405 Force with a parked thread). `cargo build --workspace` clean;
`cargo test --workspace` 1,977 passed / 0 failed / 45 ignored; ctest at `-j 2` 173/173, 0 failed, 488.16 s;
verify_packages 9 packages, all class bodies exact.
**TEST-FOLLOWUPS (2026-11-19, desktop, on main 5554ead):** AI-assisted (Claude Code, Claude Opus 5.5). Tests only:
three review follow-ups (#407 pack order, #405 Force with a parked thread, #413 a script `AllParts` loop through the
engine iterator). `cargo build --workspace` clean; `cargo test --workspace` 1,987 passed / 0 failed / 45 ignored;
ctest at `-j 2` 173/173, 0 failed, 432.30 s; verify_packages 9 packages, all class bodies exact.

**FIX-PHASELOCK-CAST part B (2026-11-19, desktop):** AI-assisted (Claude Code, Claude Opus 5.5).
`ActionSkill.OnActionSkillActivated` and `OnActionSkillDeactivated` bound in Off (Reference unbound). `cargo build
--workspace` clean; `cargo test --workspace` 1,880 passed / 0 failed / 42 ignored before the rebase (four new
`ow-skills` tests), 1,889 / 0 / 42 after it (base f35ae69). C++ Release build clean; ctest at `-j 2` 173/173, 0 failed,
606.01 s before the rebase; after it 170/173 on a loaded machine, the 3 misses leak-gate Python suites timing out at
120 s (two passed on rerun; `py-leak_gate_policy` passes 19/19 run directly). verify_packages 9 packages, all class
bodies exact. Installed Phaselock test passes in both modes: in Off `OnActionSkillStarted` now stops at the unbound
`GearboxPawn.GetSMComponent()`; Reference still stops at `ActionSkill.OnActionSkillActivated`.

**FIX-PHASELOCK-CAST part A (2026-11-19, desktop):** AI-assisted (Claude Code, Claude Opus 5.5). `Object.VLerp` in
`ow-vm` (both modes), the two provider natives read into NATIVE_BEHAVIOR_PROVIDERS.md (not bound). `cargo build
--workspace` clean; `cargo test --workspace` 1,876 passed / 0 failed / 42 ignored (one new `ow-vm` test). Fresh C++
Release build (VS 18, `-T version=14.50`, `--parallel 2`) clean; ctest at `-j 2` 173/173, 0 failed, 466.46 s;
verify_packages 9 packages, decoded bytes, counts and export fields match, all class bodies exact. Installed
Phaselock test (ignored, `OW_BL2_ROOT`) passes in Off and Reference: the lift runs 43 frames, Z 0 to 200, never down;
`OnActionSkillStarted` stops at the unbound `ActionSkill.OnActionSkillActivated`.
**FIX-FIRE-TO-HIT (2026-11-19, pc-hummer, main 86c28d00 plus this lane):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 1,882 passed / 0 failed / 42
ignored (7 new: 6 `ow-combat` shot tests, 1 `ow-world` replication-info fallback). Release CLI build clean.
`decisions_index.py --check` OK (1 added); `crate_edges_test.py` 19 tests OK; Fire checker and combat checker
self-tests pass. `verify_packages.py`: 9 packages, 3,339/3,339 class bodies exact with the C++ reader (same sources)
and with this branch's Rust reader. Fresh C++ Release build (`-T version=14.50`, `OW_REQUIRE_TEST_TOOLS=ON`, `--parallel 4`)
clean; `ctest -j 4` 173/173 passed, 0 failed, 126.12 s (the 7 Rust CTests included).
Installed, Off: `--combat-run` (W6-COMBAT's 15 steps) 0 rows on main, 13 after (spawns pass; hits stop at the
`WillowAIComponent` hold override; swings at unbound `WillowGameInfo.TravelPostCountdownInProcess`); Reference 29
rows, identical before and after. `--slice-run` Off byte-identical before and after (exit 3 at the Pawn
`IAIInterface` refusal, 12 steps; the checker cannot parse the partial report). New `weapon:` / `fire:` steps: Reference
4 shots land (18 rows); Off each shot stops at the same hold override. No host or real-game run; every rule stays
`UNVERIFIED` in game. Decision 2026-11-19-06; logs under ignored `local/p2/fix-fire-to-hit/`.

**Object.Localize path correction (2026-11-18, desktop):** AI-assisted (Codex/GPT-6.1 Sol).
The CLI now derives localization roots from the package store's absolute, lexically normalized root.
A child test process covers `--cooked .`, relative parent components and an absolute parent-component path
without changing the parent test process's working directory. The regression failed before the fix (0 passed /
1 failed) and passes within the workspace: 1,870 passed / 0 failed / 42 ignored. Workspace build and release
CLI build pass; Rust CTests 7/7, 0 failed, 62.58 s. Fresh Reference capture matches saved main exactly
(22/22 OK, 0 errors, 39 events); Off still exits 3 with the same partial output and Pawn IAIInterface refusal
(12 emitted OK steps, 14 events; final errors array unavailable). Earlier C++/Python and Fire evidence is
reused, not rerun. Logs are under ignored `local/p2/fix-localize-off/review-r2-*`. No host or real-game run;
native/gameplay parity remains `UNVERIFIED`.

**Object.Localize INT / Off (2026-11-17, desktop; operations recovery 2026-11-18):** AI-assisted
(Codex/GPT-6.1 Sol). Workspace build and release CLI build pass (`--locked -j 2`); workspace tests
1,870 passed / 0 failed / 42 ignored. Focused tests: 9 passed / 0 failed / 1 installed test ignored;
the first run's one path-separator assertion failure is preserved. Explicit installed test: 1 passed,
3/3 INT lookups, no localized payload printed. Rust CTests: 7/7, 0 failed; Fire self-test passes and
Reference installed checker: 33/33 rules. Saved-main Reference capture matches exactly: 22/22 OK,
0 errors, 39 events. Off remains exit 3 after 12 complete emitted OK steps / 14 events at the earlier
Pawn IAIInterface refusal; partial stdout and fatal stderr match saved main, final errors array unavailable.
Unchanged C++/Python coverage is reused separately: 166/166 and 9 packages / 3,339 exact class bodies,
not rerun here. The interrupted original workspace test had no completed exit; recovery ran the outstanding
test/release gates only. Logs and binaries: ignored `local/p2/fix-localize-off/`. No host, visual or
real-game validation; native behavior and gameplay parity remain `UNVERIFIED`. Decision 2026-11-17-11.

**NAV-MESH review r2 (2026-11-17, zuhu desktop):** AI-assisted (Codex / GPT-6.1 Sol). One additional synthetic
refusal-table case reproduces a neighbour edge with repeated endpoints: it failed before the sorted-pair check and
passes after it. Workspace build and release CLI build clean; workspace tests 1,784 passed / 0 failed / 30 ignored;
crate edges 19/19. After building the release Rust reader, configure with `OW_REQUIRE_TEST_TOOLS=ON` registered the
seven Rust CTests absent from the earlier build: all 7 passed, 0 failed, 74.47 s. The prior 166 C++/Python checks
apply to byte-identical inputs; they were not repeated. Fresh C++ reader verification: 9 packages, 3,339/3,339 class
bodies exact. These are automated checks; no host, visual or running-game validation was performed.

**NAV-MESH review r1 (2026-11-17, zuhu desktop, research/rust-navmesh):** AI-assisted (Codex / GPT-6.1 Sol),
continuing partial Claude Code fixes. `cargo build --workspace --locked -j 2` clean; `cargo test --workspace --locked
-j 2`: 1,784 passed / 0 failed / 30 ignored (1,782 passed before this fix round). The four synthetic navmesh tests
pass; the ignored installed-package driver run separately passes 1 / 0, reading all four Sanctuary meshes with the
stricter structural checks (12,092 / 191 / 148 / 46 triangles, unchanged). `crate_edges_test.py`: 19 tests OK.
Fresh C++ Release build at `--parallel 2` clean; CTest at `-j 2`: 166/166, 0 failed, 404.61 s. Package verification:
9 packages, decoded bytes/counts/export fields match, 3,339/3,339 class bodies exact. The optional `rust-parity`
entry was not registered because the Rust release CLI is absent; the separate `py-rust_parity` entry passed.
Installed-package checks are automated parsing evidence; no running-game, host or visual validation was performed.

**FIX-POOLS-OFF recovery (2026-11-17, pc-hummer, main 03df52d9 plus this lane):** AI-assisted
(Claude Code; recovery: Codex / GPT-6.1 Sol). Workspace build passes (`--locked -j 2`), workspace tests
1789 passed / 0 failed / 35 ignored. Recovered prior ctest: 173/173 (C++ unchanged); reader verification:
9 packages, 3,339/3,339 class bodies exact (recovered Rust log and fresh C++ reader run). Fire checker self-test
passes and 33/33 installed rules hold; combat checker self-test passes; crate edges 19 tests OK. Legacy Python
combat suite: 3 identical failures on the saved main binary and this lane in Reference; not a pass. Saved installed
combat trace checker: 8 PASS / 0 FAIL / 51 MISSING of 59, 43 loot rows not listed. Off CLI combat reaches unbound
`ResourcePoolManager.ShutDown`; isolated slice still stops at #370's missing template holder. No host or real-game
validation in this recovery. Decision 2026-11-16-82; full logs under ignored `local/p2/fix-pools-off/`.

**Rust switch (2026-11-14, branch rust-switch):** on main 08bcf3e plus the switch, zuhu desktop: ctest 160/160 (177 entries before; the 17 removed covered only the UE5 host or its scripts: `anim-import-frames`, `controls-keys`, `movie-hud-switch`, `play-slice-launcher`, `suite-map-tracked` and 12 `py-*`), 189 s with `-j 4`; verify_packages 9 packages, decoded bytes, counts and export fields match, all class bodies exact; `cargo build --workspace --locked` clean and `cargo test --workspace --locked` 4 passed / 0 failed (`ow-host`'s install check; the other crates hold no tests yet); `python tools/decisions_index.py --check --baseline origin/main` OK, 1 added; `viewer_test.py` and `level_test.py` pass; `tools/leak_gate.py --staged` 0 errors (warnings: the `ci.yml` gate change, and `.rs`/`.toml`/`.lock` as file types the gate does not know yet).

**Rust scaffold (2026-11-14, zuhu desktop, feature/rust-w0-scaffold on main 74d7dda, lane W0-SCAFFOLD after review r1):** 22 crates; ctest 161/161 (160 before; the new entry is `py-crate_edges`), 185 s with `-j 4`; verify_packages 9 packages, decoded bytes, counts and export fields match, 3,339/3,339 class bodies exact (the C++ reader, unchanged); `cargo build --workspace --locked` clean, `cargo test --workspace --locked` 28 passed / 0 failed (`ow-core` 17, `ow-cli` 6, `ow-host` 4, `ow-container` 1), `cargo build --release -p ow-cli --locked` clean; `python tests/crate_edges_test.py` 13 tests OK (same numbers after review r2); `python tools/decisions_index.py --check --baseline origin/main` OK, 0 added. The Rust `ow-package` exits 3 (not ported) for every command.

The UE5-host suite numbers below (`test_quest`, `test_mover`, `test_inventory_actions`, `test_weapon_firing`, the HUD
shots, the slice gate and `ambient`) are history; those suites left the tree with the host on 2026-11-14. The ctest,
verify_packages, Fire checker and combat checker numbers below were the baseline of their day.

**Baseline (2026-10-08, commit c33ed0c, second machine):** ctest 15/15; verify_packages 9 packages, 3,339/3,339 class
bodies exact; Fire event order self-test and 32 of 32 rules; test_quest 117/117 plus resume 11/11; test_mover 16/16;
test_inventory_actions 49 pass / 0 fail; verify_combat_pawn `--vm` 25 pass / 0 fail / 4 missing (the Defense Boost rows).

**After lane Sf (2026-10-09, S-f down state and respawn):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` 55 pass / 0 fail / 4 missing (the 25 earlier passes plus 30 down rows).

**After lane Sg (2026-10-09, S-g loot on death):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` on the S-a to S-f run still 55 pass / 0 fail / 4 missing (of 59); with the loot run's JSON added (`--vm all_rows.json loot.json`, [COMBAT_LOOT.md](COMBAT_LOOT.md)) 97 pass / 0 fail / 4 missing / 1 open (the live 3-item money drop) of 102.

**After the Weapons momentum/sweep change (2026-11-03, pc-hummer, on main 95e48fb):** ctest 18/18 (one test fewer:
`weapon_refresh_firing` retired); verify_packages 9 packages, all class bodies exact; `tools/test_weapon_firing.ps1` 14 PASS /
0 FAIL with the new push, wide and flight-timing checks (WEAPON_PROJECTILES.md section 5).

**After the HUD lane (2026-11-03, pc-hummer, main 95e48fb + feature/hud-integrity, Fight For Your Life in the UE host):** ctest 19/19;
verify_packages 9 packages, all class bodies exact. `test_quest` now has 112 checks; the new `player_death_goes_down_first` and the respawn
after the 20 s bleed-out pass. `run_hud_shots.ps1 -Down` 10/10, `-Tracker` 4/4, default 16/16; `test_action_skill_hud` 19/19;
`test_hud_minimap` 15/15. Rebased on main 490661e (PR #61) with a regenerated audio manifest: ctest 21/21, `test_quest` first run
112/112 and resume 11/11. On main 542d0c6 (PR #62): ctest 20/20, `test_quest` 112/112 + resume 11/11, `-Down` 10/10, `-Tracker` 4/4,
`test_weapon_firing` 14/14 (with the local slice recipes' `firing` block regenerated).

**Slice gate (2026-11-04, pc-hummer, main be94f4a + feature/slice-gate):** `tools/test_slice_gate.ps1` 9 PASS / 0 FAIL /
4 NOT_AVAILABLE of 13 links (the gaps are pieces other lanes own; [SLICE_GATE.md](SLICE_GATE.md)); ctest 21/21.
On main 8bb74a0 (after #88): 10 PASS / 0 FAIL / 3 NOT_AVAILABLE (the pickup link now checks the pickup card); ctest 22/22.
On main 195a016: 10 PASS / 0 FAIL / 4 NOT_AVAILABLE of 14 (new decoy-pickup link); ctest 22/22.
On main 7eee9ee (2026-11-07, zuhu, Z23): 14 PASS / 0 FAIL / 0 NOT_AVAILABLE of 14 in four runs (no switch twice, `-Shield`, `-DownAfterFight`); ctest 28/28; verify_packages exact; range 36 / 0; firing 14 of 14; interactives 11 / 0; mission page 21 of 21 on its rerun (the first run failed 4 rows with the movie unavailable). `-Tracker` 3 of 4 (`tracker_page` saw 6 of 7 states) on this PC.

**Ungated tests registered (2026-11-11, zuhu, main c66f24e + fix/ungated-tests):** ctest 119/119 (34 before; the 85 new `py-*` entries took 76 s of the 211 s run); after review r1, 118 (84 `py-*`, `slice_npc_assets` moved out); verify_packages 9 packages, all class bodies exact. The tests left out are in "Tests outside ctest" below.

**Node and numpy groups (2026-11-11, zuhu, main 969307f + feature/ci-test-deps, Node 26.4, numpy 2.5.4, Python 3.14.6):** ctest 145/145
(119 before; `needs-node` 18 entries in 1.8 s, `needs-numpy` 8 in 2.5 s). Configured with a Python without numpy and no Node: 119
entries and one message per group left out.

**With PR #221 (2026-11-11, zuhu, feature/ci-test-deps + feature/audio-mix-2d-level, numpy 2.5.4):** ctest 154/154 (`needs-numpy` 9 entries in 5.7 s,
`listen_report_fit` among them, 10 cases, none skipped; `needs-node` 18, `needs-pillow` 8).

**Real-game tool tests registered (2026-11-13, zuhu, lane C, main 8ee59bb + test/register-realgame-tests):** ctest 174/174 (170 before).
New entries `py-controls_capture`, `py-real_game_capture_safety` and `py-real_game_pending` (43, 58 and 70 s; three runs each, spread
under 1.5 s) and the guard `py-realgame_registration`. All six real-game test files that were outside ctest also passed in a shell with
no `OPENWILLOW_*` or `OW_*` variable, PowerShell 7's module path and a working directory outside the checkout. See "Tests outside ctest".

**On main 81d87f9 (2026-11-13, CI job "Windows / MSVC / LZO=ON" on PR #264's merge ref):** `100% tests passed out of 174`. The desktop runner kept that job's log (`local/z58/cand33/ci_job.log` in its worktree). On the zuhu desktop the same tree registers 174 entries and passes 174 of 174 (LZO on, 361 s); a configure with `-DOPENWILLOW_LZO=OFF` registers 172 (`ctest -N`).

**Fire checker level rule (2026-11-12, zuhu, lane R3):** `verify_fire_event_order.py` self-test passes and 33 of 33 rules hold on the real packages (32 before; the new one: no `level` event in the default run).

**Ambient gate (2026-11-12, zuhu, lane R3):** the `ambient` suite's PASS now also needs a capture-based manifest (`observed`, or worn pieces): `tools/test_ambient.ps1` names the manifest's look in its result line and fails a census-only one before launch unless `-CensusWorld` is passed. First run in the engine on main a3daacf (2026-11-13, zuhu desktop, hub candidate 26), with the capture-based file: `OWAMBIENT SUMMARY result=PASS pawns=35 reached_a_node=35 walked_50cm=3 at_perch=24 seconds=60 manifest observed=true spawns=35 worn=89` (log: the desktop runner's `local/z58/cand26/suites/ambient.log`; the same line again on candidate 30, tree of main 0d2405e). The citizens' movement is a host stand-in, UNVERIFIED.

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

- The slice gate's `-Shield` run was a UE5-host run and left the tree with it; its last numbers are history
  (`git show 08bcf3e:docs/verification/GATES.md`).

## When combat changes
- `python tools/verify_combat_pawn.py --self-test`, and `--vm <json>` on a fresh `--combat-run`.
- `python tools/verify_combat_pawn.py --live` checks the formulas against ignored live dumps when they exist.

## When the Rust host changes

### Host (Rust, H1): proposed by lane H1, 2026-11-16
The host's first checks (decision `docs/decisions/2026-11/2026-11-16-44-engine-crates-wgpu-and-winit-in-ow-host-host-skeleton-h1.md`).
CI already runs most of them: the "Windows / Rust workspace" job runs the build, the tests and the refusal check, and
the Windows / MSVC jobs run ctest, which includes `crate_edges_test.py` (`py-crate_edges`). The one new CI step
proposed here is `--headless-probe`, which waits for the maintainer's yes on a workflow change
(`.github/workflows/ci.yml` is unchanged).
- **CI, and every change to `ow-host`, `Cargo.lock` or `tests/crate_edges_test.py`:** `cargo build --workspace
  --locked`, `cargo test --workspace --locked` (the `ow-host` unit tests among them), `python tests/crate_edges_test.py`
  (also ctest `py-crate_edges`), and the existing refusal check (`ow-host --game <not an install>` must exit non-zero).
  Proposed, not yet in CI: `ow-host --headless-probe`: exit 0 prints the adapter; exit 4 means no adapter on that machine and counts as
  "not run", not as a failure; any other exit fails. With the engine crates, the build downloads them from crates.io,
  so the CI job's comment "nothing is downloaded" must change with the job.
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

## Records
- `python tools/decisions_index.py --check --baseline origin/main` after adding, editing or moving decisions (with the
  default baseline, `HEAD`, it passes trivially on committed work). Add `--at HEAD` to check the committed blobs.

## Tests outside ctest
Since 2026-11-11 every `tests/*_test.py` that passes in a clean export (no `local/`, no build, no installed game, no network) under
a Python with no third-party packages, with none of its cases skipped, runs in ctest as `py-<name>` (the list in `tests/CMakeLists.txt`, which `CMakeLists.txt` includes). CI also runs
`level_test.py` and `viewer_test.py` directly. The rest run only by hand (`python tests/<name>` or `node tests/<name>`); run those
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

**H-PAWNS review correction (2026-11-17, desktop, PR #378 after 7c7c387):** AI-assisted
(Codex/GPT-6.1 Sol). An empty static scene now still invokes the attached-world draw callback.
The invented offscreen GPU regression checks exactly one callback with and without static instances:
old code 0 passed / 1 failed; correction 1 passed / 0 failed, 191 filtered out, 0.46 s.
The test is ignored by default because it needs an adapter; it was explicitly run locally.
No new window, real-game session or visual/gameplay parity claim. Prior reported workspace and C++
results were not rerun for this one-line rendering correction; final-head branch CI remains required.

**FIX-BEHAVIOR-CONTEXT-OBJECT (2026-11-18, pc-hummer, base 2fcae040):** AI-assisted
(Codex / GPT-6.1 Sol). Off runner selection now uses the explicit context object when there is no Context link;
a Context link still wins even when empty. Native/gameplay semantics remain UNVERIFIED. Corrected synthetic
regression RED: 2 passed / 3 failed; GREEN: 5 passed / 0 failed (included in the workspace run).
`cargo build --workspace --locked -j2`, `cargo test --workspace --locked -j2 --no-fail-fast` and the release CLI
build exit 0; workspace totals 1,865 passed / 0 failed / 41 ignored. Seven Rust CTests: 7/7 passed, 13.46 s.
Reused non-Rust CTests: 166/166; reused C++ reader verification: 9 packages, 3,339/3,339 class bodies exact.
These were not rerun: the C++/Python input blobs match the earlier local evidence (506 paths checked against
its source dadca792), and no src/tools/tests/CMakeLists input path changed. The earlier full 173-entry run had
two Rust failures; those are superseded here only by the fresh seven-entry Rust run.
Fresh parent/child Reference slice captures: both exit 0, 22 steps, 39 events, 0 errors, equal after excluding
only timing fields. Off captures: both exit 3, identical truncated JSON with 12 complete steps and 14 events;
the Pawn IAIInterface routing refusal remains. This does not establish an improvement in the installed Off run.
Captures and failed regression attempts are ignored under `local/p2/fix-behavior-context-object/`.
Fire checker self-test passes; installed C++ Fire check: 33 of 33 rules hold; crate edges: 19 tests OK.
No host, visual, GPU or running-game validation; decision 2026-11-18-01.
