# BL2_ReBased roadmap

The live tracker. Phases, steps and gates come from
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md); this file
records where each step actually stands, with the verification record that
backs it. Checkbox states are conservative: a step is checked only when its
verification is written down.

Legend: <img src=".github/assets/icons/done.svg" width="18" align="absmiddle" alt=""> done and verified · <img src=".github/assets/icons/now.svg" width="18" align="absmiddle" alt=""> in progress · <img src=".github/assets/icons/todo.svg" width="18" align="absmiddle" alt=""> not started.
Items marked *Caveat:* are done with a recorded limitation.

**Phase 0 gate:** 2026-09-10 · **Now:** Phase 0R (Rust core; engine route picked) · **Last update:** 2026-11-25

**2026-11-14: the host engine changes.** The maintainer reversed the Unreal Engine 5 host choice
([decision](docs/decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md)). New runtime code is Rust; Python stays for tools and analysis. The UE5 host
(`host/ue5/`, its scripts and suites) left the tree; main 08bcf3e is the last commit that holds it, and
`git show 08bcf3e:<path>` recovers any of it. The C++ core in `src/` is being ported to Rust crate by crate and stays the reference until each crate matches it: the
container decoder and the package reader are ported
([decisions](docs/decisions/2026-11/2026-11-15-01-rust-container-decoder-own-lzo1x-full-and-partial-forms.md),
[2026-11-15-03](docs/decisions/2026-11/2026-11-15-03-rust-package-reader-contract-as-shipped.md)), the rest is in
progress. The engine route is picked: on 2026-11-15 the maintainer chose an own engine
on wgpu + winit over Bevy from the Phase 0R benchmark ([decision](docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md)); `ow-host` walks
Sanctuary and loads it as a world (see How it's going). Host results below are the retired UE5 host's: they say what that host did and no longer count
toward a gate. Host-independent evidence (real-game captures, native notes, script disassembly, reader checks, the
core's VM, combat, mission and item results) keeps its status. Nothing about the Rust route is verified in the running game yet.

---

## Priority: the vertical slice

Porting all 82 maps is the *easier* part of this project; the harder, unproven
part is Phases 2-4 (script VM, stock UE3 natives, Gearbox's undocumented
natives). Rather than finish map breadth first and find out later that a later
phase doesn't hold, the phases below are scoped to prove the hard parts on one
map and one character first. Adopted 2026-09-18 from outside feedback; see
[DECISIONS.md](DECISIONS.md).

Current slice target: **Sanctuary** (closest map to done) + **Maya** (chosen
2026-09-18) + one hand-picked mission + a handful of guns, working
end-to-end: spawn, fight, loot a gun, equip it, use her action skill
(Phaselock), complete the mission, die, respawn. That's Phase 4's gate below.
Broad map coverage (79 more maps) and the remaining Vault Hunters are
deferred until that gate is met; they move to Phase 5/6.

The slice priority is unchanged by the switch to Rust (2026-11-14); the order
in front of it is: port the C++ core to Rust to parity (Phase 0R; the engine
benchmark is done and the maintainer picked an own engine on wgpu + winit),
then build the slice again in the Rust host, Sanctuary first and Maya next, reusing the
host-independent rules and evidence already recorded.

---

## How it's going

*History. Everything below that says "the host" or "in the host" describes the
UE5 host, retired on 2026-11-14 ([decision](docs/decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md)); it is
kept as a record of what that host did and as a target, not as a description of
a host that exists. The Rust route's status is the first block below.*

The project is worked on actively. There are no completion dates, no time
estimates and no completion percentages here; this section only records what
the repository shows as of 2026-11-13 (`origin/main` at ccc6fa5; the elapsed counts, dated milestones and measured numbers
below stop at 2026-11-02), and each line names the record that
backs it ([DECISIONS.md](DECISIONS.md) entry date or verification file). Where
a number is an automated check it says so; "passes" is never parity with the
original game.

**As of 2026-11-25, the Rust route** (automated checks and host runs; none of it is parity, and every rule is
UNVERIFIED in the running game)

- **The reader:** the Rust container decoder and package reader are ported
  ([container](docs/decisions/2026-11/2026-11-15-01-rust-container-decoder-own-lzo1x-full-and-partial-forms.md),
  [reader](docs/decisions/2026-11/2026-11-15-03-rust-package-reader-contract-as-shipped.md)). In lane
  FIX-FIRE-TO-HIT, on main 86c28d00 (pc-hummer, 2026-11-19), `tools/verify_packages.py` read the 9 code packages with 3,339 of 3,339 class bodies exact, with the
  C++ reader and with the Rust reader ([gates](docs/verification/GATES.md), row FIX-FIRE-TO-HIT). The C++ code in
  `src/` is still the reference; none of it has retired yet.
- **The crates:** the 22 crates of [crates/README.md](crates/README.md) hold the ported reader, object model,
  definition store, script VM and the game rules ported so far. Rules are ported one native at a time in
  `ParityMode::Off`, each from an own-words note, and answer a typed `NotYet` where a rule is not read yet. The
  workspace test counts are in [gates](docs/verification/GATES.md) (the top rows). Float-oracle tests whose bit
  parity is gated on Windows (rule N1 in `crates/ow-core/src/num.rs`) are not expected to pass on Linux.
- **The engine:** our own engine on wgpu + winit, picked by the maintainer on 2026-11-15
  ([decision](docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md)). wgpu 30.0.1 and
  winit 0.30.13 are the only direct third-party dependencies, used by `ow-host` alone, with their transitive crates
  (approved 2026-11-16,
  [THIRD_PARTY.md](THIRD_PARTY.md), "Engine crates: wgpu and winit").
- **The host:** `ow-host` walks Sanctuary in a window from the installed data (H4-PLAY, 2026-11-16) and fires a gun
  read from a weapon file (H4-WEAPON). It also loads `Sanctuary_P` as a world in Off (H-WORLD, 2026-11-19): 2,363
  actors, and it still halts at tick 1, since H-MATINEE (2026-11-19) on the level-load event that starts a Matinee
  op binding a `WillowSpectatorPoint`; the window drew 1 of 11 pawns (H-WORLD baseline)
  ([gates](docs/verification/GATES.md), the three host sections).
- **The slice gate**, headless (`ow-package --slice-gate`, installed data, Off, strict; the hub's run on main
  d3c7a13, pc-hummer, 2026-11-25): 4 links ok, 10 NotYet, 0 errors. The mean per-link share of slice steps passed is
  37.7%. Links 2, 10, 11 and 12 pass; each of the others stops at a rule the port has not read or built yet
  ([gates](docs/verification/GATES.md), row SLICE-GATE-READING; per link in
  [SLICE_RUN_PLAN.md](docs/rust-port/SLICE_RUN_PLAN.md), section 2). `ow-host --play` does not count slice links
  yet, and the gate counts only there ([plan](docs/rust-port/SLICE_RUN_PLAN.md), section 1), so the Phase 4 gate is
  not met.

**As of 2026-11-13, in the retired UE5 host** ("in the host" means the UE5 build with labelled stand-ins; a passing suite says the host did this, not parity)

- The slice loop ran in the host in one session (`tools/test_slice_gate.ps1`, removed 2026-11-14): the forced variants passed 14 of 14 links in
  every recorded run since 7eee9ee (2026-11-07), the later runs as the hub's log reported them ([slice gate](docs/verification/SLICE_GATE.md));
  the `-Shield` run equipped one generated level-8 card from game data, 14 of 14 on main 25a5ce2 (the card is UNVERIFIED in game)
  ([gates](docs/verification/GATES.md)).
- The unforced variant was not reliable: 3 of its first 8 runs (2026-11-11) failed a link ([slice gate](docs/verification/SLICE_GATE.md),
  "The unforced variant"). The Phase 4 gate is not claimed as met.
- Fight: range enemies with a host brain; the adult's and the regular's swing cycles and the adult's stand distance were graded in the
  engine against live reads ([range](docs/verification/COMBAT_RANGE.md)). Leaps and thrown rocks, the biggest gap, are described in an own-words
  note, UNVERIFIED in game, and not built ([note](docs/verification/NATIVE_AI_LEAP_THROW.md)).
- Down and respawn: the host's respawn wait on the station path was split into the moments seen in six live respawns (2026-11-12, two
  maps, one player level) ([down and respawn](docs/verification/COMBAT_DOWN_RESPAWN.md), "What the host does").
- Loot, equip, Phaselock and the Fire mission ran end to end in the gate ([slice gate](docs/verification/SLICE_GATE.md)); a critic agent scored
  Phaselock's captures 5.1 to 5.7 of 10, not parity ([Phaselock](docs/verification/PHASELOCK_STOCK_DATA.md)).
- Sanctuary's ambient citizens wore their captured pieces (ambient suite PASS on main a3daacf, 35 pawns, 89 worn pieces); their
  movement was a host stand-in, UNVERIFIED ([gates](docs/verification/GATES.md), "Ambient gate").
- Tests: ctest had 174 entries after three real-game tool tests and a registration guard joined it (2026-11-13); CI passed 174 of
  174 on main 81d87f9, LZO on; removing the UE5 host's tests changes this count, and the current one is in
  [gates](docs/verification/GATES.md).
- Evidence: two unattended capture sessions on 2026-11-12 (9 item runs, 0 game crashes; [real-game reads](docs/verification/REALGAME_GROUND_TRUTH.md)),
  among them downs on Three Horns Divide and in Sanctuary and a den capture, recorded in own words
  ([down and respawn](docs/verification/COMBAT_DOWN_RESPAWN.md), [range](docs/verification/COMBAT_RANGE.md)).
- Not started, by plan: the other 79 maps and the other Vault Hunters (after the Phase 4 gate).

**Elapsed.** First commit 2026-09-10, so 53 calendar days. 401 commits on
`origin/main` (`git rev-list --count`, which includes the commits of merged
branches) across 28 days with at least one commit (author dates; the 98 commits
merged as #59 carry their 2026-10-05 to 2026-10-09 dates). Built with heavy AI
assistance (see the README). The first commit imported earlier local
prototypes, so the repository's clock started a little after the work did.

**Dated milestones**

- **2026-09-10:** package reader reads 2,008 / 2,008 packages; Phase 0 gated
  (UE5 chosen as host; one mesh and one texture render in it).
- **2026-09-13:** Ash and Sanctuary load as frozen scenes in UE5 with a
  free-flight camera (Southpaw Factory follows on 2026-09-14).
- **2026-09-14:** placeholder walking with collision on Sanctuary.
- **2026-09-15:** Sanctuary terrain and BSP floors imported with walkable
  collision.
- **2026-09-23 to 2026-09-25:** Maya imported with animated first-person arms;
  Phaselock cast in the host; recipe-driven weapon inventory screen.
- **2026-09-30:** read-only script bytecode disassembler, then a C++ bytecode
  loader, VM runtime and Core natives (prototype).
- **2026-10-01:** the VM drives backpack navigation in UE5; one Sanctuary door
  opens from installed Matinee/Kismet data; the stock Fire mission plays end to
  end in the host with labelled stand-ins; weapon balances, loot pools, world
  placement and Phaselock data decoded for the slice. Policy change: analysing
  `Borderlands2.exe` locally is allowed, output stays out of the repository
  (DECISIONS 2026-10-01).
- **2026-10-02:** Ghidra tooling and the first native-analysis notes (mission,
  behavior and Kismet dispatch, progression, Phaselock targeting); the executors
  in `src/behavior.*`, `src/kismet.*` and `src/mission.*` now follow the
  dispatch notes; weapon paint passes 1 to 3; a `ParticleSystem` template
  reader; inventory open time measured and reduced. First real-game capture
  session (the installed game driven through the community mod SDK): the level
  curve, maximum health, skill points, the Fire mission's XP and the Phaselock
  bob and cast gates confirmed in game; 69 golden weapon cards with their exact
  parts ([record](docs/verification/REALGAME_GROUND_TRUTH.md)).
- **2026-10-03 to 2026-10-22, Phaselock rounds 2-20** (#40, #41, #45, #50):
  presentation compared with matched real-game frames; the bubble size rule read
  from the game's emitters in the running game (DECISIONS 2026-10-03); from round
  16 a stock bullymong is the lifted target with its own PhaseLock clips and stock
  blends; rounds 17-20 corrected the capture FOV and pitch. An independent critic
  agent's scores stayed between 4.5 and 6.3/10 (rounds 17-20: 5.4-5.9; the 8/10
  target was not reached)
  ([record](docs/verification/PHASELOCK_STOCK_DATA.md); DECISIONS 2026-10-03 to
  2026-10-05, 2026-10-21, 2026-10-22).
- **2026-10-03 to 2026-10-22, guns parity** (#40, #41, #48): weapon card rules
  and the golden card compare (69/69, automated; DECISIONS 2026-10-03); parts and
  card stats read from the running game; first-person placement from the weapon
  type's view offset; four rounds on four slice guns (hold-to-aim, reload clips,
  muzzle flash, casings). Independent critic agent 5.8, 5.5, 6.3, 6.5/10 over the
  last four rounds (a judgement against real frames, not parity), 8/10 not
  reached ([record](docs/verification/WEAPON_VISUALS.md); DECISIONS 2026-10-04,
  2026-10-05, 2026-10-22).
- **2026-10-04 to 2026-10-09, native notes** (#41, #59): about thirty own-words
  notes written from Ghidra readings (Phaselock presentation, weapon visuals and
  firing, the mission script bridge, dialog, skills, use key, save/load,
  attributes, hit pipeline, death, loot, AI targeting, melee and awareness, the
  down state), all `UNVERIFIED` unless a line says how it was confirmed
  ([index](docs/verification/NATIVE_INDEX.md); DECISIONS 2026-10-04 to 2026-10-09).
- **2026-10-05 to 2026-10-09, merged 2026-11-02 as #59** (record per step below):
  script swaps 1-11 put Fire mission pieces on the script VM (accept and turn-in,
  experience, objectives, Marcus's use chain and dialog choice, level-up refill
  and cooldown reset); real-game lanes L1-L9 recorded the level-up, the Fire
  mission's event order, a bullymong fight, kill XP, projectiles, and bullymong
  targeting, melee and awareness; and the combat steps S-a to S-g run on the VM
  through `ow-package --combat-run` (not in the UE host): pawn health, the hit
  chain, the death chain, kill XP, a bullymong's melee blow as a fixture without
  AI, Maya's down state with bleed-out, second wind and respawn, and loot on death
  as item records. `tools/verify_combat_pawn.py --vm` 97 PASS / 0 FAIL / 4
  MISSING / 1 OPEN (automated; "the VM reproduces the live row", the native
  bodies stay `UNVERIFIED`)
  ([down/respawn](docs/verification/COMBAT_DOWN_RESPAWN.md),
  [loot](docs/verification/COMBAT_LOOT.md),
  [awareness live](docs/verification/AI_AWARENESS_LIVE.md); DECISIONS 2026-10-05
  to 2026-10-09 and 2026-11-02).
- **2026-10-07 and 2026-10-23, REA native analysis** (#55): REA installed as an
  analysis front end over Ghidra (DECISIONS 2026-10-07); why its first query timed
  out on this executable, and the one-session warm-up recipe (about 20 minutes)
  ([NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md), "Using REA on Borderlands2.exe").
- **2026-10-21 and 2026-10-22, inventory and Skills rounds 16-22** (#46, #49): the
  movie's own 3D turn, selectable empty backpack cells, card content from the
  parts, Skills keyboard navigation against matched real-game references. An independent
  critic agent's scores (judgement, not parity): 7.27 before round 16, 7.67 and 7.57 at rounds 18 and 19, 7.85 at round 21
  (kept; round 22 scored lower and was reverted); 8/10 not reached
  ([record](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md),
  [cards](docs/verification/INVENTORY_CARD_STATS.md); DECISIONS 2026-10-21,
  2026-10-22).
- **2026-10-21 and 2026-10-22, Sanctuary parity rounds 1-4** (#43, #44, #51):
  inspected material fallbacks and component owners, blocking volumes and
  sphere/capsule collision, recorded walking routes, then four matched-viewpoint
  rounds (source lighting on Sanctuary only, decal default-deny, host
  calibrations opt-in). Independent critic agent (not parity) 4.5/10 before
  round 1, 5.0/10 for the default state after review; 8/10 not reached
  ([record](docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md),
  [walking](COLLISION_WALKING_VERIFICATION.md); DECISIONS 2026-10-21, 2026-10-22).
- **2026-10-22, the HUD** (#47, #54, #53): Maya's action-skill indicator on BL2's
  own HUD movie with its rules read from the live movie; the HUD movie is the
  default gameplay HUD, with the turned clusters drawn through a perspective warp;
  the minimap drives BL2's own minimap clip from rules read in the live game.
  Independent critic agent (not parity): 9/10 (action skill), 7.7/10 (HUD),
  8.2/10 (minimap)
  ([action skill](docs/verification/ACTION_SKILL_HUD.md),
  [HUD](docs/verification/HUD_PARITY.md),
  [minimap](docs/verification/HUD_MINIMAP.md); DECISIONS 2026-10-22).
- **2026-10-23, weapon parts catalog, loot generator, part library and firing
  from the parts** (#57, #58, #56): a census of every weapon balance and part; the
  loot generator ported to C++; every gestalt mesh and 300 of 301 skins imported
  into UE once (the BuzzAxe melee material is unsupported), guns assembled from their parts at run time; what a shot does read from the
  gun's parts (firing modes, light bullets, rockets, splash, status effects, the
  Tediore throw) ([catalog](docs/verification/WEAPON_PARTS_CATALOG.md),
  [part library](docs/verification/WEAPON_PART_ASSETS.md),
  [firing](docs/verification/WEAPON_PROJECTILES.md); DECISIONS 2026-10-23).

**Measured** (automated checks and timings; one PC unless stated)

- Package reader: 2,008 / 2,008 packages, 4,751,329 serialized exports
  (Phase 0 entries, 2026-09-10). Scripts: 12,968 of 12,978 functions decode
  structurally ([record](docs/verification/SCRIPT_BYTECODE_DISASM.md)). Both are
  structural agreement, not runtime compatibility.
- Suites on PR #59 (DECISIONS 2026-11-02): quest suite first run 111/111 and
  resume 11/11 at 5be3581 and again at a8a3dd5 (the tree merged as 683a083);
  CTest 19/19, packages 9/9 and the UE5 module build at 5be3581.
  `tools/test_weapon_firing.ps1` was not run (the worktree had no generated
  firing recipes). Automated host checks only, not parity.
- Weapon generator (DECISIONS 2026-10-23,
  [record](docs/verification/WEAPON_PARTS_CATALOG.md)): `ow-weapongen` reproduces
  the Python roll and evaluation in 2,919,834 fields over 29,880 balance rolls,
  7,680 pool rolls and 6,795 pool guns with 0 mismatches (base game only; DLC not
  yet). Golden cards 69 / 69 / 68 of 69. Automated.
- Part library (DECISIONS 2026-10-23,
  [record](docs/verification/WEAPON_PART_ASSETS.md)): 300 of 301 skins imported;
  the oracle passes 210/210 sample guns on fragments and triangles. Independent
  critic agent (not parity) 7.32/10
  Inspect, 5.36/10 first person on 39 matched guns.
- HUD (DECISIONS 2026-10-22): HUD shots 17/17, action-skill suite 19/19, minimap
  in-engine 15/15 (host suites, not parity).
- Shot effects (DECISIONS 2026-10-23,
  [record](docs/verification/WEAPON_PROJECTILES.md)): an independent critic agent's
  matched-frame scores (judgement, not parity) 2.1,
  3.9, 4.2/10 over three rounds, stalled; host shot effects are additive stand-ins,
  not BL2's particle systems.
- Phaselock round 16 (DECISIONS 2026-10-21): a stock bullymong as the lifted target (its four PhaseLock clips, stock
  blends, the hold's mesh lowering observed live); CTest 10/10, packages 9/9, quest suite 79/0 and 11/0. Critic overall
  5.8 / 5.7 / 5.5 over three rounds (target 4.5); an agent's judgement against real frames, not parity.
- Inventory and Skills rounds 16-19 (DECISIONS 2026-10-21): the pages apply the movie's own 3D turn, empty backpack cells are
  selectable, Inspect and Skills details follow fresh real-game captures; independent critic agent (not parity) 7.27 before, 7.57-7.67 after (8/10 not reached).
  Inventory suite 49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE (bounded item set), navigation 30/30, CTest 10/10, packages 9/9.
- Suites after the Phaselock, ambient-citizen, gun and menu rounds (DECISIONS 2026-10-05): CTest 10/10,
  packages 9/9, quest suite 79/79 first run and 11/11 resume, door suite 16/16, inventory suite 49 PASS /
  0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE, navigation 27/27. Automated checks only; the visual state is in the
  critic records (blind A/B between rounds), not a parity claim.
- Suites after the dispatch-rule change (DECISIONS 2026-10-02): CTest 10/10,
  packages 9/9, quest suite 73/73 first run and 10/10 resume, door suite
  16/16, inventory suite 45 PASS / 0 FAIL / 2 NOT_RUN / 2 KNOWN_DIVERGENCE
  (sort order) on a regenerated worktree with a short backpack; 47 PASS / 2
  KNOWN_DIVERGENCE on the fully seeded one (DECISIONS 2026-10-01).
- Inventory open time (DECISIONS 2026-10-02,
  [record](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md)): first open with the
  page loaded 443 ms (358-488, 4 launches) before, 234 ms (213-252, 3 launches)
  after moving one-time VM and mesh work to level start; repeat opens about 60
  ms. An open pressed in the first ~5 s of play still takes about 5.5 s (the
  page's movie player booting). The original game: ~126-156 ms (below).
- Native registration (DECISIONS 2026-10-02, [notes](docs/NATIVE_ANALYSIS.md)):
  the executable's tables hold 6,877 natives in 770 tables and all 199
  numbered script natives resolve by name. A census over 133 missions puts all
  3,420 behavior links in the predicted id ranges (structural only).
- Weapon paint (DECISIONS 2026-10-01 and 2026-10-02): static parameters decode
  exactly in 631 of 631 `Startup.upk` MICs; `tests/weapon_paint_test.py` 29
  passed on invented values. An independent critic agent comparing host stills
  with wiki in-game screenshots scored the Maliwan uncommon pistol 6.5/10 and the
  Jakobs common pistol 5.0/10; that is an agent's judgement, not a real-game
  comparison.
- Particle templates ([record](docs/verification/PHASELOCK_STOCK_DATA.md),
  "Particle template reader"): `tests/particle_system_test.py` 16 passed; all
  17,506 non-empty baked distribution tables in four packages fit the assumed
  layout. Reader and layout only: nothing is rendered from it yet.
- Real game (2026-10-02, [record](docs/verification/REALGAME_GROUND_TRUTH.md)):
  the native level curve matches the host's single-precision formula at levels
  1-59; max health base `80 × 1.13^L` at levels 2, 8, 17 and 70; Fire mission XP
  395 at stage 8; the Phaselock bob rule reproduces a lifted enemy's height to
  0.001 units RMS over 435 frames. Weapon cards: the host evaluator on the exact
  rolled parts matches 52 of 69 cards in every printed stat. Inventory open in
  the original game: ~126-156 ms to the first page frame. Paint: an independent
  critic agent scored the host guns 3-4.5/10 against real captures.
- Real game, 2026-10-03 to 2026-10-23 (bl-sdk sessions with saves blocked; the
  record of each is named): a level-up refills health to the new maximum and a
  natural level-up adds a 30 s weapon-damage scale (lane L1, DECISIONS 2026-10-07);
  the Fire mission's event order recorded and compared with the VM, 38 events
  matching, 4 out of order, 5 missing, 4 extra before script swap 8 followed it
  ([record](docs/verification/FIRE_MISSION_GOLDEN_TRACE.md)); a bullymong fight,
  kill XP and the Defense Boost factor (lanes L3, L5b, L6,
  [record](docs/verification/COMBAT_GOLDEN_TRACE.md)); a bullymong's target
  priority formula on 134 samples to 5e-8 (lane L8,
  [record](docs/verification/AI_MELEE_TARGETING_LIVE.md)); the den-based notice
  rules (sight, and proximity at 768) and the threat level over time (lane L9,
  [record](docs/verification/AI_AWARENESS_LIVE.md)); light bullet flight, air
  friction, rocket speed, per-pellet damage on 4 of 5 guns, splash with and
  without falloff, the status-effect damage clock and the Tediore throw
  ([record](docs/verification/WEAPON_PROJECTILES.md), "confirmed live" rows);
  40/40 spawned generated guns carry the generated balance, parts, level, name and
  value, and 39/39 spawned guns show the fragments and skin the part library
  predicts (DECISIONS 2026-10-23); the action-skill HUD and minimap rules read
  from the live movie (DECISIONS 2026-10-22).

**UNVERIFIED** (read or fitted, not confirmed by running the original game)

- Every native rule in the native notes not listed as confirmed above, and the
  executor and VM behaviour built on them: synthetic tests show the code follows
  the notes, not that the notes are right
  ([dispatch](docs/verification/NATIVE_MISSION_DISPATCH.md),
  [progression](docs/verification/NATIVE_PROGRESSION.md),
  [targeting](docs/verification/NATIVE_PHASELOCK_TARGETING.md),
  [index](docs/verification/NATIVE_INDEX.md)). The Phaselock targeting,
  constraint and presentation changes that were blocked on 2026-10-02 have since
  passed the quest suite and the Phaselock shots (DECISIONS 2026-10-21,
  2026-10-22); the targeting rules themselves are still not compared with the
  game.
- The VM combat steps S-a to S-g reproduce recorded live rows; the bodies of the
  natives they bind are role-level readings. The respawn timers on the station
  path are confirmed live (2026-11-12, six samples); the station choice, the
  path without a station and the live 3-item money drop stay `OPEN`, and the
  shield numbers are stand-ins
  ([down/respawn](docs/verification/COMBAT_DOWN_RESPAWN.md),
  [loot](docs/verification/COMBAT_LOOT.md)).
- Sanctuary's host lighting calibrations (sun, sky light, exposure, white
  balance, cloud cap) are screenshot fits, not decoded
  ([record](docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md)).
- The whole weapon paint and decal reading, its display scale and shading
  inputs (`USE_SHADER_SHADING` stays off until the scene lighting is
  calibrated); environment reflection, emissive and in-game lighting are not
  modelled.
- The slice host stand-ins: kickoff played right after acceptance, dialog
  outputs selected together, a save-state fixture for the dependency mission, a
  lent mission weapon ([route record](docs/verification/SANCTUARY_RPG_MISSION.md)).

**Blocked or open**

- Captures of the original game now exist for the Fire mission's event order
  (lane L2), a bullymong fight on another map (lanes L3-L9), Phaselock frames and
  the menus and HUD; none covers the whole slice loop in Sanctuary yet.
- In the retired UE5 host the range's enemies noticed, chased and hit Maya with a host brain (a stand-in, with live-read
  notice radii, the swing cycles of the adult, baby, regular and slinger, and the adult's and baby's stand distances), Maya went down, bled out and respawned at a station, and a
  range kill rolled its loot at the stock chance ([range](docs/verification/COMBAT_RANGE.md),
  [down and respawn](docs/verification/COMBAT_DOWN_RESPAWN.md), [slice gate](docs/verification/SLICE_GATE.md)). The VM
  (`--combat-run`) runs the hit, death, kill-XP, down and loot steps; its melee blow is a guaranteed-hit fixture
  ([record](docs/verification/COMBAT_MELEE.md)). That host's enemy brain was not the game's AI: leaps and thrown
  rocks are described in a note ([note](docs/verification/NATIVE_AI_LEAP_THROW.md), UNVERIFIED) and not built.
- Runtime flight timing of projectiles has no in-engine check since the #59 merge
  (DECISIONS 2026-11-02), and the spread distribution and status proc rate are
  not measured ([record](docs/verification/WEAPON_PROJECTILES.md)).
- Sanctuary visual parity, the stripped sky graph, native material graphs and
  the 79 untouched maps (see Now / next and Phase 1).
- Slice audio played in the retired UE5 host from the player's own install: decoded with vgmstream (outside the
  repository; maintainer decision of 2026-11-07) into ignored `local/`, then played through UE's procedural
  sound wave, for fire, hit, the Phaselock cast, equip, reload, doors and the mission's dialog lines; the
  pickup had no sound. A recording of a host run (2026-11-11) holds the played files; nobody has confirmed
  by ear that a sound is right (UNVERIFIED), and this is not parity
  ([record](docs/verification/SLICE_AUDIO_CHAIN.md), sections 3, 5 and 7a).
- Two quest-suite first runs on 2026-10-02 ended silently after check 67; the
  cause is unproven (a mixed build state is likely: a clean rebuild passed and
  exited cleanly)
  ([record](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md), "Suites").

No phase after Phase 0 has passed its gate.

---

## Now / next

The Rust route, in the order it is being taken (2026-11-14; status updated
2026-11-25). Each status line names its record. A ticked box is an automated
check or a decision, not parity, and nothing here is verified in the running game.

- [x] Port `ow-container` (LZO1X and the chunk container) to Rust; the same
      synthetic fixtures as the C++ tests give the same results
      ([decision](docs/decisions/2026-11/2026-11-15-01-rust-container-decoder-own-lzo1x-full-and-partial-forms.md)).
- [x] Port `ow-package` (header, name, import and export tables, tagged
      properties), reusing the offsets the C++ reader already recovered and
      checked; nothing about 832/46 is re-derived
      ([decision](docs/decisions/2026-11/2026-11-15-03-rust-package-reader-contract-as-shipped.md)).
- [x] `tools/verify_packages.py --reader <the Rust reader>` gives the same
      counts and exact class bodies as the C++ reader: 9 packages, 3,339 of
      3,339 class bodies exact with both readers (lane FIX-FIRE-TO-HIT on main
      86c28d00, pc-hummer, 2026-11-19; [gates](docs/verification/GATES.md)). Only then does the
      matching part of `src/` retire. *Status:* nothing in `src/` has retired yet.
- [x] Engine benchmark: Bevy and an own engine on wgpu + winit, the same
      small scene in each, prototypes under ignored `local/bench/`
      ([record](docs/verification/ENGINE_BENCHMARK.md)). *Caveat:* one PC per
      candidate with different draw strategies, so the frame times are not
      like for like; Fyrox was not built; Ruffle was embedded in the wgpu
      prototype only.
- [x] The maintainer picks the engine route from the measurements: an own
      engine on wgpu + winit (2026-11-15, [decision](docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md)).
- [ ] `THIRD_PARTY.md` entries for the engine crates (wgpu, winit, glam,
      bytemuck, pollster, rapier3d, the Ruffle crates from git, and a JDK to
      build Ruffle), approved by the maintainer before any of them lands,
      when the host work needs them. *Status:* wgpu and winit have entries
      (approved 2026-11-16, [THIRD_PARTY.md](THIRD_PARTY.md)); none of the
      others has one.
- [ ] Port the script VM and the game rules (combat, missions, items,
      weapons) crate by crate, each against the C++ results. *Status:* in
      progress. The VM runs parts of the slice's script headless, most of it in
      Reference only; in Off each link stops early
      ([SLICE_RUN_PLAN.md](docs/rust-port/SLICE_RUN_PLAN.md), section 2). The
      rules are ported native by native in Off. The slice gate reads 4 ok / 10 NotYet /
      0 errors (2026-11-25; [gates](docs/verification/GATES.md), row
      SLICE-GATE-READING).
- [ ] Rebuild Sanctuary in the Rust host (Phase 1), then Maya (Phases 3-4),
      with the retired UE5 host's records as the behaviour target. *Status:*
      started. The host walks Sanctuary in a window and loads it as a world,
      which halts at tick 1 on a Matinee op's level-load event (H-MATINEE) ([gates](docs/verification/GATES.md), the host
      sections); no slice link is counted in `ow-host --play` yet.

The UE5-era list (sky, materials, terrain and BSP, visual defects, walking
collision, performance, matched screenshots) is in
`git show 08bcf3e:ROADMAP.md`. Its data-side findings stay in the records it
linked; its host-side results were the UE5 host's.

---

## Phase 0: Foundation and spikes <img src=".github/assets/icons/done.svg" width="22" align="absmiddle" alt="">

*Gated 2026-09-10; most of the reader already existed as a Python prototype.*

Goal: a repo, a build, a package loader, and a decided host engine.

- [x] Repo, README, `CONTRIBUTING.md`, issue tracker
- [x] License chosen: MIT (2026-09-13; reasoning in [DECISIONS.md](DECISIONS.md))
- [x] Dev environment: Visual Studio 2022, CMake, Python, Git
- [x] Package loader ported to C++ from `research/native_count.py`: LZO1X via
      vendored lzokay (MIT), chunk container, partial-compression chunk
      tables, name/import/export tables. Verified byte-for-byte against the
      Python reader on all nine code packages.
- [x] Census over every package in the install: 2,008 / 2,008 packages read,
      4,751,329 serialized exports; two UHD sidecar files identified and
      skipped. *Caveat:* Counts serialized copies, not unique assets; umodel
      cross-check pending.
- [x] Tagged-property reader: scalars, object/class/component refs, fixed
      structs (`Vector`, `Rotator`, `Guid`, `LinearColor`, `Color`, `Quat`,
      `Vector2D`), nested structs, arrays via explicit element-type schema.
      *Caveat:* Array element types are not serialized in 832/46; reflection-driven
      typing is future work. *Caveat:* The property-start offset must be supplied;
      the observed 4-byte prefix is `UNVERIFIED` as a rule.
- [x] Property dump of a real `WeaponPartDefinition`
      (`AR_Barrel_Jakobs_Sawbar`) compared against BLCMM: all 13 tags match,
      zero trailing bytes in three installed copies. *Caveat:* One
      generated-subobject presentation discrepancy recorded in
      [DECISIONS.md](DECISIONS.md).
- [x] Texture spike: `Texture2D` to PNG, DXT1/DXT5, inline, TFC-streamed and
      LZO-compressed bulk, every resident mip. Viewed; it is the texture.
- [x] Static mesh spike: all render LODs, 16/32-bit indices, all UV sets; one
      LOD to OBJ.
- [x] Host engine decided: **Unreal Engine 5**. Hello-world project builds;
      the probe mesh and its verified diffuse texture render in UE 5.8.2.
      First screenshot user-verified. *Caveat:* reversed on 2026-11-14; the
      engine decision is Phase 0R's now.

**Gate passed.** The census exists; one mesh and one texture from BL2 render in
the host engine. Records: [DECISIONS.md](DECISIONS.md) entries dated
2026-09-10.

---

## Phase 0R: Rust core and engine benchmark <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*Added 2026-11-14 with the switch to Rust ([decision](docs/decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md)).*

Goal: the engine-agnostic core in Rust, matching the C++ core, and a host
engine route picked from measurements.

- [x] Cargo workspace at the repository root with skeleton crates (22 of
      them since the scaffold; the list and their edges are in
      [crates/README.md](crates/README.md)) and no third-party crates.
      *Caveat:* skeletons at the time; the crates have filled since (Now / next),
      and wgpu and winit joined `ow-host` on 2026-11-16.
- [x] `ow-container` ported; same results as the C++ code on the same
      synthetic fixtures ([decision](docs/decisions/2026-11/2026-11-15-01-rust-container-decoder-own-lzo1x-full-and-partial-forms.md))
- [x] `ow-package` ported; same results on the same synthetic fixtures
      ([decision](docs/decisions/2026-11/2026-11-15-03-rust-package-reader-contract-as-shipped.md))
- [x] `tools/verify_packages.py --reader` on the Rust reader: same counts and
      exact class bodies as the C++ reader (3,339 of 3,339; lane FIX-FIRE-TO-HIT
      on main 86c28d00, pc-hummer, 2026-11-19; [gates](docs/verification/GATES.md))
- [x] Engine benchmark run (Bevy, own engine on wgpu + winit; Fyrox, the
      optional one, not built) and recorded in
      [ENGINE_BENCHMARK.md](docs/verification/ENGINE_BENCHMARK.md). *Caveat:*
      one PC per candidate, so frame times are not like for like.
- [x] Engine route picked by the maintainer and recorded as a decision: own
      engine on wgpu + winit, 2026-11-15 ([decision](docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md))

**Gate:** the Rust reader matches `verify_packages.py`, the benchmark is
recorded, and the maintainer has picked the route.

*Whether this gate is passed is the maintainer's call; not marked.*

---

## Phase 0.5: External extraction support for the vertical slice (in progress) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*This supports the Sanctuary/Maya pipeline proof of concept and can speed up
asset preparation; it does not replace the runtime, gameplay or parity
phases.*

Goal: establish whether a mature community exporter can provide repeatable local
visual payloads for the Sanctuary/Maya vertical-slice importer without
replacing OpenWillow's metadata, reference and verification responsibilities.

- [x] Acquire UModel / UE Viewer outside the repository. The current local
      candidate is build 1590 from the upstream `gildor2/UEViewer` checkout;
      source commit, binary hash and provenance are recorded in the
      [external-tool benchmark](docs/verification/EXTERNAL_TOOL_BENCHMARK.md).
- [x] Confirm Borderlands 2 detection and package listing: 920 files scanned,
      `Ash_P.upk` recognized as version `832/46` with 21,834 exports.
- [x] Export representative static and skeletal meshes plus a TFC-streamed
      texture: `Ash_Road01` produced glTF and its binary buffer in 0.1 seconds,
      `MetalRoadConcrete_Dif` produced a 1024x1024 DDS in 0.09 seconds, and
      `Skel_BugMorph` produced glTF in 0.08 seconds. UModel warnings and output
      inventories are recorded; host import and visual parity are not yet
      verified.
- [ ] Export representative animations, materials and sounds.
- [ ] Run a bounded multi-package batch and inventory successes, failures,
      unsupported types, duplicates, warnings, output bytes and elapsed time.
- [ ] Compare the external inventory with the `ow-package` census and prepared
      scene manifests.
- [ ] Import representative outputs into the Rust host (after Phase 0R) and
      choose the most reproducible adapter format (glTF, PSK/PSA or another proven path).
- [ ] Decide whether UModel becomes the default visual payload backend. Keep
      `ow-package` authoritative for package identity, object paths, placement
      metadata and verification regardless of the outcome.

**Gate:** representative exports are repeatable, all failures and unsupported
categories are enumerated, at least one output path reaches the host, and the
external inventory can be related back to package/object identity. Until then,
the full-install export remains an unmeasured experiment.

Record: [external-tool benchmark](docs/verification/EXTERNAL_TOOL_BENCHMARK.md)
and [tooling reference](docs/TOOLING.md#external-tools).

---

## Phase 1: World viewer (M1) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

*Started 2026-09-10 in the UE5 host; the host side went back to not started on
2026-11-14 when that host was retired. What carried over: the package reader,
the importer APIs and the decoding results (meshes, textures, collision hulls,
placement, terrain and BSP data, material parameters), which are core code.
What did not: rendering, lighting, camera, walking and the in-game selector,
which were the UE5 host's. Those items are unticked and marked "retired UE5
host" until the Rust host does them; their records stay as history.*

Goal: walk around any BL2 map in a modern 64-bit renderer. Ship publicly.

- [x] Reusable importer APIs (`PackageStore` with lazy indexing and
      cross-package import resolution; texture and mesh importers behind C++
      APIs instead of CLI-only spikes)
- [ ] Static mesh importer for all meshes
  - [ ] External visual-payload adapter after the Phase 0.5 gate
  - [x] All render LODs, all UV sets, section material references
  - [x] Collision hulls from `RB_BodySetup` (box and convex; sphere and
        capsule elements and blocking volumes since 2026-10-21) *Caveat:*
        cooked PhysX shapes unsupported; refreshed on Sanctuary only
  - [ ] Source mesh data
- [ ] Texture importer with TFC streaming
  - [ ] External texture-payload adapter after the Phase 0.5 gate
  - [x] `Textures.tfc`, DXT1/DXT5
  - [x] `PF_A8R8G8B8` (automated extraction verified)
  - [ ] Other pixel formats
  - [ ] `CharTextures.tfc`, `Lighting.tfc`; UHD pack optional
- [ ] Material translation
  - [x] v1: named diffuse/normal/specular/emissive parameters, parent
        inheritance, base-material defaults; opaque lit; fixed roughness
        *Caveat:* an approximation of UE3 shading, not graph translation
  - [x] Diffuse inference from cooked Material resource texture lists when
        no named parameter exists *Caveat:* recorded as inference; graph
        connectivity, tint and masks `UNVERIFIED`
  - [ ] v2: node-graph translation, cel-shade edge, ink lines
        Native resource-tail structure now matches 335 observed Ash/Sanctuary
        materials in diagnostic tooling; field semantics and graphs remain
        `UNVERIFIED`. See [resource census](docs/verification/MATERIAL_RESOURCE_CENSUS.md).
- [ ] Level loader
  - [x] Persistent map + serialized sublevel references (`_P`, `_Px`,
        `_Light`, `_Audio`, `_Combat`, `_Dynamic`, `_FX`)
  - [x] `StaticMeshActor`, `InterpActor`, `StaticMeshCollectionActor`
        transforms (collection tails: observed 84-byte layout, Ash and
        Sanctuary) *Caveat:* observed layout, not a general format guarantee
  - [x] `_Dynamic` props placed as static
    - [~] Skybox: observed `Sky_Dome` placement and its named inputs
          (gradient column, cloud layer) are decoded; the labeled
          approximation rendered in the retired UE5 host; `_Outer` hull
          opt-in with mesh-default materials; master graph, time-of-day
          animation, Kismet activation and visual parity remain open
  - [~] Terrain / BSP *Caveat:* Sanctuary only; single-layer terrain and
        planar-UV BSP approximations, both labeled `UNVERIFIED`
  - [ ] Runtime streaming (all sublevels currently load at once)
- [ ] Lighting
  - [ ] Inspection rig: movable sun, neutral skylight, reflection capture,
        auto exposure, AO. For geometry/material checks only *(done in the
        retired UE5 host)*
  - [ ] Source lighting: Sanctuary's decoded directional light, sky light,
        height fog and post-process settings drove the UE5 host (2026-10-22)
        *(retired UE5 host; the decoded settings carry over, the calibrations
        to UE5 units were screenshot fits, `UNVERIFIED`)*
  - [ ] Modern: dynamic GI in the host
  - [ ] Fidelity: parse SM3 lightmaps from `Lighting.tfc`
- [ ] Camera and movement
  - [ ] Free-flight spectator pawn, saved start pose and FOV *(done in the
        retired UE5 host; automated test on Ash and Sanctuary)*
  - [ ] Placeholder character controller with collision *(done in the
        retired UE5 host with UE5 movement defaults, Sanctuary start only)*
- [ ] Map coverage: **0 / 82** in the Rust host (3 / 82 loaded in the retired
      UE5 host: `Ash_P`, `Sanctuary_P`, `SouthpawFactory_P`)
      *Caveat: broadening past Sanctuary is deferred until the Phase 4
      vertical-slice gate is met; see the priority note above.*
  - [x] Command-line base-game map selector (`tools/viewer.py`)
  - [x] Optional DLC package discovery (82 installed map names total)
  - [ ] In-game map selector *(done in the retired UE5 host: Tab list, digit
        keys, imported scenes only)*
  - [ ] Loading times and memory profile *Caveat:* one settled frame-time
        and process-memory sample on Sanctuary was recorded in the retired UE5
        host; no load-time measurement
- [ ] Verification: side-by-side screenshots against the real game per map.
      The eight matched Sanctuary cameras and their real-game references carry
      over; the critic's 5.0/10 (2026-10-22) scored the retired UE5 host

**Gate (rescoped 2026-09-18):** Sanctuary loads, is walkable, and is visually
verified against the real game → work moves on to Phase 2 for the vertical
slice (Sanctuary + one Vault Hunter). Full 82/82 map coverage remains the
eventual completion target for this phase but is deferred until the Phase 4
vertical-slice gate is met; see Phase 5. *(Three maps loaded in the retired
UE5 host; none in the Rust host yet.)*

Records: [Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md)
· [Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md)
· [Cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md)
· [Map selection / Southpaw Factory](docs/verification/MAP_SELECTOR_VERIFICATION.md)
· [UV / winding](docs/verification/UV_WINDING_VERIFICATION.md)
· [Collision and walking](COLLISION_WALKING_VERIFICATION.md)
· [Performance / in-game selector](docs/verification/PERFORMANCE.md).

---

## Phase 2: UnrealScript VM (M2) (C++ prototype, in progress; Rust port pending) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*The VM prototype is C++ core work in `src/` and stays valid; it is the
reference the `ow-vm` crate is ported against (Phase 0R). Its host
connections were the retired UE5 host's.*

Goal: Gearbox's own gameplay code executing, scoped to what Sanctuary and the
chosen first Vault Hunter actually need (see
[vertical-slice priority](#priority-the-vertical-slice)), not full native
coverage.

- [ ] Object model: `UObject` with class/outer/name/property bag; `UClass`
      hierarchy; class default objects from package CDOs; `FName` table;
      cross-package reference resolution
- [ ] Bytecode loader for every `UFunction` / `UState`; opcode table; handle
      the Gearbox local-variable-array quirk flagged by UE Explorer.
      **Python and C++ prototypes present (2026-10-01):** both structurally
      decode 12,968 of 12,978 script functions (10 known failures), see
      [the record](docs/verification/SCRIPT_BYTECODE_DISASM.md). C++ object
      loading, interpretation, Core natives and a default-state sweep exist;
      full states/latent behavior and opcode semantics remain unverified.
      The first UE5 inventory connection (retired UE5 host) executed original
      `MoveDelta` on item-only backpack rows; the rest of the menu was on the
      host adapter.
      Class export bodies decode 3,339 / 3,339 in `verify_packages` (structural,
      DECISIONS 2026-10-06).
- [ ] Interpreter: expressions, locals, `out` params, structs, dynamic arrays,
      casts, `foreach`, `switch`, `goto`, delegates, `super`, states and
      transitions, latent functions, timers
      *Progress (work dated 2026-10-05 to 2026-10-09, merged 2026-11-02 in #59):*
      script swaps 1-11 run Fire mission pieces as installed script on the VM,
      and the combat steps S-a to S-g run the installed hit, death, down and
      loot scripts there (`ow-package --combat-run`). The Fire event-order
      tool's 32 rules hold. Latent state code is still out of scope; not ticked.
- [ ] Native dispatch table: all 7,141 natives registered as stubs that log
      `UNIMPLEMENTED name(args)`
      *Note (2026-10-02):* a scan of the executable's own registration tables
      finds 6,877 natives in 770 tables, and all 199 numbered script natives
      bind by name ([DECISIONS.md](DECISIONS.md) 2026-10-02, tooling in
      `tools/ghidra/`). The two counts come from different sources and have not
      been reconciled; no stub table exists yet.
- [ ] The 286 Core builtins (operators, math, string, name, object)
- [ ] Test harness running pure-script classes in isolation (the 268
      script-only `Behavior_*` classes), outputs compared to UDK
      **Diagnostic harness present:** synthetic execution tests, `--vm-sweep`
      and scalar UI trace replay. Default-state return agreement does not pass
      this gate; see [VM evidence](docs/verification/SCRIPT_VM_PROTOTYPE.md).

**Gate:** VM runs `Behavior_*` chains and the stat-free parts of
`WillowWeapon` without crashing; every unmet native is a logged stub.

---

## Phase 3: Stock UE3 natives (M3a) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

Goal: the UE3 natives the vertical slice needs (movement, collision,
animation playback), not all 1,914 up front. Ground truth: UDK.

- [ ] `Actor` (166): spawn/destroy, transforms, timers, traces, movement,
      attachment, tick, `Touch`/`Bump`
- [ ] `PrimitiveComponent` / collision (47) → host physics
- [ ] `Pawn` (126), `Controller` (45), `PlayerController` (73): walking,
      falling, jumping, slopes, input, camera, possession
- [ ] `SkeletalMeshComponent` (127), ~30 `AnimNode*` types,
      `PhysicsAssetInstance` (19)
- [ ] `ParticleSystemComponent` (46) + Cascade modules → the host's particle
      system
      *Caveat:* only a read-only template reader exists
      (`research/particle_system.py`, Phaselock templates, layouts checked by
      oracles); nothing is converted or rendered.
- [ ] `WorldInfo` (56), `NavigationHandle` (44), `Settings`, `Camera`,
      `Light`, `Sound` stubs

**Gate:** a scripted UE3 pawn moves and animates on a BL2 map with correct
collision, matching UDK-derived golden tests.

---

## Phase 4: Willow natives (M3b) (core rules in progress; host half reset 2026-11-14) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">: the mountain

*The native notes, the live confirmations and the core's combat, mission and
item results keep their status. The host half (Maya moving, shooting and
casting in a host, the slice gate runs) was the retired UE5 host's and is not
started in the Rust host.*

Goal: **Maya**, the vertical slice's first Vault Hunter, walks, shoots a
handful of real guns, uses Phaselock and her skill trees, and enemies on
Sanctuary fight back. Ground truth: the original game instrumented with
unrealsdk, plus community documentation.

Method (revised 2026-10-01): find each native's behaviour by analysing the game
executable locally ([NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md)), write the
rule down in our own words, implement from that note, then confirm it with the
golden-file loop: hook the native in the real game, log every call's inputs
and outputs during play, implement until our engine reproduces the log, extend
the log on mismatch. Decompiler output never enters the repository.
Status (2026-11-02; later live confirmations are in the records the Gate progress below names): over thirty native notes exist
([index](docs/verification/NATIVE_INDEX.md)); every rule in them is `UNVERIFIED` unless a line says how it was
confirmed. Since 2026-10-02 some rules have been confirmed in the running game through bl-sdk hooks and reads, each
named in its record: the progression rules and the Phaselock bob and cast gates (2026-10-02,
[record](docs/verification/REALGAME_GROUND_TRUTH.md)); the Phaselock bubble size rule (DECISIONS 2026-10-03); the
light bullet, air friction, rocket speed, per-pellet damage, splash, status-damage clock and Tediore throw rules (the
"confirmed live" rows of [WEAPON_PROJECTILES.md](docs/verification/WEAPON_PROJECTILES.md)); the generator's level,
name rule and part choice on spawned guns and the weapon material build on 39 guns (DECISIONS 2026-10-23); and, for
den-spawned bullymongs on one map, the notice rules, `ValidTarget` (six answers), the threat gate of
`ValidCurrentTarget`, `TargetChanged`'s flag and counters, and the target priority formula (lanes L8 and L9,
[awareness](docs/verification/AI_AWARENESS_LIVE.md), [targeting](docs/verification/AI_MELEE_TARGETING_LIVE.md); the
lone-bullymong case, the cone boundary and the leaps are still `UNVERIFIED`). The VM combat steps reproduce recorded
live numbers, which confirms the numbers, not the native bodies.

Priority order:
- [ ] Stat core (~120): `AttributeDefinition*`, `SkillDefinition`,
      `WeaponPartDefinition`, `InventoryBalanceDefinition`,
      `WillowDamageTypeDefinition`
      *Progress:* weapon card rules (golden cards 69/69) and the C++ generator
      (0 mismatches against the Python tools) for weapons; level-up attributes
      on the VM. Other item kinds and the general attribute system are not done.
- [ ] `WillowPawn` (246), `WillowPlayerPawn` (65): health/shield, FFYL, damage
      *Progress:* health, the hit and death chains and FFYL (down, bleed-out,
      second wind, respawn) run on the VM against recorded live rows. The retired
      UE5 host had Maya's shield (shield first, then health; recharge after a
      delay) and the down state with a station respawn timed to the live reads;
      the start shield was a slice choice and its numbers stand-ins (UNVERIFIED). Not done: the death
      camera and tunnel effects, the respawn without a station
      ([record](docs/verification/COMBAT_DOWN_RESPAWN.md)).
- [ ] `WillowPlayerController` (298): input, aiming, skills, interaction
- [ ] `WillowWeapon` (121), `WillowProjectile` (95): fire, reload, spread,
      recoil, elemental procs
      *Progress:* firing modes, light bullets, rockets, splash, status damage and
      the Tediore throw from the parts in the retired UE5 host (rules confirmed
      live, see above); spread distribution, proc rate, elemental impacts, beams and
      ricochets are not.
- [ ] `WillowInventory` (50), `WillowItem` (39), item generation from parts
      *Progress:* weapon generation from parts in C++ (and in the retired UE5
      host, `ow.RollGun`, where guns were assembled from their parts);
      non-weapon items not generated in a host.
- [ ] AI: `WillowAIPawn` (121), `WillowMind` (47), `WillowAIComponent` (45),
      GearboxFramework `AIComponent` (48), `GearboxMind` (32),
      `GearboxCoverStateManager` (48)
      *Progress:* notes and live observations for bullymong targeting, melee
      and awareness; a melee blow as a VM fixture without AI. No AI runs.
- [ ] `WillowInteractiveObject` (134): doors, chests, vendors, switches
- [ ] `WillowVehicle` (156)
- [ ] Scaleform bridge (512), `WillowHUDGFxMovie` (98), `WillowUIInteraction`
      (41) via an SWF VM; debug HUD until then
      *Progress:* BL2's own HUD, inventory and Skills movies ran in a Ruffle
      overlay over the retired UE5 host; the action-skill indicator and minimap
      rules were read from the live movie and carry over. Ruffle is a stand-in for Scaleform, not this step.
- [ ] Wwise audio (17 + `AkAudio`), Bink video

**Gate:** a full loop on Sanctuary with Maya: spawn, fight enemies, loot a
gun, equip it, use Phaselock, complete one hand-picked mission, die, respawn.
This is the vertical slice; once met, broad map and character coverage
(Phase 5/6) becomes the priority again.

Gate progress in the retired UE5 host (history: 2026-11-11, the fight and respawn lines updated 2026-11-13; "in the host" means the UE5 build, with labelled stand-ins; none of it is parity, and none of it counts toward the gate for the Rust host).
The Phase 4 gate is not claimed as met. `tools/test_slice_gate.ps1` (removed 2026-11-14) ran the whole loop in one session; its forced
variants passed 14 of 14 links in every recorded run since their first run on main (7eee9ee, 2026-11-07), on
main or on a batch's candidate; the later runs are as the hub's log reported them
([slice gate](docs/verification/SLICE_GATE.md)). Its `-Unforced` variant takes four of the test's stand-ins away:
the gun drop comes from the host's own loot roll, the Phaselock point is bought on the skills page, the mission is
accepted and turned in on the mission page, and Maya is downed by the range's enemies. Teleports and the fight's
fixtures (the hold on fire, one shot at a time, shots that ignore the gun's fire rate, the ammo refill, the decoy
drops) stay. In 8 unforced runs on
2026-11-11, 5 passed all 14 links and 109 of 112 link attempts passed; the three failures were two pickups with an
enemy standing in the view line and one Phaselock cast refused because Maya was already down.
- Spawn: Maya spawned in Sanctuary with first-person arms, collision and a stand-in start shield (a slice choice).
- Fight: the range's enemies noticed, chased and hit Maya with a host brain. Notice radii, update rate, the adult's and
  the baby's stand distances, the swing cycles of four kinds (adult, baby, regular, slinger) and the chase speeds of the
  adult, slinger and baby come from live reads (the regular's and the psycho's speeds are stand-ins); a third read of the
  adult's cycle (2026-11-12, 407 gaps) settled its value, while the rule behind it stays UNVERIFIED. The range
  suite grades the adult's and the regular's cycles and the adult's stand distance in the engine; the default range
  spawns no baby or slinger, so theirs are checked in the light tier only. The psycho has no live read and the
  regular's stand distance is open. The range itself is a host contrivance, and leaps and thrown rocks are described
  in a note ([note](docs/verification/NATIVE_AI_LEAP_THROW.md), UNVERIFIED) and not built.
- Loot a gun: range kills rolled their loot in the host at the stock chance. Unforced, a gun dropped after 20 to 41
  kills. The pickup card and the use key take the aimed drop.
- Equip: the gate equips through the call the inventory page uses. The page's own equip is covered by the
  inventory suite, on a bounded item set.
- Phaselock: bought on the skills page and cast on a range enemy in the gate. Confirmed in game: its lift bob, that
  it casts during a reload (and aborts the reload), and that it is refused during a weapon swap. A critic agent
  scored the Phaselock record's captures 5.1 to 5.7 of 10, not parity.
- One mission: the Fire mission runs from accept to turn-in in the gate and in the quest suite, and its event
  order in the VM was compared with one live trace (45 events matched, 2 out of order, 4 extra). It gives XP only,
  as the stock data does (a host loot stand-in was removed by maintainer decision, 2026-10-07).
- Die and respawn: Maya was downed by the range's enemies, bled out (12 s for a first down) and respawned at the
  decoded station with full health. The steps of the respawn wait on the station path were confirmed
  in two live sessions (six samples, two maps, one player level), and the host follows them. Which station the game
  picks was seen only where one station was both the active and the nearest, so the selection order is not told
  apart; the first respawn after a load did not reuse the load's exit point in all six (in Sanctuary, where the
  order is known, it took the next one). The ammo refill fits five pools without
  rockets but its size was not seen, and the host does not refill yet.

Records: [arms and held guns](docs/verification/WEAPON_VISUALS.md), [walking](COLLISION_WALKING_VERIFICATION.md),
[firing](docs/verification/WEAPON_PROJECTILES.md), [hit](docs/verification/COMBAT_HIT_CHAIN.md),
[death](docs/verification/COMBAT_DEATH.md), [melee](docs/verification/COMBAT_MELEE.md),
[loot](docs/verification/COMBAT_LOOT.md), [inventory](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md),
[Phaselock](docs/verification/PHASELOCK_STOCK_DATA.md), [mission](docs/verification/SANCTUARY_RPG_MISSION.md),
[golden trace](docs/verification/FIRE_MISSION_GOLDEN_TRACE.md), [down and respawn](docs/verification/COMBAT_DOWN_RESPAWN.md),
[real-game reads](docs/verification/REALGAME_GROUND_TRUTH.md).

---

## Phase 5: Campaign completable (M4) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

- [ ] Broad map coverage: the remaining ~79 maps, deferred from Phase 1's
      vertical-slice rescoping (2026-09-18)
- [ ] Remaining Vault Hunters (5 of 6), deferred from Phase 4's
      vertical-slice rescoping (2026-09-18)
- [ ] Kismet interpreter (`MissionTracker`, 81 natives). *Caveat:* slice-scoped
      Kismet, behavior and mission executors already run the Fire mission's
      installed data (rules `UNVERIFIED`; see Now / next), which is not this
      step's general interpreter.
- [ ] Matinee → a host cutscene player for cutscenes
- [ ] Mission system, objectives, fast travel, vending, ECHO/dialogue, Bink
      playback
- [ ] Save files: read/write real BL2 saves (Gibbed format)
- [ ] Full Scaleform UI: menus, inventory, skill trees, map
- [ ] All 82 maps' `_Combat` populations, spawn schedules, bosses, raids

**Gate:** Claptrap to the Warrior, single player, with a real save file.

---

## Phase 6: Parity and beyond (M5) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

- [ ] Co-op netcode (our own)
- [ ] DLC coverage; The Pre-Sequel; standalone Dragon Keep
- [ ] Mod compatibility: BLCMM text mods natively; SDK-mod layer later
- [ ] Editor for new maps; modern lighting toggle; VR; 8-player

---

## Commitment

The maintainer decided (2026-10-03) to drop the written stop conditions: the project continues. See
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md#9-commitment). Verification rules are unchanged:
nothing counts until it has been checked and the result is written down.
