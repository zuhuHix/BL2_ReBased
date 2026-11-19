# The slice gate on the Rust port: what runs, where it stops, and the lanes to the gate (2026-11-16)

AI-assisted (Claude Code), lane SLICE-PLAN. A planning record, not a verification record: it says what the Rust port
does today and proposes the order of the remaining lanes. Nothing in it was checked against the running game, and no
run was made for it; every number below is quoted from the decision or PR named next to it. Lane names marked
*(proposed)* are this plan's, not yet the hub's. The hub schedules; the maintainer decides what section 6 lists.

**Base:** `origin/main` 9f9d4db, plus the open PRs of the same day: #361 W7-WORLDLOGIC (decision 60), #362
FIX-PKG-LOOKUP, #363 H6-SKIN (64), #364 W7-AI part 2 (66), #365 H9-LIGHTMAPS (65), #366 H4-WEAPON (61), #368
FIX-PHASELOCK-CHAIN (69). Decision numbers below are `docs/decisions/2026-11/2026-11-1x-NN-*.md`; "D54" is decision
2026-11-16-54.

## 1. The gate, and how this plan reads it

[SLICE_GATE.md](../verification/SLICE_GATE.md) is Phase 4's gate: one session in Sanctuary with Maya that spawns,
fights, loots a gun, equips it, uses Phaselock, completes one mission (the Fire mission), dies and respawns. It has 14
links. Its 14 / 0 / 0 runs were the UE5 host's and stopped counting when that host left the tree (SLICE_GATE.md, the
"Retired host" note); evidence there that does not depend on the host keeps its status.

| Step | Links |
|---|---|
| Spawn | 1 `spawn_maya`, 2 `reach_fight_area` |
| Fight | 3 `kill_enemy`, 4 `enemy_attacks_maya` |
| Loot a gun | 5 `loot_drop_on_kill`, 6 `pick_up_gun`, 7 `pick_up_aimed_not_nearest` |
| Equip it | 8 `equip_gun` (draws and fires the looted gun) |
| Use Phaselock | 9 `phaselock_enemy` |
| Complete the mission | 10 `mission_accept`, 11 `mission_objective`, 12 `mission_turn_in` |
| Die, respawn | 13 `go_down`, 14 `bleed_out_respawn` |

Two readings this plan takes, both for the maintainer to confirm (section 6):

- **The gate runs in `ParityMode::Off`.** `Reference` is the C++ reading kept as a parity scaffold; its stand-ins (the
  stand-in loot inventories, the health pool, the Reference stubs of `--slice-run`) are deleted at W7-RETIRE
  (ARCHITECTURE.md section 6.1). A link that passes only in Reference is progress, not the gate.
- **The gate is passed in the host, `ow-host --play`, before it counts**, with a headless run of the same 14 links in
  Off as the step before it (lane 14 below). The headless run is where most of the rules are checked; the host run
  shows them in one session on screen, as the UE5 gate did.

## 2. Where each link stands today

"Ref" and "Off" are headless CLI runs (`ow-cli`); "host" is `ow-host`. Every result is the port's against its own
data or against the C++, UNVERIFIED in the running game.

| Link | Headless, Reference | Headless, Off | Host |
|---|---|---|---|
| 1 spawn | The mission bridge's own controller and pawn, from class defaults; no map is loaded (D34, D54) | Maya's health pool `NotYet` (D38, D54) | `--walk` and `--play` start at `WillowCoopPlayerStart_0` (D52, D57); no world pawn |
| 2 reach the fight area | not modelled (no map) | `--slice-gate` link 2 (`sanctuary-walk`, decisions 2026-11-19-66 and 2026-11-19-76): H4's walk passes on the installed Sanctuary through the shared mover and ends inside the fight area, a labelled port fixture; placing the target dummy there through `ow_world`'s spawn stops at `Default__WillowAIPawn.SpecialMoveComponent0` (D44's open item, as link 3) | walking in the window (D57); no fight area actors |
| 3 kill an enemy | `--combat-run` scripted `hit` steps; the adult's kill stops at `AmmoDropWeightAttributeValueResolver` (D54) | every spawn stops at `Default__WillowAIPawn.SpecialMoveComponent0` (D54, D55) | #366: one hit-scan ray against collision; no pawns to hit (D61) |
| 4 the enemy fights back | scripted `swing` rows equal to the C++ (D54); the AI picks the player and starts Patrol, then stops (D59, #364) | as link 3 | none |
| 5 loot drop | C++ stand-in inventories, stops at the same resolver (D41, D54) | `ItemPool.SpawnBalancedInventoryFromPool` `NotYet` (D41) | none |
| 6, 7 pick up | not modelled (pickup is host side, D41) | not modelled | none |
| 8 equip and fire | equip is library code with unit tests; `--inventory-move --script-diff` 50 of 50 weapons (D33); no run step, no gun-to-hit link | as Ref | #366 fires a gun read from a `--weapon` file, not the inventory's (D61) |
| 9 Phaselock | duration 5 from pawn and controller (D62) | ignored installed test: duration `5 + 0.5 g` at 6 of 6 grades and the lift's data (D42); the cast stops (section 3.5) | none |
| 10-12 the mission | `--slice-run`: 33 of 33 Fire event-order rules, on 20 Reference stubs and stand-ins (D48, D50, D62) | stops at the accept and at the dummy's spawn (D38) | none (no use key, no UI) |
| 13 go down | `--combat-run` S-f: 16 of 16 rows equal to the C++ (D54) | health pool `NotYet` (D54) | none |
| 14 bleed out, respawn | `down.respawn_health` equal; the fee row stops at `CurrencyAttributeValueResolver` (D54) | as link 13 | none |

In short: the rules of every step exist in some form headless, most of them in Reference only; Off stops early in
every step on a small number of shared causes (section 3.1); the host walks and draws Sanctuary and fires one ray, and
nothing joins it to the game rules yet (section 4).

## 3. Per step: the first stop and the lanes that close it

Each step lists what runs, the first stop, the lanes in dependency order (owner crate; clean room where a rule has to
be read from the executable first), and what only the running game can settle. A clean-room split means: an analyst
writes an own-words `NATIVE_*.md` note and leaves the rule `NotYet`; a fresh implementer codes it from the note alone
(docs/LANE_BRIEF.md).

### 3.1 Shared causes (they stop several steps at once)

- **No map load.** No run loads Sanctuary as a VM world: `--slice-run` and `--mission-run` build a bridge world with
  its own `WorldInfo` (D34), and the world's queries for globals, game info and replication info answer `NotYet`
  naming the map load (`crates/ow-world/src/query.rs`; D47, D49). The spawn rules D26 left unread
  (`RemoteRole`, templates other than the class default object, collision during spawn) and
  `Actor.SetInitialState`'s auto state in Off (D27) sit on the same path. No lane is named for it in the sources.
- **Template archetypes in Off.** Every Off spawn stops at the archetype reference
  `Default__WillowAIPawn.SpecialMoveComponent0` (D54). D55 left template archetypes unapplied: their prefix is not
  recorded and `PropertyPrefix` has no 16-byte case. Rule 4 applies: recover it from the packages and check it with
  structural oracles.
- **Pools in Off.** Every real `CreatePool` is refused until `ResetIdleTimer` is read (D29), and Maya's health pool is
  a Reference-only stand-in (D38, D54). Both need an analyst note first.
- **`Actor.Destroy`** of a controller or player is not read (NATIVE_ENGINE_CORE.md has no entry; D54), which stops
  the baby bullymong's kill (S-d) after 2 of 9 rows.

Lanes, in order:
1. **FIX-TEMPLATE-PREFIX** *(proposed)*, `ow-defs`: the template archetypes and the 16-byte prefix case, from the
   packages. No native reading.
2. **W5-MAPLOAD** *(proposed)*, `ow-world`: Sanctuary's level actors in a `World`, the world info, game info and
   replication info from the map, the local player's controller and Maya's pawn at the player start, the local-player
   list `Actor.LocalPlayerControllers` needs. The unread spawn rules go through an analyst note first.
3. **FIX-POOLS-OFF** *(proposed)*, `ow-world`: `ResetIdleTimer` and the health pool's class-defaults routine. Analyst,
   then implementer.
4. **FIX-ACTOR-DESTROY** *(proposed)*, `ow-world`: the controller and player case. Analyst, then implementer.

Real game: whether actors spawned during a tick tick in that frame, and where `RemoteRole` is set (D26).

### 3.2 Spawn (links 1 and 2)

- **Runs:** the bridge's player in `--slice-run` and `--combat-run`, from class defaults (D54); the dummy's spawn
  through the definition store in Reference (D48). In the host, `--walk` passes and `--play` walks Sanctuary in the
  window at a fixed step (D52, D57). The fight area is the UE5 gate's range, which the port has no counterpart of.
- **First stop:** Off, Maya's health pool (`NotYet` at `health:` and at a level-up, D38; `NotYet { NotPorted }` at
  `maya` in `--combat-run`, D54). The dummy's `Behavior_AIHold` is `NotYet { NotPorted }` in Off (D38).
- **Lanes:** 2 and 3 of section 3.1; then **FIX-AIHOLD** *(proposed)*, `ow-behavior` with `ow-ai`'s holds (#364
  binds the component's `Hold` natives). COMBAT_GOLDEN_TRACE.md question 8 saw the dummy's first-hit `AIHold` make it
  demigod; which part of the hold does it is UNVERIFIED, so a short analyst note may be needed first.
- **Fight area:** the gate needs enemies placed where Maya can reach them. Whether that is Sanctuary's own population
  or a test fixture like the UE5 range is a scheduling choice; the population lane (dens) is named only as a seam
  (D59). The hub chose a port fixture for now (decision 2026-11-19-76): a fixed spot H4's walk reaches, with the
  target dummy placed there by the slice driver, labelled a fixture and not parity.
- **Real game:** none beyond section 3.1.

### 3.3 Fight (links 3 and 4)

- **Runs:** `--combat-run`, Reference: W6-COMBAT's 15 steps give 20 rows, all equal to the C++, 9 of its 29 missing
  (D54, #356); S-f's four swings equal (D54). AI: awareness and targeting of a lone component, 8 of 8 placements in an
  ignored installed run (D59, #357); the action tree perceives the player and starts Patrol at 0.25 s (#364).
- **First stops:**
  - Off: every spawn (section 3.1, template archetypes).
  - Reference: the adult's hit and kill stop in its death roll at `AmmoDropWeightAttributeValueResolver` (D54).
  - The player's attack is a scripted `hit` with a damage number; nothing goes from a held weapon through
    `ow_combat::firing` to the hit chain. Off damage stages answer `NotYet { NotPorted }`, the shield stage among them,
    which the enemy's blow on Maya's shield needs (D35).
  - The AI stops at 0.5 s on whether the running action is locked (`NotYet`: clean room, NATIVE_AI_ACTION_SEQUENCE.md
    in #364). Its attacks answer `NoTarget` because target records live in Rust, not as VM objects for
    `GetTargetRecord`; the latent moves (`FollowPath` and the rest) are `NotYet` (the VM's latent actions are `Sleep`
    only); nothing calls the AI tick from the world (D59, D66).
  - Off, the crit bonus with base 0 is `NotYet { NeedsRealGame(8) }` (D35).
- **Lanes:**
  1. **FIX-ATTR-RESOLVERS** *(proposed)*, `ow-attr`: `AmmoDropWeightAttributeValueResolver`,
     `RandomAttributeValueResolver`, `CurrencyAttributeValueResolver` (also needed by loot and respawn). Analyst first
     where the resolver is native.
  2. **FIX-FIRE-TO-HIT** *(proposed)*, `ow-combat`: a `fire` step from the held weapon through `firing` into the hit
     chain, and the Off damage stages from the existing NATIVE_DAMAGE_DEATH reading (implementer; analyst only where
     the note has a gap).
  3. **W7-AI-LOCK** *(proposed)*, `ow-ai`: `StartAtomic`, `StopAtomic`, `IsAtomic`, `IsRunning`,
     `IsActionReadyToFinish`, `RangeIsValid`, `GetRangeValue` and the lock question, from NATIVE_AI_ACTION_SEQUENCE.md
     alone, by a lane that has not read the executable for them.
  4. **W7-AI part 3**, `ow-ai`: target records as VM objects for `GetTargetRecord`; `Action_GenericAttack.InitData`,
     the attribute and flag expression evaluators and the five `WillowMind` queries the run reached (analyst
     questions first; D59 lists nine); then the melee rules of G25 so the bullymong's swing comes from its tree rather
     than a scripted step.
  5. **VM-LATENT-HOST** *(proposed)*, `ow-vm`: a host-driven latent action, so `FollowPath` and the waits can run.
  6. The world's call into `tick_brain` (`ow-world`), with lane 2 of section 3.1.
- **Real game:** question 8 (Maya's crit source); question 17 (the lone bullymong, the cone edge); the six bl-sdk reads
  D59 lists for the AI.

### 3.4 Loot a gun (links 5 to 7)

- **Runs:** `--combat-run`'s S-g steps, 12 lists byte-identical to the C++ on invented packages in Reference (D41,
  #340); installed, 33 rows, 29 equal to the C++ (D62). Outside combat, Reference: `--item-roll --weapons` 29,880 of
  29,880 (D21), `--item-pool` 7,680 of 7,680 (D25); Off `--item-pool` 6,360 complete and 1,320 `NotYet` (D25).
- **First stops:** Reference loot is the C++ stand-in inventories, not real guns (D41). Off,
  `ItemPool.SpawnBalancedInventoryFromPool` is `NotYet { NotPorted }`, checked before any draw (D41). The three
  `WillowInventory` natives `InitializeInventory` calls are bound in Off, and `GetItemTossVelocity` is `NotYet` on
  the toss rotation (2026-11-19-29); no installed Off run reaches the death roll yet (link 1 stops at
  `GetPlayerPawnDataManager`). An Off rolled
  weapon on the installed game stops at the weapon's change notification (D24). Pickup is host side, with no lane
  named (D41).
- **Lanes:**
  1. FIX-ATTR-RESOLVERS (section 3.3).
  2. **FIX-POOL-SPAWN** *(proposed)*, `ow-items` and `ow-inventory`: `roll_pool` with a context source and an outcome
     report, the native in Off spawning real inventories with `InitializeInventory`, the loot-spawn behaviors and
     their handler consumer (`ow-behavior`). NATIVE_LOOT.md does not read the loot-spawn behaviors, so analyst first
     (D41).
  3. Pickup: in the host, lane H-INTERACT (section 4).
- **Real game:** question 20 (the live three-item money drop; the row stays open, D41).

### 3.5 Equip it (link 8) and use Phaselock (link 9)

- **Equip, runs:** `ow-inventory`'s equip and unequip through the attribute effects, unit tests only; the two script
  overrides against the installed script, 50 of 50 weapons and 19,610 values (D33). **First stops:** no pack binds the
  `AttributeDefinitionBase` natives (`ResolveContext`, `AddAttributeModifier` and the rest), so script stops at
  `UnboundNative` (D33); the shield's level attribute stops at `InventoryAttributeContextResolver` (D24); comparing
  the Rust equip with the script needs the unread `AllParts` and `InitializeAttributeSlots` (D33). The host's gun is a
  file, because the equipped one needs D9 or a starting loadout (D61).
- **Equip, lane:** **FIX-EQUIP-NATIVES** *(proposed)*, `ow-inventory` with `ow-attr`: the natives above, the context
  resolver, an `equip` step in `--combat-run` that feeds FIX-FIRE-TO-HIT's `fire`. `AllParts` and
  `InitializeAttributeSlots`: analyst first.
- **Phaselock, runs:** skills, the four branches and 31 skills, the lock duration and the lift's data, in an ignored
  installed test (D42, #342); the duration in Reference (D62, #360). **First stops:** in Off the duration got past
  `Object.PathName` in #368, which now stops at the designer class default object's value resolver, not loaded by the
  store; the lift runs on `ow-vm`'s `Object.VLerp` in both modes and `OnActionSkillStarted` gets past
  `ActionSkill.OnActionSkillActivated` in Off (bound, 2026-11-19-12) and stops at the unbound
  `GearboxPawn.GetSMComponent`, which no record reads; Reference still stops at `OnActionSkillActivated`; the cast stops after `RegisterBehaviorConsumer` at
  `BeginNondeterministicProviderRegistration`, unbound, rule now read in NATIVE_BEHAVIOR_PROVIDERS.md with
  `ForceUnregisterBehaviorConsumer`, not implemented (D50, 2026-11-19-02); `EngineQuery::PlayerSkill` needs a
  grade-valued query (D42). The cooldown maximum resolves to 13 in Off once #368 lands.
- **Phaselock, lanes:** #368 first; then **FIX-PHASELOCK-CAST** *(proposed)*, `ow-skills` with `ow-behavior`, `ow-vm`
  and `ow-world`: the designer default object's resolver (`ow-defs`), `VLerp` (clean room, a stock Core native),
  `BeginNondeterministicProviderRegistration` and `ForceUnregisterBehaviorConsumer` (analyst, then implementer; D50
  lists them), the grade query, and a `phaselock` step in `--combat-run`.
- **Real game:** the repeat-lock time (about 3.7 s read against about 4.27 s, D42); the precision of `VInterpTo`
  (D42); none named for equip.

### 3.6 Complete the mission (links 10 to 12)

- **Runs:** `--slice-run`, Reference: 33 of 33 Fire event-order rules; step records equal to the C++, 22 of 22 and 17
  of 17 (D48, #351), unchanged by #352 and #360 (D50, D62). `--mission-run`, installed, 77 Sanctuary missions:
  tracker 73 of 77 in Reference and 58 of 77 in Off (D39, D48).
- **First stops (Off):** the accept and the turn-in stop on `Actor.LocalPlayerControllers` and
  `MissionTracker.GetObjectivesProgress`, refused by the hub's decision until read (D34). The first has an own-words
  note (NATIVE_ENGINE_CORE.md) and needs the local-player list of a map load; the second has no row in the native
  index. The 18 other Reference stubs (`Object.Localize`, the GFx calls, `GFxMovieDefinition.SpawnPlayerMovie` and
  the rest) stay refused (D38, D48). The dummy's spawn stops at `Behavior_AIHold` (D38). In both modes, `pawnhit:` on
  the dummy and `BehaviorBase.RunBehaviors` during the level-up are `NotYet { NotPorted }` (D38). Off `--mission-run`
  on the Fire mission stops at the lent weapon's region-stage condition, `NotYet { NotRead }` (D34).
- **Lanes:**
  1. W5-MAPLOAD (section 3.1) for the local-player list.
  2. **FIX-MISSION-OFF** *(proposed)*, `ow-mission` with `ow-world`: `LocalPlayerControllers` from its note;
     `GetObjectivesProgress` and the lent weapon's region-stage condition (analyst first); `pawnhit:` and the slice's
     health pool shared with `--combat-run`'s (the follow-up D54 left open); the level-up skill behaviors (W6-SKILLS's
     follow-up).
  3. FIX-AIHOLD (section 3.2).
  4. A hub or maintainer call on the UI stubs: whether `Localize` and the GFx calls answer a labelled no-op in Off
     until W7-UI, or the gate waits for W7-UI.
- **Real game:** question 1 (playthrough values, D40, D47); the live `GetAttributeContext` of `Actor` and
  `BalancedActor` (D49). The Fire event order itself has a golden trace (FIRE_MISSION_GOLDEN_TRACE.md).

### 3.7 Die and respawn (links 13 and 14)

- **Runs:** `--combat-run` S-f, Reference: 16 of 16 rows equal to the C++, the bleed-out, the wall and the camera
  among them, and `down.respawn_health` (D54). The enemy's kill credit of 35 equals the C++ (D54).
- **First stops:** Off, the health pool (section 3.1), and the bridge's script reaches the unbound
  `Actor.LocalPlayerControllers` (D54). The fee row stops at `CurrencyAttributeValueResolver` (D54). Which station
  respawns Maya, and what happens with none, is question 16; the no-station case is `NotYet` (ARCHITECTURE.md
  section 12).
- **Lanes:** FIX-POOLS-OFF and FIX-ACTOR-DESTROY (section 3.1); FIX-ATTR-RESOLVERS (section 3.3); #361
  W7-WORLDLOGIC (decision 60), which reads Sanctuary's four fast-travel and New-U stations from the map and leaves
  registration and respawn selection to the installed script; then **FIX-STATION-NATIVES** *(proposed)*,
  `ow-mission`: the station natives respawn selection needs (#361's open item).
- **Real game:** question 16.

## 4. The host: what joins the rules to the window

**On main:** the window and frame loop with a `Simulation` trait (H1, D44, #344); the unit and axis module (H3, D45,
#343); Sanctuary as plain data and drawn (H2, D46 and D51, #346 and #353); movement and collision, `--walk` (H4, D52,
#349); Maya's meshes and clips as data (H6, D53, #350); her AnimTrees evaluated headless (H7, D56, #354); `--play`
(H4-PLAY, D57, #355); materials, the sky and the lights (H8, D58, #358). **In flight:** H4-WEAPON (#366: one
hit-scan ray, damage from W4-STATS in a one-off `World`), H6-SKIN (#363: Maya's first-person arms skinned on the GPU),
H9-LIGHTMAPS (#365).

**Missing seams.** On main the host calls `ow_assets`, `ow_defs`, `ow_object` and `ow_package` only; `World::tick` is
called by nothing, the `Simulation` is the stub, and the player's mover lives in `ow-host`, so no input reaches a world
pawn. There are no pawns in play mode, so a hit has nothing to land on (D61). The AI's `Host` / `Senses` and
`Locomotion` have no host implementation (D59, D66), and the sources hold no reading of the nav mesh the movement
natives would query. Only the first-person arms are drawn: no third-person body, no enemy mesh, no weapon mesh; which
AnimTree node plays fire, reload and the Phaselock cast is open (D56). No use key, interactive object or Kismet runs in
the host (the sky's aurora needs the day-night Kismet, D58). The UI (H5 and W7-UI) has not started, and Ruffle needs
its `THIRD_PARTY.md` entry first.

Lanes, in order (after the in-flight PRs):
- **H-WORLD** *(proposed)*, `ow-host` with `ow-world`: a world-backed `Simulation`: W5-MAPLOAD's world ticked from the
  host frame, the H4 mover driving the world's player pawn, `set_physics_step`.
- **H-AI-HOST** *(proposed)*, `ow-host`: `Host` / `Senses` (exposure through H4's collision, the targetable registry)
  and `Locomotion`. Path queries need the nav mesh: a research lane on its format comes first (package oracles, rule
  4), or the hub accepts a labelled straight-line stand-in for the fight area only.
- **H-PAWNS** *(proposed)*, `ow-host`: enemies and Maya's third-person body skinned (H6's follow-ups), H7's open
  nodes for fire, reload and the cast, drop actors drawn.
- **H4-WEAPON2** *(proposed)*, `ow-host` with `ow-combat`: the inventory's held weapon instead of a file, hits on
  pawns through the hit chain, spread, pellets and projectiles from `ow_combat::firing`, ammo and reload, the weapon
  mesh.
- **H-INTERACT** *(proposed)*, `ow-host` with `ow-mission`: the use key, pickups (the aimed pick of link 7), the
  interactive objects #361 reads, Kismet in the host.
- **H5 and W7-UI**, `ow-ui` and `ow-host`: the HUD, the mission and dialog pages, the skills page and the inventory
  page, through the `MoviePlayer` (Ruffle). Or the maintainer accepts a labelled debug overlay for the gate, as the UE5
  gate drove the host through public calls and console commands.
- **H-DOWN** *(proposed)*, `ow-host`: the down state's camera and input, and the respawn at the station.

## 5. The ordered lane list to the gate

Ordered by dependency; lanes on one line can run together. Owner crate in brackets; "clean room" means an analyst note
and a fresh implementer.

0. **Land what is in flight:** #364 W7-AI part 2, #368 FIX-PHASELOCK-CHAIN, #361 W7-WORLDLOGIC, #362 FIX-PKG-LOOKUP,
   #366 H4-WEAPON, #363 H6-SKIN, #365 H9-LIGHTMAPS.
1. **FIX-TEMPLATE-PREFIX** [`ow-defs`; package oracles] — unblocks every Off spawn.
2. **W5-MAPLOAD** [`ow-world`; clean room for the unread spawn rules] — Sanctuary as a world, Maya's controller and
   pawn, the local-player list.
3. Together, after 1 (and 2 where noted):
   - **FIX-POOLS-OFF** [`ow-world`; clean room] — Maya's health in Off; links 1, 13, 14.
   - **FIX-ACTOR-DESTROY** [`ow-world`; clean room] — the kill's cleanup; link 3.
   - **FIX-ATTR-RESOLVERS** [`ow-attr`; clean room where native] — links 3, 5, 14.
   - **FIX-AIHOLD** [`ow-behavior`, `ow-ai`; short analyst note if needed] — links 3, 10-12.
   - **W7-AI-LOCK** [`ow-ai`; implementer from NATIVE_AI_ACTION_SEQUENCE.md] — link 4.
4. Together, after 3:
   - **FIX-FIRE-TO-HIT** [`ow-combat`] — link 3.
   - **FIX-EQUIP-NATIVES** [`ow-inventory`, `ow-attr`; clean room for `AllParts` and the slots] — link 8.
   - **FIX-POOL-SPAWN** [`ow-items`, `ow-inventory`, `ow-behavior`; clean room for the loot-spawn behaviors] —
     link 5.
   - **FIX-PHASELOCK-CAST** [`ow-skills`, `ow-behavior`, `ow-vm`, `ow-defs`; clean room for two kernel natives and
     `VLerp`] — link 9.
   - **FIX-MISSION-OFF** [`ow-mission`, `ow-world`; clean room for `GetObjectivesProgress` and the region-stage
     condition; needs 2] — links 10-12.
   - **W7-AI part 3** [`ow-ai`; analyst questions first] and **VM-LATENT-HOST** [`ow-vm`] — link 4.
   - **FIX-STATION-NATIVES** [`ow-mission`; after #361] — link 14.
5. **SLICE-RUN-OFF** *(proposed)* [`ow-cli`, `ow-mission`] — the 14 links as one headless scripted run in the loaded
   Sanctuary world, in Off, graded by a checker like `tools/check_slice_gate.py`. The first full proof, without a
   window.
6. Host, starting once 2 has landed and running beside 3-5:
   - **H-WORLD** [`ow-host`, `ow-world`];
   - then together: **H-AI-HOST** (with the nav mesh research), **H-PAWNS**, **H4-WEAPON2**, **H-INTERACT**,
     **H-DOWN** [`ow-host`];
   - **H5 / W7-UI** [`ow-ui`, `ow-host`; after Ruffle's `THIRD_PARTY.md` entry], or the maintainer's debug-overlay
     call.
7. **SLICE-GATE-HOST** *(proposed)* [`ow-host`, `tools/`] — the 14 links in `ow-host --play` (a `--play-script`
   driver), graded by the checker: the gate, 14 / 0 / 0 in Off, then the real-game questions below for what it rests
   on.

**Size, hedged.** About 25 lanes from step 1 (the nav mesh research included), about 10 of them with an analyst step
before the implementer. The
critical path looks like 1 → 2 → 3 → 4 → 5 → 7, with the host lanes beside it; each NotYet a lane removes tends to
show the next one, so the count is a floor, not a forecast. AI movement in the host (the nav mesh) and the UI
(Ruffle) are the two items with no reading at all, and the most likely to grow.

## 6. What only the maintainer can decide or grant

- **Decisions:** that the gate runs in Off and in the host (section 1); the go-ahead for the W7 lanes the slice needs
  (ARCHITECTURE.md section 13 asks for it before the Phase 4 gate; W7-AI and W7-WORLDLOGIC are already in flight);
  Ruffle's `THIRD_PARTY.md` entry before H5, or a debug overlay for the gate; D9 (the save format), which the
  equipped weapon and a loadout wait on (D61), unless the gate starts from a fixed loadout; whether the installed
  `Engine/Shaders` may be read (D65); a hand check of `--play` (D57).
- **Real-game sessions (bl-sdk), each needing the maintainer's grant**, grouped so that one session can cover several:
  - combat: question 8 (Maya's crit source), the pistol's `WeaponDamage` (D61);
  - AI: question 17 and the six reads D59 lists;
  - loot: question 20 (the three-item money drop);
  - respawn: question 16 (the station chosen; no station);
  - Phaselock: the repeat-lock time (D42);
  - mission and attributes: question 1 (playthrough values) and the live `GetAttributeContext` of `Actor` and
    `BalancedActor` (D49);
  - host: a jump trace and `InterpActor_34`'s collision (D52), screenshots from the player start for exposure, the
    light maps and the sky (D58, D65), a first-person arms frame (D64).

None of these blocks a lane from landing (ARCHITECTURE.md section 12): a lane implements what is read, labels it
UNVERIFIED, and answers `NotYet { NeedsRealGame(N) }` where nothing is read. They decide whether the gate's links rest
on confirmed rules.
