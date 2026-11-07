# Decisions and evidence

Older entries: [2026-09](docs/decisions/2026-09.md).

Newest entries are at the bottom. Read them with python tools/decisions_index.py --tail N or grep; do not open this file whole.

## 2026-10-01: Resume the VM handoff through diagnostic trace replay

AI-assisted. Preserve the two local Phase 2 commits and the existing uncommitted
batch CLI before continuing. Complete that batch/replay path rather than replacing
the interpreter architecture. No parser layout, bounds check, dependency or
license changes. Input validation rejects malformed scalar payloads, missing or
duplicate arguments and incompatible receiver classes; omitted trailing optional
parameters retain script defaults. Clear native logs before every case, including
cases that fail during lookup. Native/stub execution cannot count as a return match.

Replay uses fresh class-default receivers, not recorded live object state. Results
remain UNVERIFIED: the first 400 existing trace pairs yielded 25 return matches,
four mismatches, 19 blocked and 352 skipped. CTest 8/8; all nine package differential
checks match; both disassemblers remain at 12,968/12,978 structurally decoded.
Full evidence and the next state-faithful comparison are documented in
`docs/verification/SCRIPT_VM_PROTOTYPE.md`. No fresh in-game or UE validation.

## 2026-10-01: Connect item-only backpack movement to the original script VM

AI-assisted. Prefer the smallest live menu connection over replacing the entire
adapter at once: execute installed `InventoryListPanelGFxObject.MoveDelta` for
ordinary backpack Up/Down, supplying the list length and binding the provider's
native entry-kind interface. Resolve the source kind through reflected enum
identity instead of hardcoding a numeric enum value. Reuse the enum serialization
order already decoded in vm.cpp, with a bounded Reader, checked prefix/count/name
references and exact consumption. No existing package/container bounds checks
are loosened. CMake adds the independent navigation adapter to ow-core; the UE
module links the local Release libraries. No dependency/license change.

Fail on VM diagnostics, malformed input or invalid results. Serialize repeated
keys, reject obsolete selection/list replies and cancel on menu close. Synthetic
tests and direct installed-script checks establish the bridge plumbing and
bounded navigation behavior. Full original-game state, equipment/equip scripts,
sorting, empty/category entries and visual parity remain UNVERIFIED. Verification
details: `docs/verification/SCRIPT_VM_PROTOTYPE.md` and the inventory record.

Runtime acceptance: two new Slate-key checks pass through original MoveDelta
and update the actual page selection, 55 expressions each, zero diagnostics.
Full runner remains FAIL (41/48, seven failures from item-selection expectations,
a pickup cascade and missing shield data; no VM-disabled baseline). Dependent
gear/pickup passes are not parity evidence. CTest 8/8 and navigation 22/22 pass;
nine package differential checks match. Engine test used launch-only cache
fallback/D3D11 after two startup stalls; no project renderer/cache changes.


## 2026-10-01: one placed Sanctuary mover before broad world behavior

Reuse one prepared door mesh/material/convex-collision chain. Follow the installed
Matinee action's variable/data/group/track references using the owned reader,
resolve shared resources in the existing scene scope, and keep curves/bindings
under ignored local output. No new extraction backend or license decision.

Extend VM object materialisation to explicit placed exports: class defaults plus
tagged overrides at caller-supplied, established 4/8/26 prefixes. Validate the
prefix fits the export rather than scanning offsets or loosening bounds. Native
tails, resource object graphs and archetype inheritance stay outside this helper.
Use installed InterpActor lifecycle scripts and scoped timer natives; reject
loading/execution diagnostics and restore script state on failure. No script
listing or original game logic is transcribed into project code.

The host evaluates the installed movement keys and promotes only the bound
component to movable at runtime, restoring its pose/mobility on failure or
shutdown. E input is a developer activation path. Mission/Kismet activation,
Ak-event tracks, encroachment, checkpoint persistence and original-game relative
frame/Euler/auto-curve parity remain UNVERIFIED. Test synthetic state/timers and
real host collision separately; see the mover verification record.

## 2026-10-01: native mission/Kismet executors over installed data; struct-embedded arrays by reflection

AI-assisted. Context: the Sanctuary + Maya slice needs mission, behavior and Kismet logic, and
`MissionTracker`, `BehaviorKernel`, `Behavior_AdvanceObjectiveSet/MissionRemoteEvent/ActivateMission/
CompleteMission` and `SequenceOp` activation are native in this build (no script), while their definitions
are readable data. Decision: implement small native executors in `src/kismet.*` and `src/mission.*` that
read the installed definitions through the VM's reflection-typed property reader, report world-acting ops
at a host boundary instead of running them, and record every guess as `UNVERIFIED` (see
`docs/verification/SANCTUARY_RPG_MISSION.md`). Do not recreate recoverable content.

Parsing behaviour change (`src/vm.cpp`): arrays inside structs are decoded using the struct field's own
reflection, and struct declarations are taken from the property declaration rather than looked up by name in
the object's package (which only worked for structs the package happened to import; `SeqOpOutputLink.Links`
had silently decoded as empty). No bounds check was loosened: element decoding still requires exact
consumption of the tagged size and fails the whole property otherwise. `CMakeLists.txt` gained `kismet.cpp`,
`mission.cpp` and a synthetic test; no dependency or license change. Verified: CTest 9/9, nine package
comparisons, Kismet census 0 unresolved links over `Sanctuary_Dynamic`, in-engine door suite 16/16.
Unverified: all native semantics against the original game (no paired capture yet).

## 2026-10-01: behavior variable data and two VM reader fixes

AI-assisted. `BehaviorProviderDefinition` variable values are an untagged block after the tagged properties:
one entry per `VariableData` element, in sequence then variable order, with a size per `EBehaviorVariableType`
name (Bool/Int/Float/Object 4, Vector 12, DirectionVector 48, InstanceData 12, Attribute 20, UnaryMath 8,
BinaryMath 12, AttachmentLocation 32, Flag 8, Named*/AllPlayers 0). The table is FITTED: checked by exact
consumption on 32,185 provider exports (0 mismatches), reference/class checks and cross-copy consistency
(0 inconsistent), and trusted only when consumed exactly. `Behavior_CompareObject` now runs from data (its
script: equal objects follow link 0, otherwise link 1); the Fire dummy's sequence is chosen by its
`BehaviorSequenceEnableByMission` condition, whose evaluation is native, so the rule used is UNVERIFIED.

Parsing behaviour change (`src/vm.cpp`): enum bytes declared in another package now resolve through the
declaring `ByteProperty` (previously read as 0); struct values start from the `ScriptStruct`'s own default
tags (header 52 bytes, fitted; 1,275 of 1,275 structs in the code packages end exactly at the export end) and
only when that stream is consumed exactly. No bounds check was loosened. `CMakeLists.txt` gained the
`behavior-synthetic` test. Verified: CTest 10/10, nine package comparisons, unchanged mission/Kismet/door
runs apart from the new `SetSequence` action field. Not verified against the game: link-id semantics, the
once-per-event rule (duplicate links always differ by id byte; the Fire mission's `Default` event has two such
pairs, so this rule may drop real activations), and the enable-condition rule. Record:
`docs/verification/BEHAVIOR_DATA_DECODE.md`.

## 2026-10-01: weapon balances, card stats and loot pools decoded for the slice

AI-assisted. The mission pistol `MW_RockPaper_Fire` resolves through `Pistol_Maliwan` -> `_2_Uncommon` ->
`Pistol_Maliwan_2_Fire`; body, Fire element and material are fixed, grip/barrel/sight/accessory roll (4,608
combinations). Our part-list merge equals the game's `RuntimePartListCollection` (OpenBLCMM dumps) for 243 of
249 Startup balances; the 6 others differ in data the running game changed. `tools/weapon_stats.py` now
applies the weapon type's own effects and class defaults, divides by negative Scales instead of subtracting
them (FITTED: 4 of 6 observed cards fully reproduced, was 1), and emits projectiles, status chance/damage,
firing mode and card rounding. Still failing: launcher sale value, one Dahl SMG (1 damage point) and the
Bandit slag SMG (damage, magazine). `tools/loot_pools.py` reproduces 2,505 of 2,554 cooked
`ProbabilityDisplayString` shares; the target dummy has no loot in stock data, so the slice uses
`StandardEnemyGunsAndGear`. Selection/roll rules, the scale rule and rounding are native and stay UNVERIFIED.
Reader: additive `--properties-batch` CLI mode. Record: `docs/verification/WEAPON_BALANCE_DECODE.md`.

## 2026-10-01: stock slice world placement, values, Phaselock data, audio chain and NPC assets

AI-assisted. Tooling only; no parsing behaviour change. None of this is checked against the original game.

- World (`tools/prepare_slice_world.py`, record `docs/verification/SLICE_WORLD_PLACEMENT.md`): the GoToRange
  trigger is `WillowWaypoint_9`, a cylinder (357.81 / 145.31) completed by readable `WillowWaypoint.Touch`
  script; Marcus is the placed `WillowAIPawn_13` (not population-spawned, correcting the earlier census) with a
  scripted move-node walk 12 -> 26 -> 35 -> 40 and door Play/Reverse arrival events; the dummy comes from
  `PopulationOpportunityDen_13` at `WillowPopulationPoint_40`; the target Matinee `SeqAct_Interp_0` is driven
  by the dummy's own `MoveTargetForward`/`SendTargetBack` behaviours (the mission's `TargetForward`/`TargetBack`
  events have no Kismet listener); respawn follows the script rule in `GetBestPlayerPlacementPoint`. 20
  structural oracles pass. UNVERIFIED: native navigation, population spawning, Matinee frame, station activation.
- Values (`tools/slice_values.py`): XP reward percentage 0.05 (the amount is native; the candidate formula is
  UNVERIFIED); health 80 x 1.13^L (which of two constants applies is UNVERIFIED).
- Phaselock (`tools/prepare_action_skill.py`, record `docs/verification/PHASELOCK_STOCK_DATA.md`): lift 0.7 s,
  lock attribute 5 x target time scale, fade 1.1 s, cooldown pool 13 s paused while the target is held,
  Suspension +0.5 s per grade. The script was read, never run; the timeline, modifier rule and cooldown
  semantics are UNVERIFIED.
- Audio (`tools/audio_census.py`, `tools/audio_slice_chain.py`, record `docs/verification/SLICE_AUDIO_CHAIN.md`):
  16 slice events resolve to Wwise Vorbis media; containers tile exactly; ids are FNV-1 of the lower-cased
  name (16/16 events, 99/99 banks). Nothing installed decodes the media; a decoder is a new external tool and
  therefore a maintainer decision. No tool was downloaded.
- Assets (`tools/seed_slice_npc_assets.ps1`, record `docs/verification/SLICE_NPC_ASSETS.md`): Marcus, the target
  dummy and the Maliwan pistol candidate fragments extracted with UModel build 1590 and imported locally
  (12 jobs, 0 failed). Textures cross-check against our decoder; meshes are identity-only; materials are
  UModel's texture guess, not verified graphs.

## 2026-10-01: slice host loop uses stock world data

AI-assisted. Host and executor work; no package parsing change. Nothing here is compared against the original game.

- `-owquest` now reads world, NPC and audio data from ignored manifests (`-owslice`, `-ownpcs`, `-owaudio`).
  Marcus is a placed NPC whose walk is started by the installed Kismet; each move-node arrival re-enters the
  same sequence, so the door opens and closes through the installed links rather than host calls. The range
  objective uses the stock waypoint cylinder. The stock dummy spawns at its population point, the map's
  populated events attach it to the target carrier, its own behaviour events play and reverse the target
  Matinee, and the installed Destroy op removes it. Maya's health uses the recovered formula and respawn the
  decoded station selection. Dialog is looked up and logged, never played.
- Executor additions: `Kismet::eventsForOriginator`, `Mover::sequenceEvent` / `originatorEvent` / `output` /
  `advanceSequence` / `variables`, CLI `--kismet-run ... --originator <object-path>` (synthetic test case 13).
- Two host conventions, both UNVERIFIED: Matinee pose = Key(t) x Key(0)^-1 x placed pose (identical to the
  door's formula when the first rotation key is zero; chosen for the target on screenshot evidence only), and
  a door that receives the opposite request while moving turns around from where it is.
- Checks: quest suite 37/37 and resume 7/7, door suite 16/16, CTest 10/10, packages 9/9. Two earlier quest runs
  failed (35/37, 36/37) and are kept in the record. Not verified against the original game: the navmesh first
  leg of Marcus's walk (a straight line here), the dummy's spawn trigger and event order, touch semantics,
  station activation, mesh hit volumes, the dummy's health. Hand play is launched with `tools/run_quest.ps1`.
  Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Host loop with stock world data".

## 2026-10-01: slice player side on stock data

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The lent mission weapon is the recipe matching the mission's own `MissionWeapon` (Maliwan fire pistol), given
  at the Fire objective and shown with the imported Maliwan sample mesh. Each shot hands the held item's
  damage type path to the dummy's `OnTakeDamage` check; the host fire-damage class is removed. A normal-damage
  pistol takes the wrong-element path and has its own check.
- Hand play (`tools/run_quest.ps1`) loads `local/items/slice` with Maya at the slice gear level (8, an
  UNVERIFIED slice choice). First-person arms stay hidden until a weapon is drawn; whether the original shows
  arms when unarmed is UNVERIFIED.
- Turn-in adds the candidate XP amount (0.05 x the experience span at mission level 8 = 396; the native rule
  is UNVERIFIED) to the skills component.
- Phaselock reads `action_skill_siren.json`: lift, lock length, fade, cooldown paused while a target is held,
  miss reset, re-lock penalty, Suspension's bonus. Targeting, the cast gate and target state stay host
  stand-ins; the cooldown model and curve shapes are UNVERIFIED.
- Stock data drops no item for this mission (the dummy has no pools; the reward is XP only). A labelled
  turn-in loot stand-in (first fallback-pool seed that drops a weapon) exercises the pickup path; it is not
  stock behaviour.
- `tools/prepare_weapon_paint.py` accepts material chains without a pattern texture; the pistol decal and the
  shader channel reading are UNVERIFIED.
- Checks: quest suite 57/57 and resume 7/7, door suite 16/16, inventory suite 47 PASS / 2 KNOWN_DIVERGENCE,
  CTest 10/10, packages 9/9. Two earlier failing quest runs are kept in the record. Known gaps: Phaselock can
  lift the target into ceiling beams, XP and skill grades are not saved, health is not recalculated on
  level-up, slice guns have no inventory 3D preview. Record: `docs/verification/SANCTUARY_RPG_MISSION.md`,
  "Player side with stock data".

## 2026-10-01: progression in the quest save; health follows level

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The quest save (`-owquestsave=`) gains an optional `progression` block: level, experience, action grade and skill
  grades (available points are written for checking only; they are derived from level and grades). A save that has
  the block replaces the `-owlevel` start level before weapons are equipped; saves without it load as before. A block
  that fails validation (experience outside its level, unknown skill, grade above its maximum, more points spent than
  earned) is rejected and the quest fails, as for a rejected mission state.
- Maximum health is recomputed with the same formula whenever Maya's level changes. Current health on a level-up is
  read from installed data: `WillowPlayerController.OnExpLevelChange` (script) calls the native
  `RecalculateAttributeInitializedState` and then runs `CharClass_Siren.OnLevelUp` =
  `GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp`, whose skill definition adds `HealthMaxValue` x 1 to
  `HealthCurrentValue` (`MT_PostAdd`). The host refills to the new maximum.
- UNVERIFIED: that the pool caps the sum at the maximum; that the timed effect acts once as a heal; the 1 s guard
  between level-ups (not modelled); the argument order of `OnExpLevelChange` (taken from export order); level
  decreases (test fixtures only, treated the same). The health formula's own open constant is unchanged. The same
  skill definition also scales the action-skill cooldown; the host does not model that.
- Checks: four new first-run checks and three new resume checks (names in the record), a synthetic round trip in
  `OpenWillow.Skills` (not run in this pass). Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Progression,
  Phaselock rules and dummy behaviours".

## 2026-10-01: Phaselock lift, target rule and cast gate from script and data

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `LiftActionSkill.BeginLifting` (read, not run) computes the lift end from the pawn's centre: a trace down
  `HeightFromGround`; on ground the end is ground + collision half height + `HeightFromGround`; a trace up the
  centre's path lowers the end to a surface minus the half height. The host now does the same with a 1 uu box on
  `ECC_Visibility` and its targets' collision bounds standing in for the cylinder (UNVERIFIED). The rule only traces
  the centre's path, and the bob is not clamped.
- Correction to the earlier record: "Phaselock can lift the target into ceiling beams" was a misreading of the
  screenshot. Measured in the lane, the nearest surface is about 460 uu above the target's centre and the lifted
  target's top stays about 160 uu below it; a beam nearer the camera hides the upper part of the view. The suite
  exercises the clamp with a labelled test fixture (an invisible blocking box), because the lane has no low surface.
- `SelectTarget`/`CanPhaseLockTarget` are followed as far as the host has state: alive, not already locked, and a
  host property standing for `Flag_Skills_CanPhaseLock`. A blocked target is not lifted; its damage is not applied
  because the amount is not recovered. The targeting range is the auto-aim data's `MaxTargetDistance`
  (`GD_Autoaim.Default`); the native `GetPreferredTarget` is not reproduced and the view ray plus 30 cm sweep stay a
  host stand-in (UNVERIFIED).
- `Skill_Phaselock.SkillConstraints` evaluators are native. The host maps weapon action to "not reloading", healthy
  to "health above 0" and treats on-foot as always met (UNVERIFIED). While-active constraints, ladders, rider seats,
  friendliness, vehicles and Resurrect are not modelled.
- `tools/prepare_action_skill.py` writes format `openwillow.action_skill/2` (constraint evaluators, the
  `CanLiftTargetIf` flag chain, auto-aim data). A /1 manifest makes Phaselock unavailable with a message; regenerate.

## 2026-10-01: dummy world behaviours run in the host; attach socket from data; dependency fixture kept

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `Behavior_Transform` and `Behavior_RegisterTargetable` are readable script. The first sets
  `WillowAIPawn.TransformType`, whose readable consumer is `GetTargetName` (the balance's per-playthrough transformed
  display name; the lookup itself is native). The second registers the pawn in the native global `TargetableList`.
  The host now runs both for the target dummy: a transform type with its target name, and a targetable flag that no
  host targeting reads yet. IntMath and ChangeInstanceDataSwitch stay logged.
- The `AttachToActor` "bone" `Target` is the holder's `SocketComponent` of that name, not a skeletal bone, so no
  carrier mesh import is needed. `tools/prepare_slice_world.py` emits its pose; the same composition convention
  reproduces the holder's own Base attachment to 0.0 uu (new oracle, 21 of 21 pass). The host puts the dummy's origin
  on the socket at attach. By eye the dummy now stands on the lane floor instead of being sunk to its knees.
- UNVERIFIED: native `SeqAct_AttachToActor.Activated` and `bUseConstructAttachment`; the transformed-name lookup rule
  and the playthrough (1 assumed); which native code reads the targetable list.
- The `GD_Episode03.M_Ep3_CatchARide` dependency cannot be satisfied from installed data: the mission lists it as a
  plain dependency, Sanctuary is reachable while that mission is still active, and the only bulk-completion path
  (the mission fast-forward) is native with a trigger set at run time. It stays a fixture standing in for save state,
  now labelled as such, logged at start and filled from the mission's declared `Dependencies` instead of a
  compiled-in name.
- Checks for the three entries above (one build, this machine): quest suite 73/73 first run and 10/10 resume
  (`run-first-20261001-223529`, `run-resume-20261001-223643`), door suite 16/16, CTest 10/10, packages 9/9. One
  earlier run failed 67/68 (`run-first-20261001-222312`): the new ceiling check found no low surface in the lane.
  Inventory suite on this machine: 45 PASS, 0 FAIL, 2 NOT_RUN (the locally seeded backpack has fewer than nine
  rows), 2 KNOWN_DIVERGENCE.

## 2026-10-01: weapon paint reads Master_Gun's parameter defaults and draws a decal layer

AI-assisted. Tooling and editor importer; no package parsing change; nothing compared against the original game.

- Master_Gun's graph is stripped, but its 43 parameter expressions survive in `Startup.upk` and decode completely
  with the owned reader (`ParameterName`, and `DefaultValue` where it differs from the class default; an absent value
  is taken from the expression class default in `Engine.upk`, and the scalar default, which serializes nothing, is
  read as zero). `tools/prepare_weapon_paint.py --reader/--package` uses them only for scalar and vector parameters
  that no MIC sets and records each value's source. Reader and UModel agree on all 116 MIC parameter values of the
  nine slice MICs; UModel exports nothing for the Material itself. This makes the two Jakobs common chains
  preparable (their B and C zone colours are the base defaults), so all five slice guns and the mission pistol are
  now painted instead of grey.
- The decal parameters are emitted and the importer draws a decal by this reading: placement UV1 x scale + offset
  with the installed texture address modes; weight = zone weights dotted with the mask channels, times decal alpha;
  colour multiplies the zone colours, or replaces them as `p_ReplaceDecal` goes to 1. Evidence is structural only:
  of six candidate placements only this one puts the clamped Hyperion stripe along the gun; the Jakobs decal and
  pattern zone weights are exact complements; two chains' decal colours normalise their decal's mean to about 1.
- UNVERIFIED: the whole paint and decal reading. `p_DecalRotate` and the flip switch are not applied; static
  parameter overrides (undecoded trailing bytes on the MICs) are unknown; the replace mode is inferred from its name.
- Checks: `tests/weapon_paint_test.py` 9 passed; imports ran for six guns ("decal used" on each); quest suite 73/73
  and resume 10/10 afterwards (`run-first-20261001-224649`). Visual, by an independent critic agent that could not
  fetch real first-person images and judged from text descriptions of the skins plus one unrelated local reference
  (so low to medium confidence): Jakobs common pistol 2/10, Maliwan fire pistol 4/10 (2/10 before the decal). Its
  main finding is that both guns show irregular blotchy patches where the real skins have clean zones, so the zone
  mask reading itself is suspect. That is open work, not a result.

## 2026-10-01: analysing the game executable is allowed; transcribed code and decompiler output stay out of the repository

Maintainer decision. AI-assisted drafting. Supersedes the rule, in earlier entries and in LEGAL.md, README and the
plan, that forbade disassembling or decompiling `Borderlands2.exe`.

- **Why:** the slice's hard remainder is native code (mission tracker, behavior kernel, auto-aim, constraint
  evaluators, experience and loot rules, Gearbox's natives, stock UE3 natives). The packages and the script do not
  describe it, and recovering each rule by capturing the running game is slow. The maintainer's priority is to get the
  port done as fast as possible. My rough estimate (not measured) is a week or two saved on the current slice and
  months across Phases 2 to 4; implementation, assets, the host and verification are unaffected.
- **What is allowed:** disassembling and decompiling the executable and its DLLs locally, scripting that analysis, naming
  functions and recovering structures, by anyone working on the project including AI assistants.
- **What is not allowed in the repository, unchanged in kind:** game files and game-derived data (now explicitly including
  decompiler output, recovered headers and analysis databases), leaked Gearbox/2K/Epic/UE3 source, any code transcribed,
  translated or mechanically converted from decompiler output. Project code is written from a behaviour note in our own
  words. Rules read from native code and not confirmed by running the game stay `UNVERIFIED`.
- **Maintainer's assessment and accepted risk:** the maintainer's view is that the rights holder is unlikely to object to a
  non-commercial port that does not distribute the game. That is an assessment, not a legal conclusion. Code derived from
  analysing a binary remains derived from it, and rights holders have removed projects that published such code. The policy
  keeps the published repository to original code and behaviour notes, and the maintainer accepts the remaining risk. The
  decompile-and-publish option was considered and not chosen.
- **Two machines:** the maintainer works on two PCs. Regenerable data is regenerated from the installed game; behaviour
  notes and code are committed; non-regenerable game-derived files (analysis databases, hand patches) move through a store
  outside the repository with `tools/private_sync.ps1` (`docs/NATIVE_ANALYSIS.md`). A private GitHub repository is possible
  but is still a copy of game-derived material on a third party's servers; that is the maintainer's call.
- **Changes:** `docs/LEGAL.md` (rules, new "Analysing the executable", contributor certification), new
  `docs/NATIVE_ANALYSIS.md`, README, CLAUDE.md, AGENTS.md, ROADMAP method note, CONTRIBUTING, CODE_OF_CONDUCT, PR and issue
  templates, the plan documents; `.claude/hooks/sensitive_guard.py` and `.gitignore` now also refuse decompiler/disassembler
  database file types; `tools/private_sync.ps1` added.
- **Follow-up, same day (maintainer decisions):** Ghidra 12.1.4 with a portable Temurin JDK 21 is the chosen tool and has
  a `THIRD_PARTY.md` entry (checksums verified against the published ones); `tools/ghidra_import.ps1` imports and
  analyses the executable headlessly. The private store is the private GitHub repository `zuhuHix/BL2_ReBased-private`.
  All time-to-completion estimates were removed from ROADMAP.md, README.md and the plan documents; ROADMAP.md now has a
  "How it's going" section stating what was done in what elapsed time, with no forecast.
- **Still open:** no native function has been analysed yet, and no analysis result is confirmed against the game.

## 2026-10-02: weapon paint from Master_Gun's compiled shader data; MIC static parameters decoded

AI-assisted. Tooling, editor importer and two read-only CLI modes (`src/cli.cpp`: `--payload-file <index> <out>`,
`--names`); no change to `src/package.cpp`, the container code or `CMakeLists.txt`, no bounds check changed. Nothing
compared against the running game.

- **Static parameters.** The bytes after a MaterialInstanceConstant's properties are its fully resolved static
  parameter set; `tools/material_static_parameters.py` decodes them exactly (sizes meet) in 631 of 631 MICs in
  `Startup.upk`. On the slice chains they only pick channels: `p_WeapClassSelect` the detail atlas channel (B for
  pistols, matching the earlier guess), `p_PatternChannel` and `p_DecalChannel` the pattern and decal channels;
  `sw_FlipDecalOnRightSide` is off. The hypothesis that undecoded static overrides caused the blotches is refuted.
- **Shader data.** `RefShaderCache-PC-D3D-SM3.upk` keeps, per static permutation of Master_Gun, the uniform expression
  set (which parameter feeds which constant and sampler) and the compiled ps_3_0 shaders. A small token reader written
  from Microsoft's public D3D9 bytecode description (`research/d3d9_bytecode.py`; its listings are game-derived and stay
  under `local/`) and a diff of three permutations give the colour model described in our own words in
  `tools/weapon_paint_model.py`: `p_Masks` holds two stacked maps (lower half zone mask, upper half highlight/shadow
  map); zone tones go Midtone -> Hilight -> Shadow and are blended over `p_DColor`; pattern and decal multiply or
  replace by squared mask weights; the decal UV is shifted, rotated by `p_DecalRotate` x pi and scaled about the
  centre; the result is multiplied by the selected detail channel.
- **The bug:** the previous reading sampled `p_Masks` over its full height, so the light/dark map became zone weights
  (the camouflage blotches). Also fixed: detail used as a tone selector, `p_DColor` and the two intensities ignored,
  decal placement and rotation, the single-channel decal path.
- UNVERIFIED: the whole reading until compared with the running game; `DISPLAY_SCALE` (0.4) chosen by eye; the
  environment reflection (`P_SimpleReflect`), emissive, digistruct and lighting are not modelled; the material
  resource words before the static parameters are not interpreted.
- Checks: `tests/weapon_paint_test.py` 25 passed (invented values), CTest 10/10, packages 9/9, six guns re-imported
  with 0 errors, quest suite 73/73 and resume 10/10 afterwards (`run-first-20261002-005221`). Visual: an independent
  critic agent compared host stills with in-game inspect screenshots from the Borderlands wiki (kept under ignored
  `local/paint_research/ref_online/` with sources): Maliwan uncommon pistol **6.5/10** (was 4), Jakobs common pistol
  **4.5/10** (was 2). Its remaining findings: a cool blue cast on bare metal and on the Jakobs wood (wood reads grey,
  not brown), the Maliwan barrel looks painted rather than chrome (no reflection term), orange slightly too wide on
  the Maliwan grip. No real-game capture yet (the game would not launch under the logged-in Steam account).

## 2026-10-02: weapon paint pass 2: texture colour space from data, reflection term; albedo checked against screenshots

AI-assisted. Tooling and editor importer only; nothing compared against the running game.

- Textures default to sRGB (`Default__Texture`); only `p_Masks` and the normal maps switch it off. The detail atlas was
  imported as linear, which washed out grime and rust. The preparer now reads each texture's SRGB flag and the
  importer and thumbnail renderer follow it. The by-eye `DISPLAY_SCALE` is removed; the shader colour is used unscaled
  (clamped to 1, keeping hue).
- The environment term (`P_SimpleReflect`, `p_ReflectColor`, `p_ReflectionChannelScale`, `p_ReflectColorScale`) is
  drawn as the compiled shader combines it: it brightens surfaces that are already bright and cannot turn a dark base
  silver.
- Re-checked from the compiled shader: the shadow amount is the clamped product of the light/dark map's green and
  `p_ShadowsIntensity`, moving the tone from Midtone toward Shadow; the zone mask is the lower half of `p_Masks`
  (97% of its texels are flat, against 74% in the upper half).
- Numeric check (unlit model albedo against in-game wiki inspect screenshots, median sRGB of matching regions; the
  references include the game's lighting): the Jakobs wood and frame match closely; the Maliwan barrel has the same tint and is about 40% darker (reflection and
  lighting omitted in the render). So the blue cast seen in first person most likely comes from the host's lighting
  and its fixed metallic 0.35 / roughness 0.55, not from the paint formula (UNVERIFIED).
- Checks: `tests/weapon_paint_test.py` 29 passed, six guns re-imported with 0 errors, quest suite 73/73 and resume 10/10
  (`run-first-20261002-010701`). Critic (in-game stills and thumbnails vs wiki screenshots): Maliwan 6.5/10 (same),
  Jakobs 5.0/10 (was 4.5).

## 2026-10-02: first native analysis: registration tables, query tooling, mission and behavior dispatch notes

AI-assisted. Tooling and behaviour notes only; no executor or parsing change yet. Policy: LEGAL.md "Analysing the
executable". Raw output stays in the ignored analysis folder; nothing here is listing or address.

- **Machinery** (`tools/ghidra/`, method in `docs/NATIVE_ANALYSIS.md` "Native registration and queries"): each native
  class has a table of name/function pairs (`<Class>exec<Function>`); fixed script native numbers bind by name. The
  table scan finds 6,877 natives in 770 tables, and all 199 numbered script natives resolve. A batch query names and
  decompiles functions by registered name, native number, string, callers or virtual slot (virtual natives such as
  `Behavior_*.ApplyBehaviorToContext` share one exec function and are reached through the class's virtual table).
  `tools/ghidra/class_layout.py` computes 32-bit field offsets from the packages (oracle: `Core.Object` 0x3C).
  About 15 s per run.
- **Behaviour notes** (`docs/verification/NATIVE_MISSION_DISPATCH.md`, all UNVERIFIED, read from native code, not
  confirmed in the game):
  - Behavior link id byte: a signed occasion selector; the event caller passes a filter (-1 = all links); a behavior
    chooses outputs by id, and the default output is followed only when `bSupportsDefaultOutputLink` is set. There
    is no once-per-event deduplication (a behavior reached by two links runs twice); events honour `bEnabled`,
    `MaxTriggerCount` and `ReTriggerDelay`; threads run depth-first.
  - Kismet: ops run from a stack, at most 1,000 per frame; a link's delay is the input's plus the output's; an input
    hit twice runs the op twice; an activated event fires all its outputs.
  - MissionTracker: updates are queued; an objective completes when progress reaches `ObjectiveCount`; a completed
    set makes the mission ready to turn in when `bCanCompleteMission`, else activates the next set when
    `bAutoEnableNextSet`, else waits for a behavior; `AdvanceObjectiveSet` only moves to `NextSet`. Mission event link
    ids: a census over 133 missions puts all 3,420 links in the predicted id ranges (structural only).
  - Consequence for the slice: the host fires every `Default` link on accept, which is why `TargetBack` appears 3 s
    after accepting; natively the first set is activated by the kickoff dialog's Finished output.
- Not yet implemented in `src/`; the host behaviour is unchanged. Checks: CTest 10/10, packages 9/9.

## 2026-10-02: weapon paint pass 3: shading inputs read from the shader, kept behind a flag

AI-assisted. Importer and notes only. The compiled base and light passes of Master_Gun multiply the material colour
by 0.4 (a factor most other shaders in the cache do not have) and take no specular from the material (the light
pass's specular is the engine override only). Mapped to UE5 that is base colour 0.4 x colour, metallic 0, specular
0, roughness 1 (`USE_SHADER_SHADING` in `host/ue5/import_weapon_paint.py`, UNVERIFIED). With the host's uncalibrated
Sanctuary lighting those inputs render the guns 4-7x darker than the reference screenshots, and the red-down/blue-up
tint measured on the stills (about x0.8-0.9 red, x1.1-1.27 blue against the unlit albedo) is unchanged by them, so the
tint comes from the host scene lighting. Maintainer-facing choice made by the orchestrator: the importer keeps the
pass-2 host stand-in (scale 1, metallic 0.35, roughness 0.55) until the scene lighting is calibrated; the six guns were
re-imported with it (0 errors).

## 2026-10-02: native dispatch rules implemented in the behavior, Kismet and mission executors (UNVERIFIED)

AI-assisted. Executor change; the rules are those of `docs/verification/NATIVE_MISSION_DISPATCH.md` (read from native
code, written in our own words; nothing here comes from a listing). **Every rule below is UNVERIFIED against the running
game**: synthetic tests check that the executors do what the note says, not that the note is right.

- **Behavior kernel** (`src/behavior.*`): the link id byte is read signed; `fireEvent` takes a link-id filter (-1 = all);
  every matching event entry is gated by `bEnabled`, `MaxTriggerCount` and `ReTriggerDelay` with per-process
  `TriggerCount`/`LastTriggerTime`; threads run depth-first (the first selected link continues the thread, the others
  start first); handlers return the recorded output ids (duplicates kept) and -1 is added only when
  `Context.bSupportsDefaultOutputLink` is set (read with class defaults: true on most behaviors, false on
  TriggerDialogEvent/CompareObject); the once-per-event deduplication is gone; at most 60 behaviors per thread per call
  (a capped thread resumes on the next tick: what the game does was not read). Not modelled: `FilterObject`, latent
  behaviors, the "sequence still enabled" condition on running threads.
- **Kismet** (`src/kismet.*`): queued ops form a stack (the first link's target runs next; an op already queued keeps
  its place); link delay = target input's `ActivateDelay` + output's; disabled inputs receive nothing; an input hit
  twice runs its op twice; an activated event fires all its outputs; at most 1,000 ops per `run()` (was 10,000 impulses
  over the instance's life). Not modelled: one op seeing several inputs at once, event `MaxTriggerCount` /
  `ReTriggerDelay`.
- **Mission tracker** (`src/mission.*`): mission events carry the note's link ids (status change `Default` 6 + status:
  accept 7, ReadyToTurnIn 9, Complete 10; kickoff 12/13; set activated 4 / completed 5; objective progress 3 and
  completed 2 after the set check; custom events 0, refused only when Complete). Accept no longer fires every `Default`
  link. `UpdateObjective` is one queued +1 (or a bit OR-ed in; the count of a bit mask is its number of set bits, a
  guess for `TranslateObjectiveCount`); completion at `ObjectiveCount`. Set completion: `bCanCompleteMission` ->
  ReadyToTurnIn, else `bAutoEnableNextSet` -> next set, else wait; `AdvanceObjectiveSet` only to the active set's
  `NextSet` (or the initial set while none is active); `bActivateInitialObjectiveSet`; `ObjectiveDependency` in
  `available()`. Status transitions ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn. Not modelled:
  RequiredObjectivesComplete, Failed, `bRepeatable`, collection/branching sets (an error if activated), blocking sets,
  the level-load replay, the mission weapon at status Active/Complete (still lent with its objective, because the
  host's checks expect that and `IsValidMissionWeapon` was not read).
- **Host stand-ins** (`src/slice.cpp`, `src/mission.cpp`): the slice plays the kickoff (`Default` id 12) right after
  acceptance (what does this in the game is unknown), and a `Behavior_TriggerDialogEvent` selects both its outputs at
  once (Out, then Finished) because no dialog is played. CLI: `--mission-run` gains `kickoff` and `obj:<name>:<bit>`;
  `--behavior-run` gains `fire:<id>:<event>`; `--kismet-run` gains trailing `--tick <s>` steps.
- **Real data (local only)**: the Fire mission now runs as the note predicts in its section B9: no set until the
  kickoff dialog finishes, `RocksPaper_FinalObj` 0.5 s after `RockPaper_GoToRange`, `Targetable` 1 s later, no
  `RocksPaper_TargetBack` after accepting, two `TargetBack` 3 s after `Fire`, Active -> ReadyToTurnIn directly, no
  errors. Its dialog lines are the same set as before. All 28 events of the range's Kismet sequence reach the same
  host ops in the same order as before. Link-id census: 0 links outside the predicted sets.
- Checks: CTest 10/10 (new: behavior acceptance tests 1-6, Kismet test 7; changed expectations follow the stack
  order and the per-frame cap), packages 9/9. The quest suite's step 7 (`OpenWillowQuest.cpp`) now waits for the
  FinalObj set (0.5 s after the touch) instead of the objective's completion; with that change, on a clean rebuild of
  `ow-core` and the UE module: quest 73/73 first run and 10/10 resume, mover 16/16, inventory actions 45 PASS / 0 FAIL /
  2 NOT_RUN / 2 KNOWN_DIVERGENCE (unchanged baseline).

## 2026-10-02: inventory open time measured and the one-time work moved to level start

AI-assisted. Host change, no parsing change. Measured on one PC (1280x720, `-d3d11`), key press to the page showing
the open inventory: first open with the page loaded 443 ms (358-488, 4 launches) -> 234 ms (213-252, 3 launches);
repeat opens about 60 ms (unchanged); first preview of each starting weapon about 32 ms with spikes up to 1036 ms ->
about 1 ms. The cost was the inventory VM build (about 120 ms) and Maya's menu meshes (45-115 ms) in the open frame,
plus each weapon mesh loading on first preview; these now preload at level start (about 140-270 ms added there;
`-owinvnopreload` turns it off for comparisons; `-owinvopenbench` and `OWINVTIME` log lines measure it). Not fixed:
pressing open in the first ~5 s of play still takes about 5.5 s, which is Ruffle booting the converted movie and its
shared libraries (two fetched more than once); a 60 Hz page frame rate and HTTP caching made no measurable difference
and were reverted. Also: page hover selects a cell only when the pointer actually moves (UE resent an unchanged position
about 500 times per open, which re-selected an equipped cell and broke the suite), and the quest save is written only
when its text changes. No real-game open time exists yet, so nothing here is compared with the game. Details:
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (2026-10-02 section).

## 2026-10-02: native progression and Phaselock targeting notes (UNVERIFIED)

AI-assisted. Notes only, no code change apart from `tools/ghidra/class_layout.py` sizing Gearbox attribute properties
(it stopped at the first one before). Read locally from `Borderlands2.exe` in Ghidra and from script, written in our
own words; every rule is UNVERIFIED and each section of the notes names the in-game check that would confirm it.

- `docs/verification/NATIVE_PROGRESSION.md`: balance formulas evaluate `Multiplier x (Level^Power + Offset)`
  (`tools/weapon_recipe.py` and `tools/weapon_stats.py` put the offset outside; no slice number changes, a census is
  pending); mission XP is truncated, not rounded (395 at level 8 where the host gives 396) and the mission level is
  Sanctuary's region game stage (clamp(player level, 7, 9) on playthrough 1); the level curve is
  `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)` (it reproduces the one real-game threshold on record, 2,715,586 at level
  46; the host is one point low at most levels), level cap 50; skill points `max(0, L - 4)` match the host; max health
  `max(20, 80 x 1.13^L)` matches the host (the 94 constant is never used).
- `docs/verification/NATIVE_PHASELOCK_TARGETING.md`: Phaselock takes the auto-aim strategy's instantaneous best
  target (screen-space magnetism cone, score favouring the crosshair then distance, line of sight to the aim point),
  where the host uses a view ray and a 30 cm sweep; reloading does not block the cast but putting a weapon away does;
  an injured Maya cannot cast and going down ends the lock; the lift bob is timed from the cast and smoothed by
  `VInterpTo` at speed 1 (about 16 units visible, not 30).

## 2026-10-02: progression rules from the native notes in the tools and the host (UNVERIFIED in game)

AI-assisted. Implements `docs/verification/NATIVE_PROGRESSION.md` sections 1-4. Those rules were read from native code,
and none has been confirmed by running the game. Package parsing is unchanged: `src/` and `CMakeLists.txt` were not
touched.

- **Formula order.** `tools/weapon_recipe.py` (`formula_value`), `tools/weapon_stats.py` and `tools/loot_pools.py` now
  evaluate `Multiplier x (Level^Power + Offset)`. Before, they added the offset outside the multiplier.
  - A census of the base-game packages (local only) found 228 distinct enabled formulas. 19 of them have both a
    non-zero Offset and a Multiplier other than 1: enemy health and damage, enemy and world-discovery XP, melee damage,
    class-mod bonuses, three Soldier/Mercenary skill formulas, vehicle damage and the XP curve. DLC packages were not
    included.
  - No slice number changes. The slice gear recipes and manifest are byte-identical before and after. The loot display
    check is unchanged (2,505 of 2,554 agree). Maya's health formula has no offset, and the XP curve's offset cancels
    in the reward span.
- **Level curve and cap.** `UOpenWillowSkills::ExperienceForLevel` is now `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)`,
  evaluated in single precision. It was `floor(60 n^2.8 - 60)`.
  - Changed thresholds: level 2 357 -> 358, level 5 5,375 -> 5,376, level 8 20,207 -> 20,208, level 9 28,125 -> 28,126.
    Level 46 stays 2,715,586, the one real-game threshold on record.
  - The level cap of 50 applies in `AddExperience`, `SetLevel` and save restore. DLC cap increments are a TODO.
  - Float and double evaluation differ by one point at levels 17, 22, 33, 42, 45, 47 and 49. Which value the game
    gives at those levels is not known.
- **Mission XP.** `MissionXp` is now `trunc(span x percentage)` on the integer curve. Before, it rounded a span computed
  on unrounded doubles.
  - Changed amounts: stage 8 396 -> 395, 9 484 -> 483, 10 579 -> 578, 11 682 -> 681. Stage 7 stays 316.
  - The mission level is now Sanctuary's region game stage, not the slice gear level. `tools/slice_values.py` decodes
    the playthrough-1 `RegionBalanceData` entry into `world.json` `values.xp.region_stage` (default 7..9,
    WelcomeToSanctuary 8..11, later overrides).
  - The host fixes the stage as `clamp(level + boost, min, max)`, using the largest completed override or else the
    default. It does this when the walker sets Maya's level at session start and keeps the value in the quest save.
  - A manifest without the block gets a logged STAND-IN with the note's bounds.
  - The suite's Maya starts at level 8, so the stage is 8 and the reward is 395.
- **Max health** already matched the note. `SLICE_WORLD_PLACEMENT.md` 2b now records that the 94 constant is never
  used (native reading).
- **Checks.**
  - CTest 10/10 and packages 9/9 pass.
  - Synthetic Python tests pass: weapon_recipe 8, weapon_stats 19, loot_pools 9, slice_world 17.
  - The `OpenWillow.Skills` automation test passes (curve values, synthetic order and truncation cases, cap).
  - Quest suite: 75/75 first run and 11/11 resume. New checks are `mission_level_is_region_stage_fixed_at_start`,
    `level_curve_matches_tool_integer_curve` and `resume_region_stage_from_save` (the stored stage 8 is kept where a
    fresh computation at the resumed level 11 would give 9). `xp_amount_is_candidate_formula_at_mission_level` is
    renamed `xp_amount_is_truncated_rule_at_region_stage`.
  - Door suite 16/16.
  - Every rule above is still unverified in game. The note lists the confirmations: turn-in XP 395 at stage 8,
    "next level at" 358 at level 2, and the first skill point at level 5.

## 2026-10-02: ParticleSystem template reader (research)

AI-assisted. Research prototype only (`research/particle_system.py`, `tests/particle_system_test.py`, 16 synthetic
tests; not registered in CTest). Templates are delta-serialized against their archetype chain; baked distribution
tables are read as two range values plus entries of `ChunkSize` floats; `BurstList` and dynamic parameters are tagged
structs. All 17,506 non-empty baked tables in four packages fit that layout (structural oracle). Curve sampling between
table entries is fitted, UNVERIFIED. Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Particle template reader".

## 2026-10-02: Phaselock stock presentation and targeting in the host (work in progress, no suite run)

AI-assisted. Host and tooling; no parsing change. Written but **not verified by any suite**: after the module was
rebuilt at 09:18, Windows Application Control (Smart App Control) blocked `UnrealEditor-OpenWillow.dll`
(`GetLastError=4551`, Code Integrity events 3033/3077). Per project rules nothing was done to get around it; UE work
stops until the maintainer clears it.

- Presentation (`OpenWillowPhaselockFx.*`, `OpenWillowCombatTarget.*`, `OpenWillowWalker.*`): the decoded emitter
  templates are played with plane/mesh components (no Niagara, no new module dependency). From data: hand orb at the
  0.25 s notify on `L_Weapon_Bone` with the socket offset and scale 0.35; bubble scaled by bounds radius / 66.7, 0.2 s
  intro, collapse 0 -> 0.75 over the last 2 s; point light radius 500, brightness 4, a blue-violet colour; tattoo glow
  curve over 1 s. Host stand-ins (UNVERIFIED): what each stripped material does with its textures, the tattoo mask
  channel, the screen effect as a full-view quad, UE3 brightness -> UE5 intensity, burst timing, the dummy's auto-aim
  radius/aim point. The dummy keeps its idle (it has no PhaseLock clips).
- Rules from `NATIVE_PHASELOCK_TARGETING.md` (UNVERIFIED): screen-space magnetism target choice replaces the view
  ray and 30 cm sweep; reloading no longer refuses the cast, a holstered weapon does; going down ends the lock; the
  bob is timed from the cast and smoothed.
- Tooling: `tools/prepare_phaselock_fx.py` (manifest from installed data), `host/ue5/import_phaselock_fx.py` with
  `tools/import_phaselock_fx.ps1` (textures and meshes from UModel output under `local/`), `tools/run_phaselock_shots.ps1`.
- One in-engine capture run (before the block) showed the lift, the smoothed bob (about +/-16 uu), the collapse
  reaching 0.749 and the light ramp; defects seen: an overexposed white-pink core where web screenshots show a
  violet sphere with a dark core, a large violet light pool, loop sprites not showing, and first-use texture
  compilation delaying the hand orb to +0.53 s. Fixes for some of these are written and not run.
- Quest suite: check 65's reload refusal is replaced by "reload does not refuse" and "holstered refuses"; new checks
  for the presentation, an off-crosshair target and going down. Not run. Expected totals once it can run: 79 first-run
  checks (75 baseline + 4) and 11 resume checks (unchanged).
- Resumed later the same day under the maintainer's one-rebuild rule. A normal rebuild with no source change did not
  relink (the 09:18 DLL stayed, still blocked). After the one requested source change (the stale targeting comment in
  `OpenWillowQuest.h`), the module was relinked at 10:01 and Smart App Control blocked that DLL as well
  (`GetLastError=4551`, Code Integrity events 3033/3077/3118 at 10:01:40). UE work stopped again; nothing was done to
  get around the block.

## 2026-10-02: weapon generation rules read from native code; card audit 9 of 9 with runtime data (UNVERIFIED in game)

AI-assisted. Tools and notes only: no host C++, no `src/`, no `CMakeLists.txt` change; package parsing is unchanged.
Read locally from `Borderlands2.exe` in Ghidra and from script, written in our own words in
`docs/verification/NATIVE_WEAPON_RULES.md`; every rule is UNVERIFIED in game and each section names its confirmation.

- **Attribute stack (settles the fitted rule).** The game sums PreAdd, PostAdd, positive and non-positive Scales in
  single precision and computes `(base + PreAdd) * (1 + up) / (1 - down) + PostAdd`, with no clamp; integer stats (clip,
  projectiles, shot cost, burst count) truncate. The 2026-10-01 "split" rule was right; its clamp at 0 was not. Enum
  orders and class defaults were decoded from the packages.
- **Effect order** (script): type, parts in slot order, attribute slots (activated ones; grades count every increase),
  then prefix and title. **Card rounding** comes from the presentation data (damage up, clip down, the rest half up to
  `FloatPrecision`); single precision decides the damage ceiling on one observed launcher.
- **Part pick.** An entry without a `Manufacturers` list weighs a flat 100 (1,572 of 2,473 weapon entries); stage
  windows use truncated bounds; zero weights are dropped, a duplicate keeps its later weight, and a slot with nothing
  left stays empty (not a uniform pick). **Names** are deterministic (type lists first, then parts in slot order, highest
  priority, later wins ties; class defaults priority 1, level window 1..100). **Level** = the spawn game stage
  (`bInterpolateExpLevel` default true). **Rarity** = sum of truncated part rarities looked up in
  `RarityLevelColors` (was: max). **Value**: the prefix's `MonetaryValueMod` is in the part product; this was the
  launcher value gap (inferred from script order and data; the value function itself was not resolved).
- **Runtime data.** The observed cards were captured with Gearbox hotfixes active; OpenBLCMM's dumps of the running game
  show 39 weapon objects with changed stat data, which explain the four hotfixed legendaries. `weapon_card_audit.py
  --runtime-overlay` reads them locally as an oracle input; whether the port applies hotfix data is a maintainer decision.
- **Card audit** (9 distinct cards in this machine's 2026-09-26 traces; the record's 6-card set is not on this PC):
  before (HEAD) 4 of 9 cards on the main four stats and on every printed field; HEAD rules with runtime data 8 / 7 of 9;
  read rules on cooked data 5 of 9; read rules with runtime data **9 of 9 on every printed field, name included**.
  Ablation: without single precision 8 of 9; without name parts 7 of 9 numerically; adding the slot base grade 0 of 9.
- **Changes:** `tools/weapon_recipe.py` (evaluator order and precision, `entry_weight`, `pick`,
  `choose_name_parts`), `tools/weapon_stats.py` (`combine`, effect order, slots, `rarity_of`, `present`/`display`, name
  parts in the value, launchers' calculator checked; new card fields `rarity_level`, `rarity_rating`, `rarity_color`),
  `tools/weapon_card_audit.py` (name parts and `name` check, `--runtime-overlay`, every-field counts),
  `tools/weapon_balance.py` (docstring), tests, `WEAPON_BALANCE_DECODE.md` pointer. Regenerating `local/items/slice`
  changes the slice guns' parts and names (new pick sampler and name rule); it was not regenerated in place.
- **Checks:** weapon_recipe 14, weapon_stats 25, weapon_balance 7, loot_pools 9, weapon_paint, skill_stats, slice_world
  and golden_card_compare tests pass; CTest and packages in the lane report. Not done: any in-game check, the host
  changes listed in the lane report (magazine and shot-cost truncation, HUD card rounding, E-tech colour).

## 2026-10-02: real-game ground truth: driver tooling and the first capture session

AI-assisted. Tooling (`tools/real_game/`) and records; no change to `src/`, `CMakeLists.txt`, the reader or the
host. The maintainer allowed unattended launches of the installed game for captures (orchestrator brief, 2026-10-02).
Details and method: `docs/verification/REALGAME_GROUND_TRUTH.md`; everything recorded stays under ignored
`local/realgame/`.

- **Tooling.** `tools/real_game/realgame.ps1` (run lock shared with UE runs, save backup, launch, window capture,
  scan-code keys including arrows, click/wheel/drag, QPC-stamped burst capture, `Invoke-GamePy[File]`) and
  `openwillow_realgame`, our own Library mod for the community SDK that runs command files on the game thread, with
  `block_saves()`. Command scripts: `scripts/weapon_cards.py` (spawn by balance or exact definition, weapon record,
  card trace in the uitrace row format), `scripts/phaselock.py` (per-frame lift-skill sampler, cast and weapon-call
  marks, damage immunity, `face`). `golden_cards.py` joins records, card trace and screenshots;
  `golden_card_compare.py` (written by a subagent, reviewed) evaluates `tools/weapon_stats.py` on the exact rolled
  parts; `tests/golden_card_compare_test.py` 12 synthetic tests. Smoke test: the channel answered at the main menu
  15 s after launch and the driver was removed afterwards.
- **Saves.** Backed up first; all 22 save files are byte-identical to the backup after the session (four files the
  game had written at start, character selection and one load, while the first hook version was broken, were restored
  from it). One game crash came from a command reusing an invalidated weapon reference; nothing was written.
- **Confirmed in game** (each with how; the notes' other rules stay `UNVERIFIED`):
  - level curve: `GetExpPointsRequiredForLevel` for 1-80 matches the single-precision formula at every level 1-59;
  - "next level at" and skill points `max(0, L − 4)` at levels 2, 8, 17, 70;
  - max health base `80 × 1.13^L` (health pool base value) at levels 2, 8, 17, 70; the HUD adds the Badass Rank;
  - Fire mission XP 395 at stage 8 (`MissionDefinition.GetExperienceReward`);
  - Phaselock bob: the note's sine-from-cast plus `VInterpTo` speed 1 reproduces a lifted enemy over 435 frames to
    0.001 units RMS; a cast during a manual reload starts and aborts the reload; a cast during a swap put-down is
    refused (2/2, with controls).
- **Observed, for the lanes:**
  - Phaselock presentation timings (hand orb ≈ 0.45 s after the key, not 0.25 s; dark-cored violet bubble; cast
    vignette, target burst and cyan release ring that the host lacks); skill 5.7 s = 0.7 + 3.9 + 1.1 at level 8.
  - Weapon cards (69, exact parts): every printed stat matches on 53/69 with the evaluator before `bb2a444` and 52/69 after it (fire rate fixed; one 1.75 reload now prints 1.8, the game 1.7); damage prints rounded up;
    the live weapon-type objects differ from the cooked `Startup.upk` decode (Bandit pistol magazine 36 vs 30, Dahl
    pistol 16 vs 12, Bandit shotgun 10 vs 9 and reload 4.1 vs 4.4) and no live hotfix entry touches them (source
    open); single precision fixes the 1.25 fire-rate display; the game falls back to the type's title; no level line
    at stage 1 or on mission weapons.
  - Paint: an independent critic agent scored the host 4.5/10 (Maliwan) and 3/10 (Jakobs) against real inspect
    captures (earlier 6.5 and 5.0 were against wiki screenshots): too dark even unlit, wrong Maliwan orange hue,
    no element glow.
  - Inventory open in the original game: ~126-156 ms to the first page frame (screen capture, upper bound).
- **Not done:** downed/injured Phaselock checks, the auto-aim radius thresholds, a real mission turn-in, the
  level-54 save. No UE suite was run (Smart App Control blocks the module DLL; no rebuild attempted).

## 2026-10-03: Phaselock presentation compared with the game: shader warm-up, capture clock, dark bubble core

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Smart App Control was
off and the module built and loaded. The host's Sanctuary dummy was compared with the 2026-10-02 game captures (a
bullymong in Three Horns). Details, causes and the remaining differences are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Host presentation pass, second round". Frames and the side-by-side stay under ignored `local/phaselock/`.

- **Invisible hold bubble: a first-draw shader compile, not the effect data.** Every import recreates the host
  materials and compiles nothing (`-nullrhi`), so the next game run compiled them on first draw and skipped the
  translucent quads for about 2.5 s. An unchanged rerun drew the bubble on time. `FOwFxTemplate::Preload` now loads all
  Phaselock materials and meshes when the manifest loads and compiles the parent materials synchronously in editor
  builds. The first run after a fresh import then drew every effect on time (checked once, 17:55 run).
- **Capture clock.** Screenshot stalls (0.36 s, then about 0.13 s per shot) had moved the labelled shots late (the
  "0.25 s" frame was at +0.52 s). That made the arm look twice as fast as in the game and the hand orb look absent.
  `-owphaselockshots` now caps game time at 1/60 s per frame and logs each shot's actual time. With that, the arm
  timing matches the game to the eye. The stock data gives the cast clip `PlayRate` 1 (no `RateScale`); the caller's
  `SpecialMoveData` was not resolved.
- **Emitter playback (UE3 conventions, UNVERIFIED here):** spawn-time distributions are read at the emitter's time in its
  loop, and a linear sub-image layout follows the SubUV module's `SubImageIndex`. The second change removed the black
  wedge at the collapse: the end smoke now runs its frames 15 -> 0 instead of 0 -> 15.
- **Host stand-ins, chosen against the capture (UNVERIFIED):** all host FX parents render before DOF. The darkening
  modulates (`Mat_SirenOrbBlackMOD`, `_NoBias`, `Mat_SirenOrbEnergySpikesMOD`) are drawn as translucent black, because
  UE5 applies modulate apart from the additive layer. The textureless black orb uses a disc mask. `Mat_SirenGlowMOD`
  weights by alpha. The screen particle is a modulate by the colour's hue. The lock light reaches only the target,
  although its data (`LAC_DYNAMIC_AND_STATIC_AFFECTING`) says it lights the floor; the capture shows no pool. The
  hold now reads as a near-black core with a violet rim, the release as a cyan-white ring that shrinks, and
  1.2-1.5 s as a blue screen tint.
- **Open:** the game's dark blob around the raised hand at about 0.27 s and its solid blue palm orb (the host shows
  flashes and swirls); the 0.8 s intro burst draws as a white band; the screen tint starts at 1.05 s, against about
  0.82 s in game (a burst `Time` in seconds would fit; not changed); what `SphereCollapse` drives in the stripped graph.
- Also: the `OpenWillowPhaselock.h` lock-duration comment now cites the modifier stack read from native code instead
  of the old fitted rule; FX logs print mesh-particle scales with decimals and each quad's blend, sort priority and
  visibility.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the game captures only; no new
  game capture was made.

## 2026-10-03: host card rounding and integer truncation follow the native weapon rules (UNVERIFIED in game)

AI-assisted (Claude). Host and page only; no change to `src/`, `CMakeLists.txt`, package parsing or `tools/weapon_stats.py`.
This closes the host items the 2026-10-02 weapon-rules entry left open. The rules come from
`docs/verification/NATIVE_WEAPON_RULES.md` sections 1 and 2, which were read from native code and presentation data. The
only in-game evidence is the golden-card set in `REALGAME_GROUND_TRUTH.md`. Those cards show damage rounded up and
accuracy printed with one decimal and no `%`. They do not test this host code.

- **Truncation.** `UOpenWillowInventory::MagazineSize` and `ShotCostRounds` now truncate toward zero instead of rounding to
  nearest, because `ClipSize` and `ShotCost` are integer attributes. The slice recipes were evaluated before that rule
  and still carry fractional magazines (10.5 and 13.8 now give 10 and 13, not 11 and 14). The magazine's floor of one
  round is a host guard, not a game rule. A shot cost below 1 now truncates to 0, meaning no ammo cost; no local recipe
  has one.
- **Card numbers.** The host still sends raw stats. `inventory.js` `cardRound` rounds the stored single-precision value
  the way `present()` does: damage up, magazine down, and fire rate, reload and accuracy half up to one decimal. The
  accuracy line drops its `%`. Compare deltas are now differences of the printed numbers. The page does not print the
  recipes' `stats.card.display`, because the local slice's copy predates single precision (it holds 1.2 for a 1.25 fire
  rate that the game prints as 1.3).
- **Rarity colour.** Weapons now forward the card's `rarity_color` as `rarityColor` when the recipe has it. The page
  already prefers it, so E-tech gets its own colour. The local slice predates the field, so nothing changes until it is
  regenerated; that regeneration is still a separate decision.
- **Tests.** `tests/card_rounding_cases.json` holds invented cases. Both `tests/weapon_stats_test.py` (Python
  `display`) and `tests/inventory_navigation_test.js` (the page) check them, so the two implementations must agree.
  The navigation test was already failing at HEAD because its `document` stub lacked `addEventListener` (since
  `988b0b9`); the stub now has it. The inventory self-test checks truncation on invented values.
- **Changed card text in the suite:** the demo Infinity pistol with damage 753.22 now prints 754 (was 753), and the
  compare delta follows (+105 against 649, was +104). This was seen in the `backpack_transfer_changes_destination`
  frame. The 780.44 pistol would print 781 (was 780), but no captured frame shows it. Its other stats print as before.
  Suite frames from before and after are under ignored `local/card_rounding/`.
- **Checks (automated):** inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN
  rows, as before); inventory self-test passed; quest first run PASS 79 / 0 errors and resume PASS 11 / 0; CTest 10/10;
  `verify_packages.py` 9/9; navigation test 23/23; weapon_stats 26, weapon_recipe 14, weapon_balance 7 and weapon_paint
  29 tests OK. **In-game:** none. The host widget fallback (`OpenWillowInventoryWidget.cpp`) still formats unrounded
  values.

## 2026-10-03: Phaselock presentation round 3: LDR-like clip, brightening screen tint, burst time in seconds

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. This round follows an
independent critic's score of 4.5/10 for the round-2 side-by-side. Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Round 3". Frames and side-by-sides stay under ignored `local/phaselock/`.

- **Common cause checked first.** Auto-exposure is already off project-wide; the difference is UE5's filmic tone curve
  against UE3's per-channel clip. Host stand-in (UNVERIFIED): additive FX layers cap each channel at 1, translucent ones
  at 1 / opacity, so that HDR particle colours stay saturated instead of turning white. The global tone mapper is
  unchanged.
- **Screen effect.** Its tint now divides the colour by its luminance, so it brightens toward blue instead of darkening
  to grey-brown (stand-in). Burst `Time` is now read as seconds of emitter time (UNVERIFIED). Only the screen burst
  moves (1.05 s -> 0.70 s), matching the game's tint onset of about 0.82 s.
- **Modulate readings (stand-ins).** `Mat_SirenGlowMOD` reads its colour as a brightness-keeping tint (cobalt cast
  flashes, as in the game frames). `Mat_SirenOrbBlackMOD` reads darkness as mask x (1 - alpha), the one reading that
  fits its three emitters. It adds the game's dark blob around the raised hand at about 0.27 s and keeps the bubble's
  dark core. Its host disc is softer (full to half the radius), so the hold rim reads violet-magenta.
- **Not changed:** the bubble size (stock draw-scale rule; a matched-distance game capture or the bullymong's bounds
  radius is needed to compare) and the palm orb size (it follows the hand, which is about 2.5x smaller on screen in
  the host: arms placement or FOV). The shots add a 5.0 s capture.
- **Open:** the release ring is blue-violet where the game's is cyan-white (the same `Mat_SirenGlowMOD` reading that
  fixes the cast flashes weakens it); the 0.6 s white starburst; the dummy's lifted pose (no stock clips);
  `SphereCollapse`.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the 2026-10-02 game captures
  only; no new game capture.

## 2026-10-03: Live weapon data read from the running game (real-game lane)

- **What.** `tools/real_game/scripts/weapon_dump.py` reads the stat properties of every weapon part, name part and type from
  the running game (SDK, main menu, saving blocked, saves restored byte for byte) into ignored `local/realgame/cards/`.
  `tools/real_game/live_overlay.py` and `golden_card_compare.py --live-data` use it as an overlay;
  `tools/real_game/openwillow_valuewatch/` logs chosen values from SDK load (local developer tools).
- **Confirmed in game (2026-10-03, method above).** The live values equal OpenBLCMM's static dump on all 78 values of W's
  61 changed objects; 37 objects carry real value differences against the cooked decode (ClipSize 7 types, ReloadTime 5,
  InstantHitDamage 2, 24 part effect lists), all in W's list. On the 69 golden weapons with exact parts the main four
  stats match 52/69 on cooked data and 69/69 with the live overlay.
- **Correction.** The source is not an online hotfix: the values exist 0.01 s after SDK load, before the `Micropatch`
  configuration, and its 23 entries touch no weapon. Not a package override (only `Startup.upk` defines the objects among 2,010
  packages) and not an installed mod. The origin is UNVERIFIED (load-time change by the game, or a decode gap). The wording in
  NATIVE_WEAPON_RULES section 7 still says "hotfix"; the weapon lane's owner should change it.
- **Open after the overlay.** Rocket launcher sale value about 4 % high (4 weapons), one status chance 33.3 vs 33.4, the
  level line rule, the stage-15 Maliwan pistol reload rounding (1.8 vs 1.7).
- **Not done / needs the maintainer.** The dump is game data and stays in `local/`; whether the port may read live values or
  must carry them some other way is the maintainer's call. No sensitive files touched; no tests rerun (no code under src/ or host/).

## 2026-10-03: Phaselock at a matched 650 uu in the real game (real-game lane)

- **What.** A requested capture for the Phaselock lane: the real game (Ice_P, level 8 Maya, saving blocked, saves restored byte for
  byte afterwards) casting Phaselock at one Adult Bullymong placed 650.0 uu away (horizontal; 652.9 in 3-D), every other enemy
  held still at least 2,500 uu off. New `tools/real_game/scripts/phaselock_matched.py` (list, bounds, isolate, place, measure,
  mark_lock); `phaselock.py` now samples the skill instance that moves (see below). Frames, sampler lines and a notes file stay in
  ignored `local/realgame/phaselock/matched_650/`.
- **Measured in game (one enemy type, one level, one cast pair; not a rule).** The bubble is 404-427 px across on the 1280x720
  frame, about 0.32 of the width: radius 167-174 uu at 650 uu depth (horizontal FOV 77.55 degrees, focal length 796.7 px). That is
  about 1.1 times the target's collision radius (150) and 0.55-0.58 times its mesh bounding sphere (300.8; the sphere moves with
  the animation: 241.9 and 315.3 on other reads). Inside the rim the view is deep violet-blue, with a near-black darkest tenth; the held target stays visible at roughly a third of its normal brightness. Timeline from the per-frame
  sampler, measured from the skill start (15 ms after the key): lift to 0.70 s (target 190 uu higher), hold to 4.77 s, release to
  5.88 s; the screen vignette is up at 0.25 s, the hand orb at 0.5 s, a cyan-white burst at 0.8 s, the bubble from about 1.5 s,
  gone by 5.0 s. A second cast of the same setup (run1) agrees by eye.
- **Method findings.** `SetLocation()` returned False for AI pawns and for Maya and moved nothing; assigning `Location` moves
  her (the capture uses that) and `Destroy()` removed nothing. A target whose AI controller was detached was not locked: the
  skill ended at once and the target took damage (cause not read from script). Freezing pawns with `CustomTimeDilation = 0`
  held them only for a while. What worked: detach the controllers of every other pawn, keep the target's controller and set its
  `GroundSpeed` to 0. The sampler's `lift_skill()` took the last listed `LiftActionSkill`, which was the idle data object; it now
  takes the instance whose `SkillStartTime` is largest (the world instance moves; the `GD_Siren_Skills` object stays at 0).
- **Not done.** No host run compared with these frames; other enemy sizes and distances; a cast at other levels.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

## 2026-10-03: Phaselock presentation round 4: matched-distance comparison, depth-biased darkening, release override

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. The host is compared
with the matched-distance game capture (an Adult Bullymong at 650 uu, horizontal FOV 77.55 degrees) at the same
distance and FOV (`-owfov=62.15`). Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 4".

- **Stock data:** `Mat_SirenOrbBlackMOD` has a `DepthBias` parameter (-20), `Mat_SirenGlowMOD` a `Bias` (-15) and the
  smoke a `DepthBias` (-18). The host reads a negative bias as a camera-ward offset of the sprite plane, with the size
  scaled to keep the outline (UNVERIFIED). The black orb now covers the lifted target's front.
- **Darkness, calibrated against the one capture (stand-in):** the black orb is fully dark to 0.6 of its radius (thin,
  dim rim), with darkness capped at 0.9 on opaque geometry just behind it (the game keeps the target at about a third of
  its brightness) and at 0.96 over the background. The second cap compensates for UE5's float target, where the
  additive layers sum to about 2.5 before the black, against UE3's clamped 8-bit target.
- **Release:** a labelled per-emitter override gives the end template's `Brighten` the plain multiply. The release is a
  cyan-blue ring; the cast flashes keep the brightness-keeping tint.
- **Size: not changed, not confirmed.** The stock rule (mesh bounds sphere radius / 66.7, full-width sprites) puts this
  bullymong's rim at about 356 uu, while the capture measures 167-174 uu. Two readings fit and cannot be separated with
  this enemy, so the report carries a capture request for a second enemy type with an SDK read of the bubble emitter's
  draw scale.
- **Open:** the release timing (game 4.77 s in this capture, host 4.60 s; not changed); the floor glow seen in the
  matched frames against the light-channel stand-in; the magenta-leaning interior; the 0.6 s starburst; the host's
  default capture FOV (106 degrees against the game's 77.55).
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the matched-distance game capture
  only.

## 2026-10-03: Phaselock bubble size rule read from the game's emitters (real-game lane)

- **What.** A follow-up to the matched 650 uu capture, requested by the Phaselock lane: read the spawned bubble emitters at lock
  time on three enemies (Baby, Adult and Ranged Bullymong, Ice_P, level 8 Maya, saving blocked, saves restored byte for byte) and
  measure the rim in frames. New `tools/real_game/scripts/phaselock_size_rule.py` (probe), `tools/real_game/bubble_frames.py`
  (frames at chosen times and a rim width), `aim()` in `phaselock_matched.py`. Frames, probe lines and notes stay in ignored
  `local/realgame/phaselock/size_rule/`.
- **Measured in game (one map, level and skill build).** The two bubble emitters of one pawn (`Part_SirenASEnemyOrbBegin` at lock,
  `Part_SirenASEnemyOrb` 0.2 s later) have different `DrawScale` values (adult 4.2825 and 3.9026, ranged 4.3144 and 3.7926, baby
  1.8181 and 1.9218) while the collision radius never changes (150, 150, 64). The intro value times `BubbleFXScale` (66.7) is within
  1 to 2.3 percent of the pawn's mesh bounds sphere radius read at +0.72 s (285.6 against 290.3, 287.8 against 290.0, 121.3 against
  124.1). So the size input is the mesh bounds sphere at spawn time, not the collision radius. `DrawScale3D` and the particle
  component's scale are 1 on every emitter. The visible rim radius per unit of the loop emitter's `DrawScale` is 47.6, 48.6 and 49.5
  uu on the baby, adult and ranged runs (about 0.73 times 66.7): a constant of the particle template, not of the enemy.
- **Limit.** All three pawns have mesh sphere over collision radius of 1.93 to 1.94 at the lock pose (the same animation), and no
  enemy with a very different ratio (a Skag, a Brut) was on the map, so size ratios alone do not separate the readings; the
  emitters changing with the pose and the baby's half-size bubble following its 0.5 mesh scale do.
- **Timing, game clock.** Lift 0.70 s, hold 3.9 s, release 1.1 s (`LockFadeOutTime`), duration 5.7 s; release began 4.608 s after the
  skill start in all three casts. The earlier matched capture read 4.786 s (hold 0.175 s longer, on a second lock of the same pawn;
  cause not read, the skill's tick rate is 0). `LockDurationFormula` is a 7.0 constant plus the designer attribute
  `Att_Phaselock_Duration` (its value was not read).
- **Method findings.** A cast at pitch 0 did not lock a baby at 650 uu (no target within the auto-aim); aiming the crosshair at the
  target's origin did. Several pawns were found dead or gone after their neighbours' controllers had been detached (cause not
  read), so one capture needs its setup and cast within seconds.
- **Not done.** No host comparison; a Skag or Brut; other levels; the duration attribute's value.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

## 2026-10-03: weapon card rules: level line, single-precision rounding, float bases; golden cards 69/69 (weapon lane)

AI-assisted (Claude). Tools, tests and notes only: no `src/`, no `CMakeLists.txt`, no host or page code; package
parsing is unchanged. The rules are written in our own words in `docs/verification/NATIVE_WEAPON_RULES.md`. Raw
decompiler output stayed in the analysis store. Evidence: the 69 golden cards (exact rolled parts) recorded by the
real-game lane.

- **Level line** (read from script and data, section 4). A mission-balance weapon requires level 0. Any other weapon
  requires its item level minus the floor of the player's level-requirement bonus (0 in this data unless the player has
  something that raises it), at least 1. The card prints the line only when the requirement is above 1, so every
  mission weapon and every level-1 weapon prints none. New `weapon_stats.level_requirement` and the card fields
  `level_requirement` and `level_line`. Not modelled: the over-level text and the DLC message.
- **Rounding precision** (section 2). The Float rounding scales, adds the half and floors on the x87 unit. A golden
  status chance that is exactly a float tie (stored 33.349998, printed 33.4) shows the unit runs at single precision.
  `half_up` now rounds `f32(value * 10^p)` before adding the half. How the game sets the x87 precision was not read
  (UNVERIFIED; Direct3D 9's default). Consequence: an invented 87.35 accuracy now prints 87.4, not 87.3.
- **Float bases** (section 1). Plain weapon-type fields enter the stack as floats. The class-default 2.1 reload is
  2.0999999, so a -20 % scale gives 1.7499998, which prints 1.7 as in the game, not exactly 1.75 printed as 1.8.
- **Launcher sale value** (section 6). The evaluator was right. The golden comparison tool does not pass the recorded
  prefix and title, so the launcher prefixes' price multiplier was missing. The value function itself is still not
  read: its vtable could not be resolved, a second time.
- **Status rows** come from their own presentation data: Float, one decimal. The chance row is a remap whose slope equals
  the Generic BaseChance, except for slag (30.03 against 30; no slag card, UNVERIFIED).
- **Golden counts**, tracked comparison tool, main four / every printed stat / every field:

  | data | before | after |
  |---|---|---|
  | live | 69 / 68 / 54 | 69 / 69 / 54 |
  | cooked | 52 / 52 / 40 | 53 / 53 / 41 |

  Per field (live), status chance went from 15/16 to 16/16. On cooked data reload went from 63 to 64/69.
  A local copy of the tool that passes prefix, title and balance and uses `card['level_line']` gives every field on
  68/69 live (sale value 69/69, level line 69/69) and 52/69 on cooked data. The one remaining live miss is the host's
  own slice name. That tool change belongs to the real-game lane.
- **Tests and shared cases.** `tests/card_rounding_cases.json` gains two reload cases at the half (1.7499998 prints
  1.7; 1.75 prints 1.8). Its 87.35 accuracy case moves to `pending_page_change` with the new text 87.4, which only the
  Python test reads. `inventory.js` `cardRound` still prints 87.3 for it. To match, the page would round
  `Math.fround(Math.fround(stored * scale) + 0.5)` before the floor, with `scale = Math.fround(10 ** decimals)`. That
  change needs the in-engine suite and was not made. `weapon_card_audit.py` audits cards without a level line at level 1
  instead of failing.
- **Checks:** weapon_stats 28, weapon_recipe 14, weapon_balance 7, weapon_paint 29, golden_card_compare 12 tests OK;
  navigation 23/23; CTest 10/10; `verify_packages.py` 9/9. **In-game:** none beyond the golden cards above. No
  sensitive files touched.

## 2026-10-03: Golden card compare passes the balance and name parts (real-game lane)

- **What.** `tools/real_game/golden_card_compare.py` now gives the evaluator the record's balance and its prefix and title name
  parts, renders the level line from the evaluated `level_requirement` instead of the spawn stage, and blanks it when the model
  says there is no level line. Requested by the weapon lane, whose commit `13d9e87` made the rules need these inputs for sale
  value, the name and the level line.
- **Result (automated, golden-card evidence; the rules stay UNVERIFIED).** On the 69 golden weapons with exact parts: cooked data
  alone 53 matching the main four stats, 53 every printed stat, 52 every field; with the live-data overlay 69 / 69 / 68. The one
  miss is the host slice's own display name (`host_name`, 5 of 6). `tests/golden_card_compare_test.py` passes (12 tests).
- **Not done.** No new in-game capture; the rules behind the level line and rounding are still read from native code and
  confirmed only against the golden cards.
- **Checks.** Compare tool run twice as above; no sensitive files touched.

## 2026-10-03: Phaselock presentation round 5: bubble size from the confirmed rule, floor light, interior colour

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details are in
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 5".

- **Size rule, confirmed in game on 2026-10-03** (the real-game lane's SDK reads of the bubble emitters' `DrawScale` plus
  frames, three bullymong variants):
  - each bubble emitter's `DrawScale` = the lifted pawn's mesh bounds sphere radius at its own spawn / 66.7;
  - the loop's visible rim sits 48.6 uu per `DrawScale` unit.
- **Host calibration (UNVERIFIED cause):**
  - each template now takes its own draw scale at spawn;
  - all three bubble templates are drawn at 48.6 / (0.44 x the `Sphere` StartSize) of it, because the host drew the rim
    ridge at 0.88 of the `Sphere` half-width (about 1.8 times the game's size);
  - size check on host frames at the matched FOV: rim radius 79.7 uu at 1.5, 3.0 and 4.5 s against the expected
    48.6 x 1.640 = 79.7 uu.
- **Floor light:** it reaches the floor again, as its data says. The 10-03 game frames show a pale blue pool, which
  undoes the round-1 stand-in.
- **Interior (stand-in, calibrated against one capture):** the bubble's black orb blends toward a deep blue-violet
  instead of black, standing in for UE3's clamp after every blend. Host interior (38, 37, 84), close to the game's
  and slightly lower in blue. The hand orb stays black.
- **Timing:** first locks in game release at 4.608 s, which the host matches; no change.
- **Open:** the dummy stays lit in front of the smaller bubble; the floor pool is fainter on dark asphalt; the 0.6 s
  starburst.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared with the matched-distance and size-rule game
  captures only.

## 2026-10-03: Maintainer README/plan edits, written stop conditions dropped, worktree provisioning from a source worktree

Maintainer decision. Docs and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.

- **Maintainer edits kept as written:** the README intro, status-table notes for Phases 1, 3 and 4, the FAQ and
  "Who's making this" wording, and the plan's opening/credit lines. The Phase 3 and Phase 4 notes ("working well",
  "roughly 6/10 parity") are the maintainer's own testing and have no verification record; they are worded as such.
- **Stop conditions dropped:** the plan's section 9 (now "Commitment") and ROADMAP's "Kill criteria" section no longer
  list stop conditions. Links and descriptions that pointed at them were updated (README FAQ, document table, the
  Phase 1 gate sentence in ROADMAP). Verification rules are unchanged.
- **Worktree provisioning:** `tools/provision_worktree_assets.ps1 -SourceWorktree <path>` copies missing files from
  a populated worktree's Content and every `local/` folder (the shared seed only holds four folders and goes stale).
  Documented in AGENTS.md and `tools/worktree-assets.md`. Parse-checked only; not run in a new worktree.

## 2026-10-04: native analysis notes: Phaselock presentation, weapon visuals, ambient NPC movement, backpack sort

AI-assisted (Claude). Docs only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing. Four own-words
notes under `docs/verification/`, read from the installed script (`research/script_disasm.py`), installed class
defaults and a local Ghidra 12.1.4 reading of the executable (`tools/ghidra/`). Raw output stays under ignored
`local/analysis/E/`; the notes contain no listings or pseudo-code.

- **Phaselock presentation** (`NATIVE_PHASELOCK_PRESENTATION.md`): the lifted target plays four stock special moves
  (lift, loop, fall, land) from its own AnimSets, starting at the cast; the fall clip is stretched to `DropTime`
  (0.5 s); the first-person hand effect comes from a notify 0.25 s into the cast clip, attached at an arms socket; the
  only bubble instance parameters are `PhaselockLifeTime` (once) and `SphereCollapse` (every tick).
- **Weapon visuals** (`NATIVE_WEAPON_VISUALS.md`): the weapon material is a new instance over the Material part's MIC
  with the parts' vector parameters applied as linear colours (only elemental parts carry any); a per-shot impulse drives
  the emissive scale; the first-person mesh attaches to the arms' weapon socket; foreground FOV 45 with a weapon, 60
  without.
- **Ambient NPC movement** (`NATIVE_AMBIENT_NPC.md`): script and data driven (scripted-NPC actions walking move-node
  chains, perches with weighted idle variants); a native load balancer admits at most 7 walkers, one per 0.5 s. Next-node
  choice and speed rule not read.
- **Backpack sort** (`NATIVE_INVENTORY_SORT.md`): the five modes are data; comparator chains, filters and header rules;
  reproduces every 2026-09-30 observation. Ties are unordered in the engine's sort.
- **All rules UNVERIFIED in game;** each note names the capture that would confirm it.
- **Checks:** none needed (docs only).

## 2026-10-04: Phaselock presentation rounds 6-7: effect materials from own-words notes on the compiled shaders

AI-assisted (Claude). Host and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.
Details: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 6" and "Round 7".

- **Method:** the cooked effect-material graphs are stripped. Their compiled pixel shaders in
  `RefShaderCache-PC-D3D-SM3.upk` were read with `research/d3d9_bytecode.py`, the same method as the weapon paint model
  (2026-10-02). What each one computes was written down in our own words, and the 17 effect materials in
  `host/ue5/import_phaselock_fx.py` were written from those notes as UE Custom nodes, with plain constants and prose
  comments. The listings stay under ignored `local/`.
- **Findings that replace earlier rounds** (UNVERIFIED in game unless compared below):
  - `DepthBias` is a soft-particle fade distance, not a camera-ward shift.
  - The bubble sphere is a warped read of the bubble texture.
  - The tattoo mask is the B channel of one quadrant of the mask texture; rounds 1-5 lit the whole sleeve.
  - The bubble ring sits at 0.78 of the sprite half-width.
  - The sigil under the target is HUD, not an effect.
- **Host calibrations (UNVERIFIED):**
  - bubble warp strength driven by the collapse value;
  - core haze ×3;
  - layer colours above 1 normalised;
  - blue tints on the rim and haze;
  - a per-emitter darkening cap and fade floor;
  - floor light gain ×4 (the data's brightness is still what `PhaselockLightIntensity()` reports);
  - lighter swirl, star-burst and brighten.
- **Visual review:** an independent critic agent compared matched host and real-game frames. It scored round 6 at 5/10
  and round 7 at 6/10 (hand 6, bubble 7, release 5). Largest remaining gaps:
  - the release does not collapse the sphere;
  - the palm orb is lost around 0.55 s;
  - the interior is more see-through than the game's.
- **Target animation:** the stock clips exist only on enemy AnimSets and the dummy has none, so it is still not
  exercised.
- **Checks:**
  - CTest 10/10; `verify_packages` 9/9 match; `particle_system_test` OK.
  - Quest suite first run PASS 79/0, resume PASS 11/0 on the round-7 build.
  - Mover and inventory suites not run for this lane (untouched areas); the end-of-session run covers them.

## 2026-10-04: inventory and Skills pages: Skills preload, stock sort list, backpack focus layout, compare frames from real-game captures

AI-assisted (Claude). Host page, host forwarding and the suite only; no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Skills preload, real-game comparison and the stock
sort list: 2026-10-04") and `docs/verification/INVENTORY_CARD_STATS.md` ("What the page prints on a weapon card").

- **Skills page:** in the baseline frame it still said "Loading Maya's skill tree...". The page was created on the key
  press and took about 3.4 s. It is now loaded hidden at level start, like the inventory page:
  - key to a populated page: 49 ms (page log lines);
  - cost: the inventory page's own boot grows from 4.4 s to 6.1 s;
  - real game: page visible about 0.28 s after K, settled by 0.65-0.85 s. No timing parity is claimed.
- **Real-game session (Maya L8):**
  - Saves were backed up and compared byte-for-byte afterwards, and the driver was removed.
  - The install runs community mods: part-name lines on cards and instant gear equip are theirs, so gear compare was
    not observed.
  - Confirmed in these captures:
    - compare frames: the moved item is green, the other yellow (the host had them inverted);
    - compare rows show arrows only;
    - red gear cells appear in a weapon compare;
    - backpack focus layout: enlarged centred panel, receded equipped panel, "BACKPACK used/capacity" plate;
    - the lists of all five sort modes;
    - the full-screen Inspect.
- **Stock sort list:**
  - ALL/TYPES/BRANDS/ITEMS/VALUE with sub-headers replace the host's own modes and category filter, following the
    comparators of `NATIVE_INVENTORY_SORT.md`. Those rules stay UNVERIFIED; every point the capture could test agreed.
  - PageDown and PageUp step through the modes, and the first item is selected on each change.
  - Ties keep pickup order. This is a host choice: the game's sort is unstable.
- **Cards:**
  - They now show the projectile count, the status rows for elemental guns, and values in the label colour.
  - For the six slice recipes the page text matches the golden cards, except the two shotgun recipes (reload and
    magazine one step off). That is a recipe/evaluator question, not a page one.
- **Inspect:** now full screen. The picture is the 3D preview keyed against black, so black gun parts can show as holes.
- **Slot art:** the slots now show item art, from previews rendered locally for the slice guns (local data only).
- **Visual review:** an independent critic agent scored 11 matched host/real-game pairs at 5.6/10 overall. Main gaps:
  - flat panels against the movie's tilted glass (Ruffle ignores the movie's 3D transforms);
  - card text about 20% smaller;
  - sort sub-headers overlapping cards;
  - the inset selection fill;
  - the compare layout;
  - the Skills tiles.
- **Open:** selectable empty backpack cells, gear compare in the real game, white flavour lines on cards, Q on the
  Skills page, and the Phaselock eye sigil on the HUD.
- **Checks:**
  - `tools/test_inventory_actions.ps1`: 49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE (baseline 45/0/2/2).
    - Steps 15/16 now assert the stock sort.
    - The two wheel steps run because the test adds four synthetic filler weapons.
    - Slate keys only; the suite does not judge visuals.
  - `node tests/inventory_navigation_test.js`: 27/27.
  - CTest 10/10 and `verify_packages` 9/9 on the same tree.

## 2026-10-04: Phaselock presentation round 8: release size, hand timing, interior measured against the game

AI-assisted (Claude). Host only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 8".

- **Release "does not collapse":** this was a test-aid error, not an effect rule.
  - No bubble shader reads the collapse value. The game sizes each bubble template from the pawn's mesh bounds at that
    template's spawn, and the bounds shrink as the pawn is lifted: 290 uu at the lock and 193 uu lifted, read with the
    SDK driver in the real game.
  - `-owbubbleradius` now takes one value per template (`290,260,185` stands in for an adult bullymong).
- **Hand (host calibrations, UNVERIFIED):**
  - The effect starts 0.08 s before the clip's notify.
  - The cast clip plays at 0.85 speed (`PlayAction` gained a rate argument).
  - Orb texture ×0.6 and star-burst alpha ×0.2.
  - Basis: the game frames show an opaque palm orb at 0.44 s and the arm dropping about 0.05 s later than the host's.
- **Interior opacity, measured on the game frames:** about a quarter of the background's contrast survives inside the
  bubble (regression slope 0.26 at 1.5 s, 0.21 at 3.0 s).
  - The host darkening caps (0.72 loop, 0.45 end) bring the host to 0.11 / 0.26.
  - The host bubble centre in the measurement is approximate.
  - The method and its script are kept local.
- **Floor light:** wider, softer and paler (UNVERIFIED calibration).
- **Visual review:**
  - An independent critic gave an absolute score of 5.5/10, against 6/10 for round 7.
  - A blind A/B critic, judging against the same game frames, preferred round 8 on the hand and release sheets and called
    the bubble sheet a tie.
  - Absolute scores from separate critic runs vary by about a point, so the A/B result decided the commit.
  - Remaining gaps:
    - the bubble is about 20% large and sits high-left;
    - the floor glow is now too faint;
    - the 0.30-0.40 s black sphere and swirl are small;
    - the tattoos are overexposed after 0.55 s;
    - no ice-shard flash facets;
    - straight release shards;
    - no fist clench;
    - no target animation.
- **Checks:** quest suite first run PASS 79/0 and resume PASS 11/0 on the round-8 build; CTest and `verify_packages`
  were not rerun for this commit (no `src/` change).

## 2026-10-04: Sanctuary ambient citizens: census, assets, host perch cycle and node walks behind a flag

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md`; native reading in `NATIVE_AMBIENT_NPC.md` (UNVERIFIED).

- **Census (our reader):** the civilians come from four population definitions in `Sanctuary_Combat` and two pawn
  archetypes, male and female Sanctuary Citizen. Their routes are decoded data: initial destinations, weighted next
  nodes, perch start/idle/stop clips, loop and lerp times, and Kismet scripted moves. Counts are in the record.
- **Real-game observations (SDK driver; Maya level 8 at mission Plan B):**
  - Saves were backed up and blocked in game. The game rewrote two files, which were restored, and the folders then
    compared identical.
  - 33 live citizens; 36 of 52 dens and encounters enabled, none of the 7 crowd dens.
  - A walking citizen's velocity read 150.
  - The town-wide patrols are Resistance fighters, not citizens: 94-95 uu/s by displacement, along the data's node
    circuits for 3 of 7 sampled.
- **Host:**
  - Both kinds are imported through the Marcus pipeline pattern: 112 bones and 42 clips each. Textures are bound by
    UModel's guess and the clip mapping is UNVERIFIED.
  - `-owambient=<manifest>` spawns them. Otherwise nothing spawns.
  - Stand-ins, all UNVERIFIED:
    - The live set is copied from one real-game moment.
    - Walking is in straight lines at 150 uu/s, with the floor taken from a trace.
    - A load balancer follows lane E's note.
    - There are no hats, hair or outfit variants, no Resistance fighters, and no talking or look-at.
- **Visual review:** an independent critic scored the host citizens 5/10 against real-game frames (mesh/outfit 4,
  poses 5, walk 6, scale 7). Main gaps:
  - bald identical heads;
  - no ink outline and washed-out colours;
  - the female reads as male;
  - a duplicated pawn at one perch;
  - several perch poses misaligned with their props.

  The real frames are matched by activity, not position: the population could not be held for close-ups.
- **Checks:**
  - `tools/test_ambient.ps1`: PASS (33 pawns, 33 reached a node, 3 walked, 24 at a perch).
  - Quest suite with ambient on: 79/0 and 11/0. One earlier first run failed check 61 on a Phaselock light reading; the
    repeat passed and the flake is with the Phaselock lane.
  - Quest suite without ambient: 79/0 and 11/0.
  - CTest 10/10; `verify_packages` 9/9.

## 2026-10-04: Inventory and Skills pages round 10: sub-header rows, compare layout, selection band, card fit, Inspect clip, Q overview

AI-assisted (Claude). Page and HUD capture sequence only; no change to `src/`, `CMakeLists.txt` or package parsing.
Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Round 10").

- **Sort sub-headers:** each sits on its own row between groups. They were centred on the header clip's bounds, which
  are taller than the text, so every header sat about half a row high over the previous card.
- **Compare view:** the Equipped panel is narrowed into the gap between the cards with slots 1-4 drawn, the Backpack
  panel moves right so "(COMPARE)" stays readable, and both cards carry the price chip and Accuracy row (the synthetic
  test variants lacked those fields).
- **Selection:** the movie's own highlight symbol is stretched to the panel width behind the selected row (width and
  shift set by eye).
- **Card size:** cards are rescaled to visible widths measured on the real captures. A fit, UNVERIFIED.
- **Inspect:** the movie is clipped to the card frame and hint strip, so the backdrop box is gone.
- **Skills:** Q toggles an overview of the three trees. The layout numbers start from the installed `Gfx_SkillTree`
  defaults and are enlarged by eye; how the game combines them was not read, so UNVERIFIED.
- **Visual review:** an independent critic scored round 10 6.6/10 against real-game frames (round 9: 5.6) and judged
  round 10 closer than round 9 on all ten inventory pairs. Main gaps: the perspective tilt and curved glass (3D
  transforms, which Ruffle ignores), backpack-focus list geometry, compare panel positions and padlocks, Inspect card
  level strip, Skills page layout.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE, rerun after the weapon lane's preview and stat changes with the same result.

## 2026-10-04: Sanctuary ambient citizens round 2: heads, hair and hats from live pawns, ink line, perch root motion

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 8.

- **Causes of the round-1 defects:**
  - The "duplicate pawn" was two real neighbouring citizens (`Perch_66` and `Perch_140`, 240 uu apart) on the camera's
    line. The capture camera now rejects lines that pass within 130 uu of another pawn.
  - Lean, legs and squat: the stock perch clips carry root motion (the observed pawn-to-node offset matches the start
    clip's travel, e.g. 24.1 vs 25.8 uu at `Perch_66`), and the floor trace missed the real pawn height by more than
    10 uu on 19 of 33 pawns. The host now applies each clip's root travel when it ends and keeps the observed height
    for idle and held pawns.
- **Observed in the real game** (third SDK session, saves blocked, byte-identical afterwards): each live citizen's
  materials and attached static meshes are readable. 33 pawns carried 88 attachments (20 meshes, 8 head textures).
- **Host:** those heads and attachments, bone-attached; Maya's inverted-hull ink line and matte constants for the
  citizens. UNVERIFIED: the attachment transform (judged by eye), the hair tint stand-in, the outline thickness and the
  shader. Not done: `Master_NPC` zone colours, body garment variants, Resistance patrols, matched real-game close-ups.
- **Visual review:** an independent critic scored round 2 5.5/10 (round 1: 5), judged better than before, no
  overlapping pawns. Main gaps: the female still reads as male (face patch, hair), a mis-parented hat at one stop, scalp
  showing through blonde hair, one bald untextured head, the bang-on-wall fist not meeting the wall, a squat clipping
  a pipe.
- **Checks:** quest suite with ambient on 79/0 and 11/0, with ambient off 79/0 and 11/0; `test_ambient.ps1 -Shots`
  PASS (36 pawns, 36 reached a node); CTest 10/10; `verify_packages` 9/9.

## 2026-10-04: Phaselock presentation rounds 9-11: ring factor, release size, soft streaks and ground wash, hand spikes, check-61 fix

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 9" to "Round 11"). All calibrations UNVERIFIED.

- **Bubble too large (cause):** round 6 had replaced the ridge detector's ring factor (0.88) with the bubble texture's
  own ring (0.78), which drew the bubble about 13% too large. Restored; the per-template stand-in radii are 290, 233
  and 210. The release shell was re-measured on the game frames (112 px radius at 4.80 s, 176 px at 5.00 s); round 9's
  smaller figure was a mis-measure.
- **Hand timing:** round 8's whole-effect early start shrank the swirl and disc. Now only the palm orb (0.03 s) and the
  disc's alpha (0.1 s) run ahead; the disc's size curve is read at its unshifted age. The hand effect starts at the
  cast clip's notify; the clip still plays at 0.85.
- **0.65 s spike rays:** they come from the hand template's star-burst emitter; the game frames show none, and why was
  not found. The host scales that emitter to 0.15 (a new per-emitter gain).
- **Look:** soft pale-blue ground wash (light gain 6.5, radius 0.85x, colour 20% toward white), wider and softer
  streaks, release spikes and ribbons at reduced strength, tattoo glow x0.3.
- **Quest check 61 flake** (light 32 vs 4 once): traced to an intermediate build that reported UE5's intensity rather
  than the data value (32 = 4 x a gain of 8). The light's data-unit brightness is now stored when it is set, so the
  check no longer depends on the host gain. The check itself is unchanged.
- **Visual review:** independent critics scored rounds 9, 10 and 11 at 5.5, 5.5 and 6.3. Blind A/B against round 8:
  round 9 and round 10 each won one sheet of three; round 11 won bubble and release and tied hand, so it is committed.
- **Open:** the 0.30 s dark void (about 200 px against the game's 330; two enlargements made it fade, cause not found),
  the 0.80 s whiteout, interior opacity, rim weight, ice-shard flash facets, straight release shards, fist clench, target
  animation (no stock clips on the dummy), slight egg shape.
- **Checks:** quest suite first run 79/0, resume 11/0 on the final build. CTest and `verify_packages` in the final pass.

## 2026-10-04: Inventory and Skills pages round 11: list scroll origin, compare and Skills layout, Inspect strip

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 11).

- **Cause of the off-centre selection band:** round 10 gave the row list a scroll rectangle with a negative x origin.
  Ruffle shifts the content right by that amount instead of revealing it, so every row sat about 23 px right of the
  panel centre and a band shift constant hid it. The origin is zero now and the shift constant is gone; the band is
  sized from the panel frame.
- **Compare:** the Equipped panel is wider and the Backpack panel moved left; padlocks show on locked slots (checked
  with `-owslots=2`, the real session's count, only). Compare from equipped keeps the highlight on the chosen
  equipped slot (drawing only; behaviour unchanged).
- **Skills:** tab group and trees moved right, Phaselock card refitted after each tree tween, overview footer below
  the panels and locked tiers dimmed. **Inspect:** level strip and price chip restored, thinner hint strip.
- All placements are fits read off the captured frames (UNVERIFIED).
- **Visual review:** an independent critic scored round 11 7.3/10 (round 10: 6.6) and judged it closer than round 10
  on every pair, except that compare-from-equipped lost the highlight on the focused backpack tile. Main gaps: the
  perspective tilt and glass (not reproduced; Ruffle ignores the movie's 3D transforms), band overrun at the panel's
  right edge, card text about 15 px against 17 px, Skills plate colliding with the footer hint.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-04: Phaselock presentation rounds 12-13: hand effects in first-person space, saturated swirl, deeper orb

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 12", "Round 13"). All calibrations UNVERIFIED.

- **First-person space:** with the weapon lane's `-owfpfov` (the game's separate foreground FOV for arms and gun) the
  hand sprites render as first-person primitives, so they stay on the hand; the bubble, screen effect and light stay in
  world space. With it the swirl, disc and orb are about game-sized.
- **Round 12's "enlarging the void fades it"** was a misreading: the wider core did grow, but its soft gradient let
  the street show through. The normal-FOV widening and a thick rim halo were tried and reverted after a blind A/B.
- **Hand look:** energy swirl alpha 0.8 with a more saturated cyan; palm orb a deeper blue with its texture veins.
- **Diagnostic:** `-owfxscalar` sets one effect material scalar per emitter at run time (TOOLING).
- **Visual review:** round 12 6/10, round 13 6/10. Blind A/B: round 13's hand beat round 12's (both close); the bubble
  and release were close to ties with rounds 11 and 12. Scores have stayed between 5.5 and 6.3 since round 7.
- **Open:** near-black violet void and interior, a compact ground disc, side streaks, thick release ribbons, orb vein
  cracks, the forward fist, the 0.80 s whiteout, ice-shard facets, target animation.
- **Checks:** quest suite with `-owfpfov=45` 79/0 and 11/0 (via a local copy of `tools/test_quest.ps1` that adds the
  flag).

## 2026-10-04: Guns: parts and card stats from the running game, a tone-mapper-aware gun material, first-person foreground FOV

AI-assisted (Claude). Host, tools and tests; no change to `src/`, `CMakeLists.txt` or package parsing. Numbers and
paths: `docs/verification/WEAPON_VISUALS.md`.

- **Real-game session** (SDK driver, saves backed up, blocked and byte-identical afterwards, guns in memory only):
  six exact-part guns captured in first person and in the Inspect view; live part mesh lists and material instances read.
- **Part assembly (confirmed in game):** the host used only the main gestalt mesh. Adding the body-variant meshes and
  drawing nothing for `*_None` parts makes all six fragment lists equal the live part mesh names, with exactly equal
  triangle totals. The Jakobs pistol's hidden-bone triangles (moon clip, bullet) are cut. Not confirmed: that the
  `*_None` name is the game's own test.
- **Colour, round 1:** with the right meshes the existing paint model was largely right (Maliwan colours within 0.82x
  median and 2.7 degrees of hue of the real Inspect view). The material became an Unlit evaluation of the base pass's
  structure with stand-in lights (UNVERIFIED) and the part's elemental emissive vectors.
- **Colour, round 2 (cause):** the oversaturation, crushed blacks, red-for-orange accents and hot Infinity shroud came
  from UE5's film tone mapper acting on the Unlit output (scene-linear 0.18 displays 0.03, 1.0 displays 0.51; measured
  with a grey ramp in the material). The material now applies the inverse of the measured curve (a 28-point table used
  per channel; an approximation) and clips per channel. Median linear ratio host/real against the Inspect view:
  Maliwan 1.06, Jakobs 0.69, Infinity 1.01, SMG 1.16, rifle 1.05, shotgun 0.86 (round 1: 0.77, 0.72, 1.06, 0.93,
  1.08, 0.64).
- **First person:** the game draws arms and gun with a foreground FOV of 45 (read through the SDK; world FOV 77.55
  with the config's 90). UE 5.8's first-person FOV now does the same by default; making it the default is a host
  choice, `-owfpfov=0` opts out. The Phaselock hand effects follow it (rounds 12-13). The SMG, rifle and shotgun were
  held in the pistol clips; their own clip sets are imported and chosen by weapon type. The weapon socket and its
  90-degree yaw, and the arms mesh and material, are the game's own data.
- **Shotgun cards (lane D's report):** 4.4/9 and 3.7/13 came from stats stored by an older evaluator and from the
  cooked weapon type differing from the running game's. Re-evaluated on the live overlay: 4.1/10 and 3.5/14, matching
  the game's Inspect card for the exact parts. Where the live values come from is still unexplained.
- **Mission pistol:** the quest lends the recipe's own corrected mesh where it is imported, with the earlier rolled
  sample as the fallback (one block in `OpenWillowQuest.cpp`).
- **Visual review:** independent critics scored round 1 7.2/10 and round 2 6.8/10; round 2 was judged closer than round
  1 on all six Inspect pairs and on five of six first-person pairs (one tie). Main gaps: guns about 20% large with too
  little cant in first person, the Infinity shroud still too pink, the fire pistol's barrel too light, no ink outlines.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `tests/weapon_recipe_test.py` 14 OK,
  `tests/weapon_stats_test.py` 28 OK; quest suite with the foreground FOV default 79/0 and 11/0. Inventory suite in the
  final pass.

## 2026-10-05: Inventory and Skills pages round 12: flat selection band, focus-view extents, compare highlight, Skills chrome

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 12).

- **Band overrun:** the movie's highlight symbol has a glow tail that ran about 20 px past the panel's right edge. The
  focus view now draws a plain filled band (colour sampled from a real capture) inside the panel frame.
- **Extents:** the focus panel is 15 px shorter, the list starts 7 px higher, the hint line is lifted 9 px and the
  Inventory tab group shrinks 7%. Compare from equipped keeps the backpack tile highlight (clipped at the tile) and
  the compare hint has no Sort entry. Skills tab group and footer re-placed so the hint clears the Siren plate. Card
  text is 16 with bold values; the Inspect card is narrower with a hint strip clipped to its text; the mini equipped
  column hides its clipped title.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 12 over round 11 on 11 of 12 screens (one could not be judged),
  means 7.0 against 5.5 (absolute scores drift about a point between critics). Main gaps: the list clips about one row
  early, compare from equipped should still show the Sort hint, the selected tile in compare from backpack should be
  green, Skills hint about 38 px right of the game's, perspective tilt and glass not reproduced.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE (run with the gun lane's preview and paint changes in place).

## 2026-10-05: Phaselock presentation rounds 14-15: why the bubble interior read navy, violet interior

AI-assisted (Claude). Host and import script only (`OpenWillowPhaselockFx.cpp`, `import_phaselock_fx.py`); no change to
`src/`, `CMakeLists.txt` or package parsing. Details: `docs/verification/PHASELOCK_STOCK_DATA.md` (Round 14).

- **Cause:** blend mode, draw order and the colour the template feeds the dark layer all match the game (checked
  with `-owfxscalar` and against the own-words shader notes). The navy, see-through interior came from the host's own
  round 5-8 compensations (dark-layer cap and fade floor, a blue-tinted haze, a blue ring tint).
- **Changes (UNVERIFIED calibrations):** the ring texture's dim disc is tinted violet while its bright rim keeps the
  white-blue tint; dark-layer cap 0.8 and fade floor 0.65; the purple wisps at alpha x0.6 and colour x5. A compact
  ground light (round 14) made the ground disc disappear and was reverted in round 15.
- **Visual review:** a blind A/B critic preferred round 14's interior but not its missing ground disc; round 15 against
  round 13 was a tie on the bubble and release sheets and narrowly better on the hand sheet (about 6/10). This lane
  stops here. Open: interior still too translucent, side streaks (the streak sprites take a random rotation whose
  rule was not read), the 0.80 s whiteout, straight release shards, ice-shard flash, fist clench, target animation.
- **Checks:** quest suite with the foreground FOV default 79/0 and 11/0; CTest and `verify_packages` unaffected
  (no `src/` change) and green in this session.

## 2026-10-05: Sanctuary ambient citizens round 3: attachment transforms, zone colours, the grey stand-ins

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `ambient_npc_assets.py`, `ambient_npc_attach_editor.py`,
`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 9.

- **Hats, hair and gear:** their placement lives on the attached mesh component itself (translation, rotation, scale),
  not on the attachment entry; rounds 1-2 ignored it. Observed on live pawns through the SDK script, now applied.
- **Zone colours:** a host material mixes the diffuse with the pawn's own zone colours through its light map and zone
  mask. The mixing formula is applied by analogy with the weapon master reading and is `UNVERIFIED` for the NPC master.
- **Grey citizens in the 2026-10-04 showcase:** the first version of that material failed to compile (a linear sampler
  given an sRGB default), so UE drew the default material. Fixed; the shot tour log has no compile failure and 35 of
  35 pawns render textured.
- **Visual review:** a blind A/B critic judged round 3 a little closer than round 2 (about 5.9 against 5.6 per stop;
  the female now reads as female). Main gaps: packs float beside the shoulder, some heads read as flat discs, the
  palette is warmer than the game's night-lit blue-grey, the wall-bang and kick idles do not read, weak face cues.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns, 35 reached a node, 24 at a perch); quest suite with
  ambient on and off 79/0 and 11/0; CTest 10/10.

## 2026-10-05: Inventory and Skills pages round 13: list height, compare hints, focus spacing, Skills footer

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 13).

- The focus view shows 7.6 rows, ending near y 625 like the game (was 6.5); the panel title sits 8 px higher and the
  sub-header-to-tile gap is about 12 px. The Sort hint shows in compare from equipped and not in compare from backpack,
  as observed. Skills footer moved 38 px left, tab group 5 px down, description text one size smaller.
- The moved tile in compare from backpack gets a host-drawn green frame (colour chosen by eye); the critic did not see
  it in the frames, so it is an open item.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 13 over round 12 on 9 of 12 screens (3 ties), means 6.8 against
  5.8. Main gaps: compare-from-backpack tile frame and list position, Skills card height and line wrap, Inspect card
  height (no part or flavour lines), perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Inventory and Skills pages round 14: compare tile frame, list positions, Skills tree height

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 14).

- The stray yellow rectangle in compare view was the HTML hit box's browser focus ring, drawn around the larger hit
  box; it is off (the movie draws the selection). The moved tile's green frame is thicker with a light tint.
- Compare and equipped lists start about 30-34 px higher ("WEAPONS" near y 170 as in the game); Skills trees, action
  bar and HARMONY label 20 px lower, the card 2.5% taller; focus hint and backpack plate nudged.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 14 over round 13 on 10 of 11 scored screens (1 tie), means 7.2
  against 7.0. Main gaps: Skills vertical layout (icon pitch 67 against 72 px, plate low), Skills card wraps to five
  lines against six, equipped-view header collision, compare lists one row short, the game's solid lime fill on the
  moved tile, perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Sanctuary ambient citizens round 4: the attachment frame, read from a live sample

AI-assisted (Claude). Tools and a synthetic test (`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script,
`tests/ambient_transform_test.py`); no change to `src/`, `CMakeLists.txt`, package parsing or host C++. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 10.

- **Cause of floating packs, disc heads and sideways hats:** the bone frame and the worn piece's own transform were
  combined in the wrong convention. One live sample (bone world matrices of two bones on 33 pawns, saves backed up
  and restored, driver removed) showed that the extracted bone frames are the live ones turned 180 degrees about the
  bone's X axis, and that imported static meshes keep the original coordinates while the imported skeleton is
  mirrored. Only the new composition puts the pack behind the middle of the back with the live matrices. The result
  is a reflection, carried as one negative scale; the synthetic test checks the decomposition (4 OK, needs numpy).
  `UNVERIFIED` beyond that one capture and two bones; the helper's attempt to read the pieces' own world matrices
  failed, so the live bounds were not captured.
- Palette and idles unchanged (palette still `UNVERIFIED`; the night-lit blue-grey in some real frames is lighting).
- **Visual review:** a blind A/B critic preferred round 4 over round 3 on six of seven stops (one tie), about 6.6
  against 4.4 per stop. Main gaps: a hair bun floating behind one head, a background pawn's mask, flat faces from
  above, weak ink outline, packs unconfirmed on front-facing pawns.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` all packages matched.

## 2026-10-05: Inventory and Skills pages round 15: equipped header, compare rows, Skills plate and wrap, Inspect hint

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 15).

- Equipped view: the panel title sits 13 px higher, so it no longer collides with the first sub-header. Compare
  views show 7.6 rows like the focus view; the moved tile has a translucent lime fill (solid would hide the gun art).
- Skills: trees 7 px higher, the SIREN plate 22 px higher with the card kept in place, description text one size
  larger so it wraps to six lines like the game, MOTION name label hidden as in the game. Inspect: hint strip moved
  right and the right-edge fade is off while inspecting.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED). This lane stops here.
- **Visual review:** a blind A/B critic preferred round 15 over round 14 on 5 of 12 screens and tied the rest, means
  7.4 against 6.9. Open: perspective tilt and glass (Ruffle ignores the movie's 3D transforms), Skills action plate
  about 25 px low and line pitch 25 against 22 px, Inspect card height (no part or flavour lines) and watermark,
  black gun parts keyed out as holes in Inspect, selectable empty cells, gear compare not observed in the real game,
  the Phaselock HUD sigil.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Guns rounds 3-4: first-person placement from the weapon type's view offset and mesh FOV

AI-assisted (Claude). Host and tools (`OpenWillowWalker.cpp/.h`, new `tools/weapon_view_model.py`, a debug mode in
`import_weapon_paint.py`); no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/WEAPON_VISUALS.md` sections 9-10.

- **Round 3** read the foreground FOV 45 as a vertical angle. A blind A/B split by type: better pistols, worse long
  guns. Not committed on its own.
- **Round 4, cause:** the host never placed the arms as the game does. Read through the SDK on five held weapons:
  the arms origin is the view point plus the weapon type's `PlayerViewOffset`, and the controller's foreground FOV is
  the type's `FirstPersonMeshFOV` (45; SMG 50). The cooked values are read from the weapon types by the new script
  into an ignored `weapon_view.json`; the walker applies them on weapon select, plus the idle clip's Camera-bone
  offset. Host bone positions then match the live ones to about 0.1 cm. With that placement the 45 fits as a
  horizontal angle on all six guns (silhouette widths within 3-12% of the real frames). The data values and the live
  equality are read facts; how the engine applies them and the angle's axis are `UNVERIFIED`.
- **Exposure:** the maintainer saw all-white guns. Those were the lane's solid-white silhouette runs
  (`OW_Debug` 4, measurement only). In normal runs the in-silhouette brightness is within about 25 levels of the real
  frames; the real Jakobs metal is near-white too and the real frames are lit blue. No exposure change.
- **Visual review:** a blind A/B critic preferred round 4 over the committed round 2 on five of six guns (one tie),
  about 7.0 against 5.3 per gun. Main gaps: paint too warm (Infinity, rifle and shotgun barrels), the plain pistol
  slightly low and large, long-gun bodies running a little far right, the rifles' forearm tint.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `weapon_recipe_test` 14 OK, `weapon_stats_test` 28 OK;
  `verify_packages` OK; quest suite 79/0 and 11/0. The Phaselock cast hand moves with the arms; re-checked separately.

## 2026-10-05: Sanctuary ambient citizens round 5: ink line sized in pixels and on worn pieces

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `prepare_ambient_world.py`,
`ambient_npc_attach_editor.py`, `seed_ambient_npc_assets.ps1`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 11.

- The ink hull was a fixed 0.5 cm, about 1 px at tour distance; the real game's line looks 2-3 px at any distance
  (by eye on one frame). The hull thickness now follows the camera distance for a target of 3 px (`UNVERIFIED`), and
  hats, hair and packs get their own hull. `-Steps outline` rebuilds only the ink material (a failed editor run had
  deleted it during this round; rebuilt).
- The floating "hair bun" at stop 00 is the raised fist of the wall-bang clip seen behind the head, not a worn piece.
  Whether the fist meets the wall depends on the real perch's wall distance (`UNVERIFIED`). Packs confirmed on the
  back from behind.
- **Visual review:** a blind A/B critic preferred round 5 over round 4 on every stop, about 6.0 against 3.4 (absolute
  scores drift between critics). This lane stops here. Open: line width still uneven (1.3-5 px) and navy rather than
  black, a gap between the line and the body on thick lines, creases drawn inside the silhouette, a flat untextured
  cap and a featureless face from above, body garment variants, Resistance patrols, matched real close-ups.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` exit 0; `tests/ambient_transform_test.py` OK; module build exit 0.

## 2026-10-05: native census of the Fire mission's script call graph

AI-assisted (Claude). Phase 2 step A. Added opt-in call counters to `vm::Runtime` (filled in `Interp::invoke`; off by
default, existing callers unaffected), `ow-package --native-census` (`src/census.*`; entry file
`tools/slice_native_census_entries.txt`, 319 entries) and `tools/slice_native_census.py`; record in
`docs/verification/NATIVE_SLICE_CENSUS.md` (names and counts only).

- Result: 126 natives reached dynamically, 577 in closure A (primary static edges), 1,934 in closure B (upper bound with
  subclass overrides and interface implementers); 24 of the 514 non-operator natives in A are implemented. All 319 entries
  complete on default objects with stubbed natives, but only 90 enter a second script function: the dynamic numbers are a
  floor.
- Verified (automated): counters and closures on a synthetic package (`tests/vm_test.py`), every dynamically entered function
  inside the static closure B, CTest 10/10, packages 9/9.
- UNVERIFIED: every number as a description of the real game (no trace was taken); the static resolution of virtual and
  interface calls; calls made by native code back into script (the entry list stands in for them).
- Finding, left unchanged here: `ObjectConst` of a class export evaluates to a stand-in object because class exports have
  class reference 0 (`src/interp.cpp`), so static calls through class constants (e.g. `GetWillowGlobals`) do nothing in the VM.
- Sensitive file: `CMakeLists.txt`, one source (`src/census.cpp`) added to `ow-core`; nothing else changed.

## 2026-10-05: native notes for the mission script bridge: accept, complete, kickoff, rewards (UNVERIFIED)

AI-assisted (Claude), analyst lane C1, Ghidra; own-words note `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md`, no
listing text. Accept and turn-in are script (`WillowPlayerController.AcceptMission` / `ServerCompleteMission`) driving the
natives `MissionTracker.ActivateMission` / `CompleteMission`, which share one status routine. That routine calls back into
script (`WillowPlayerController.UpdateMissionStatus` on each local controller) before observers and the `Default` event (id
6 + status). Rewards are granted by script: `UpdateMissionStatus` → `ServerGrantMissionRewards` → credits, `ExpEarn`,
items, reward UI. The kickoff after acceptance is a pending record the tracker's tick consumes (`Default` id 12 one tick
after id 7), answering an open item of NATIVE_MISSION_DISPATCH. `ExpEarn` only raises the pool; the level-up comes from
the pool's update. Corrections to NATIVE_MISSION_DISPATCH B4 (hook order) and NATIVE_PROGRESSION (`ExpLevelUp(bCheated)`)
are recorded in the new note with pointers from both. Every rule UNVERIFIED in game except the 395 XP amount (2026-10-02).

## 2026-10-05: native notes on loot rolls (UNVERIFIED) and a native index

AI-assisted (Claude), analyst lane G1, Ghidra; own-words note `docs/verification/NATIVE_LOOT.md`, no listing text. How an
item pool picks entries (one flat weighted list per pool, stage window, `Quantity` draws with replacement, nested pools
recurse), how a balance becomes an item (first grade window containing the capped stage, spawn-modifier interpolation,
level = capped stage), enemy drop lists and the death roll, mission reward items. Confirms that the Fire mission's stock
data carries no reward item, so the turn-in loot remains a labelled host stand-in. The note lists corrections to
`tools/loot_pools.py` and NATIVE_WEAPON_RULES section 4 (not applied yet). All UNVERIFIED in game.
`docs/verification/NATIVE_INDEX.md` starts a one-row-per-native index over the note files (mission script bridge and loot).

## 2026-10-05: native notes on stock Engine/Core natives: timers, spawn, iterators, states (UNVERIFIED)

AI-assisted (Claude), analyst lane G3, Ghidra; own-words note `docs/verification/NATIVE_ENGINE_CORE.md`, no listing text.
Actor timers (replacement, clear-by-zero-rate removed on the next update, strict greater-than firing, loops firing
`floor(count/rate)` times per pass, tick order Tick → state code → timers → LifeSpan → physics), `Spawn` (the tag is not
used; event order GainedChild → PreBeginPlay → PostBeginPlay → SetInitialState) and `Destroy`, the actor iterators (lazy
walks, start indices, filters), `GetALocalPlayerController` / `GetWorldInfo`, and the state machine (EndState/BeginState
order, default label `Begin`, push/pop events, probe-mask gating by `Enable`/`Disable`, latent `Sleep`). Much of the
engine-side machinery has no registered name and was identified by call structure; the note marks those identifications.
Differences from our code are listed in the note (`src/mover.cpp` timer validation, `src/natives_core.cpp` state model)
and are not changed yet. All UNVERIFIED in game.

## 2026-10-05: VM: a class constant evaluates to its class

AI-assisted (Claude), implementer lane I1. `EX_ObjectConst` in `src/interp.cpp` decided "is a class" by the name of the
export's class reference; class exports have class reference 0, so class constants became stand-in objects and static
calls through them (e.g. `GetWillowGlobals`) did nothing. Class reference 0 now means a UClass (the rule `vm.cpp`'s
`classNameOf` already used). Verified: new synthetic case in `tests/vm_test.py` (fails before, passes after); the
real-data `--slice-run` and `--inventory-move` outputs are byte-identical before and after; the mover scripts' only class
constant sits in a `foreach` header the interpreter skips. Door suite not rerun for this step.

## 2026-10-05: script swap 1: Fire mission accept and turn-in run the installed controller script (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, written from `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md` only (no
analysis output opened). Phase 2 step D, first stand-in replaced.

- `FireMissionSlice::accept` runs `WillowPlayerController.AcceptMission` and `turnIn` runs `ServerCompleteMission` on a VM
  controller (authority role, `WorldInfo.GRI.MissionTracker` → a VM `MissionTracker`) with the installed mission object.
  New `src/mission_script.*` binds, to the bridge's own objects only, `MissionTracker.ActivateMission`, `CompleteMission`,
  `PlayTurnIn`, `GetMissionStatus` per the note, driving `MissionSystem`, which stays the single owner of mission state.
  Each status change calls the script `UpdateMissionStatus` and `TriggerMissionStatusChangedDelegates` before observers and
  the `Default` event. The host stand-in "kickoff at once" is replaced by the note's pending record consumed on the next
  tick (the 22 host events on real data are unchanged; the three kickoff events move from accept to the next tick).
- Inferred without a note (labelled in code): `MissionTracker.IsDataValid` (true), `NativeGetMissionIndex` (list lookup).
  Stand-ins kept: `ExpEarn` only records its call (the host still grants XP); `GetExperienceReward` returns the host's
  amount. On real data the script reaches `ExpEarn(395, SideMissionAward)`, equal to the host's 395 by construction.
  Not modelled: `CompleteMission`'s chain, untracking, unlock queue and fast-forward prompt. The slice refuses a turn-in
  that is not ReadyToTurnIn (the real screen offers it only when `CanEndMission` holds).
- 19 natives remain logged stubs on this path (listed in `SANCTUARY_RPG_MISSION.md`, "Script swap 1"); their zero results
  match the Fire mission's data. Known VM gap: static-array fields inside a zero struct are not materialised (reward
  struct reads log out-of-bounds; harmless for this mission).
- Checks: CTest 11/11 (new `mission-script-synthetic`), packages 9/9, UE module build Succeeded, quest suite first run
  PASS 80/80 (79 before, one new check) and resume PASS 11/11. Sensitive file: `CMakeLists.txt` (one source and one test
  added). All native rules UNVERIFIED in game.

## 2026-10-05: native notes on mission-linked behavior conditions, population spawning and kernel leftovers (UNVERIFIED)

AI-assisted (Claude), analyst lane G2, Ghidra; own-words note `docs/verification/NATIVE_BEHAVIOR_POPULATION.md`, no listing
text. `BehaviorSequenceEnableByMission`: every `MissionReaction*` recomputes the verdict from the tracker's state and
re-applies Enable/Disable to all linked sequence records (events fire only on a real change); objective states
NotStarted/Active/Complete are derived with mission-status gating; `ObjectiveSetRestrictions` apply; the condition
registers as an observer at provider registration and gets an immediate level-load call (before `OnSpawned`).
`SequenceEventEnableByMission` uses the same verdict. Population: `MissionPopulationAspect` is script; the Fire den is
enabled only while its objective is active and spawns on a later master tick under radius/height/capacity/time gates;
the spawn order up to `OnSpawned` is recorded. Kernel: a thread stops at the next behavior boundary when its sequence is
disabled; context-list resolution; latent copies with a 1/60 s minimum wait; `FilterObject` only with a caller filter.
Corrections recorded in the note: the enable rule in `src/slice.hpp`/BEHAVIOR_DATA_DECODE (not applied yet), the
set-changed notification source (NATIVE_MISSION_DISPATCH B3), and `tools/ghidra/class_layout.py` offsets on
`WillowPawn`-derived classes (4 bytes lower than the executable from `ConsumerHandle` on; cause not investigated). All
UNVERIFIED in game.

## 2026-10-05: native notes on controller and helper natives the mission script calls (UNVERIFIED)

AI-assisted (Claude), analyst lane G4, Ghidra; own-words note `docs/verification/NATIVE_CONTROLLER_HELPERS.md`, no listing
text. Covers the census's top helpers: `GetCurrentPlaythrough` (GRI's `CurrentPlaythrough`, else 0),
`NativeGetMissionIndex` (first `MissionList` entry with the same `MissionDef`, else -1; matches the provisional
implementation of script swap 1), `MissionTracker.IsDataValid` (the tracker's `bDataValidated` bit set by `ValidateData`,
not a constant: the provisional `true` is a stand-in), `GetHUDMovie`, `CanAffordToUseUsableObject` / `PayForUsedObject`
and the currency caps, `GetPawnInventoryManager`, `IsPrimaryPlayer`, `WorldInfo.IsMenuLevel`, the globals getters,
`WillowAIPawn.IsComponentUsable`, `Object.Localize` (search roots, `INT` fallback, `?INT?Package.Section.Key?` for a
missing entry) and `Object.QueryInterface`; presentation-only natives are marked as such. All UNVERIFIED in game.

## 2026-10-05: native notes on the UnrealScript–Scaleform bridge (UNVERIFIED)

AI-assisted (Claude), analyst lane G5, Ghidra; own-words note `docs/verification/NATIVE_GFX_BRIDGE.md`, no listing text.
The contract the host's Ruffle adapter must meet: the `ActionScript*` family forwards the calling script function's own
parameters and converts the result to its return type (conversion tables in the note; missing movie or path is a silent
no-op); `Invoke`/`SetVariable*`/`GetVariable*` conversions by `ASType`; ActionScript→script calls through
`ExternalInterface.call` (named script function on the player's `ExternalInterface` object), `SetFunction` bindings,
`fscommand` and the CLIK widget hooks (the VM must be re-entrant there); wrapper lifetime and `Close` event order; markup
translation in `SetText`; `PlayUISound`; `FocusOn` (presentation only); `QuestAcceptGFxMovie.UpdateMissionTextList` (the
accept screen's category headers). Scaleform's own semantics (paths, sticky variables, conversions) were not read. All
UNVERIFIED in game.

## 2026-10-05: script swap 2: Fire mission experience through the script path (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_PROGRESSION.md and NATIVE_MISSION_SCRIPT_BRIDGE.md only. The host's
own XP computation is gone: `UpdateMissionStatus(Complete)` → `ServerGrantMissionRewards` → `GetExperienceReward` +
`ExpEarn(amount, 4)` on the VM, then the pool update runs the script `ExpLevelUp` → `OnExpLevelChange`.

- New `src/progression.*`: a bounded attribute evaluator (constant, simple-math and global-level resolvers, value formula,
  conditional on `PlayThroughCount`, range and rounding; any other shape throws) and the experience curve.
  `GetExperienceReward` = `trunc(float(span × percentage × m))` at the mission's locked game stage (m = 1 first playthrough
  below 50; other cases throw "not implemented"). `ExpEarn` scales (taken as 1: not decoded), clamps to the maximum level's
  XP and never decreases a VM-side pool. Also from the bridge note: `GetGameStage`, `GetMaxExpLevel`,
  `GetExpPointsRequiredForLevel`, `GetCurrencyRewardType`, `GetCurrencyReward` (multiplier 0 only),
  `ShouldGrantAlternateReward`, `GetItemRewardsForPlayer` (empty rewards only; pool rolls logged as not implemented).
- Design: the VM owns the pool and level; the host keeps Maya's display, skill points, health and save, pushes its inputs
  (region stage, level, experience) before accept and turn-in, applies the `Experience` event and compares the `Level`
  event with its own level. The host's earlier formula remains only as the suite's oracle.
- VM fix: struct default tags fill a static array (`ArrayDim` > 1) element by element (before, the last tag replaced the
  whole field). `ProviderDefinitionPathName.PathComponentNames` became an array as a result; `providerPathLeaf`
  (`src/behavior.*`) reads its last name for the two users. The slice's 22 host events are unchanged.
- Real data: 395 XP at stage 8 (the amount confirmed in game on 2026-10-02), 316 at stage 7; level 8 → 9 by the script from
  27,900 XP. Turn-in stubs 19 → 14.
- Checks: CTest 11/11 (synthetic invented curve, locked stage, level-up, clamps; static-array struct case), packages 9/9,
  UE build Succeeded, quest suite first run PASS 81/81 (new check `script_pool_update_levels_up_to_host_level`) and resume
  PASS 11/11, door suite PASS 16/16, inventory suite PASS 49 / FAIL 0 / NOT_RUN 0 / KNOWN_DIVERGENCE 0. Sensitive file:
  `CMakeLists.txt` (`src/progression.cpp` added). UNVERIFIED: XP scales = 1, the conditional "all expressions hold",
  the BaseValueMode numbering, everything but the 395 amount.

## 2026-10-05: native notes on the dialog system: event selection, talker, priority, line end (UNVERIFIED)

AI-assisted (Claude), analyst lane G6, Ghidra; own-words note `docs/verification/NATIVE_DIALOG.md`, no listing text.
`Behavior_TriggerDialogEvent` selects Out (id 0) on its first run and Finished (id 1) only once the event's talk act is no
longer live (polled every 0.1 s; at once when no line starts; immediate mode selects both), so the host stand-in that
selects both at once is wrong in timing. Event selection within a group, template-event wiring (a data-layout check on
the Fire group agrees with SLICE_AUDIO_CHAIN's pairing), talker resolution (talker variable, instigator, random
`TalkData` by name tag; echo callers), the priority rule (index in the globals' `Priorities`, tracked-mission floors), and
the line end (Wwise playing id stops, then `OutputDelay`). Predicts that the Fire mission's first objective set starts
after Marcus's first line ends, not at the kickoff. All UNVERIFIED in game.

## 2026-10-05: native notes on skills: points, upgrades, grade effects, cooldown (UNVERIFIED)

AI-assisted (Claude), analyst lane G7, Ghidra; own-words note `docs/verification/NATIVE_SKILLS.md`, no listing text. Skill
points are awarded by the script `ExpLevelUp` (`GeneralSkillPoints += PointsPerLevelUp` after the level rises; the
confirmed `max(0, L − 4)` total follows) and announced through `FireSkillPointsChangedDelegates`. Spending: UI
`RequestSkillUpgrade` → `CanUpgradeSkill` → `ServerUpgradeSkill` → native `PlayerSkillTree.UpgradeSkill` (refusals, tier
unlock by summed `PointsToUnlockNextTier`, child branches; the level-5 gate is UI-only). Grade to effect:
`Base + PerGradeUpgrade × ((g − start) div max(interval, 1)) + bonus` (bonus = the single best `BonusUpgradeList` entry),
modifiers applied through the attribute stack and refreshed on the next tick after a grade change. Cooldown pool refill and
drain (the Phaselock pause reading is consistent). `tools/skill_stats.py` lacks the bonus rule and defaults
`GradeToStartApplyingEffect` differently (not changed yet). All UNVERIFIED in game.

## 2026-10-05: script swap 3: mission-linked sequence conditions and the controller helpers (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_BEHAVIOR_POPULATION.md section 1 and NATIVE_CONTROLLER_HELPERS.md
only.

- The dummy's `BehaviorSequenceEnableByMission` conditions replace the earlier UNVERIFIED rule in `src/slice.*`:
  `MissionSystem` raises the tracker's observer notifications (status change after the script hook and before `Default`,
  active-set switch, objective progress, objective complete), and every notification recomputes each condition (objective
  state with mission-status gating, objective-only bit for objective-specific conditions, `ObjectiveSetRestrictions`,
  mission-level bits) and applies Enable/Disable, with events only on a real transition. The provider registers as a
  consumer at `spawnDummy` (bEnabledOnSpawn pass, then the immediate level-load verdict, then `OnSpawned`); nothing observes
  the mission before the spawn. `bSequenceEnabledMutex` and the enabled/disabled event order follow the note (no dummy
  sequence sets the flag; synthetic test only).
- Real data: the 22 host events are identical and in the same order, the final enabled sequences are the same. New:
  `Idle`'s enabled event now fires at registration and reaches `Behavior_SpecialMove` (an animation request), listed as a
  host-boundary call that the host does not run yet.
- Helpers: `IsDataValid` reports the tracker's `bDataValidated` flag, set by running the script `ClientValidateMissionData`
  once when the VM graph is built (a slice shortcut: the standalone trigger is not known); `GetCurrentPlaythrough`,
  `NativeGetMissionIndex` (range-checked), `IsPrimaryPlayer`, `GetHUDMovie` (None), `IsMenuLevel`, the globals getters and
  `GetGlobalsDefinition`; `UpdateLcdMissionStatus` and `PlayUIAkEvent` as documented no-ops. Turn-in stubs 14 → 12 (7 gone,
  5 newly reached further down the script: `Localize`, `AllExpansionSideMissionsComplete`, `IsLocalPlayerController`, a
  `SpawnPlayerMovie` call on a data-object stand-in, an engine iterator).
- Not modelled: per-instance objectives, `bInstanced`, the Kismet twin `SequenceEventEnableByMission`, the
  RequiredObjectivesComplete / Failed statuses, `ObjectiveCleared`.
- Checks: CTest 11/11 (new synthetic slice scenario on invented packages), packages 9/9, UE build Succeeded, quest suite
  PASS 81/81 and resume PASS 11/11, door suite PASS 16/16. No sensitive file touched.

## 2026-10-05: native notes on the use key: usable choice, mission giver, accept screen (UNVERIFIED)

AI-assisted (Claude), analyst lane G8, Ghidra; own-words note `docs/verification/NATIVE_USE_INTERACTION.md`, no listing text.
The current usable object comes from one view ray of `PlayerInteractionDistance` (350 uu in the stock globals), first
usable hit nearest-first, a blocking hit ends the search, no angle cone or type priority, at most 30 updates per second
(so the host's 250 cm reach and sweep are not the stock rule). The use key runs script (`Use` → `ServerUse` →
`PerformedUseAction` → `CanAffordToUseUsableObject` → `UseObject` → `PayForUsedObject`); `WillowAIPawn.UseObject` fires
the AI `OnUsed` events (Generic, then HasMissions/NoMissions); Marcus's `Brain` behavior falls through his story checks to
`Behavior_ShowMissionInterface`, which opens the mission screen with Marcus as context; the screen's buttons call the
`AcceptMission` / `ServerCompleteMission` scripts already running on the VM (script swap 1). Talk state: `BeginUse` /
`EndUse` at screen open/close, 30 s linger, look-at, focus camera. The trace flag word is recorded as a number with a
labelled guess at its bits. All UNVERIFIED in game.

## 2026-10-05: native notes on how the Fire mission's objectives are progressed (UNVERIFIED)

AI-assisted (Claude), analyst lane G9, Ghidra; own-words note `docs/verification/NATIVE_OBJECTIVE_TRIGGERS.md`, no listing
text. GoToRange is completed by a placed `WillowWaypoint` (its cylinder `Touch` by a player-owned pawn, gated by
`IsMissionObjectiveActive`, then `UpdateObjective` without a player argument); the waypoint also observes the mission and
re-checks actors already inside when the active set changes (a host that only checks on entry misses a player standing in
the range). The tracker fans out `UpdateMissionObjective` to every local controller. Fire: `Pawn.TakeDamage` →
`NotifyTakeHit` → the AI controller fires `OnTakeDamage` on the AI definition's then the AI class's provider, with
`DamageType` = the pipeline's damage type definition and a threshold filter; `Behavior_UpdateMissionObjective` is script.
`AdvanceObjectiveSet`, `MissionRemoteEvent`, `ClearObjective` act only with the tracker as consumer; mission remote events
match by name and mission. Corrections to C1/G2 notes and two slice records are listed in the note. All UNVERIFIED in game.

## 2026-10-05: script swap 4: Marcus's dialog through the stock trigger, poll and line-end rules (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_DIALOG.md and NATIVE_BEHAVIOR_POPULATION.md section 3 only. Replaces
the stand-in where `Behavior_TriggerDialogEvent` selected Out and Finished at once.

- Behavior kernel (`src/behavior.*`): latent behaviors (`run().wait`, 1/60 s minimum, per-thread state); `tick()` steps to
  each due wake, so a long frame passes every wake and poll; a waiting thread whose sequence was disabled ends.
- New `src/dialog.*`: Out on the first run, the dialog triggered one kernel wake later, Finished on the first 0.1 s poll
  where the talk act is no longer live (`bForcePlayImmediate`: Finished then Out at once); event selection (last enabled
  entry for the tag, inline act or link-table act), talker (registered pawn by exact name tag, else echo caller), priority
  arbitration (index in `Priorities`, tracked-mission floor), line end from a host line-player interface plus
  `OutputDelay`. With no line player (no audio device) no line starts and Finished follows on the triggering wake, as the
  note describes. Nodes not on the Fire route throw "dialog: not implemented".
- Host: each chosen line is a host event (tag, group, talker, talk act, AkEvent, outcome); the host resolves the AkEvent in
  its audio manifest (new check `dialog_lines_name_the_manifest_ak_events`) and registers Marcus as the pawn talker (his
  name tag is a host choice: the manifest's most common talker tag). Nothing is played.
- Real data: the 16 non-dialog host events are identical and in order; Marcus's walk now starts about three frames after
  the accept (kickoff on the next tick, dialog one wake later) instead of inside the accept call. With a test line player
  the first objective set waits for the line's end, as the note predicts. Two lines (03a, 03b) now report "no talker":
  their acts name the instigator, and what context a mission behavior has is not in the note (open).
- Quest suite: step 4 records `use_key_accepts_mission` at once and waits up to 2 s for the walk before the two unchanged
  walk checks (timing follows from the note). Checks: CTest 11/11 (new dialog scenario on invented data), packages 9/9,
  UE build Succeeded, quest PASS 82/82 and resume PASS 11/11, door PASS 16/16. Sensitive file: `CMakeLists.txt`
  (`src/dialog.cpp` added).

## 2026-10-06: native notes on pickup, backpack, equip and the mission weapon (UNVERIFIED)

AI-assisted (Claude), analyst lane G12, Ghidra; own-words note `docs/verification/NATIVE_INVENTORY_EQUIP.md`, no listing
text. Pickup, backpack readying, quick-slot assignment and `WillowPawn.EquipItem` are script the VM already runs; the natives
are thin: backpack count and limit are stored numbers (limit default 12, `SetInventoryMaxSize` clamps to at least 12),
`GetWeaponReadyMax` is a stored value, new weapons take the first free quick slot 1..4 with 3 and 4 gated by the maximum,
`FindLeastValuableWeapon` picks which readied weapon yields; equip/unequip/pickup natives only raise behavior events. A
mission-balance weapon has required level 0, cannot be dropped, sold or saved, is refused from the backpack once its mission
is inactive and is taken back through the script event `RemoveMissionWeapons`. Save fix-up floor of weapon slots: 2, +1 per
completed plot mission 5 and 10, at most 4. Open: new-character `WeaponReadyMax`, SDU values, slot-effect natives, the
"seen pickup" decision. All UNVERIFIED in game.

## 2026-10-06: native notes on the hit pipeline, OnTakeDamage, status effects, death and respawn (UNVERIFIED)

AI-assisted (Claude), analyst lane G11, Ghidra; own-words note `docs/verification/NATIVE_DAMAGE_DEATH.md`, no listing text.
Order of one hit: `TakeDamage` chain → precondition → native `AdjustDamage` pipeline (per-stage factors recorded in
`DamageEventSummary`: AI scale, source, element, level difference, backstab, hit region and crit, shield, protection timer,
caps) → Kismet damage events → health → `NotifyTakeHit` (raises `OnTakeDamage` after the health change, outputs Instigator,
Damage, ShieldDamage, DamageSource = the shot's source class, DamageType = the pipeline's definition; the AI-class raise
filters on `Damage + ShieldDamage < DamageThreshold`) → status-effect roll → depleted or hit presentation. Incendiary status
ticks report the Status damage type, so only the Impact hit matches the dummy's Fire chain. AI death order (tracker
`NotifyPawnDied` first, loot drop, kill XP), player down state (bleed-out 12/8/4/1 s, second wind floors), respawn at full
health and shield, death fee 0.07 × credits rounded half up. Corrects the slice record on `DamageSource` and G2's open item on
the event filter. A local class_layout copy sized `MapProperty` as 60 bytes (assumption, checked only by field names landing).
All UNVERIFIED in game.

## 2026-10-06: native notes on weapon firing, ammo, accuracy, recoil and reload (UNVERIFIED)

AI-assisted (Claude), analyst lane G13, Ghidra; own-words note `docs/verification/NATIVE_WEAPON_TRIGGER_AMMO.md` (named NATIVE_WEAPON_FIRING.md until the 2026-11-02 merge), no listing text;
builds on NATIVE_WEAPON_RULES.md. A press fires once in `WeaponFiring`, then on a timer re-armed per shot with
`GetFireInterval`; auto-reload one interval after the last round. The slice pistols fire Bullet (a light projectile with
travel time), not hit-scan; range `WeaponRange` 16,384. Clip and pool are separate counters (clip = max(min(clip, ClipSize,
pool) - ShotCost, 0)). Accuracy: a player pool cone (max 12, min 2, impulse per shot, decay 8/s after 0.2 s) scaled by
`1 - ZoomEffect` plus a per-pellet `Spread` cone that zoom does not remove. Recoil kick = impulse × 182.0444 scaled by zoom
and FOV ratio, decaying over `WeaponKickRecoveryTime`. Reload refills the clip at `ReloadTime × ReloadCompletePercent`;
melee, grenade, swap and action-skill start stop it (the Phaselock abort matches the 2026-10-02 observation). Open:
held-trigger burst restart, interrupted-reload refill, pass-through. All UNVERIFIED in game except the cited observation.

## 2026-10-06: native notes on Matinee playback, move and event tracks, remote events and Marcus's walk (UNVERIFIED)

AI-assisted (Claude), analyst lane G10, Ghidra; own-words note `docs/verification/NATIVE_KISMET_MATINEE.md`, no listing text.
`SeqAct_Interp`: position advances by PlayRate × dt and the clamped end is applied exactly; Completed/Reversed is raised one
frame after the last update and only within 0.0001 of the end or of 0 (a stop mid-run fires neither); `bRewindOnPlay` resets
before group instances are built. Move tracks evaluate Hermite keys with stored tangents (the Fire door and target tracks
play as smoothstep over 1.5 s and 2.0 s); pose = key × initial × base, with the initial transform captured at every start
from stopped so the actor starts from its placed pose. Event-track keys fire over half-open intervals by direction. Remote
events fire every enabled same-name event across loaded sequences; mission remote events match mission and name. Marcus's
walk: random destination, arrival/leaving events per node, Finished only at the chain's end or on abort. Corrections listed
in the note: `src/mover.hpp`'s first-key-delta reading differs when keys rotate; `src/kismet.cpp` fires Completed a frame
early; `tools/ghidra/class_layout.py` cannot size classes with map fields. Open: how the runtime reaches the move-track pose
code, Marcus's arrival radius and speeds, move-node link order. All UNVERIFIED in game.

## 2026-10-06: script swap 5: GoToRange through the stock waypoint script (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, finished by the orchestrator after the subagent hit a usage limit; both worked from
NATIVE_OBJECTIVE_TRIGGERS.md only. The placed `WillowWaypoint_9` runs its own script on the VM (observer registration, `Touch`, the
set-changed re-check over `Touching`); new natives `Actor.IsPlayerOwned`, `MissionTracker.IsMissionObjectiveActive` /
`IsMissionObjectiveComplete` / `IsObjectiveSetActive` / `UpdateObjective` / `RegisterMissionObserver`, and the controller's
`UpdateMissionObjective` after each applied update. The host only reports overlap begin/end of the player and Marcus. VM: interface
casts succeed when the class defines every function of the interface (structural stand-in; the implemented-interface table is not
decoded, UNVERIFIED). Not yet swapped: the dummy's `Behavior_UpdateMissionObjective`. Checks: CTest 11/11 (new scenario F), packages
9/9, UE build Succeeded, quest PASS 83/83 and resume PASS 11/11, door PASS 16/16. No sensitive file touched.

## 2026-10-06: script swap 5, second commit: the Fire objective through the stock behavior script (UNVERIFIED rules)

AI-assisted (Claude), orchestrator acting as implementer from NATIVE_OBJECTIVE_TRIGGERS.md and NATIVE_ENGINE_CORE.md only. The dummy's
`Behavior_UpdateMissionObjective` runs its own `ApplyBehaviorToContext` on the VM instead of the slice's direct update; new binding
`BehaviorBase.GetWorldInfo` (the bridge's world), and the VM world's `Role` is authority. The context object is None (no VM dummy
pawn), which yields bit 0 as a pawn context does. Checks: CTest 11/11 (toy WorldInfo is now an Actor, as the stock class is),
packages 9/9, UE build Succeeded, quest PASS 83/83 and resume 11/11, door PASS 16/16. No toy coverage of the script behavior yet.

## 2026-10-06: Matinee Completed / Reversed one frame after the last pose (UNVERIFIED rule)

AI-assisted (Claude), orchestrator from NATIVE_KISMET_MATINEE.md ("Deactivated": a finished `SeqAct_Interp` is noticed on the next
update). The host mover (`OpenWillowMover.cpp`) now applies the end pose, then on the next frame runs `InterpolationFinished` and
fires Completed / Reversed, for both the door and the target track; a Play / Reverse arriving in between keeps the action running
and suppresses that output, as the note's update order implies. `DoorRunning` / `TrackRunning` stay true until the deactivation.
Checks: UE build Succeeded, door PASS 16/16, quest PASS 83/83 and resume 11/11 (CTest and packages unaffected: host only). The
note's other correction (the move-track pose composition differs when keys rotate) is not applied yet.

## 2026-10-06: script swap 6a: the use key chooses Marcus with the stock use ray (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1 from NATIVE_USE_INTERACTION.md only. The host's 250 cm reach stand-in is replaced by one
ray from the camera along the view, `GlobalsDefinition.PlayerInteractionDistance` long (read from the installed data by the VM
bridge: 350 uu), first usable hit before a blocking hit, no radius or cone. Marcus's mesh has no collision, so the host adds a
query-only capsule from the mesh bounds that blocks only the visibility channel (host-chosen shape, UNVERIFIED). Not modelled: the
1/30 s throttle, blocking flags, icons and prompts. The use key still accepts and turns in through the slice's direct calls; 6b
(Marcus's `OnUsed` chain as script) waits for a note on the native `Behavior_IsSequenceEnabled`, which opens his chain. Checks:
CTest 11/11, packages 9/9, UE build Succeeded, quest PASS 86/86 (three new use-ray checks) and resume 11/11, door PASS 16/16.

## 2026-10-06: native notes on save and load of player and mission state (UNVERIFIED)

AI-assisted (Claude), analyst lane G15, Ghidra; own-words note `docs/verification/NATIVE_SAVE_LOAD.md`, no listing text, no real
save opened. `MissionTracker.InitializeWorldMissionState` rebuilds the tracker from the controller's mission list on every load
(reset, apply in order, drop entries whose progress length mismatches, defer unmet dependencies, re-register listeners and re-grant
the mission weapon for Active/RequiredObjectivesComplete, re-evaluate, replay behavior events, observer kind 0). Pre-load fixup
clamps level, backpack (12..39), bank (6..24) and currencies and recomputes unspent skill points as max(0, L-4) minus points spent.
Save triggers: station use, status-menu close, quit, preference/playthrough changes; mission events never save. Load order and the
save's contents are listed; a mission saved Complete with `bNeedsRewards` still set pays again on load. Corrections to the
controller-helper, inventory and skills notes are recorded in the note. Open: whether the lent pistol returns for the standalone
primary player, DLC level-cap increments, the world-readiness test. All UNVERIFIED in game.

## 2026-10-06: native notes on the action skill runtime (Phaselock activation, lifetime, cooldown) (UNVERIFIED)

AI-assisted (Claude), analyst lane G17, Ghidra; own-words note `docs/verification/NATIVE_ACTION_SKILL.md`, no listing text. Key
press → `StartActionSkill` → `ServerStartActionSkill` (refusals: ladder, cooldown pool above 0 or skill active, vehicle seat; a
second press during the lock or cooldown does nothing); injured and weapon state are the skill's constraints inside
`Skill.Activate`, no target is not a refusal (the skill fizzles). Activate order: constraints, spawn the action-skill actor, the
skill's `OnActivated`, then the state delegate that refills the 13 s cooldown pool; the drain starts at `OnReleasedTarget`. The 120 s
`InitialDuration` is a backstop that asks once to deactivate. Base first-lock timeline (cast, lock 0.7 s, release 5.7 s, end 6.7 s,
ready 18.7 s) and every early end are listed. The reload abort comes from the cast special move's `bBlocksWeaponActions`, which
corrects NATIVE_WEAPON_TRIGGER_AMMO; other corrections (NATIVE_SKILLS, PHASELOCK_STOCK_DATA, NATIVE_PHASELOCK_PRESENTATION) are in the note.
All UNVERIFIED in game.

## 2026-10-06: native notes on the attribute system (resolvers, modifier stacks, change notification) (UNVERIFIED)

AI-assisted (Claude), analyst lane G14, Ghidra; own-words note `docs/verification/NATIVE_ATTRIBUTES.md`, no listing text. Context
resolver chains stop at the first None; value resolvers fold forward; `GetValue`/`GetBaseValue` try the override context first.
Only `ObjectPropertyAttributeValueResolver` accepts add/remove/set-base (effects on constant or derived attributes do nothing). Stack
attributes keep value, base and a modifier stack (a second add of the same modifier is refused, remove deletes every copy, value
recomputed with the NATIVE_WEAPON_RULES stack formula, integers truncated); simple attributes edit the value in place (Scale
multiplies directly; remove is a no-op). The change notification fires only for notify-flagged properties (`ResourcePool` clamps,
`Skill.Grade` forces a modifier refresh). About 26 resolvers/natives mapped; `ApplyAttributeEffects` is script. Corrections to
NATIVE_PROGRESSION, NATIVE_SKILLS, NATIVE_WEAPON_RULES and PHASELOCK_STOCK_DATA are in the note. Open: slot `ComputedModifierValue`,
how typed attribute assignments execute in the VM, resolver chain order (object arrays not decoded). All UNVERIFIED in game.

## 2026-10-06: native notes on Marcus's use chain (`Behavior_IsSequenceEnabled` cascade) (UNVERIFIED)

AI-assisted (Claude), analyst lane G18, Ghidra; own-words note `docs/verification/NATIVE_MARCUS_USE_CHAIN.md`, no listing text.
Correction: `Behavior_IsSequenceEnabled`, `Behavior_HasMissions`, `Behavior_PlayAIMissionContextDialog`, `Behavior_ShowMissionInterface`
and `Behavior_RemoteCustomEvent` are script, not native; the VM runs them. The natives behind them: `BehaviorKernel.IsBehaviorSequenceEnabled`
(true only when the consumer has that provider registered, the provider has a sequence of that name and its enabled bit is set; a None
provider or an unknown name is false), `BehaviorHelpers.ResolveBehaviorProviderDefinitionReference` (non-empty path wins, else the reference,
else the behavior's Outer) and extra detail on `ActivateBehaviorEventFromScript` (None provider fires nothing, omitted filter means all
links). Check outputs: 0 enabled, 1 not enabled. Marcus's nine checks are a cascade ending in the context dialog, `Behavior_HasMissions` and
the mission interface; three of the names have no sequence in his provider. `OnUsed` has one output link (id 2) and writes the instigator
into `Brain`'s `PlayerWhoUsedMe`. This unblocks script swap 6b. Corrections to SANCTUARY_RPG_MISSION, NATIVE_USE_INTERACTION and
NATIVE_MISSION_DISPATCH are listed in the note. All UNVERIFIED in game.

## 2026-10-06: native notes on player movement, sprint, crouch, jump, falling and FOV (UNVERIFIED)

AI-assisted (Claude), analyst lane G16, Ghidra; own-words note `docs/verification/NATIVE_PLAYER_MOVEMENT.md`, no listing text. Maya's
cooked movement data (GroundSpeed 440 from the class default, AirSpeed 500, JumpZ 630, WalkingPct 0.3, CrouchedPct 0.5, MaxFallSpeed
1500, PlayerAirControl 0.11, cylinder 80 x 42, crouch 50, eye height 70, sprint FootSpeed +0.35 = 594), the script rules for sprint
entry and exit, crouch, jump and falling damage, and native behaviour in our own words: pawn speed fraction, velocity step and
braking, falling step, eye-height and bob update, WeaponBob, view location and the FOV helpers. Supersedes the Walker's estimates
(450 / 420 / 650 / 45 degrees). Correction to NATIVE_WEAPON_TRIGGER_AMMO: sprint does add to the accuracy pool. Open: the native that copies
class speeds onto the pawn, walking/ladder/step physics, and a measured jump (gravity -500 with JumpZ 630 predicts a high apex).
All UNVERIFIED in game.

## 2026-10-06: script swap 6c, Marcus's mission lists run as script (UNVERIFIED natives)

AI-assisted (Claude), implementer lane I1. The accept / turn-in screen's list logic runs as the game's script on a VM Marcus built from his
stock archetype (`GD_Marcus.Character.Pawn_Marcus`, his own directive table): `GetEligibleMissions`, `GetInProgressMissions` and
`GetRedeemableMissions` run unchanged. The tracker's availability queries `CanStartMission`, `CanEndMission` and `GetCompletedBranch` are bound
to MissionSystem predicates that `accept()` shares, UNVERIFIED (read from native code, not confirmed in game). `AcceptMission` and
`ServerCompleteMission` now get Marcus as the director. The host's use key confirms the one offered entry (redeemable first), a stand-in for
the button press; the screen itself is not hosted. Not modelled: Failed, repeatable and blocked missions, RequiredObjectivesComplete; the
tracker holds one mission record, so Marcus's other six directive entries are never offered. Checks: ctest 11/11, verify_packages 9/9, UE
build succeeded, quest suite 88/88 + resume 11/11 (was 86 + 11), door suite 16/16. No new stubs reached. Verified: automated suites only.

## 2026-10-06: native notes on the Class export body and its interface table (UNVERIFIED)

AI-assisted (Claude), analyst lane G19, Ghidra; own-words note `docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md`, no listing text. The
Class export body for version 832/46: struct part (with script bytes), state part (probe mask, label offset, state flags, function map),
class part (class flags, where 16384 marks an interface; within; config name; component map), then the interface table (count, then
interface class reference and `VfTable_<Interface>` property reference, or null), several name lists, a few fixed-size fields of unknown
meaning, and the default object reference. An oracle script outside the repository fits all 3,339 Class exports of the nine code packages
exactly; it has not been run in-tree, so the layout stays UNVERIFIED until an in-tree oracle passes. Compared with the structural stand-in
`Runtime::implements`: no disagreement on IMission, IUsable, IMissionObjective or IMissionDirector, but 975 missed pairs (interfaces with no
functions of their own), 66 false positives (mostly IGFxMenuScreenTickable and IInstanceData), and 19 real interfaces whose names do not
start with I. Nothing confirmed in the running game.

## 2026-10-06: native notes on the Gearbox bytecode opcodes 4C-50, 5E and 5F (UNVERIFIED)

AI-assisted (Claude), analyst lane G20, Ghidra; own-words note `docs/verification/NATIVE_BYTECODE_OPCODES.md`, no listing text. Opcodes
4C-50 are typed hidden temporaries (int, float, byte, bool, object): an lvalue into a frame slot named by the function header's
local-variable array (frame offset and tag pairs; the operand indexes the in-memory array, which is the file array shifted by one).
Opcode 5E is the attribute value-property reference and runs the same handler as instance-variable opcode 01. Opcode 5F is "let
attribute": it writes the base companion, recomputes the value from base and modifier stack (NATIVE_ATTRIBUTES formula), calls a
replication hook and does not fire the attribute-changed notification. Package checks over the nine code packages: 4,061 functions,
7,839 pairs, every pair referenced, no operand out of range. Corrections: NATIVE_ATTRIBUTES assumed 5F notifies; `src/interp.cpp` treats 5F
as a plain Let (stale base) and leaves unset typed temporaries as None. Opcodes 57 and 59 carry a 16-bit operand after the array (neither
occurs in the packages). All UNVERIFIED in game.

## 2026-10-06: script swap 6b, the use key runs Marcus's stock OnUsed chain on the VM (UNVERIFIED natives)

AI-assisted (Claude), implementer lane I1, from `docs/verification/NATIVE_MARCUS_USE_CHAIN.md`. Pressing use on Marcus raises `OnUsed`
(link filter 2, payload instigator plus used component) on his AI-definition provider through the existing BehaviorProvider; the payload
fills `Brain`'s `PlayerWhoUsedMe` by connection index. The chain's behaviors (nine `Behavior_IsSequenceEnabled` checks, remote events, the
context dialog, `HasMissions`, `ShowMissionInterface`) run their own script. Bound natives, all UNVERIFIED: `IsBehaviorSequenceEnabled`,
`ResolveBehaviorProviderDefinitionReference`, `ActivateBehaviorEventFromScript`, `ActivateBehaviorOutputLink`, `Object.QueryInterface` (still
on the structural implements stand-in) and `WillowPawn.GetBehaviorConsumerHandle` (an accessor of the pawn's own field; no note describes it).
A small VM hook, `Runtime::overrideScript`, lets a host binding replace one presentation-only script RPC (`ClientGFxPlayMovie`); the host's
confirm of the offered entry (swap 6c) now runs from that event instead of directly from the key. On real data all nine checks answer "not
enabled" and the chain reaches the mission interface. Open: `BehaviorBase.GetBehaviorContext` is a stub, so the on-use dialog stays silent;
the spawn-time behaviors of his provider are not run. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 90/90 +
resume 11/11 (was 88 + 11), door suite 16/16. Verified: automated suites only.

## 2026-10-06: native notes on behavior context resolution and Marcus's on-use dialog (UNVERIFIED)

AI-assisted (Claude), analyst lane G21, Ghidra; own-words note `docs/verification/NATIVE_BEHAVIOR_CONTEXT.md`, no listing text.
`BehaviorBase.GetBehaviorContext` is a pure resolver over its arguments: the context struct's selector picks Self, Instigator, the other
participant or the struct's own ContextObject; event data or any other value gives None. It reads no kernel state. The behavior thread
runner fills that struct from an Input link (first resolved object, selector 4) before the behavior runs, and clears it afterwards; both
parts are needed for Marcus's on-use dialog. `WillowAIPawn.PlayOnUseDialog` (script) picks redeemable, eligible, in progress or no missions,
in that order, and triggers the matching generic on-use tag; it has no "first time" gate. `WillowPawn.GetBehaviorConsumerHandle` is a field
read; the field starts at -1 (invalid) and `InitializeBehaviorProviders` assigns it once. A Context link that resolves to nothing skips
the behavior for that pass. Corrections to NATIVE_BEHAVIOR_POPULATION, SANCTUARY_RPG_MISSION, NATIVE_USE_INTERACTION, SLICE_AUDIO_CHAIN and
NATIVE_MARCUS_USE_CHAIN are listed in the note. All UNVERIFIED in game.

## 2026-10-06: Class export bodies decoded; interfaces from the real table; typed temporaries and let-attribute (UNVERIFIED in game)

AI-assisted (Claude), implementer lane I2, from `NATIVE_CLASS_SERIAL_LAYOUT.md` and `NATIVE_BYTECODE_OPCODES.md`. Parsing change: the VM now
decodes every Class export body (`decodeClassBody` in `src/vm.cpp`) and keeps the class flags, within class, config name, interface table and
default object; the rest is consumed for its size only. The structural oracle `ow-package --class-check` (run by `tools/verify_packages.py`)
decodes 3,339 of 3,339 Class exports of the nine code packages exactly; every default object is `Default__<Class>` of its class, and all 367
interface entries name interface-flagged classes. The first body word is not resolved (AkAudio's WwiseSoundVolume points one past the export
table); no bounds check was loosened. `Runtime::implements` now answers from the interface tables of the class chain; the old structural rule
stays as `implementsStructurally`, used only for synthetic classes without a decoded body. On real data the slice's interfaces (IMission,
IUsable, IMissionObjective, IMissionDirector) give the same answers as before; across all classes 975 pairs become yes and 66 become no.
Interpreter: hidden typed temporaries (opcodes 4C-50) read as the typed zero when unset, with operands checked against the function header's
local-variable array; opcode 5F writes the attribute's base companion and recomputes the value from the modifier stack (truncating int and
byte) without the change notification; 5E was already the instance-variable read. Not yet: the attribute add/remove/set-base natives. Checks:
ctest 11/11, verify_packages 9/9 with all class bodies exact, UE build succeeded, quest suite 90/90 + resume 11/11 (rerun by the
orchestrator), door suite 16/16, no new stubs. Verified: the byte layout by the in-tree oracle; the meaning of the decoded fields and the
interpreter rules are UNVERIFIED in game.

## 2026-10-06: script swap 6d, Marcus's on-use dialog picks its tag as script (UNVERIFIED natives; line not played)

AI-assisted (Claude), implementer lane I1, from `docs/verification/NATIVE_BEHAVIOR_CONTEXT.md`. `BehaviorBase.GetBehaviorContext` is bound as
the pure resolver (selectors 0, 1, 2 and 4; anything else None; the instance-data path is not implemented). The behavior thread runner fills a
`BehaviorContextData` Input property (first linked object, selector 4) before a behavior runs and clears its object afterwards; a Context link
that resolves to nothing skips the behavior and follows only a supported default link. The VM Marcus's `ConsumerHandle` starts at -1 and is
assigned once at consumer registration; his mind (a bare `WillowMind` with his archetype's AI class) and dialog component come from his stock
data, so `WillowAIPawn.PlayOnUseDialog` runs its own guards and choice: MissionsAvailable while Fire is NotStarted, AllMissionsInProgress while
Active, MissionComplete at turn-in, NoMissions after. `GearboxDialogComponent.TriggerEvent` is reported as a host event, not played: the
two-step dispatch (generic group no-match, Trigger act, Marcus's own group) is not implemented in DialogSystem. Also bound:
`WillowDialogGlobalsDefinition.Get`. Checks: ctest 11/11, verify_packages 9/9 with class bodies exact, UE build succeeded, quest suite 92/92 +
resume 11/11 (was 90 + 11; rerun by the orchestrator), door suite 16/16. The `GetBehaviorContext` stub is gone; no new stubs. Verified:
automated suites only.

## 2026-10-06: script swap 6e, Marcus's on-use line chosen by the stock dialog data (UNVERIFIED)

AI-assisted (Claude), implementer lane I1, from `NATIVE_DIALOG.md` and `NATIVE_BEHAVIOR_CONTEXT.md`. The existing DialogSystem gains the
component `TriggerEvent` route: group search with the ParentGroup rule, the speaker as instigator, a Talk act's no-match output into a Trigger
act, and the Trigger act re-firing the event on the instigator's own groups. On real data the generic `VO_NPC_OnUse_*` tag has no Marcus entry,
so the no-match path fires Marcus's own `DET_NPC_OnUse_*` tag: NotStarted plays `Ak_Play_VOCT_Marcus_Quest_New`, Active `..._Quest_During`,
after completion `..._Quest_No_New`. ReadyToTurnIn is silent in the data (an entry for Marcus without an AkEvent); no line was invented.
"Plays" means chosen and found in the audio manifest; the host has no audio device. Stand-in: the pawn's dialog groups are its body class's
`DialogGroups` plus the dialog globals' `NPCDialogGroups`; `WillowPawn.GetDialogGroups` has no note yet (UNVERIFIED). Checks: ctest 11/11,
verify_packages 9/9, UE build succeeded, quest suite 96/96 + resume 11/11 (was 92 + 11), door suite 16/16 (rerun by the orchestrator), no new
stubs. Verified: automated suites only.

## 2026-10-06: native notes on a pawn's dialog groups and the group search (UNVERIFIED)

AI-assisted (Claude), analyst lane G22, Ghidra; own-words note `docs/verification/NATIVE_DIALOG_GROUPS.md`, no listing text. The pawn's
`GetDialogGroups` lists its body class's `DialogGroups`, then (only when the body class has `bNPCDialog`) its name tag's DLC expansion groups and
the dialog globals' `NPCDialogGroups`, then always the globals' `DefaultTemplateGroup`; no de-duplication. Interactive objects and echo actors
have their own simpler lists. `GetMatchingEvent` takes the first listed group with an enabled entry and appends a non-matching group's parent to
the end of its search, unique; template groups only when the caller allows them. For Marcus this gives 127 groups (his own, the 125 NPC groups,
the template group), which matches swap 6e's stand-in on the on-use route; the note lists the differences (the bNPCDialog condition, the
template group) and corrections to NATIVE_DIALOG. A second on-use press while his line is live is blocked by equal priority. All UNVERIFIED in game.

## 2026-10-06: a pawn's dialog groups from the GetDialogGroups rule (replaces the swap 6e stand-in, UNVERIFIED)

AI-assisted (Claude), implementer lane I3, from `docs/verification/NATIVE_DIALOG_GROUPS.md`. `DialogSystem::pawnDialog` builds a pawn's group
list as the note describes: body class `DialogGroups`, then only for `bNPCDialog` bodies the name tag's DLC expansion groups and the globals'
`NPCDialogGroups`, then always the globals' `DefaultTemplateGroup`; no de-duplication. The group search follows `GetMatchingEvent`: first listed
group with an enabled entry wins, a non-matching group's parent goes to the end once by identity, template groups only when allowed (not on a
re-trigger with reused event data). The old 32-entry search cap is removed (it would cut a 127-group search). A dialog reference that no installed
package resolves is looked up by the same path among the dialog package's exports, otherwise it still fails. Marcus: 127 groups, on-use lines
unchanged. Corrections applied to NATIVE_DIALOG.md. Not modelled: `SetDialogNameTag`, the manager's `bEnabled` gate and registered list, the
echo-actor and interactive-object variants. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 96/96 + resume 11/11,
door suite 16/16, no new stubs. Verified: automated suites only.

## 2026-10-06: script swap 7, level-up skill points and max health from the script path (UNVERIFIED natives, one stand-in)

AI-assisted (Claude), implementer lane I4. The VM's `ExpLevelUp` now awards skill points through its own script: `EvaluateInitializationData`
is bound to the attribute evaluator (from NATIVE_ATTRIBUTES / NATIVE_PROGRESSION, UNVERIFIED), which now resolves the player's replication info
and the `PlayerExperienceLevel` condition, so the installed `INI_SkillPointsPerLevelUp` gives 1 point per level from level 5 and 0 below.
`RecalculateAttributeInitializedState` (called by `OnExpLevelChange`) is a labelled host-boundary stand-in: its body was not read, so it
evaluates the health pool's base maximum (the data's `Init_PlayerHealth`, 80 x 1.13^L, minimum 20) for the new level. The slice hands the host
`skill_points` and `max_health` events; the host compares them with its own numbers (both agree on real data, for example 8 to 9: 1 point,
240.323) but does not yet adopt them, because `UOpenWillowSkills` has no points counter or health setter. Not modelled: the class's `OnLevelUp`
health refill, respec, playthroughs above 1, DLC skill-point definitions. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest
suite 98/98 + resume 11/11 (was 96 + 11), door suite 16/16, no new stubs. Verified: automated suites only.

## 2026-10-07: REA MCP server installed as an analysis front end over Ghidra (maintainer request)

AI-assisted (Claude). At the maintainer's request, `rea-agents` 4.1.0 (MIT, https://github.com/morluto/rea) is installed globally with npm,
outside the repository, and registered as an MCP server for Claude Code with the existing Ghidra 12.1.4 and Temurin JDK 21. Its doctor check
reports the Ghidra provider available; Hopper and IDA are not configured. Provenance and the package integrity hash are in THIRD_PARTY.md. The
clean-room rules for Ghidra apply unchanged to REA: no REA output enters the repository, and notes are written in our own words. Also recorded:
the maintainer prefers reading data and observable behaviour from the live game through the SDK (the approach of their BL2_RTSE mod) over
package decoding or disassembly where the answer is observable at runtime.

## 2026-10-07: gestalt decode cross-checked against live-game dumps from the maintainer's BL2_RTSE mod

AI-assisted (Claude), lane R1; record `docs/verification/RTSE_GUN_DATA_CROSSCHECK.md` (counts and conclusions only, no dumped values). The 16
GestaltDef tables the maintainer's in-game mod dumped (9 item families plus 7 Remaster weapon variants) match our package decode exactly:
646/646 part entries, 636/636 bounds, 544/544 socket mappings, 541/544 mesh sockets (three pistol scope eye sockets differ by about a unit,
cause UNVERIFIED). This confirms our struct-array decoding of these objects against the running game. The Remaster tables exist in
`Mancana_StartupRemaster.upk` with different ranges; our earlier live gun capture matches the original tables, not the Remaster ones. Gaps
and recommendations (muzzle and eject sockets in the host, a duplicate-name bug in `slice_npc_assets.py`, live golden checks over part
combinations) are in the record. Nothing from BL2_RTSE was copied into the repository; reusing its code is a maintainer provenance decision.

## 2026-10-07: native notes on the level-up attribute recalculation; the health refill observed in game

AI-assisted (Claude), analyst lane G23, Ghidra; own-words note `docs/verification/NATIVE_LEVEL_UP_ATTRIBUTES.md`, no listing text.
`RecalculateAttributeInitializedState` re-initialises the character class's pawn damage modifiers and rebases every pool of the controller's
resource pool manager through the script `ResourcePool.CalculateBaseValues(true)`: the base maximum comes from the definition (health 80 x
1.13^L), the modifier stack is kept, and the current value follows the new maximum only if the pool was full at its last recorded values and
the definition allows it. The native alone leaves a half-full health pool's current value unchanged; the refill comes from the
`PlayerBehavior_LevelUp` behaviors that `OnExpLevelChange` runs (add the effective maximum once, clamp), behind a strict more-than-1-second
guard. **Observed in game** by lane L1 on 2026-10-07 (two captures calling the real `ExpLevelUp(false)`, 8 to 9 from half health and 9 to 10
from a quarter): the recalculation raised the maximum and left current alone, then current equalled the new maximum; the shield recharge
boost was also seen. So a level-up refills health; it does not keep the fraction. The cooldown reset and the 1 s guard were not observed.
Corrections to NATIVE_ATTRIBUTES, NATIVE_PROGRESSION, SANCTUARY_RPG_MISSION (swap 7's stand-in gives the right maximum but not the rule),
NATIVE_SAVE_LOAD and REALGAME_GROUND_TRUTH are listed in the note.

## 2026-10-07: turn-in loot stand-in removed (maintainer decision); the Fire mission gives XP only

Maintainer decision: "remove the turn in loot since that's not how missions are gonna look." AI-assisted (Claude), lane I4. The stock data
gives `M_RockPaperGenocide_Fire` no item reward (only `ExperienceRewardPercentage` and a credit multiplier of 0), and the script's
`GetItemRewardsForPlayer` returns nothing, so the turn-in now gives XP only. Removed: `weapon_slice_gear.py --reward-only` and its seed search,
the host's reward pickup, and the `reward_roll` recipe. Kept: the pickup, backpack and use-key pickup code that the lent mission pistol and the
inventory suite use. Quest checks `turn_in_drops_loot_stand_in_pickup` and `use_key_collects_loot_pickup_into_backpack` were removed;
`turn_in_gives_no_item_reward` was added. Other machines should delete a stale local `slice_reward_roll.json`. Checks: ctest 11/11,
verify_packages 9/9, UE build succeeded, quest suite 97/97 + resume 11/11 (98 - 2 + 1), door suite 16/16, inventory suite PASS 49 FAIL 0
(same as the last run).

## 2026-10-07: real-game lane L1: level-up health, Marcus's use chain, interface table and UpdateGrade observed in game

AI-assisted (Claude), real-game lane L1. Method as in the 2026-10-02 entry (one game process under the run lock, saves backed up first, saves blocked, memory-only, driver removed),
now with `Install-Driver -BlockSavesAtStart` so the save block holds from the first frame even when a startup mod loads a character. The level-8 Maya was loaded with the startup
mod's `-Character=Save0008.sav` argument. Confirmed in game (each by hooks on script-visible functions plus reads of the live objects; details in REALGAME_GROUND_TRUTH.md):
(1) a level-up refills current health to the new maximum (three level-ups from 25 to 50 % health), writes the new maximum inside `RecalculateAttributeInitializedState`
(base `80 x 1.13^L`, times the profile's Badass modifier), adds one skill point at the start of `ExpLevelUp`, leaves current and maximum shield alone but raises the shield recharge rate
by half the maximum per second for 4 s (stock `PlayerBehavior_LevelUp`), and for a natural level-up adds a 30 s weapon-damage scale of +1.0 (`PlayerBehavior_LevelUpNaturally`); no other
numeric attribute of the controller, pawn, replication info or pools changed apart from the game stage; the cooldown effect of `PlayerBehavior_LevelUp` was not shown to do anything
(inconclusive). (2) Marcus's use: nine `IsBehaviorSequenceEnabled` calls in the noted order, all false for a character with none of the six missions, then the dialog behavior, `HasMissions`
and `ShowMissionInterface`; the result rule (unknown name, `None` provider, handles -1 and 0 are false) by direct calls; tag `VO_NPC_OnUse_MissionsAvailable` with 1 eligible, 0 in progress,
0 redeemable missions; `GetDialogGroups` gives 127 entries in the noted order; consumer handle 49. (3) the real interface table matches the engine's own class check on all 66 stand-in
false positives and all 203 non-`Core.Interface` stand-in misses. (4) `Skill.UpdateGrade(N)` writes `Grade` and its base together. Refuted or corrected: `UpdateGrade` stores
`max(N, 1)`, not "N plus one"; the NATIVE_CLASS_SERIAL_LAYOUT count of `Core.Interface` pairs is 772, not 771. Not reached: the second (native) dialog lookup through the Trigger act and the
`TriggerEvent` return, "no change notification" for opcode 5F, the cooldown effect, skill points below level 5. Save evidence: 33 files hashed before and after, all byte-identical.
Verified in game: the items above; automated checks: ctest 11/11, verify_packages 9/9; nothing in `src/` changed.

Also: `tools/test_inventory_actions.ps1` deleted the run lock as stale after 15 minutes while this lane's game session still held it. A lock now counts as stale only when it is older than 15 minutes and no Unreal editor or Borderlands 2 process is running.

## 2026-10-07: script swap 7b, level-up pool rebase and refill on the VM; the host adopts the VM's health

AI-assisted (Claude), implementer lane I4, from `docs/verification/NATIVE_LEVEL_UP_ATTRIBUTES.md`. The VM controller now has a resource pool
manager with one health pool built from Maya's stock `HealthPoolDefinition`, seeded with the host's current health. `RecalculateAttributeInitializedState`
rebases every pool through the installed script `ResourcePool.CalculateBaseValues(true)` (base maximum from the data; current follows only a full pool),
replacing swap 7's stand-in. `OnExpLevelChange`'s pawn branch runs as script, so the more-than-1-second guard is the script's own. The refill is a labelled
stand-in for `BehaviorBase.RunBehaviors` over the class's `OnLevelUp` collection: the `Behavior_AttributeEffect` skills' plain-attribute effects are
applied to the VM's pools (the heal adds the effective maximum, then the pool clamps). `GetAWillowPawn` and `SetGameStage` are small stand-ins with no
note. The host adopts the VM's `health` event, so a level-up from half health ends full, as observed in game (lane L1); its own formula stays as the
oracle. Not modelled: the class-level damage modifiers and status resistances (0/1 constants for Maya), the action-skill cooldown reset, the shield
recharge boost, the natural-level-up weapon-damage scale. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 98/98 + resume
11/11 (new `script_level_up_refills_health`), door suite 16/16, inventory suite PASS 49 FAIL 0. The refill rule is confirmed in game; the rest
is UNVERIFIED.

## 2026-10-07: Fire mission golden trace recorded in the real game and compared with the VM (lane L2)

AI-assisted (Claude), real-game lane L2, method as lane L1 (one game process under the run lock, saves blocked from the first frame, memory
only, driver removed; all 33 save and config files byte-identical afterwards). The level-8 Maya accepted the Fire mission at Marcus, touched the
range waypoint, burned the target dummy with the lent pistol and turned in; hooks recorded the mission tracker, behavior, dialog, XP and reward
calls in order (`docs/verification/FIRE_MISSION_GOLDEN_TRACE.md`, `REALGAME_GROUND_TRUTH.md` lane L2; raw trace under ignored `local/realgame/L2/`).
Observed in game: the status routine order for Active, ReadyToTurnIn and Complete; the lent weapon goes into the backpack inside
`ActivateMission` and is removed inside `CompleteMission`; turn-in `ExpEarn(395)` with an empty item reward (26,218 to 26,613 XP, level 8 kept);
behavior-link delays 0.505, 2.002 and 3.002 s. Against `--slice-run`: 38 events match, 4 out of order, 5 missing in the VM, 4 extra. Differences to
fix: the mission weapon grant and removal positions; the dummy's `RegisterTargetable` running about 1 s after spawn together with the mission's
`ChangeRemoteBehaviorSequenceState` (absent in the VM); `AIHold` on the dummy's spawn (absent in the VM); `PlayMissionTurnedInDialog`
(`VO_NPC_MissionTurnedIn`) not run. Host-side: the mission screen's close and reward confirmation, and the dialog event one wake late. The VM run
used a binary from before swap 7b. Dialog lines were shortened by an installed dialog-skipper mod, so line-length delays are not measured.

## 2026-10-07: gun muzzle and eject points from the decoded gestalt sockets

AI-assisted (Claude), implementer lane I5, following lane R1's live cross-check. The gestalt array schema is now tracked
(`tools/gestalt-arrays.schema`: the two part-table lines plus socket mappings, part bounds and mesh sockets). The new
`tools/gestalt_sockets.py` decodes the six weapon families' sockets into ignored `local/gestalt/`, and the recipes carry them
(`weapon_refresh_fragments.py --sockets-only`). The host's muzzle flash and tracer now start at the held gun's decoded `Muzzle` socket
(bone transform times the cooked offset); the old "27 cm past the Barrel bone" estimate is a logged fallback, used 0 times in the suites
and gun-shot runs. The estimate was 14 cm too far on the slice pistol, and the SMG has no Barrel bone. `EjectPort` is exposed but the
host spawns no casings. Fixed: `slice_npc_assets.py` kept only the last range of a fragment name with several ranges. Tests:
`tests/gestalt_sockets_test.py` (9) and `tests/slice_npc_assets_test.py` (2, needs numpy). Checks: ctest 11/11, verify_packages 9/9, UE build
succeeded, quest 98/98 + 11/11, door 16/16, inventory 49/0. The socket values match the live game (R1); that the game's own flash starts
there, and our axis and scale conversion, are UNVERIFIED (an in-engine check against the offline mesh agrees to 0.00 cm).

## 2026-10-07: VM fix, an assignment evaluated its target twice

AI-assisted (Claude), found by implementer lane I6 while matching the golden trace. `EX_Let` evaluated the left-hand side again to return
the assigned value, which repeated its side effects: `arr[arr.Length] = x` grew the array twice and stored two elements. In the slice this
put two users in Marcus's `PawnsUsingMe` and silenced his on-use dialog. The assignment now returns the target it already wrote. New
synthetic case `Foo.Append` in `tests/vm_test.py` fails without the fix. ctest 11/11, verify_packages 9/9.

## 2026-10-07: script swap 8, the Fire mission's event order follows the live golden trace

AI-assisted (Claude), implementer lane I6, with lane L2's live trace (`FIRE_MISSION_GOLDEN_TRACE.md`) as the evidence. Against the live trace,
`--slice-run` goes from 38 matched / 4 out of order / 5 missing / 4 extra to 43 / 2 / 2 / 4. Changes: the mission weapon is granted inside
`ActivateMission` (into the backpack, not readied; the player equips it) and removed inside `CompleteMission`, as observed. The dummy's `AIHold`
now runs, because the boundary handler for `Behavior_SpecialMove` selects output 0, as the installed script does on its first run. Turn-in now
runs `PlayMissionTurnedInDialog` (`VO_NPC_MissionTurnedIn`): the use path emulates the mission screen's open (`OnPlayerOpenedMissionUI`,
`BeginUse`); the turned-in line is silent for this mission because the act's output is unlinked (UNVERIFIED). Explained, not changed: the dummy's
1 s `RegisterTargetable` delay was already in the VM (a data link with `ActivateDelay` 1.0), and the dialog event "one wake late" is the
behavior's own first-run wait. Remaining: the 2 out-of-order items are those dialog markers; the 2 missing items are host UI (the screen close at
accept, the reward confirmation); the 4 extra are VM dialog lines the live run never selected (cause not isolated). New `tools/verify_fire_event_order.py`
checks 18 event-order rules (18 of 18 hold). Quest checks: `mission_weapon_removed_after_objective` became `mission_weapon_kept_after_objective`;
added `mission_weapon_granted_on_accept`, `lent_pistol_readied_from_backpack_by_player`, `mission_weapon_removed_on_turn_in` and
`turn_in_raises_mission_turned_in_dialog_tag`. Newly reached stubs from `BeginUse` (`AIComponent.ActivateEvent`, `Actor.ClearTimer`, one evaluator
context) change no event. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest 102/102 + 11/11, door 16/16, inventory 49/0.

## 2026-10-07: real-game lane L4, the Fire dialog without the dialog skipper

AI-assisted (Claude), real-game lane L4 (work queue item 3), with the same method as L1 and L2: one game process under the run lock, saves and the
SDK settings folder backed up and hashed first, saves blocked from the first frame, memory-only. Launch 1 had the installed Dialog Skipper mod
disabled in memory (`Mod.disable(dont_update_setting=True)`, so no settings write); launch 2 had it on, which was L2's condition. The new probe is
`tools/real_game/scripts/dialog_probe.py`.

Observed in launch 1: the whole Fire dialog plays. Line 03b (6.59 s) is followed by `_1184`, which plays 03a with the target dummy pawn as
speaker (2.01 s), and then by `_1183`, which plays 04 (5.01 s). Marcus's on-use `DET_NPC_OnUse_MissionsAvailable` (3.10 s) is real too. So the
4 "extra" VM dialog items of the L2 comparison come down to 1, `AttemptStatusEffect_9` (the hit model).

The cause of L2's gap: the skipper stops each line of a non-hostile pawn at its first instant. `_1186` then selects Finished in the frame in
which the dummy spawns, before the dummy is written into the Kismet variable `TargetDummy` that is `_1184`'s context. `_1184` is therefore not
applied. Also observed: a dialog behavior's resumed runs are applications of a transient copy, which L2's name filter could not see.

UNVERIFIED: that the variable is empty at that instant (the row order fits, but it was not read in that frame), and that an empty context list
drops the application.

Not changed yet, for a VM lane: bind `TargetDummy` to the dummy at spawn, run `_1184` with the dummy as context and talker, and give an empty
context no application. Live line lengths are recorded for a host line player.

A Python re-trigger of one line with `None` object arguments crashed the game once, after the data had been dumped; nothing was written.

Save evidence: 33 save and config files are byte-identical after both launches, and the 19 SDK settings files are unchanged. Checks: ctest
11/11, verify_packages 9/9 (docs and a game-side probe only).

## 2026-10-07: real-game lane L3, a real fight recorded; a level-up resets the action-skill cooldown

AI-assisted (Claude), real-game lane L3 (work queue items 1 and 2). Method as lanes L1 and L2. The level-8 Maya travelled to Three Horns Divide
(`Ice_P`) with the game's own `InitiateTravel` and fought bullymongs with real mouse shots. The new probe is
`tools/real_game/scripts/combat_probe.py` (463 hooks plus a per-frame sampler), and the record is `docs/verification/COMBAT_GOLDEN_TRACE.md`.

Confirmed in game:
- **Hit chain:** each pellet runs `TakeDamage`, `ReduceDamage`, `AdjustDamage`, `SetHealth`, `NotifyTakeHit`, `TookDamageFromEnemy` and
  `CheckInjured`, in that order.
- **AI death:** `NotifyPawnDied`, `Died` with `DropLootOnDeath` first, `OnKilledBy`, `Killed`, `CalculateExperiencePointsForKill`,
  `AwardCombatExperience`, in that order.
- **Kill experience:** awards of 112.5 and 50.0, credited to the pool as +45 and +7.
- **Drops and pickups:** drops came from the pools the script calls (`Pool_Money_1or2`: 3 items for an adult, 1 for a baby; `Pool_GunsAndGear`).
  Money is picked up by touch and a weapon by the use key. A full backpack refuses.
- **Being hit:** the shield takes the hit first (31.616 of a 44.22 grenade) and the overflow goes to health. The shield recharges after 1.61 s at
  25.9 per second.
- **Down, death, respawn:** down at health 0, the targeted state 2.28 s later, bleed-out 12.0 s, death camera 5.76 s, then
  `ResurrectPlayer(IDied)` with full health and shield. The fee is 0.07 x cash, rounded half up, charged 11.8 s after arrival.
- **Second wind:** a kill while down restored health to 25 % of max in the kill frame and shield to 90 % on the next frame.
- **Part B:** a level-up (`ExpLevelUp(false)`) sets the action-skill cooldown to 0 in the same call, both at its 13.0 maximum during a real
  Phaselock hold and at 7.99 during the drain. The 4.0 s shield boost (90.29 per second) and the 30 s weapon-damage scale (41.4 to 61.9) were
  observed again.

Contradicted: NATIVE_DAMAGE_DEATH's 2.0 s fee timing (11.8 s observed).

UNVERIFIED or not shown: the shortened bleed-out of successive injuries; the pipeline stage factors behind 0.715 (grenade) and 0.741 (melee);
the experience level factor; `Pool_Money_1or2`'s quantity rule; the respawn station choice.

Mods: Drop Chance Multiplier was not active at runtime. Loot Collector and Be Gone Out Of Bounds Loot were switched off in memory with no
settings write.

Script stand-ins in the session: camera aim, player placement, and the misc slot limit raised in memory for one pickup.

Save safety: 60 files hashed before and after. The 33 save and config files and the SDK settings folder are byte-identical. Sanity Saver's own
`Save0008.json.gz`, which that mod rewrites at load, was restored from the backup, and after that 0 of 60 differ.

Checks: ctest 11/11, verify_packages 9/9 (docs and a game-side probe only).

## 2026-10-07: script swap 9, the natural level-up's timed weapon-damage scale

AI-assisted (Claude), implementer lane I7 (work queue item 2). Observed in game (lanes L1 and L3): a natural level-up raised the held weapon's
damage from 41.40 to 61.92 for 30 s (scale sum 2.017 to 3.017).

What changed:
- **VM.** A natural level-up runs `PlayerClass.OnLevelUpNaturally` through the existing labelled `RunBehaviors` stand-in. Its WeaponDamage
  effect (weapon context, Scale) is not a VM pool, so it is handed to the host as the new event `weapon_damage_scale`. The value (1.0) and the
  duration (30 s, `DurationType` Timed) are read from the installed skill. Whether a level-up is natural is decided by the installed script
  (`ExpLevelUp(bCheated)` calls `OnExpLevelChange(true, not bCheated)`).
- **CLI.** The new test-only `--slice-run` step `cheatlevel` runs `ExpLevelUp(true)`. That level-up, and a natural one inside the script's 1 s
  guard, give no event.
- **Tools and host.** `tools/weapon_stats.py` now writes each card's WeaponDamage stack terms (`damage_stack`); `damage` is unchanged. The host
  keeps each scale with its end time in game time and shoots with (base + PreAdd) x (1 + up + s) / (1 + down) + PostAdd, the observed
  (S + s) / S. A card without the stack falls back to card damage x (1 + s), a labelled UNVERIFIED stand-in. For the slice pistol the factor is
  1.735.
- **Local slice cards.** Only `damage_stack` was injected, by re-evaluating the five existing recipes. Re-evaluated cards also differ from the
  stored ones in rarity rating and colour, and the shotgun in reload, magazine and display. That is drift since the cards were generated; it is
  left alone and recorded as open.

UNVERIFIED: the VM stand-in, the host's application of the stack rule (the rule was read from native code), the skill's 0.5 s context re-gather
(not modelled: the scale follows the weapon held at the shot), and per-activation stacking. Not done here: the shield recharge boost (no shield
pool) and the cooldown reset (confirmed live by L3, next swap).

Tests: synthetic scenario S9 in `tests/mission_script_test.py`, two new `tests/weapon_stats_test.py` cases (30 OK), and the quest check
`natural_level_up_scales_weapon_damage_for_duration`.

Checks (rerun by the orchestrator): ctest 11/11, verify_packages 9/9, event order self-test and 18/18, UE build succeeded, quest 103/103 + 11/11,
mover 16/16, inventory 49 PASS / 0 FAIL.

## 2026-10-07: script swap 10, a level-up resets the action-skill cooldown

AI-assisted (Claude), implementer lane I7 (work queue item 2). Observed in game (lane L3, `COMBAT_GOLDEN_TRACE.md` section E): `ExpLevelUp(false)`
set the cooldown to 0 within the same call, both at the 13.0 maximum during a Phaselock hold and at 7.99 during the drain.

Read from the data: the effect is on `OnLevelUp`, not `OnLevelUpNaturally`. It is the second effect of the first level-up skill: Scale of the
cooldown pool's `CurrentValue` by the pool's minimum, which is 0 for Maya.

What changed:
- **VM.** The VM builds a minimal `ResourcePool` over the class's `SkillCooldownPoolDefinition`, with the minimum read from the data, so the
  effect's value comes from the data and is not assumed. The pool is kept out of the manager, whose rebase would evaluate a designer-attribute
  maximum. The effect is handed to the host as the new event `action_skill_cooldown` (modifier type, value), after `health`.
- **Host.** `ModifyPhaselockCooldown` applies the modifier to the Phaselock cooldown it keeps, as an offset on its drain model that the next
  cast clears. A running cast is not touched. The host has no handler for the game's `ActionSkillCooldownComplete`, so it only logs completion.

UNVERIFIED:
- that the reset also follows a not-natural level-up (the script runs `OnLevelUp` for both; the trace has natural ones only);
- the VM pool construction;
- a reset during a held cast in the host (only logged, not tested).

Tests: synthetic scenario S10 (an invented minimum of 0.25, so the value is visibly the data's) and the quest checks
`level_up_resets_action_skill_cooldown` and `action_skill_cooldown_modifier_scales_then_resets_host_cooldown`.

Also today (docs only): NATIVE_DAMAGE_DEATH section 10.4 now records the live station-path fee timing (11.8 s after arrival, lane L3) in place of
"about 2 s".

Checks (rerun by the orchestrator): ctest 11/11, verify_packages 9/9, event order self-test and 18/18, UE build succeeded, quest 105/105 + 11/11,
mover 16/16, inventory 49 PASS / 0 FAIL.

## 2026-10-07: combat groundwork native notes (pool references, pawn health, pipeline objects, kill notification, kill experience)

AI-assisted (Claude), analyst lane G24. The notes are in `docs/verification/NATIVE_COMBAT_GROUNDWORK.md` (20 index rows). They were read from
`Borderlands2.exe` with Ghidra and from the installed script and data, and are written in our own words. The forbidden-pattern scan is clean.
Nothing here changes code or parsing. **Every rule is UNVERIFIED in the running game.**

Summary:
- **Pool references.** A pool reference is a manager, a slot (0 to 15), a one-byte generation and a cached pool. It is valid only if its slot is
  non-empty and the generation byte matches. `CreatePool` takes the lowest free slot and allows one pool per resource. The pool's owner is the
  manager's owner (the controller). The player's slot order is read but not observed: Maya's seven ammo pools first, then accuracy, experience,
  action-skill cooldown, melee cooldown, health and shield.
- **Pawn health natives.** `GetHealth`, `SetHealth` (values in [0, 1) become 0), `GetMaxHealth` and `SetMaxHealth`.
- **Damage pipeline objects.** The globals object keeps a growing pool of pipeline objects for reuse.
- **NATIVE_DAMAGE_DEATH section 1's open items:**
  - two stages are in the other order (the final damage is unchanged);
  - `IntrinsicArmorReduction` divides by 1 + 0.01 x armor (0 armor for Maya and the dummy);
  - `RecipientDamageTypeReduction` is never written;
  - the crit base is 2.0 (the earlier 0 was the property decoder's empty value for attribute defaults);
  - the AI attacker's extra level factor only applies from playthrough index 2 (UVHM).
- **Kills.** `NotifyActorDied` and `NotifyPawnDied` handle defend targets and kill objectives, with no effect on the Fire mission. Kill
  experience is max(10, multiplier x 10 x (level + 4)), split by player count, scaled by a level-difference table, truncated, and passed to
  `ExpEarn(amount, 1, 1)`.

Open contrast: lane L3's two observed pool credits (45 of 112.5, 7 of 50) are smaller than the note's table gives for a single player killing a
higher-level enemy. Those kills may have been partial credit, since AIs on `Ice_P` killed each other. A kill-experience lane must settle this live
before implementing.

## 2026-10-07: VM facilities for the combat swaps: attribute defaults, GotoState events, engine iterators

AI-assisted (Claude), implementer lane I9, working from the swap plan of scout lane S1. There are three general VM changes, one commit each, all
checked synthetically in `tests/vm_test.py`:
- **Attribute properties in class defaults load.** Tagged `FloatAttributeProperty` and `IntAttributeProperty` defaults used to be skipped (98
  on a `WillowPlayerController`, 42 on a pawn), so script saw 0. The layout was fitted from the packages: all 242 such tags in the 914 installed
  packages have size 4, and both the value and its `...BaseValue` companion are tagged. Value and base agree on all 1,513 pairs across 3,254
  default objects. The modifier stack stays empty. No `ByteAttributeProperty` tag exists.
- **`Object.GotoState` runs the state events.** When the state changes, or `bForceEvents` is set, `EndState(new)` runs on the old state, then
  the name is set, then `BeginState(old)` runs on the new state. This is the public UE3 order, UNVERIFIED against the game. Nesting is capped at
  8. A `GotoState` inside `EndState` wins over the outer one. An undeclared state only sets the name. Latent state code, labels and
  `bKeepStack` are not run, and are logged.
- **Engine `foreach` over `Actor.AllActors` and `DynamicActors`.** The layout was fitted and checked on all 790 iterator statements: the end
  offset lands on `IteratorPop` after `IteratorNext`. These natives visit the runtime's live objects of the class in creation order. UNVERIFIED:
  `DynamicActors` is treated as the same list as `AllActors`, and the list is a snapshot taken when the loop starts. Other iterators still stub.
  Also fixed: an empty dynamic-array `foreach` popped the enclosing loop's state.

The Fire `--slice-run` is unchanged apart from 42 removed "skipped" log notes. `--vm-sweep` success and failure counts are unchanged.

Checks (rerun by the orchestrator on the lane's build): ctest 11/11, verify_packages 9/9, event order self-test and 18/18. UE suites: run with
the next host commit.

## 2026-10-08: maintainer answers on BL2_RTSE, tracked live-dump tools, the hostile enemy and the Remaster tables

AI-assisted (Claude) record of answers the maintainer (zuhu) gave in conversation on 2026-10-08.

- **BL2_RTSE** (https://github.com/zuhuHix/Bl2_RTSE) is the maintainer's own project. Its licence and provenance question is settled: it is theirs, and the
  README now links it. That is a statement about ownership; any code taken from it still gets a note in the commit that brings it in. Nothing has been copied
  so far.
- **Tools that read live dumps may be tracked**, as long as they are tools: scripts and the code that reads and compares. The dumps and any values
  taken from the game still stay under ignored `local/` and never go into a tracked file, commit or doc.
- **Hostile enemy for the Phase 4 gate: a bullymong**, built on the VM from the game's own data (Three Horns bullymong archetype), not the dummy plus a stand-in
  attacker. The Fire dummy is still the first VM pawn for the stock hit chain (its real health is 778.658 with a floor of 1.0, see lane L5), as a stepping stone.
- **Remaster tables** (`Mancana_StartupRemaster.upk`): the maintainer has no view on them yet. Still open; nothing is decided, and no record is written.

## 2026-10-08: real-game lane L5b, kill XP, cheated level-up and cooldown, successive injuries, bullymong values

AI-assisted (Claude), real-game lane L5b (continues lane L5's partial run). Method as lanes L3/L5 (one game process under the run lock, saves, config, SDK settings and Sanity Saver's private copies hashed before and after, `Install-Driver -BlockSavesAtStart`, memory-only, closed through its window, driver removed). Observed in the running level-8 Maya on `Ice_P` (hook rows and live reads, `docs/verification/COMBAT_GOLDEN_TRACE.md`, "Open-rule checks"):
kill experience = `trunc(max(10, mult x 10 x (level + 4)) x LowerLevelEnemyExpScale[|d|-1])` on 5 of 5 player-only kills (awards 50, 137.5, 50, 50, 75; credited +35, +123, +7, +7, +11; d = 2, 1, 4, 4, 4), with L3's d = 3 the entries 1-4 are observed, credit inside `AwardCombatExperienceToParty` (G24 5.1/5.2 confirmed; a constant factor contradicted); `ExpLevelUp(bCheated=True)` sets the action-skill cooldown to 0 in the same call during a real Phaselock hold
(13.0 -> 0.0, lock unaffected), like `False`, but does not apply `PlayerBehavior_LevelUpNaturally` (no 30 s weapon-damage scale: `bNaturalLevelup = not bCheated`); successive injuries: the bleed-out was 12.0 s for the first and second injury (N = 0, 1) and **8.0 s for the third (N = 2, 7.45 s after the previous)**, so the cut is `total x (1 - (N-1)/3)` for N = 1, 2, contradicting NATIVE_DAMAGE_DEATH's `total/3 x N`; second wind health 0.25 x max and shield 0.9 x max (3 of 3);
bullymongs: no shield, `bIsInvulnerable` false, attack damage `8 x 1.13^level x EnemyNonWeaponDamageMultiplier` (adult 5, regular 1.5, baby 0.75, ranged 3), health = class multiplier x base(level) (102.3, 117.8, 135.4 at levels 5-7), drops from `Pool_Health_All` and ammo pools only in 8 deaths. Lane L5 (2026-10-07) had found the Fire dummy at 778.658 health with a 1.0 floor and confirmed the pool slot order and the crit base.
UNVERIFIED: equal-level, higher-level and d >= 5 kill XP, N >= 3 bleed-out and the 1 s floor, the 30 s window edge, the mechanism of the Fire dummy's floor, what the shortened-bleed cut reads (a field was not captured). Save safety: 60 files hashed before and after; Sanity Saver's `Save0008.json.gz` (rewritten at load) and the SDK's own update-check stamp in `python-sdk.json` differed and were restored from the backup, then 0 of 60 differ. Automated checks: none (no code under `src/` or `host/`; one `py_compile` of the probe).

## 2026-10-08: combat comparator for the bullymong swap (plan S2, step 0)

AI-assisted (Claude). `tools/verify_combat_pawn.py` is a table of 25 rows: four pawn health values and three per-level bases (`50.5 x (1 + 0.02 L) x 1.13^L` times the class health multiplier), six melee and grenade arrival values (`8 x 1.13^L` times the class damage multiplier), five applied ratios (`0.65 x (1 + 0.02 L)`), and seven kill-experience rows (award `max(10, mult x 10 x (L + 4))`, credit `trunc(award x LowerLevelEnemyExpScale[d - 1])`). The constants and inputs are those recorded in COMBAT_GOLDEN_TRACE.md and NATIVE_COMBAT_GROUNDWORK.md 5.1-5.2; no measured value is stored in the file. `--live` compares the rows with the ignored dumps of lanes L5 and L5b with a relative tolerance of 1e-3; `--vm` compares a VM JSON with the same row ids (schema in the file's docstring); rows with no value are MISSING and do not fail.
Verified (automated, against the live dumps, 2026-10-08): 21 of 25 rows PASS: 4 of 4 health values, 2 of 3 per-level bases, 6 of 6 arrival values, 4 of 5 ratios, 5 of 5 kill rows (award and credit). Self-test on invented inputs passes.
Not verified or open: the adult level-6 ratio FAILS (the single unshielded L6 hit is 40.417, ratio 0.485, not the 0.728 the formula gives; an unshielded 2/3-of-formula variant also appears at L5); level-5 base missing from the named dumps; L3 kill rows not read; equal-level, higher-level and d >= 5 kill rules, and any VM output, remain UNVERIFIED. The formulas are fits to dumped data, not the native code's behaviour.

## 2026-10-08: Script swap 11, Marcus's mission screen closes at the accept and the reward page waits for the player

AI-assisted (Claude), implementer lane I10b (continuing lane I10).

- **What:** the VM keeps a stand-in `QuestAcceptGFxMovie` (plus a `WillowGFxUIManager` on the controller) while Marcus's screen is open. `GetPlayingMovie`, the movie's native `Close` (raising an emulated `OnClose`:
  `ServerPlayerEndUseNPC`, then `OnPlayerClosedMissionUI`), `PlayUISound` (no-op), `DisplayRewardsPage` (waits) and `DetermineQuestEntries` (rows built in C++) are the host's; the controller's and the movie's
  other script runs unchanged. Accept order: `dismissal_dialog`, `dialog`, `mission_interface_closed`, `status Active`, `mission_weapon_granted`. Turn-in with the screen open: `experience`, `reward_page`, `status Complete`,
  weapon removed, `reward`, `turn_in_dialog`; `MissionRewardsReceived` waits for the player (`--slice-run` step `confirmreward` gives `reward_received`; `close` is Escape). Host: the use key confirms the page and closes the screen.
- **Bug fixed on the way:** the row builder iterated a temporary's elements (freed memory), so the list stayed empty and the screen never closed. The VM's untyped-empty-array limitation was not changed.
- **Verified (live, lane L2/L4 and installed script):** the close order inside the accept, the dismissal tag, the reward confirmation after the turn-in, the screen not closing by itself after it. **UNVERIFIED:** the
  VM's emulation of `OnClose` (one `EndUse` against two live), the dismissal line's talker (live: another NPC; VM: no talker), that the use key closes the screen after the confirmation (a host choice), everything not about order.
- **Checks:** ctest 11/11; verify_packages 9/9, 3,339/3,339; verify_fire_event_order self-test and 24 of 24 (18 old, two reworded for the new events, 6 new); UE module build succeeded; test_quest first run 107/107 (105 +
  `accept_closes_mission_screen_with_dismissal`, `reward_received_on_confirmation`), resume 11/11; test_mover 16/16; test_inventory_actions 49 pass / 0 fail. FIRE_MISSION_GOLDEN_TRACE.md: 45 matched, 2 out of order, 0 missing, 4 extra.

## 2026-10-08: real-game lane L6, why bullymong hits sometimes applied 2/3 or 1/2, and why the Fire dummy floors at 1.0

AI-assisted (Claude), real-game lane L6. Question 7 was decided offline from lane L5b's 665 recorded hits to the player, then two data reads in the running level-8 Maya; question 8 needed two launches of the original game under the run lock (saves, config, SDK settings and Sanity Saver's copies hashed before and after, `Install-Driver -BlockSavesAtStart`, memory-only, closed through the window, driver removed).
Observed: the 0.715-0.741 arrival-to-applied factor (`0.65 x (1 + 0.02 x attackerLevel)`) holds for all 130 melee and 23 grenade hits taken outside a second-wind boost, including 41 melee hits that began below half health; the odd rows (0.477, 0.364, 0.485 of arrival) are hits taken within 15 s of a second wind by a kill, when `GD_PlayerShared.injured.PlayerInjuredDefinition`'s "Defense Boost" skill
(15 s, Scale -0.5 on the controller's received bullet, melee, grenade and rocket modifiers, read live and in the packages) is active: 8 melee + 4 grenade hits at exactly 2/3, 2 melee hits at exactly 1/2 with two overlapping boosts, i.e. `x 1/(1 + 0.5 n)`. A half-health or protection-timer cap, crit/region and the time since the previous hit are contradicted (health kept falling, bone None, no pattern).
No unboosted adult level-6 melee hit exists in the dump, so 0.728 there is the formula, not an observation; the comparator's FAIL row is a boosted hit. The Fire dummy: `Pawn.IsDemigodMode()` is False until the first player hit and True from the call of the dummy provider's `Behavior_AIHold_59` (reason `Safe`, type 3) on, and health then floors at 1.0 (the hit that would pass it sets `SetHealth(1.0)`, later hits apply 0); the mind's `GetAIDemigod/GetAIGod/InGodMode` stay False,
and `HoldAIForMatinee`, `MAT_BeginAIGroup` and the mind's `SetAIDemigod/SetAIGod` never ran from script, so the plan's Matinee candidate is contradicted.
UNVERIFIED: why Scale -0.5 reads 1/1.5, n >= 3, bullet and rocket types under the boost, the Power Boost effect, which part of the AIHold sets demigod (the type here has no God bit), whether releasing it clears demigod, other pawns. Save safety: 60 files hashed before and after; Sanity Saver's `Save0008.json.gz` and the SDK's own update-check stamp in `python-sdk.json` differed and were restored from the backup, then 0 of 60 differ. Automated checks: none (no code under `src/` or `host/`; one `py_compile` of the probe).

## 2026-10-08: combat comparator reads the Defense Boost count (lane H3)

AI-assisted (Claude). `tools/verify_combat_pawn.py --live` now reads `L6/q7_rows.json` (boost count per hit, ignored local data) and computes the
applied-ratio rows from unboosted hits only. Adult level 6 has no unboosted melee hit in the dump, so that row is MISSING, not FAIL. New
`ratio_boost1`/`ratio_boost2` rows check the `1/(1 + 0.5 n)` factor lane L6 observed live (adult L5 and L6 melee and ranged L5 grenade PASS at 1e-3).
Checks: self-test on invented inputs passes; the live run gives 24 PASS, 0 FAIL, 5 MISSING of 29. Open: no 2-boost melee sample; status-effect and
empty-type hits at adult L6 with one boost show no reduction in 39 hits (adult L5 status hits do show it), so the boost rule for non-melee sources stays UNVERIFIED.

## 2026-10-08: Fire dialog context, `Behavior_TriggerDialogEvent_1184` runs on the dummy (implementer lanes I8/I8b)
AI-assisted (Claude). From the live record of lane L4 (the behavior's context is the target dummy pawn, from the Kismet object variable `TargetDummy`; with
the Dialog Skipper on the chain is never applied). VM: (1) `BehaviorProvider::bindKismetVariable`: a `BVAR_NamedKismetVariable` link resolves to the object
bound under that name (the slice binds `TargetDummy` to the stock dummy pawn after its spawn behaviors; before that it is empty); (2) `DialogSystem::behavior`
reads its context list at the first run: an empty list drops the application, else the context's registered talker is the event's instigator; the dummy is
registered as a talker under its body class's dialog name; (3) an instigator-talker act with no instigator actor takes a TalkData entry (line 03b, spoken by
Marcus with the mission tracker as context). Consequence: the default `--slice-run` (no line player) applies 03a to an empty variable and drops it, 04 never
starts (the live skipper-on result); with `talker:<Marcus> lines:<s>` the unmodded order plays (03b, dummy bound, 03a spoken by the dummy, 04). The UE host
still plays 03a and 04 (the dummy is spawned before the chain). `--mission-run` gains `kismet:<variable>|<name tag>`; `verify_fire_event_order.py` gains a lines
run: 32 rules (HEAD's 24 hold unchanged). Verified (automated): ctest 11/11, verify_packages 9/9 (3,339/3,339), event-order self-test + 32/32, test_quest
107/107 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0 (rerun by the orchestrator on HEAD e0094fc: same numbers). UNVERIFIED: see above. Not changed: ChangeRemoteBehaviorSequenceState_171, line lengths.

## 2026-10-08: pawn health on the VM for any pawn (plan S-a), the dummy and the bullymong variants from the data

AI-assisted (Claude), implementer lane Sa, working from the own-words notes `NATIVE_COMBAT_GROUNDWORK.md` (G24, sections 1-2) and `NATIVE_LEVEL_UP_ATTRIBUTES.md` section 4 and the live records `local/realgame` L5/L5b/L6; no game code or decompiler output was read or reproduced.
New unit `src/combat.*`: pool references (valid exactly while the slot holds a pool of the same GUID), `CreatePool` (lowest free slot, one pool per resource, a manager made for an owner with authority), `DeletePoolByReference`, `IsResourcePoolValid`, `GetResourcePoolCurrentValueOrDefault`, `Pawn.SetHealthResourcePoolDefinition`, `GetHealth/SetHealth/GetMaxHealth/SetMaxHealth`, and the ResourcePool natives that were scoped to the player's one pool now take any pool (the player's controller context for `EvaluateInitializationData` still comes from the mission bridge). The pool's own start state is the installed script (`InitializeAuthoritative` -> `CalculateBaseValues`) run on the VM. The evaluator gained the pawn and mind context resolvers, the noun ("AI parameter") value resolver, and the conditions on the number of players (1) and champion (0); an import of a Startup group (`D_Resources`, `D_Attributes`) that the VM cannot resolve is looked up by path in Startup (evaluator and combat only). Maya's VM pawn now carries a `HealthPool` reference to her pool. `ow-package --combat-run` spawns the stock archetypes (target dummy from Sanctuary_Dynamic, the four bullymong variants from Ice_Dynamic) at a given level and stage with a `WillowMind` and reports the rows `tools/verify_combat_pawn.py --vm` reads.
Verified by running (against the live numbers of lanes L5/L5b, through the data chain, not the closed form): dummy level 8 = 778.658 (live 778.6583), adult level 7 = 812.637, ranged level 6 = 471.021, baby level 6 = 58.878, per-level base 102.347/117.755/135.439 at levels 5/6/7; slot 0, GUID 0, provider = the mind, as the live dummy's reference. The regular bullymong (level 6: 235.510) equals its closed form but has no live value. Not verified in the game: the noun resolver's default path (the four classes all list the attribute), the `CreatePool` reinitialise quirk, `ResetIdleTimer` (a guess), the `SetHealth` rules, `NumberOfPlayers` = 1 and champion = 0; the pawn's level and game stage are inputs (the stock spawn path that derives them is not modelled).
Checks: ctest 12/12 (new `combat-synthetic`: reference validity and stale references, `SetHealth(0.5)` -> 0 and `SetHealth(-3)` kept, lowest free slot, one pool per resource, destroy then fall back to `HealthVar`), verify_packages 9/9 with 3,339/3,339 class bodies, Fire event order 32/32, `--slice-run` unchanged except the stub line "EvaluateInitializationData: a context source other than the player controller" (Marcus's mind is now a known source), `verify_combat_pawn.py --vm` 7 PASS (4 health, 3 base), 0 FAIL, 22 MISSING (arrival, ratio and kill rows belong to later steps). `CMakeLists.txt` (sensitive): two source files and one test added.
UE suites (rerun by the orchestrator): UE build succeeded, test_quest 107/107 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0.

## 2026-10-08: the stock hit chain on the VM for a player hit on an AI pawn (plan step S-b), the dummy's floor from the game's own script

AI-assisted (Claude), implementer lane Sb, working from the own-words notes `NATIVE_DAMAGE_DEATH.md` and `NATIVE_COMBAT_GROUNDWORK.md` and the live records of lanes L1, L3, L5 and L6 and the installed script; no decompiler output was read. New `src/combat_hit.cpp`: `CombatSystem::hit` runs the victim's installed `TakeDamage` with a player controller as instigator, and the natives under it are bound (pipeline objects, `AdjustDamage` for a player hit on a flesh AI pawn, preconditions, `ActorTakeDamageInner`, `RecordRecentDamage`, surface and resistance modifiers, `NotifyTakeHit` which raises `OnTakeDamage` through a handler after `SetHealth`, the allegiance rule, the hit-region lookup, the AI component's demigod hold). `--combat-run` gained `player`, `attr`, `sequence`, `hit`, `hold` and `release` steps; `--slice-run` gained `pawnhit`; `FireMissionSlice::damageDummy` takes an optional damage (with a pawn it uses the chain, otherwise the legacy event-only path, so the host is unchanged).
Reproduces the live numbers (not "verified in the native"): the Fire dummy's pistol shot 74.013847 applies 111.020767 (incendiary flesh modifier 1.5), the splash 59.2111 applies 179.0419 (live 179.0418) once Maya's controller carries her live `InstigatedGrenadeDamageModifier` 2.015859603881836 (her skills are not in the VM; the value is from another launch's snapshot of the same character), and the whole sequence follows the live health trail to 1.0, where the capping hit applies 86.512 and every later hit 0; the L3 shotgun pellets (41.398 at level 8 against 5) take 360.876 to 319.478 / 278.080 / 236.682. The floor is the game's own script: the first `OnTakeDamage` runs `Behavior_AIHold_59` (Reason Safe, Type 3), whose installed script selects `HoldDemigod` for Type 3 (enum AI, Target, Movement, Demigod, God, Dialog; lane L6 had read the live values as bit flags), `IsDemigodMode` answers true and the stock `AdjustDamage`/`TakeDamage` scripts cap at 1.0.
UNVERIFIED: the bodies of every native (the hold natives keep one hold per reason), the stages of `AdjustDamage` beyond the ones the live rows exercise (critical regions, melee extras, intrinsic armor), the pawn resistance attributes (the VM does not initialise them; read as 1), the player's allegiance (Neutral), a stand-in that applies the hold to the pawn's mind because the VM's class tables do not list `IAIInterface` for `WillowAIPawn`. Not implemented: AI-instigator stage 1, shields, backstab, reflection, averted death, protection timer, the bullymong class provider's `Behavior_Conditional`.
Checks: ctest 12/12 (combat-synthetic extended: demigod holds, floor 1.0, event order), verify_packages 9/9 with 3,339/3,339 class bodies, Fire event order 32/32, `--slice-run` byte-identical to the HEAD build on both step lists, `verify_combat_pawn.py --vm` 7 PASS / 0 FAIL / 22 MISSING, test_quest 107/107 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0. `CMakeLists.txt` (sensitive): one source file added.
Rerun by the orchestrator: ctest 12/12, verify_packages 9/9, event order 32/32, the L5 sequence (same numbers), UE build, test_quest 107/107 + 11/11, test_mover 16/16, test_inventory_actions 49/0. Known wart: `CombatSystem::bind` now ignores any `std::exception` while registering a native (it used to catch only the runtime's own error), which could hide an unexpected failure there.

## 2026-10-08: native note for AI target choice and melee (lane G25), and REA 6.0.0 cannot open Borderlands2.exe

NATIVE_AI_MELEE.md (lane G25): first read of how an AI picks a target and lands a melee hit, for plan step S-h. Script-first: the bullymong chain is Action_AnimAttack (script) ->
ActionSequence.TriggerBehavior("Attack") -> animation -> Behavior_MeleeAttack (script) -> MeleeDefinition.MeleeAttack (native). Target choice and action selection are native AIComponent code driven by
AIDefinition.NodeList and GD_AI_Targeting.Targeting_ShortRange data. Read: AIComponent cadence (target update then state update, one UpdateRate gate), FindTargets (XY radius), AddTarget, stale removal,
CalcTargetInfo, PrioritizeTarget (weighted mean of iterator OnePriority), ChooseBestTarget, SetTarget (Target_Lost / Target_Acquired), ValidCurrentTarget (CanTargetIf to acquire, StopTargetingIf to drop),
the targeting iterators' formulas, tree selection (action / random / priority / sequence, TimedStart armed on first target), TriggerBehavior, MeleeAttack radius volume (vertical band centre +- DamageHeight,
half-angle DamageConeAngle / 2, 3D radius, line-of-sight trace), hit application (hit-once, allegiance flags, damage and knockback, behavior lists). Everything UNVERIFIED; the note lists the pyunrealsdk
reads that confirm each rule. Not read: WillowAIComponent overrides, WillowMind awareness natives, IMeleeAttacker implementations, the over-time repeat. Tooling: REA 6.0.0 could not open Borderlands2.exe
(fixed 330 s startup deadline); the existing Ghidra project was driven with tools/ghidra/run.ps1 instead. No tracked source changed.

## 2026-10-08: real-game lane L7, projectiles and firing patterns observed in the running game

AI-assisted (Claude), real-game lane L7. Method as lanes L3/L5/L5b (one game process under the run lock, saves, config, SDK settings and Sanity Saver's private copies hashed before and after, `Install-Driver -BlockSavesAtStart`, memory-only, closed through its window, driver removed). Enumerated every `FiringModeDefinition` (150: Bullet 101, Rocket 43, Beam 6, HitScan 0) and `ProjectileDefinition` (159) with all properties, and mapped parts to modes: 110 barrels carry a `CustomFiringModeDefinition` (Bullet 70, Rocket 39, Beam 1; none on elemental parts), the other 92 use the weapon type's `DefaultFiringModeDefinition` (30 Bullet, 5 Rocket types). The cooked `Startup.upk` has 62 such barrels, all identical live (62 of 62; the other 49 are DLC); 33 of 33 type defaults agree; 2 of 88 mode scalars and 1 of 79 projectile definitions differ.
Shots (168, about 26 gun builds incl. the five slice guns, replaced barrels, launchers; hooks on the script-visible shot functions and per-frame reads of the light projectile manager): `GetFiringModeDefinition` returns barrel custom mode else type default; one pellet loop per shot of `ProjectilesPerShot` pellets, each = pattern offset, then the weapon spread cone (disc of `Spread/2` degrees, 0 of 271 outside; none when `bSuppressWeaponSpread`), then one bullet; pattern lines are chosen sequentially (index advances per pellet, reset each shot only with `bResetPatternAfterEachShot`) or randomly, a start-point line is a fixed offset and a rectangle line a random point on its segment, scaled by `max(Spread/BasePatternSpread, MinPatternScale)` when `bScalePatternByWeaponSpread`; bullet speed = mode `Speed` x `WeaponProjectileSpeedMultiplier` (13 modes exact), a negative `AirFriction` accelerates it as `v *= 1 + |f| dt`; stored and delivered damage per pellet = the weapon's `InstantHitDamage`; momentum = direction x type momentum x 100; rockets fly at `ProjectileDefinition.SpeedFormula` x multiplier from about 125 to 135 uu ahead of the eye, damage = weapon damage via `Behavior_Explode`; a beam barrel takes the beam path (no bullet). The host's `FireWeapon` (one figure-eight trace, instant hit, one damage call) differs on every point; the concrete change list is in `docs/verification/PROJECTILE_FIRING_PATTERNS_LIVE.md` section 7.
UNVERIFIED: random-line selection with several lines, pattern scale above 1, the spread draw's density, positive air friction and `Acceleration`, ricochet and penetration follow-through, rocket spawn point against the decoded muzzle socket, pattern index on weapon swap, and why the spawned guns' live damage is 0.7589 of the card damage. Save safety: 60 files hashed before and after; Sanity Saver's `Save0008.json.gz` and the SDK's update-check stamp in `python-sdk.json` differed and were restored from the backup, then 0 of 60 differ. Automated checks: none (no code under `src/`, `host/` or `tests/`; `py_compile` of `firing_probe.py`).

## 2026-10-08: the stock death chain for a player kill of an AI pawn on the VM (plan step S-c)

AI-assisted (Claude), implementer lane Sc, working from the own-words notes `NATIVE_DAMAGE_DEATH.md` (section 9) and `NATIVE_COMBAT_GROUNDWORK.md` (sections 4 and 6), the live record `COMBAT_GOLDEN_TRACE.md` and the installed script; no decompiler output was read. New `src/combat_death.cpp`: `enableDeaths()` gives the `--combat-run` world a `WillowGameInfo` (the installed `Pawn.Died` returns at once without `WorldInfo.Game`, and the kill credit is script on it) and a mission tracker; `prepareForDeath` gives an adopted pawn an empty inventory manager, its status and special-move components, its death definition, a consumer handle and walking physics; the natives the chain reaches are bound (the tracker's `NotifyPawnDied`, the death definition's `OnKilledBy`/`OnPlayDeathPizazz`/`OnDeathNonGib`/`OnDeathGib` raised on its provider by the driver, `BroadcastBehaviorEventFromScript`, the expression evaluators of `Behavior_Conditional` read from the data, `IsHumanControlled`, presentation no-ops); `DropLootOnDeath` is logged (loot is plan step S-g). `Runtime::callTrace` records every call in order (the earlier census was alphabetical). The slice does not call `enableDeaths`, so the host's dummy is unchanged.
Reproduces the live order: for the L3 killing pellet (adult level 5, `SetHealth(-11.698)`) all 21 calls of the live death list occur in the live order (`TookDamageFromEnemy` ... `CheckInjured`), `OnKilledBy` runs its three behaviors, `AIDefinition.OnKilledPawn` does not run for the player's kill, the health pool is gone afterwards (`DestroyHealthPool`), and the bullymong class provider's `Behavior_Conditional` now selects the link to `Behavior_SpecialMove` as the live pellets did (Sb's open item). The Sb numbers are unchanged (L5 pistol/splash sequence, L3 pellets).
UNVERIFIED: the bodies of every native above, the expression evaluators (a plain reading of the data), `IsHumanControlled` (name only), the stand-ins (walking physics, an empty inventory, the game class), `OnPlayDeathPizazz` (3 behaviors here, 4 live), the kill-stat registry (the tracker has none), `CheckInjured` (stub).
Checks: ctest 12/12 (combat-synthetic: a toy killing pawn's call order and post-state), verify_packages 9/9 with 3,339/3,339 class bodies, Fire event order 32/32, `--slice-run` byte-identical to the HEAD build on both step lists, UE build + test_quest 117/117 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0. `CMakeLists.txt` (sensitive): one source file added.

## 2026-10-08: kill experienceon the VM, read from the data (plan step S-d)

AI-assisted (Claude), implementer lane Sc, working from `NATIVE_COMBAT_GROUNDWORK.md` section 5 and the live rows of lanes L3 and L5b; no decompiler output was read. New `src/combat_xp.cpp`: `WillowExperiencePipeline.CalculateExperiencePointsForKill` evaluates the globals' `BaseEnemyExperienceFormula` for the killed pawn with the existing attribute evaluator (the pawn's `ExperiencePointMultiplier` x the playthrough multiplier x (level + 4); the multiplier is written at adopt from the class's starting value) and applies the critical factor; `AwardCombatExperienceToParty` walks `WorldInfo.GRI.PRIArray`, takes the party weight row, the level-difference table row (lower or higher), truncates and calls the controller's `ExpEarn(amount, 1, 1)`, all tables read from the data. `--combat-run` gained `maya:<level>[:<exp>|:short<n>]` (a run built over the mission bridge: the player's experience pool and the installed level-up chain take the credit), `tick`, `players` and `killtag`, and the kill rows `kill.<class>.L<level>.p<player level>` = `{award, credit}`; the bridge's player info is now listed in `GRI.PRIArray`.
Reproduces (not "verified in the native"): the seven live kills, 50/35, 137.5/123, 50/7 twice, 75/11 (L5b) and 112.5/45, 50/7 (L3), all PASS in `verify_combat_pawn.py --vm` (lower-level entries d = 1 to 4 only); a kill 10 points short of level 9 takes the pool over the line and the next tick rebases the health pool through the existing level-up chain (212.676 to 240.323).
UNVERIFIED: the natives' bodies, equal level, a victim above the player, d >= 5, more than one player and playthrough 2+ (the synthetic test with invented tables only), the critical factor, the combat multiplier (1.0 here). One disagreement left open: the note writes the award as max(10, ...), the installed definition has its minimum restriction switched off (no real class reaches 10, so no row separates them); the evaluator follows the data.
Checks: ctest 12/12 (combat-synthetic: invented tables, 45 / 33 and 7 / 60 / 30), verify_packages 9/9 with 3,339/3,339, Fire event order 32/32, `--slice-run` byte-identical, `verify_combat_pawn.py --vm` 14 PASS / 0 FAIL / 15 MISSING (7 kill rows), UE build + test_quest 117/117 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0. `CMakeLists.txt` (sensitive): one source file added.
Orchestrator note: the UE suites (117/117 + resume 11/11, mover 16/16, inventory 49/0) were run by lane Sc over other lanes' uncommitted host edits; they are rerun on the committed tree when the host firing lane lands.

## 2026-10-08: host gun fire follows the live firing mode (lane Hf)

 AI-assisted (Claude). The host fired one instant ray per press along a
made-up figure-eight. It now follows the observed rules of PROJECTILE_FIRING_PATTERNS_LIVE.md (R1, R3 to R6; section 7 items 1 to 6 and 8): the
item card carries a `firing` block written by `tools/weapon_refresh_firing.py` from the L7 live dump (the cooked reader still cannot decode
`FiringPatternLines`; no offsets were invented); a press makes `ProjectilesPerShot` bullets, each with the pattern offset then the weapon spread cone,
flying at the card's `projectile_speed`, 16,384 uu range, swept per tick, damage applied per pellet that hits. **Verified (host suite, not the real
game):** 10-pellet shotgun press fires 10 bullets and takes the ammo once; pellet directions stay inside pattern + cone (6.89 of 7.91 deg); 8 of 10
pellets landed, one damage call each; landing is never before length/speed; the pattern rules on an invented weapon. Quest 117/117 + resume 11/11,
mover 16/16, inventory 49/0. **UNVERIFIED:** accuracy-pool cone (none in the host), random line draw, two-axis rectangle shape, scale above 1, spread
density, positive friction/Acceleration, ricochets, rocket and beam modes (carried, fired as bullets with a warning), the Jakobs rifle's type momentum,
tracer look; the in-game parity of any of it. The 0.7589 damage factor (open item 5 of the record) is NOT applied: the host damage is still the card
damage.

Orchestrator rerun on the combined tree: UE build succeeded, test_quest 117/117 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0; ctest 13/13 (the new `weapon-refresh-firing-synthetic` registered in `CMakeLists.txt`, a sensitive file: one `add_test` line).

## 2026-10-08: a bullymong's melee blow on Maya on the VM, without AI (plan step S-e)

AI-assisted (Claude), implementer lane Se, working from the own-words notes `NATIVE_AI_MELEE.md`, `NATIVE_DAMAGE_DEATH.md` (section 1) and `NATIVE_COMBAT_GROUNDWORK.md` (3.2 to 3.5), the live records of lanes L3, L5b and L6 and the installed data; no decompiler output was read. `--combat-run` gained `swing:<attacker>:<kind | definition path>`: a fixture blow that always connects. The damage is read from the definition (`MeleeDefinition.Damage`, or for the Slinger's rock the `ProjectileDamage` entry of its throw behavior scaled by the projectile's `Behavior_CauseDamage` formula) and evaluated with the attacker's `WillowMind` as the context source; it is delivered to Maya's VM pawn through the installed `WillowPlayerPawn.TakeDamage` chain with the mind as the instigator and the definition's damage source and type. `AdjustDamage` stage 1 now multiplies an AI instigator's damage by the globals' `GlobalAIDamageScale` (`Init_GlobalAIDamageBalanceMultiplier_Part1`, evaluated with the instigator as context; 0.65 x (1 + 0.02 x the attacker's level) on one player, first playthrough), and the level lookup answers for an AI mind, so stage 2 uses the AI table.
Reproduces (not "verified in the native"): arrival 73.6974 / 83.2781 / 94.1042 (adult 5/6/7), 24.9834 (regular 6), 12.4917 (baby 6), 44.2184 (ranged rock 5); applied/arrival 0.715 / 0.728 / 0.741 / 0.728 / 0.728 / 0.715; applied values 52.694, 69.731, 18.188, 9.094, 31.616 (L5b) and 60.63 (L3) on level-8 Maya at full health; `verify_combat_pawn.py --vm` 25 PASS / 0 FAIL / 4 MISSING (the Defense-Boost rows). The comparator checks the closed form, so the adult-L6 ratio is the formula plus the one L3 observation (every adult-L6 hit in the L5b/L6 dump is boosted).
UNVERIFIED: the bodies of every native reached (all bound in S-b to S-d), the higher-level-attacker table entry (no live hit), the AI weapon scales, which throw/projectile the live Slinger uses, the damage causer, Maya's missing body class (no hit region). Not implemented: hit volume/cone/allegiance/knockback/OnHitEnemy behaviors (S-h), the Defense Boost, the shield stage, the protection timer, averted death, the down state (S-f). `MeleeDef_PrimalBeast_RadiusBadass` has no damage source in its data and is refused.
Checks: ctest 12/12 (combat-synthetic: toy melee definition, thrown rock, error cases), verify_packages 9/9 with 3,339/3,339 class bodies, Fire event order 32/32, `--slice-run` byte-identical to the HEAD build on both step lists, Sb L5 sequence unchanged, `verify_combat_pawn.py --vm` 25 PASS / 0 FAIL / 4 MISSING. UE build and suites not run. No sensitive file touched.
Orchestrator rerun: ctest 13/13, verify_packages 9/9, event order 32/32, a fresh `--combat-run` gives 25 PASS / 0 FAIL / 4 MISSING in `verify_combat_pawn.py --vm`. The UE suites ran on the combined tree with Hf's host edits (117/117 + resume 11/11, mover 16/16, inventory 49/0).

## 2026-10-08: token budget for agents: one-page lane brief, monthly decisions archive, lookup tools

AI-assisted (Claude), at the maintainer's request. Subagent lanes were re-reading about 15 to 20k tokens of instructions each (AGENTS.md, CLAUDE.md,
docs/LEGAL.md, docs/NATIVE_ANALYSIS.md and an ignored local rules file that does not travel between worktrees), and some opened DECISIONS.md (436 KB)
or NATIVE_INDEX.md (102 KB) whole. Now: `docs/LANE_BRIEF.md` condenses the binding rules on one page (about 1.2k tokens; the long documents stay
authoritative and unchanged, and a pointer at the top of AGENTS.md and CLAUDE.md sends subagents to it); `docs/verification/GATES.md` holds the gate
commands and the current baseline; the 88 entries of 2026-09 moved verbatim to `docs/decisions/2026-09.md` (DECISIONS.md now 246 KB; months rotate the
same way, `docs/TOOLING.md` "Decisions log: monthly archive"); `tools/decisions_index.py --check` confirms all 216 pre-split entries are present exactly
once and identical after CRLF normalisation (checked against f6504f3); `tools/native_index_find.py` prints matching index rows. `docs/TOOLING_TRIM_PROPOSAL.md`
lists repeated sections of TOOLING.md for the maintainer to approve; nothing was deleted. Checks: ctest 15/15, verify_packages 9/9. All tracked, so the
setup persists in new worktrees.


## 2026-10-09: real-game lane L8, bullymong targeting and melee observed in the running game

AI-assisted (Claude), real-game lane L8. Method as lanes L3/L5/L5b/L7 (one game process under the run lock, 60 files hashed before and after, `Install-Driver -BlockSavesAtStart`, memory-only, closed through its window, driver removed). Read `GD_AI_Targeting.Targeting_ShortRange`: no `CanTargetIf`/`StopTargetingIf`, search-radius override off, four Knowledge iterators plus a fifth entry holding no object, prioritisation weights Nearby 1.0 (MaxDistance 4096), Exposed 1.0, Previous 0.4, Bonus 1.0, Idle 0 and Attacking 0 (sum 3.4). On `Ice_P` with a level-8 Maya: `AIComponent.UpdateRate` 0.2 and `LastTargetUpdateTime` equal to `LastStateUpdateTime` in 2,433 of 2,433 frames (spacing 0.1999 minimum); the record's `Priority` equals `(max(0, 1 - Distance/4096) + TargetExposureToMe + 0.4 if current target + BonusPriority) / 3.4` on 134 samples to 5e-8 (3D `Distance`; the exposure term is unclamped, -1.0 was seen); the 4096 XY search radius of NATIVE_AI_MELEE is not a filter on the list (records at 4.1 to 7.0 km, to 10 km at the arrival ledge, dropped at about 7.07 km in one geometry, added at 6.94 km; height ignored); a record is not a current target (acquisition followed sight: an adult with `Exposure_None` never acquired down to 2,925 uu). The adult's Close Melee: `TriggerBehavior("Attack")` to `Behavior_MeleeAttack`/`MeleeDefinition.MeleeAttack` 0.400 to 0.425 s (mean 0.4103, 35 swings), swing interval median 1.249 s, all with `MeleeDef_PrimalBeast_Melee`; leaps call `MeleeDef_PrimalBeast_RadiusLeap` at landing. 71 controlled swings of that definition on the player: reach 640 horizontal from a point 100 uu behind the attacker, half-angle 64 deg from that point, damage 94.104, momentum 75,000 horizontal from the attacker's `Location` (0.003 deg), vertical band -143 to +339 about the pawn's `Location` (the note's +/- `DamageHeight` plus half-height is refuted). The 0.7589 of L7 is `1.13^8` (L7 built the guns with `ManufacturerGradeIndex` 0, so `ExpLevel` 0) times the Badass Rank scale (1.01747) on the equipped gun; at grade index 8 the base damages equal the cards (63.63, 49.13, 22.26, 36.11, 20.65).
UNVERIFIED: the cause of the 7 km cut and its difference between positions and pawn types, horizontal against 3D for it, the hidden part of the current-target choice, `Target_Lost` before `Target_Acquired`, the vertical band's dependence on the victim, other melee definitions, line-of-sight blocking, the equipped grade-8 damage (128.4 is arithmetic). `AIDefinition.OnTargetAcquired/OnTargetLost` never fired through script (native-to-native). Save safety: 60 files hashed before and after; Sanity Saver's `Save0008.json.gz` and the SDK's `python-sdk.json` stamp differed and were restored from the backup, then 0 of 60 differ. Automated checks: none (no code under `src/`, `host/` or `tests/`; `py_compile` of `ai_melee_probe.py` and `firing_probe.py`). New: `tools/real_game/scripts/ai_melee_probe.py`, `docs/verification/AI_MELEE_TARGETING_LIVE.md`.

## 2026-10-09: native note for AI awareness, Willow AI component overrides and the IMeleeAttacker (lane G26)

NATIVE_AI_AWARENESS.md (lane G26): second read of the AI target and melee natives for plan step S-h, working through the "Not read yet" list of NATIVE_AI_MELEE.md. Read: the WillowAIComponent overrides (GetTargets, ValidTarget, ValidCurrentTarget, InitializeTargetRecord, TargetRemoved, TargetChanged, NotifyAttackedBy, SetThreatLevel / GetThreatLevel, leaders, combat zones, holds), WillowMind awareness natives (NotifyAttackedBy, IsAwareOfTarget, AddTargetable, NotifyTakeHit, the CheckAwareness iterator), the IMeleeAttacker implementations and the over-time repeater, the FindTargets skip, node eligibility / completion (TimedReset, LoopCount) / interruption, and the leap natives. Findings that change earlier notes: ValidCurrentTarget additionally requires threat level >= SeenInThreatRadius, so a bullymong needs to notice Maya (sight rule or AICloseThreatRadius proximity rule in WillowTI_CalcThreat) or be attacked (NotifyAttackedBy sets Provoked, 10 s forget time) before any record can become CurrentTarget; the base ValidTarget friendliness test is "Friendly" (Neutral passes) and an AI without an allegiance has no valid target; FindTargets skips Neutral-opinion registry entries when the AI has an allegiance; the AI pawn's melee origin is the actor location (aim-offset rotation) not an eye point; MeleeAttack for over-time definitions begins the over-time state and sweeps once at once, then the pawn tick repeats every max(AttackInterval, 1/15 s) until MaxMeleeDuration; ActionSequence.Finish re-selects immediately and TimedReset is the post-completion cooldown; the 1.0 s post-failure cool-off applies in normal play. Route: the Willow class tables were found by comparing the base AIComponent table with all read-only tables and confirmed from the class registration. Everything UNVERIFIED; each section lists the pyunrealsdk reads that confirm it. No tracked source changed. Tooling: REA not used (cannot open Borderlands2.exe); Ghidra via tools/ghidra/run.ps1 and three small local helper scripts under ignored local/analysis/G26/scripts.

## 2026-10-09: maintainer decision, the project targets the original gun part tables; the Remaster tables are out of scope for now

AI-assisted (Claude), recorded at the maintainer's request (option 1 of the choices offered). The slice and the UModel export pipeline use the original gestalt part tables in `Startup.upk`; the 7 weapon variants in `Mancana_StartupRemaster.upk` are not used and no repository code refers to them. Basis: `docs/verification/RTSE_GUN_DATA_CROSSCHECK.md` section 4 (the Remaster tables use the same fragment names with different ranges, order and larger triangle totals, so one cannot be applied to the other's glTF export; 31 of 31 live fragment ranges from our own capture match the original table, 0 of 31 the Remaster table). Revisit if a live capture ever matches the Remaster ranges, when the slice gate is met, or if the Remaster meshes are wanted. UNVERIFIED and left open: when the live game uses the Remaster tables (a setting or a DLC state), whether the Remaster meshes should ever be exported, and the maintainer's recollection that one of the sets is the low-poly one (the record says the Remaster totals are larger; no per-model look was checked).

## 2026-10-09: Maya's down state, second wind and respawn on the VM (plan step S-f)

AI-assisted (Claude), lane Sf. The installed `WillowPawn.HandleHealthDepleted` -> `PlayInjured` -> `injured` state runs on the VM from a lethal swing; its state-level script is two bytes (no
`Sleep`, no labels), so it is `BeginState`/`Tick`/`EndState` and needed no latent facility. `--combat-run` gained `cash:<amount>` and `bleed:<seconds>[:<dt>]` (frames; Maya's `Tick`, the script's own death camera each frame, and the live delays for the special move end 2.28 s, the death animation end 0.84 s and the respawn dialog 11.8 s as
labelled stand-ins); `src/combat_down.cpp` binds the down natives (UNVERIFIED role-level stand-ins), the evaluator got a controller context resolver and a currency value resolver (UNVERIFIED), `CMakeLists.txt` got the new source. Results: the stock script gives bleed-out 12 / 12 / 8 s for injuries 1 / 2 / 3 (the counter is updated at the end of `BeginState`;
the old note's `total / 3 x N` used the counter after the injury; no native differs), 12 / 8 / 4 / 1 for a first injury within 30 s of game time 0; second wind health 0.25 of the maximum; fee 190 / 177 / 165 on 2,715 / 2,534 / 2,357; death camera 5.75 to 5.78 s (live 5.76), `verify_combat_pawn --vm` 55 PASS / 0 FAIL / 4 MISSING. Verified only as
"the VM reproduces the live row". Not done: timers, protection timer, shields, station choice (a bare `PlayerStart` stands in), injured behaviors, Defense Boost. Slice unchanged (`--slice-run` byte-identical). Correction to NATIVE_DAMAGE_DEATH.md 10.2 and COMBAT_GOLDEN_TRACE.md question 6 (the cut is applied to the counter before the injury).

## 2026-10-09: docs/TOOLING.md trimmed per the approved proposal

AI-assisted (Claude), lane H9, maintainer approved `docs/TOOLING_TRIM_PROPOSAL.md` on 2026-10-09. Applied row by row: 89,888 -> 79,492 bytes (-10.2 KiB). Cut only prose whose content was found in the named verification record or DECISIONS entry; every command block, the headings cited elsewhere (`Tracing the real game's UI code`, `Driving the real game`, `External tools`, `Decisions log: monthly archive`) and all other headings were kept; one `##` heading was added (`Inventory preview follow-ups (paint pass and Maya pose audit)`, holding command blocks that sat under the BSP heading). Rows left unchanged because the kept copies do not hold the usage text: Tools added 2026-10-01/02, Driving the real game, Maya with the Infinity and Phaselock, Verification status of the reader, Sanctuary artifact trace. Three stale statements were dropped because the newer record contradicts them (PageUp/PageDown direction, Backpack Enter as a direct equip, the paint pass 3 base-colour formula, the single `-owbubbleradius` value). The only anchor cited from outside (`#external-tools`, ROADMAP.md) resolves. No code or behaviour changed; no gates apply.

## 2026-10-09: real-game lane L9, bullymong awareness, attack route, melee over time and AI tree observed in the running game

AI-assisted (Claude), real-game lane L9. Method as lanes L3/L5/L5b/L7/L8 (one game process under the run lock, 60 files hashed before and after, `Install-Driver -BlockSavesAtStart`, memory-only, closed through its window, driver removed). On `Ice_P` with a level-8 Maya: every sampled bullymong (about 25 pawns) is a member of a population den (non-patrol), and the notice rule of `WillowTI_CalcThreat` works on the den. Level 2 is requested at a member's target update by sight (the record's `Exposure` of the previous update is not None, `DotToTarget` above `SightViewCone` -4.37e-8, the player's horizontal distance from the den's centre below the den's `ThreatRadius`: fired at 4,660 and not at 4,680 with radius 4,665.625, one update (0.209 to 0.220 s) after the exposure turned Full; not at 4,640 with the member behind, dot -0.67) or by proximity (3D distance minus collision radius below `AICloseThreatRadius` 768 and horizontal distance from the den below its `PatrolRadius`: no at 770.0, yes at 728.0; no for 1,886 frames with her beyond the patrol radius and a member 234 uu away); `CurrentTarget` is set in the same update. `AggroTime` and `ForgetTime` are written on the den's record only (members copy the level): `ForgetTime` = `AggroTime` + 10.0 (den `MinEngagementTime`), refreshed at every update while the rule holds, +25.0 for a provoked level 3. A real shot (`TakeDamage` 41.4 -> `WillowMind.NotifyAttackedBy(player, true)`) and a script call gave level 3, stamp, +25.0; the four members acquired 0.15 s later and called `SetLookAtTarget`. The level **falls** at the den's next update after `ForgetTime` (0.03, 0.81, 0.92 s; the note's "no reader of `ForgetTime`" is refuted): to 1 in place (`AggroTime` back to 0) or the record is removed (player pushed 9 km away), and `CurrentTarget` goes 0.13 s later. Fresh records start at level 1 (0 only without a record), `TargetExposureToMe` -1.0 on the first update; `IsAwareOfTarget` is True for a fresh record, False without a record and for 0.1 s after the stamp. `ValidTarget`: player true; None, self, same-allegiance beast, cloaked player, cleared allegiance false. Target change: `bProvoked` False -> True, `ProvokedLocation` = the pawn's location, `NumAICurrentlyTargeting` +1 per AI. `MeleeDef_Melee_Charge`: hit at the call, 74 sweeps in 5.007 s (1/15 s), one hit on the player (`bHitEachTargetOnlyOnce`), ends 5.007 s after the start; the AI trace source equals `Location`. Tree: `TimedStart` +10.0003 on `Target_Acquired`, `TimedReset` +27.05 (20 to 30), random-node counters as the note.
UNVERIFIED: the cone boundary at cos 0; a bullymong without a den or in a patrol den; why a record stays at level 1 in one geometry and is removed in another; a second hit refreshing `ForgetTime`; `LastAttackedTime`/`LastHurtTime`/`LastAttacker`; the den helper; `LoopCountMax`, the 1.0 s failure block and `Finish` re-selection (not exercised); downed player, friendly NPC, vehicle. Save safety: 60 files hashed before and after; Sanity Saver's `Save0008.json.gz` and the SDK's `python-sdk.json` stamp differed and were restored from the backup, then 0 of 60 differ. Automated checks: none (no code under `src/`, `host/` or `tests/`; `py_compile` of `ai_awareness_probe.py`, `ai_melee_probe.py` and `firing_probe.py`). New: `tools/real_game/scripts/ai_awareness_probe.py`, `docs/verification/AI_AWARENESS_LIVE.md`.

## 2026-10-09: native note for the natives plan step S-f stubbed: second wind, injured queries, timers, special moves, protection timer, respawn (lane G27)

NATIVE_DOWN_TIMERS.md (lane G27): analyst read of the natives plan step S-f stubbed when Maya goes down, bleeds out, takes a second wind or respawns. Read: `SetSecondWindReason` / `ClearSecondWindReason` (a guarded
byte store: only for an injured pawn, first writer wins; no health change, so the kill-frame 25 percent health is script or data, probably the `KilledEnemyBehaviors` run in the same call), `PassSecondWindReasonToLeviathan`
(statistics hand-off), `Pawn.IsInjured` / `IsDead` / `WillowPawn.IsInjuredDead` (flags; a downed Maya is injured, not dead), the down state's timers (all one-shot; `CheckShouldBeAutoRevived` is armed once per injury, not every
2 s), `SpecialMoveComponent.Play` (returned length is the animation length divided by the effective rate; the named callback runs once at the move's end, with `bInterrupted` true when replaced, at once when the length is 0),
`GetAnimLength`, the protection timer (`EnableProtectionTimer` sets a flag, a 2 s one-shot and a random health floor between 1 and 10 percent of the maximum capped by current health; `ProtectionTimerElapsed` clears the flag),
`CheckLowHealthState` (per-frame edge trigger of the player class's low-health behavior lists), `ResetAI` (low confidence), `Pawn.SetAnchor`, `BehaviorBase.RunBehaviors` (loop over `ApplyBehavior`), and, as script facts,
`TeleporterDestination.GetNextExitPoint`, `TravelStation.CanResurrectHere` and the scheduler of `ShowRespawnDialog` (script, two 2.0 s one-shots). Corrections to NATIVE_LEVEL_UP_ATTRIBUTES.md section 9 (the guard is `IsInjured`),
NATIVE_DAMAGE_DEATH.md 10.2 (auto-revive check once) and stage 15 (random floor). Every rule lists the live observation that would confirm it. REA not used (cannot open Borderlands2.exe); Ghidra via
`tools/ghidra/run.ps1` plus G26's three local scan scripts. No tracked source changed.

## 2026-10-09: Loot on death on the VM (plan step S-g)

AI-assisted (Claude), lane Sg. A bullymong spawned by `--combat-run` gets the stock spawn's loot setup (the item game stage from the pawn balance's `BaseItemGameStage` and the truncated random `ItemGameStageOffset`; `SetupPawnItemPoolList` bound: included lists first,
the playthrough's custom lists replacing the defaults, no `PlayThroughs` entry = no list), and its death runs the installed `DropLootOnDeath` loop (replaced in C++ because the VM has no inventory actors or pickups): `FRand` per non-carried pool, the chance from the data, tried when the draw is at most the chance, then the bound
`ItemPool.SpawnBalancedInventoryFromPool` (NATIVE_LOOT.md: stage gate, one flat weighted list, quantity picks with replacement, nested pools, grade windows and modifiers, level = min(stage, 50), `bDropOnDeath` ANDed). The drop is a list of records on the kill row (and `loot.*` rows), not an object. The engine's random is a labelled seedable
stand-in (the Microsoft C runtime's `rand()`, default seed 20261009, step `seed`); the core `FRand` is not rebound, so nothing else in a run changes. The evaluator (`src/progression.*`) gained the random variance and the random, constraint, conditional and ammo-drop-weight value resolvers (UNVERIFIED readings) and returns "unsupported" as before
unless a random source is set (only the death setup sets one). Results (seed 20261009, 500 adult and 500 baby kills): the data chances (Guns 0.085, Skins 0.05, Money 0.25, Bar 0.0015, Stick 0.008, Health 0.03, NeedOnly and Emergency 0.2) read from the packages, the tried rates inside four binomial standard deviations (chi-square 18.1 for 18),
`Pool_Eridium_Stick` false at item stage 6 (minimum stage 7) and true at 7, the ammo pools always false for a player with no low ammo (the live false trials), `Pool_Health_All` true and dropped, `Pool_VehicleSkins_All` made but not dropped (`bDropOnDeath` false), the adult's item stage 4..7 (the live 4 and 7) and the baby's 6, item level = roll stage.
**Not reproduced (OPEN):** `Pool_Money_1or2` made 3 items for an adult live; the data's quantity is a random integer in -1..2 (at least 1), so the VM makes 1 or 2 (0.75 / 0.25); analyst read requested (the random variance's integer mapping, and any change of the quantity after it). Stand-ins: the health drop weight (linear below the 40 percent threshold, 0.1 to 0.25), the ammo weights
(no ammo pools: 0), items not initialised (no parts or stats). `tools/verify_combat_pawn.py` gained 43 `loot.*` rows and an OPEN status (97 PASS / 0 FAIL / 4 MISSING / 1 OPEN with the S-a to S-f run; that run alone stays 55 / 0 / 4). `CMakeLists.txt` got the new source. Slice byte-identical. Verified only as "the VM reproduces the live row".

## 2026-10-21: Sanctuary scene import: inspected material fallbacks, component owners, doubled colour buffers

AI-assisted (Claude), lane A. Details and counts: `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`.

- **Reader (`src/assets.cpp`, sensitive).** Three of the Sanctuary packages' meshes (27 of 17,605 StaticMesh exports
  in the 914 base packages) store a colour buffer of stride 4 whose count and bulk count are exactly twice the vertex
  count. The reader now accepts `vertexCount` or `2 × vertexCount`, reads the first `vertexCount` entries and skips
  the rest. It also accepts the export class `GearboxFramework.WiringMesh` with the StaticMesh layout. Oracle: with
  both changes every one of the 17,605 StaticMesh/WiringMesh exports decodes and leaves the same 73-byte trailer
  after its LODs; no export failed. Which colour entries the original renderer binds is `UNVERIFIED` (colours are
  not exported). No bounds check was loosened.
- **Owners (`tools/prepare_level.py`).** Owner actors outside the bulk scene records are read per actor with the
  observed 26-byte actor prefix and must end at the object end. `WiringActor` and `WillowInteractiveObject` components
  are placed like a `StaticMeshActor`'s; mission pickup spawners and failsafes, whose component is the actor's
  `PreviewMesh`, are listed in `excluded_components` (`mission_item_preview_mesh`), as is the second exact copy of a
  component on the same interactive object (`duplicate_component`; the copy must also match on `HiddenGame`, shadow and collision flags, so a visible component is never dropped for a hidden one). Confirmed in the running game (Plan B save):
  spawners and failsafes have no primitive component; every placed interactive-object component is attached, not
  hidden, with the package's mesh and relative pose; each vault-symbol actor attaches one plane, not two. Their
  visibility in other mission or challenge states is `UNVERIFIED`. The rules are not map-specific: on Ash they place 21 interactive-object components and list 17 duplicates (271 unsupported owners left, from 325), on Southpaw 4 (1 left, from 10); placement on those maps was not compared with the running game and is `UNVERIFIED`.
- **Materials.** Five more exact `INSPECTED_COLOR_FALLBACKS` (icicles, two distant snow fields, distant mountains,
  Roland's chemical tank), each from its parameter chain, cooked list and the decoded candidates; the icicles'
  `Ice_Dif` is the named internal-glow sampler and is not used as colour. All `partial_unverified`. The spire
  instances stay with the `prepare_sanctuary_pillar.py` bake (2026-09-23). `Mat_WorldMap` stays a gap: in game its
  packed `_Comp` channels are recoloured (pale cyan land, red markers), so the texture is not an RGB colour.
- **Host (`host/ue5/import_level.py`).** A Modulate material with no recovered colour gets a white neutral instead of
  0.5 grey, so the newly placed vault symbols and blood decal do not darken what is behind them.
- **Audit.** Terrain weighted-sum materials are counted as `terrain_weighted_blend`, not as "no supported channels";
  exclusions are counted by reason.

## 2026-10-21: Blocking volumes, sphere/capsule elements and recorded walking routes (Sanctuary)

AI-assisted (Claude), lane B. Tools (`collision_geometry.py`, new `collision_volumes.py`, `refresh_collision.py`,
new `walking_routes.py`, `real_game/scripts/walking.py`, `real_game/walking_capture.ps1`, `test_ue_viewer.ps1 -Route`)
and host (`OpenWillowCollision.*`, `WalkingTest.cpp`, `apply_collision.py`, `verify_collision.py`). No change to `src/`,
`CMakeLists.txt`, `prepare_level.py` or `OpenWillowWalker.*`. Details: `COLLISION_WALKING_VERIFICATION.md`, 2026-10-21.

- Blocking volumes are read from `BrushComponent.BrushAggGeom` (tagged `KAggregateGeom`) with the existing decoder;
  blocking comes from `bCollideActors`/`bBlockActors` with class defaults read from the installed `Engine` package.
  Rotated, scaled or pre-pivoted volumes are refused rather than guessed. Dynamic volumes enter the host only in the
  state observed in the running game. **Confirmed in game:** decoded geometry equals the live bounds for all seven
  brush volumes; brush components do not block zero-extent traces; path-blocking volumes do not collide.
- `KSphereElem`/`KSphylElem` are decoded (scaled frames refused); the capsule axis convention is `UNVERIFIED` and no
  placed Sanctuary mesh uses either.
- `refresh_collision.py` now keeps `prepare_level`'s triangle floors and `placement_collision()` exception (it had
  re-enabled one landed placement) and skips BSP/terrain entries.
- Routes are compared on ground height under the centre, not feet height: the running game's cylinder stands 35-42 cm
  above a 33 degree ramp hull (rim contact), a UE5 capsule lower. Its floor normal matches the hull, so stairs use the
  simple hull in the original as in the host. Results: 29/29 of the probes the original finished, volumes 5/5.
  Read but not observed: `JumpZ` 630 (the walker keeps its 420 estimate). The host walks two probes the original
  could not; unexplained.

## 2026-10-21: Phaselock presentation round 16: a stock bullymong as the lifted target, cast-arm blends, intro flash

AI-assisted (Claude). Host, tools and docs (`OpenWillowCombatTarget.*`, new `OpenWillowTargetAnimInstance.*`,
`OpenWillowArmsAnimInstance.*`, `OpenWillowPhaselockFx.cpp`, `import_phaselock_fx.py`, a few lines in the shared
`OpenWillowWalker.cpp`; new `prepare_phaselock_target.py`, `import_phaselock_target.ps1`, `phaselock_target_editor.py`,
`real_game/scripts/phaselock_target_anim.py`); no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` "Round 16".

- **Target animation (maintainer item 2):** the slice dummy has no `PhaseLock_*` clips, so the host's lifted-target chain
  had never run. `-owcombatpawn=Bullymong` puts the stock bullymong (all four clips) in as the test target, with the stock
  move blends (lift in over 0.4 s, others 0.1 s, fall stretched to `DropTime`), the bubble sized from its own bounds (no
  `-owbubbleradius` stand-in) and the mesh centring. Observed in the real game (SDK probe, two casts): the mesh is lowered
  119.5 uu through the hold and the base pose blends out over about 0.4 s after the cast; the host's centring is fitted to
  the first (58.7 uu bias), the second matches the stock blend.
- **Mesh scale open:** the live component reads `Scale` 1.0, the archetype 0.7; frame sizes favour 0.7, the live head
  height favours 1.0. The host uses 0.7 and divides the bounds by it (`UNVERIFIED` reading).
- **Cast arm (item 3):** stock 0.1 s blends with the blend-out ending; the round 8 rate 0.85 stays (the arm drops about
  0.1 s early at rate 1). The hand smear was UE5's per-bone motion blur, now off on the arms and the stock pawn.
- **Bubble (item 1):** the intro's `Mat_SirenGlowMOD` flashes use the shader's own overbright (1) instead of the hand's
  0.4 fit.
- **Visual review:** three independent critic rounds against the matched real-game frames: overall 5.8, 5.7, 5.5 (bubble
  6.0 / 5.5 / 5.5, target 4.5 / 4.5 / 4.5, arm 7.0 / 7.0 / 6.5). The 8/10 target was not reached. Open: the held pose shows
  the back where the game shows the front, the lift still reads early at 0.5 s, the bullymong's skin colour, the opening
  cyan sweep and the plain rim, the eye motif under the target (HUD, not this lane).
- **Checks:** CTest 10/10; `verify_packages` 9/9; `tests/prepare_phaselock_target_test.py` 4 OK, `particle_system_test`
  OK, `prepare_action_skill_test` 9 OK; Phaselock shots 28 frames; quest suite 79/0 and 11/0 (a first run failed checks
  45 and 47 because this worktree had no turn-in reward data; passed after `weapon_slice_gear.py --reward-only`).

## 2026-10-21: Inventory and Skills pages round 16: skill tiles, Phaselock plate, overview, Inspect card, card title lines

AI-assisted (Claude). Page code (`skills.js`, `inventory.js`, `inventory.html`) and the `-owinvshots` key sequence in
`OpenWillowMayaHUD.cpp`; no change to `src/`, `CMakeLists.txt` or package parsing. Details and the 2026-10-21 real-game session:
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Rounds 16-19").

- Two movie states Ruffle never cleared are cleared by the page: the skill cell's `overclocked` outline (now only with a class-mod
  bonus, UNVERIFIED rule) and the Phaselock plate's pink "disabled" tint (icon in an untinted clip once trained).
- Ruffle lacks Scaleform's `TextField.getLineLength`, so the card's `SetTitle` counted every name as two lines; the page now passes
  the field's `bottomScroll` to `SetRequiredTitleLines`.
- Skills overview poses, tree height, plate size and position, description leading, Inspect zoom and the hint line are fits to
  captured frames (UNVERIFIED). Inspect uses the card background's first (blue) frame and hides the movie's `ring` layer.
- The capture sequence selects weapon slot 2 and backpack row 2 so the engine frames show the reference frames' states.
- **Visual review:** baseline critic 7.27/10, round-16 critic (another instance) 6.85/10 on the same 11 pairs; see the record for why.
- **Checks:** navigation 27/27, skill info 9/9; inventory suite 49 / 0 / 0 / 0 with the bounded item set, 24 / 3 / 22 / 0 with this
  machine's default item set (over capacity, local data).

## 2026-10-21: Inventory round 17: selectable empty backpack cells, compare column anchor, ITEMS headers confirmed

AI-assisted (Claude). `inventory.js`, `tests/inventory_navigation_test.js`, the `-owinvshots` sequence in `OpenWillowMayaHUD.cpp`.

- The trailing "(EMPTY)" backpack cells can be selected, as observed in the original game on 2026-10-21: no card, item actions
  ignored, Sort hint kept. The VM's `MoveDelta` still receives item rows only; moves involving empty cells are host code (UNVERIFIED
  against the VM's own handling of empty entries).
- The compare view places the Equipped column by slot 1's cell instead of the panel's bounds (a transient child skewed those).
- Live category keys of a class mod, relic, grenade mod and shield (`comm`, `Artifact`, `mod`, `Shield`) and the ITEMS headers
  RELICS, CLASS MODS, GRENADE MODS, SHIELDS were read in the running game; the host's labels and order already matched.
- **Visual review:** critic 7.21/10 (11 pairs). **Checks:** navigation 29/29; inventory suite 49 / 0 / 0 / 0 (bounded item set).

## 2026-10-21: Menu pages apply the movie's own 3D turn (round 18)

AI-assisted (Claude). New `tools/hud_overlay/menu3d.js` (served by `serve.py`), `inventory.*`, `skills.*`, the page report in
`OpenWillowMayaHUD.cpp`, `tests/inventory_navigation_test.js`.

- The StatusMenu movie turns the inventory clip 11 degrees and the skills clip 7.24 degrees about the vertical axis (`_yrotation`,
  read live); Ruffle ignores it. The pages apply the same turn as a CSS 3D transform at Flash's default 55-degree field of view
  (focal length 1229 px; Scaleform's value not read, UNVERIFIED; one panel-edge ratio agrees to 0.001). This replaces the "perspective
  tilt Ruffle ignores" open item for the inventory and Skills views; the curved glass sheet (`ring`, far behind with a brightening
  blend) is still not reproduced.
- Measured screen fits are mapped back through the projection before use; the Skills overview poses were refitted.
- The page report's row-alignment check uses layout offsets, since the hit layer and the movie share the turn.
- **Visual review:** critic 7.67/10 (round 17: 7.21). **Checks:** navigation 30/30; inventory suite 49 / 0 / 0 / 0.

## 2026-10-21: Inventory round 19: Inspect backdrop, card anchor, hint line; lane stops after four rounds

AI-assisted (Claude). `inventory.html`, `inventory.js`.

- Inspect backdrop drawn in CSS after the real frames (light from the top centre, soft bands, faint grid; our own drawing, a fit).
  Single card anchor and hint line moved to measured real positions (fits, UNVERIFIED).
- **Visual review:** critic 7.57/10 (round 18: 7.67; unchanged pairs moved by up to 0.6 between reviewers). The 8/10 target was not
  reached in four rounds; open items are listed at the end of `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Rounds 16-19").
- **Checks:** inventory suite 49 / 0 / 0 / 0 (bounded item set); navigation 30/30; CTest 10/10; packages 9/9.

## 2026-10-22: Maya's action-skill indicator drives BL2's own HUD clips; the HUD clusters' 3D turn gets a 2D stand-in

The maintainer asked for the purple indicator that runs out while Phaselock holds its target. It is two clips of the HUD
movie, driven in the game by native code (`WillowHUDGFxMovie.UpdateActionSkill` and its neighbours are native), so the
rules were read from the live movie during real casts (AI-assisted capture; record
`docs/verification/ACTION_SKILL_HUD.md`):
- `p1.activeAbility` shows the Siren emblem (`siren_action`) from the lock (0.70 s) to the skill's end (5.70 s); its bar
  is `floor(1 + 99 * elapsed / lock time)`. This emblem is also the "sigil under the target" that the Phaselock effect
  record had flagged as HUD.
- `p1.character.cooldown` follows the cooldown pool: `floor(2 + 98 * (1 - pool / 13))`, 2 while the skill runs (the pool
  is held at 13 through lift, lock and release), 100 when ready. The icon picture is `UI_HUD_AAIcons.AA-Siren`, loaded
  into `cooldown.icon.placeholderTexture`; `p1.character` sits on frame 6. A miss refills the pool, which drains and
  snaps to 0 after about 1 s. The host's existing pool model already matched; no Phaselock code changed.
- The whole HUD looked smaller in the host because the side clusters carry `_yrotation` (+-25, 15, -20) that Ruffle
  ignores, projected with the movie's 70-degree FOV; the unturned plane appears 1.095 times larger about the centre
  (fitted, UNVERIFIED). `tools/hud_overlay/hud3d.js` approximates it with per-clip 2D transforms (no trapezoid).
Implemented in `tools/hud_overlay/action_skill.js`, `hud3d.js`, `index.html` and `AOpenWillowMayaHUD`
(`ActionSkillHudState`, `-owskillhudshots`), with `tools/prepare_hud_icons.py` for the icon movie. Checked: synthetic
mapping and checker tests, the in-engine run 9/9 against the real timings, critic 7/10 then 9/10. Not reproduced: the
one-frame lag of the game's HUD, a post-ready highlight whose clip was not found, other classes.

## 2026-10-22: Skills keyboard vertical movement fit (partly UNVERIFIED)

## 2026-10-22: Inventory and Skills rounds 20-22 against matched real-game references; card content from the parts

AI-assisted (Claude, with Codex sub-agents). Lane D of the 2026-10-21 orchestration. `tools/hud_overlay/inventory.*`,
`skills.*`, new `skill_nav.js` and `glass_sheet.js` (both routed by `serve.py`), `tools/weapon_stats.py`, the weapon
`fun_stats_markup` forwarding in `OpenWillowInventory.*`, and the `-owinvshots` capture block in `OpenWillowMayaHUD.cpp`
(capture-only, approved by the hub).

- **Matched references.** New real-game frames (`local/realgame/d20/`, ignored) have the host's capture state: the six
  bounded recipes in the same slots (spawned by full definition), Maya at level 36 (set in memory; saves restored and
  hash-checked), two weapon slots, $0, keyboard glyphs, Phaselock 1/1 and 31 points. The host captures run with
  `-owlevel=36`, an empty gear manifest, `-owslots=2 -owmoney=0 -owerid=0`.
- **Cards (parsing of game data; `docs/verification/INVENTORY_CARD_STATS.md`).** `weapon_stats.py` now emits the Accuracy
  row where it was missing, the coloured text lines (flavour, weapon zoom, ammo per shot, bonus damage, elemental
  effectiveness), Burn Damage / sec. and Ignite Chance, the price, and the element from the resolved damage type when no
  elemental part sets it (the Torgue explosive badge). Numeric `stats.card` changes: Wild Rifle `fire_rate` 6.557 to 5.56
  (it also changes the host's firing interval; reported to lane E), Hellfire `fire_rate` 9.09 to 8.96 and `damage` 659.2
  to 621.6, Volcano `damage` 2625.6 to 2481.0, and accuracy, reload, magazine and spread corrections listed in the
  verification record. Real-card comparison on cooked data (what the host's recipes use; 69 recorded cards, 67
  supported): damage 63/67, accuracy, fire rate, projectiles and price 67/67, reload 62/67, magazine 55/67, status DPS
  16/16, status chance 15/16; of the six slice cards, the two shotguns still differ in reload and magazine (4/6). An
  earlier "all match" log used the `--live-data` overlay of values read from the running game (67/67 with it); the
  difference between the cooked and live values is open (DECISIONS 2026-10-02: not online hotfixes). **Not
  solved:** the zoom line is 0.1 high on two of six cards (raw values 3.264 and 2.471 show as 3.2 and 2.4; no rounding
  rule fits all six, UNVERIFIED); the Nukem price differs because the bounded recipe has a different name part.
- **Navigation (read from script where named; each case matches the matched real frames).** A backpack weapon is compared with the active slot's weapon
  (`StatusMenuEquippedPanelGFxObject.GetTypeCellSuffixBasedOnThing` -> `WillowInventoryManager.GetEquippedAnalogue`: the
  current weapon, then the active quick-select slot); Left from the Backpack returns to that analogue; each sort selects
  its first entry and an empty ITEMS sort selects an (EMPTY) cell. Skills: Up/Down move between tiers
  (`SkillTreeGFxObject.HandleInputKey` -> native `Navigate`); Down x4 from Phaselock reaches Harmony's Sustenance in the
  game and in the host. The column choice between tiers of different widths, ties and the edges are UNVERIFIED. The
  ALL list breaks equal-rarity ties by descending type key (fits ALL; the BRANDS frame orders the same pair the other
  way, so one of the two always differs; the sort is native, UNVERIFIED).
- **Fixes found on the way.** `serve.py` served only listed files, so `skill_nav.js` returned 404 (a test now checks every
  script a page loads is routed); rebuilding the Skills hit layer under a resting cursor selected the tile under it, so
  hover now selects only after real pointer movement; a stale overlay server from an earlier run kept serving old routes.
- **Stand-ins (UNVERIFIED):** the glass sheet is our own drawing (normal-alpha wash, scanlines, curved edges); the
  game's brightening blend cannot be reproduced by a page composited with alpha.
- **Critic** (a fresh independent agent per round, 14 matched pairs): round 0b 5.95, round 20 7.39, round 21 7.85,
  round 22 7.57. Round 22 and a fourth attempt regressed and were not kept; the pages are at round 21. 8/10 not reached.
- **Checks:** CTest 10/10, packages 9/9, inventory navigation 40/40, skill navigation 6/6, weapon stats 36/36 (all 46
  standalone Python/JS tests under `tests/` pass; the other 8 need the reader and run inside CTest); inventory suite
  49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE with `-Items local/inventory-actions/bounded-recipes`. The suite first
  failed step 20 (and skipped the seven steps after it) because a transfer started on the analogue even after a number key had
  chosen slot 2; a number-key choice for the selected item now wins (host control, the game's behaviour is not observed).

## 2026-10-22: Phaselock presentation rounds 17-20: capture FOV, camera pitch, stock dark interior, live mesh scale

AI-assisted (Claude orchestrating, Codex GPT-6 Luna implementing and critiquing). Host, tools and docs
(`OpenWillowPhaselockFx.*`, `OpenWillowCombatTarget.cpp`, `import_phaselock_fx.py`, `prepare_phaselock_target.py` and its
test, `run_phaselock_shots.ps1`, a few additive lines in the shared `OpenWillowWalker.cpp`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 17" to "Round 20".

- **Captures:** the `-owfov` value was converted from 4:3 to 16:9 a second time, so round 17's host frames were about 94
  degrees wide instead of 77.6; with the corrected input the bubble rim measures 420 px against the game's 422 px (no scale
  fitted). The capture camera is now pitched like the game's (`-owphaselockpitch`, set after target selection so the cast
  still hits).
- **0.80 s line:** not a host bug in kind. The game frame has the same cut where the ground passes through the huge
  `InitialFlash` sprites' soft-particle fade; the host's sat higher because its camera was level. The flash overbright is
  back to the 0.4 fit (round 16's 1.0 over-bleached the view; `UNVERIFIED`).
- **Bubble interior:** the bubble's `ModulateBlack` uses the shader's own values (cap 1, fade floor 1) instead of the round
  7-14 compensations, which had made it see-through.
- **Mesh scale:** the bullymong uses the live component scale 1.0 and the plain mesh bounds (307 / 245 / 200 uu against the
  game's 290 / 230-260 / 193), replacing 0.7 and the "bounds divided by scale" reading. The earlier frames favouring 0.7
  predate the FOV fix. Archetype (0.7) and live (1.0) still disagree; `UNVERIFIED`. A 0.9 multiplier fitted from a critic's
  estimate in round 19 was removed again.
- **Facing:** the test pawn's yaw offset is 0 (the held pawn faces the player, as in the game frames); the stock rule was
  not read (`UNVERIFIED`).
- **Visual review:** independent critic, frames only: 5.9 (round 17, three frames), 5.6, about 5.8, about 5.4 (six frames).
  The 8/10 target was not reached. Open: the held pose (folded, limbs high, against compact and head-forward), the 0.80 s
  shell that hides the target in the game, the bullymong's brown colour (no material graph found), the irregular cyan rim,
  the palm orb's cobalt core and strands. The eye motif low in the view is the HUD's action-skill indicator.
- **Checks:** CTest 10/10; `verify_packages` 9/9; Python tests 49 files and JS tests 2 files pass; quest suite 79/0 and
  11/0; Phaselock shots 28 frames (cast hits the bullymong). No flaky test seen.

## 2026-10-22: Weapon paint fallback fix and first-person parity rounds for four slice guns (lane E)

AI-assisted (Claude, with Codex sub-agents). `host/ue5/import_weapon_items.py`, `tools/weapon_item_materials.py`, `OpenWillowWalker.*`,
`OpenWillowShotFx.*`, `tools/weapon_view_model.py`, `tools/weapon_reload_anims.py`, `tools/weapon_capture_items.py`,
`host/ue5/import_weapon_reload_anims.py`, `host/ue5/import_weapon_fx.py`, `tools/real_game/scripts/weapon_parity_probe.py`.

- **White Wild Rifle (bounded item set):** the item importer gave every gun except the Infinity the neutral stand-in, even when a prepared
  per-item paint material existed. It now uses `/Game/OpenWillow/Weapons/Paint/<id>/M_OW_PaintApprox` when present and warns with the missing
  path otherwise (synthetic tests in `tests/weapon_paint_test.py`). The Inspect preview and the held weapon load the same mesh, so both are fixed;
  confirmed by a UE preview render, not yet by an Inspect capture.
- **Live probe first:** one SDK session read the four guns' type fields and captured idle, ADS, burst and reload frames (`weapon_parity_probe.py`,
  output under ignored `local/realgame/laneE/`). A live skill effect doubled fire rate and halved reload on held guns, so the host keeps card base
  values; base values read live equal the cards. Values used and their sources: `docs/verification/WEAPON_VISUALS.md` sections 11-15.
- **Host:** hold-to-aim on the right mouse (live zoom FOV and zoom time, eye-socket alignment fitted to the real frames), per-type weapon and arms
  reload clips chosen by the fast-reload threshold and scaled to the card reload time, a host-drawn flame flash at the cooked muzzle socket, brass
  casings, a procedural SMG scope, data-driven kick. Rule versus fit is labelled per item; every fit and the reload-rate rule are `UNVERIFIED`.
- **Visual review:** critic 5.8, 5.5, 6.3, 6.5/10 over four rounds; 8/10 not reached. Open items (SMG flash drawn at the rear, suspected Y mirror of
  `Root`-parented sockets; SMG hip size; reload poses; ADS widths; metal tint; sound) in `WEAPON_VISUALS.md` section 15.
- **Checks:** CTest 10/10; packages 9/9; Python tests 53/53; JS tests 2/2; quest suite 79/0 and resume 11/0; inventory suite 49 / 0 / 0 / 0
  (bounded item set).
- `ReloadCompletePercent` now drives the one-time magazine transfer and keeps the missing-field `1.0` fallback for the previous full-duration
  refill; the host check covered cancellations before and after that point.
- Successful Phaselock lifts cancel reload before the cast action, and arms cleanup only clears an active reload clip; the host check passed with a
  test-only slower cast rate so the clip spans the former reload end. Original-game notes confirm only successful-cast abort behavior.

## 2026-10-22: Sanctuary matched-viewpoint rounds 1-4: WiringMesh collision refresh, source lighting, decal default-deny, host calibration

AI-assisted (Claude), lane A. Details, counts and per-round history: `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`;
collision record: `COLLISION_WALKING_VERIFICATION.md` (2026-10-22).

- **Collision refresh (`tools/refresh_collision.py`).** Accepts `GearboxFramework.WiringMesh` exports as well as
  `StaticMesh` (`MESH_CLASSES`). Before this, the Sanctuary_P cable mesh raised "Ambiguous mesh" and the refreshed scene had no
  volumes. Synthetic test `test_refresh_accepts_wiring_mesh`. Still skips the one UModel-prepared skeletal pillar. **Verified:** the
  refresh printed supported 312, absent 115, unsupported 2, hulls 518, blocking volumes 5, non-blocking 6, live mismatches 0;
  the live capture matches the committed record (`DynamicBlockingVolume_1`/`_2` collide, `_0`/`_3` do not). The full UE import
  now spawns `OpenWillow_Volumes` itself.
- **Scene manifest (`tools/prepare_level.py`, `scene.json`).** Gains `lighting` (decoded SkyLight, HeightFog, WorldInfo and
  PostProcessVolume settings, and the directional light component after its `LightmassSettings` anchor), `decals` with
  default-deny exclusions and a reason per actor and material, and `diffuse_tint` (`diffuse_tint_v1`, `partial_unverified`) for 18
  Sanctuary building materials, read from `Master_World` instances. **Verified:** the decoded records and counts (33 kept, 78
  excluded, 182 unsupported; 46 decal-like mesh placements stay on the mesh path). **UNVERIFIED:** that the game multiplies Base Color by
  `p_diffmod` in the way the host does.
- **Schema (`tools/level-arrays.schema`).** Declares `StaticLightCollectionActor.LightComponents` as an object array. This is bulk
  decode metadata only; the resolved elements are point and spot light components, 1,023 of them, of which 128 are spawned.
  The schema is keyed by property name only, so it applies to every package that has such a property. **Declaration
  census (verified, all 914 base-game packages, 0 failures):** 129 packages carry the name; the only exports named
  `LightComponents` are `StaticLightCollectionActor.LightComponents`, a `Core.ArrayProperty`, and its inner
  `Core.ComponentProperty` (an object-reference property). A tagged property's type comes from its declaring class, so every
  serialized `LightComponents` is that array of object references. The same declaration census over the 1,096 DLC `.upk` files
  (the `--include-dlc` scope) finds no declaration of that name (151 files carry it as instances; 2 texture payload files
  with no package magic cannot be opened). An instance decode pass (stopped at a 30-minute budget after
  666 packages) agreed: 119 of 119 occurrences decoded as `ObjectProperty` arrays owned by `StaticLightCollectionActor`.
  Scratch scripts under `local/laneA-census/` (ignored).
- **Light array tail (`UNVERIFIED` layout).** The 64-byte-per-element tail after a `StaticLightCollectionActor`'s properties is
  size-checked and skipped, not decoded. Its per-component layout (presumed 4x4 transform, as for `StaticMeshCollectionActor`)
  is `UNVERIFIED`. Placement uses each component's decoded `CachedParentToWorld`. Reader coverage: `tests/properties_test.py`
  (a real-reader N = 2 `ObjectProperty` array with a 128-byte tail, and refusals when the declared size disagrees).
- **Malformed light arrays contribute nothing.** A `LightComponents` array that fails any check now stages its records and adds
  them only after the whole array validates (`tools/prepare_level.py`, `static_light_collection_records`). Synthetic test:
  `test_malformed_light_array_contributes_no_lights`.
- **Lighting is Sanctuary-only.** `scene.json` gets `lighting` only when the persistent map is `Sanctuary_P`
  (`sanctuary_lighting_enabled`); other maps have no `lighting` key. Decals stay generic and default-deny: a non-allowlisted
  material is excluded for any map, so a non-Sanctuary map places no decals. Synthetic test:
  `test_sanctuary_lighting_policy_is_limited_to_sanctuary_persistent_map`.
- **Host opt-in (documented, implemented by the other lane-A agent).** `OPENWILLOW_SANCTUARY_OPT_IN` lists `grade`,
  `diffuse_tint`, `xmas_bulbs`, `static_lights`, `graffiti_decals`; all default OFF, Sanctuary_P only, and the 0.14 cloud cap
  stays on. Source inputs are still recorded in `scene.json`.
- **Test values replaced.** The visual-rules test uses invented colours and intensities (the 1.25 directional brightness,
  50/100/150 sky colour, post-process 4.0/1.3-1.5/0.05-0.07/2.5) and checks calibration formulas against the host constants,
  not literal calibrated outputs. The grafitto ink check is structural, and a far plane is 900.
- **Colour conversion.** UE3 FColor values are converted to linear with a 2.2 power curve, following the UE3 FLinearColor
  conversion as we understand it. **UNVERIFIED** against the game's output.
- **Host calibration (`host/ue5/scene_import_rules.py`).** The sun's 5.0 lux per source brightness unit, sky light 1.5, sky tint
  (3.0, 2.1, 1.0), exposure bias -1.5 EV, white balance 8200 K, saturation, contrast, shadow lift, the cloud-cap 0.14, the 25 cd
  per brightness unit with the 3.0 ceiling, the 0.01 orange-point scale, and the graffiti ink and 128 cm depth cap are
  **screenshot calibrations tuned against the matched cameras. They are not decoded from the game** and are `UNVERIFIED`.
- **Verifier crash fix (`host/ue5/verify_level.py`, decal check loop).** A module-level bound method of a UE actor
  (`actor.get_actor_right_vector`) was kept alive to interpreter shutdown. After a successful run the UE Python plugin crashed
  in `python311.dll` (access violation), so `-ImportOnly` reported `Saved scene verification failed: 3` although the assertions
  passed. The loop now reads the axis vectors as values; the full gate exits 0. The root cause inside the plugin's teardown is
  not analysed.
- **Visual review:** independent critic (Codex GPT-6-Luna, host and reference paths only, eight matched cameras). Baseline before
  round 1 4.5/10; round 1 4.7; round 2 4.1 (decal noise regression); round 3 4.3; round 4 4.4; after review 1, the default state (round-4 guesses opt-in and off) 5.0. **The 8/10 gate was not reached;
  the lane stopped after four rounds as briefed.** Remaining gaps are listed in the gaps document, round-4 ranking.
- **Checks:** CTest 10/10; packages 9/9; Python tests 42 pass (the 8 reader-argument suites run inside CTest); JS 9 and 32/32;
  UE import gate exit 0 after the crash fix; OpenWillow.Walking and BspWalking pass; TerrainWalking 23/24 (the known
  pre-existing Terrain_2 hole endpoint). Not run: RouteWalking/CollisionShapes (no route traces on this machine), live-game
  gameplay.

## 2026-10-22: Gameplay HUD parity: the movie by default, true perspective for the turned clusters, real values

AI-assisted (Claude Opus 5.5 orchestrating, Opus and Haiku sub-agents; critic GPT-6-Luna). Record:
`docs/verification/HUD_PARITY.md`. No parser behaviour changed.

- **Default HUD.** BL2's HUD movie (Ruffle overlay) is the gameplay HUD whenever its page reports ready; the host opens
  `http://127.0.0.1:8767/` without `-owflashhud`. The canvas HUD is a fallback (not ready, load error, `-owcanvashud`)
  and no longer draws the temporary Phaselock ring and orb. The movie counts only when the page's ready line echoes the fresh
  id the host put in its URL (`?owid=`); an older page from another checkout's server gives the canvas. The quest, mover, Phaselock-shots and character-capture runs pass `-owcanvashud` so their outcome does not
  depend on a server being up.
- **Perspective.** `tools/hud_overlay/hud_warp.js` draws the turned clusters through a per-clip homography from the
  fitted projection (F 914 derived from the definition's View3DFOV 70, read live; scale 1.095 fitted, UNVERIFIED), reading
  Ruffle's WebGL canvas (forced `preserveDrawingBuffer`) at 2x into an overlay canvas. The 2D stand-in (`hud3d.js`) stays
  for the minimap and as the fallback. Residuals against a real frame: 1-4 px, side clusters slightly nearer the centre.
- **Values.** Health and its rounded text, the equipped shield's card capacity (bar full: no shield model), magazine /
  reserve and grenade current / max with the game's no-break spaces, zero-padded level. Crosshair style, idle spread only (no
  widening while firing: not done) and ammo icon per weapon type from a live read (pistol 3/9/3, shotgun 7/17/5); other types UNVERIFIED and left alone.
  Ruffle hands non-ASCII ExternalInterface strings to AVM1 as UTF-8 bytes, so such text is set through `htmlText`
  entities (bench-verified).
- **Checks.** `tests/hud_warp_test.js`, `tests/hud_fields_test.js`, `tests/check_hud_shots_test.py` (36 synthetic cases); in-engine
  `tools/run_hud_shots.ps1` 17/17, `tools/test_action_skill_hud.ps1` 19/19, `tools/test_inventory_actions.ps1` 49/49
  (bounded recipe set). Critic 8.0, then 7.7 measured (crosshair spread per gun and 1-4 px side placement remain); blind
  A/B warp 8 vs stand-in 6, warp preferred in 6 of 6.

## 2026-10-22: The HUD minimap drives BL2's own minimap clip from rules read in the live game

The maintainer asked for the HUD's map. The minimap's driver (`HUDWidget_Minimap.Init/Update/SetMapMovie`) is native,
so its rules were read from the running game through the mod SDK (AI-assisted; record
`docs/verification/HUD_MINIMAP.md`; no native code was disassembled):
- each level has a tactical map movie (`WillowMapInfo.TacticalMapMovie`; Sanctuary: one bitmap and two fog blobs) that
  the HUD movie's own `MinimapController.setMapMovie` loads into `mapPlaceholder`; the map's origin is the
  `WillowTacticalMapVolume`'s bounds centre, 128 uu per map unit, shown at 102.4 %;
- the map turns by -yaw (view yaw) about the player; one minimap unit is WorldRadius / 48 = 125 uu; station icons hide
  outside a 58 x 48 box, mission and objective icons are pulled onto its edge; the objective shrinks beyond 6000 uu as
  140 x (1 - (d - 6000) / 18000) (fitted, seen to 9000 uu); the "N" marker sits on the box edge;
- which world objects get icons follows their interactive-object definition's `CompassIcon`, reached for vending
  machines through their population opportunity point's definition chain.
`tools/prepare_minimap.py` converts the map movie and writes the icon manifest (11 of the 12 live icon objects found
in the packages; one health machine is spawned another way), `tools/hud_overlay/minimap.js` maps the state to the
clips (exact against 652 live values; the "N" within one twip), `OpenWillowMinimap.cpp` sends the state with the
quest's markers (`UOpenWillowQuest::MinimapMarkers`; the marker's actor for this mission is UNVERIFIED). The panel's
3D turn is drawn by PR #54's perspective warp over the panel's own rectangle (the clip's visible children include the
movie's parked library icons); without the warp a least-squares affine stand-in in `minimap.js` replaces `hud3d.js`'s,
which sized it from bounds inflated by those templates. Checked after rebasing onto #54: node 16, Python 19 + 15 + 3,
in-engine 15/15, action-skill suite 19/19, HUD shots 16/16, CTest 10/10, packages 9/9; critic 7.5, 8.0 (affine), 8.2/10
(warp). Not done: threats, allies,
vehicles, fog state other than "not uncovered", the absolute-minimap option, the full-screen map. Review 1: the
capture script and the prep tool refuse output paths outside the ignored folders (game data cannot be written into
tracked folders), and the in-engine checker requires each capture spot exactly once.

## 2026-10-23: Weapon parts catalog and the loot generator in C++ (census, native gaps, exact port, live checks)

AI-assisted (Claude Opus 5.5 orchestrating; Opus and Haiku sub-agents; review GPT-6-Luna). Record:
`docs/verification/WEAPON_PARTS_CATALOG.md`; rules in `docs/verification/NATIVE_WEAPON_RULES.md` section 12. No package
parsing behaviour changed (`src/package.cpp` and `src/container.*` untouched).

- **Census.** `tools/weapon_catalog.py` over every installed package (2,010 files, 315 with weapon data) and
  `tools/real_game/scripts/weapon_parts_census.py` in the running game: 369 weapon balances, 908 parts, 89 weapon types, 22
  manufacturers, 876 name parts, 2,014 item pools, packages and OpenBLCMM agreeing except 7 remaster gestalts and one
  pool path. Catalog under ignored `local/weapons/catalog/`; counts only in the record.
- **Rules read from native code** (UNVERIFIED in game unless stated): the value function (type base modifier, part and
  name-part product, truncation, currency cap), the slot value ((grade - 1) * per-grade, max then min clamp), the
  part-weight hook (relics only), the grade choice and stage cap (opt-in `stage_cap`), the status card chance through the
  damage type's presentation remap (slag prints 0.1 higher), and name-part expressions (all must hold;
  `WeaponDamage_Is_<Element>` checks the weapon's damage type, confirmed live on one gun). REA's `open_binary` could not
  use the existing analysis database and its analysis calls failed, so the reading used `tools/ghidra/run.ps1`.
- **Generator.** `weapon_recipe.roll()` now defaults to `CrtRand` (the MSVC `rand()`), our seeding; the slice tools pin
  Python's generator so pre-rolled slice guns are unchanged. `tools/weapon_gen_manifest.py` writes a line-based manifest
  (base game `Startup.upk` only: 249 balances; DLC not yet); `src/weapon_gen.{hpp,cpp}` (in `ow-core`) and `ow-weapongen`
  reproduce `roll()` + `evaluate()`: 2,919,834 fields over 29,880 balance rolls, 7,680 pool rolls and 6,795 pool guns,
  **0 mismatches** (floats bit for bit), uncapped and with a stage cap of 50.
- **Host.** `OpenWillowWeaponGen.{h,cpp}`: manifest load (missing = off), conversion to `FOpenWillowWeaponItem`, new
  shared item fields `Parts` (slot order), `Material`, `GestaltMesh` for the weapon-assembly lane, `ow.RollGun`, and the
  quest turn-in reward rolled from the stand-in's own source pool (fallback: the pre-rolled stand-in).
- **Live** (real game, saves blocked and unchanged): 40/40 spawned generated guns carry the generated balance, parts,
  level, prefix, title (after the expression rule) and value; part frequencies of two common balances consistent with the
  weights (2 x 400 rolls per session, two sessions); level = stage and the name rule on 1,600 of 1,600 game rolls. Read
  stats of 44 spawned guns: 366 of 484 values equal to the generator bit for bit on cooked data, 26 explained by runtime
  data, **92 not triaged** (some look like read errors); no card captured. The Dahl rifle 9.5 % above its card (Weapons 3)
  is runtime data: the live overlay reproduces it exactly.
- **Checks.** CTest 11/11 (new `weapon-gen-synthetic`: hand-worked synthetic manifest plus parser robustness);
  `verify_packages.py` all match; Python unit tests (weapon_recipe 20, weapon_stats 42, weapon_gen 13, loot_pools,
  weapon_catalog 18, gen_confirm 15); golden cards 69 / 69 / 68 of 69, 0 unsupported (was 67 / 67 / 66 with 2 unsupported:
  an empty card presentation entry made the evaluator raise). In engine: host against `ow-weapongen` 245 fields, 0
  differences; `tools/test_inventory_actions.ps1` 49/49; the quest self-test stops before turn-in on this machine (Marcus
  not imported), so pickup of a generated reward is not checked.
- **Sensitive file:** `CMakeLists.txt` (adds `src/weapon_gen.cpp` to `ow-core`, the `ow-weapongen` executable and the
  `weapon-gen-synthetic` test; verified by the build and CTest).

## 2026-10-23: Weapon part library: every gestalt mesh and skin in UE once, guns assembled from their parts at run time

AI-assisted (Claude Opus 5.5 orchestrating; Haiku sub-agents for the census, skin export, sample and tests; critic GPT-6-Luna).
Record: `docs/verification/WEAPON_PART_ASSETS.md`. No change to `src/`, `CMakeLists.txt` or package parsing.

- **Census.** `tools/weapon_part_census.py` reads every weapon part, weapon type and gestalt in the base and DLC packages
  (904 parts, 301 skins, 6 weapon gestalts with 279 fragments; every fragment is referenced). A live SDK census of the running
  game (`tools/real_game/scripts/weapon_visual_census.py`) agrees: the six fragment tables are identical, and the 814 parts
  loaded live have the same fragments and skin. 22 Torgue Material parts carry a `p_EmissiveColor` only in the running game (the
  documented live-versus-cooked data difference); the library takes the live value.
- **Extraction.** UModel 1590 exports all 301 skin MICs with their parent chains and textures (`tools/export_weapon_skins.py`,
  301 of 301 resolvable, inventoried; one root per skin because leaf names repeat across packages).
- **Library.** Each gestalt glTF is regrouped into one primitive (UE material slot) per fragment and imported once
  (`tools/weapon_library.py`, `host/ue5/import_weapon_library.py`); the paint reading of `import_weapon_paint.py` became one
  parameterised master material plus one instance per skin (300 of 301 imported; the BuzzAxe melee material is unsupported).
  The per-recipe meshes and materials stay as the fallback.
- **Assembly.** `OpenWillowWeaponAssembly` shows exactly the fragments a gun's parts draw on its type's library mesh, gives every
  slot one instance of the Material part's skin with the parts' vector parameters, and hides the type's bones. Used by the held
  gun, inventory preview, Maya's menu weapon and pickups. Item fields added: `Parts`, `WeaponType`, `Gestalt`, `Skin` (read from
  the recipe the generator already writes). `-owweaponprebuilt` forces the old path; an explicit `MeshPath` keeps it.
- **Confirmed in game (39 spawned guns, one per type x manufacturer plus 7 legendaries):** fragment names and triangle totals
  equal the host's for 39 of 39 (and the cooked prediction for all 210 sample guns), and each gun's material instance has the
  Material part's skin as parent and the parts' vectors (39 of 39), confirming the native BuildWeaponMaterial reading for them.
- **Visual (critic, 39 matched guns):** Inspect 7.32/10, first person 5.36/10 (round 1: 8.25 on six guns / 4.26). 9/10 not
  reached; the gaps are the shared first-person view model (sniper and launcher arm sets not imported) and the paint calibration
  (HDR zone colours clipped per channel under a stand-in light level), not the library. UNVERIFIED: the paint reading itself, the
  material on the pistol gestalt's second section, elemental first-person particles (not drawn).
- **Checks:** CTest 10/10, packages 9/9, Python 157 synthetic tests OK, inventory suite 49/49 (bounded recipe set) with assembled
  guns.

## 2026-10-23: Weapon part library, PR #58 review round 1

AI-assisted (Claude Opus 5.5; review GPT-6-Luna). Record: `docs/verification/WEAPON_PART_ASSETS.md` section 6b.

- **Part list:** the item uses PR #57's shape (ordered `{Slot, Part}` array with `FindPart`, `Material`, `GestaltMesh`) plus
  `WeaponType`; the assembler reads only these, so generated guns assemble once #57 lands (rebase pending, by the hub's order).
- **No partial guns:** a gun with no part list, an unknown part, skin or section material, or a fragment missing from the library
  mesh uses the pre-built mesh, and the reason is logged. Pure planning step with a synthetic automation test
  (`OpenWillow.WeaponAssembly`, `-game` mode).
- **Per-slot material, confirmed in game:** the pistol gestalt's second section shows the mesh's own `Mati_DahlUncommon` on every
  pistol read live (4 guns, Infinity included); the host now does the same for section-1 fragments.
- **Preview framing** from the shown fragments' bounds (offline from the glTF; host check against the imported bounds caught a
  mirrored axis and is now quiet). Reload clips are re-imported after a meshes import. Material instances are cached; the menu
  preload loads library meshes and skins (39 items, 46 assets, 424.5 ms); first show 50 ms, later 0.2-0.9 ms.
- **Oracle:** exact fragment-name sets plus triangles, explicit `fallback` / `not_shown` / `no_reference`, nonzero exit on any
  non-pass. 210/210 sample guns pass; 39/39 against the running game.
- **Checks:** CTest 10/10, packages 9/9, Python 164 synthetic tests OK, `OpenWillow.WeaponAssembly` and `OpenWillow.Skills` pass,
  in-engine gun shots (full sequence on the slice guns, quick on 210), inventory suite 49/49 (bounded recipe set).

## 2026-10-23: Weapon part library, PR #58 review round 2

AI-assisted (Claude Opus 5.5; review GPT-6-Luna). Record: `docs/verification/WEAPON_PART_ASSETS.md` section 6c.

- Material instances are per component (a held gun's glow no longer reaches a pickup of the same gun); a gun needs Body, Grip,
  Barrel and Material parts and a known weapon type; unmapped or unloadable section materials fall back instead of taking the
  gun's skin; the oracle fails an empty or short index; the reload record is repository-relative; `-owgunnolook` clears old
  overrides.
- Checks: `OpenWillow.WeaponAssembly` and `OpenWillow.Skills` pass; gun shots with oracle 39/39 (`--expect 39`), no fallback;
  CTest 10/10, packages 9/9.
- After rebasing onto PR #57: eight generated guns (`ow.RollGun` from pools, every weapon type) assemble with no fallback and match
  the generator's parts exactly (fragments and triangles); inventory suite 49/49; CTest 12/12, packages 9/9.

## 2026-10-23: What a shot does comes from the gun's parts: firing modes, light bullets, rockets, splash, status effects (Weapons 3)

AI-assisted (Claude, with Claude and Codex sub-agents). Record and numbers: `docs/verification/WEAPON_PROJECTILES.md`;
native notes: `docs/verification/NATIVE_WEAPON_FIRING.md` (new, own words, raw output under ignored `local/analysis/projectiles/`).

- **What.** `tools/weapon_firing.py` adds a `firing` block to any evaluated recipe, resolved from its parts (barrel or type
  firing mode, `FireType`, pattern lines, projectile definition and behaviours, the explode behaviour's splash with the
  damage type's falloff bands, the status effect, the Tediore reload throw). The host reads that block
  (`FOpenWillowWeaponItem::Firing`, one new field) and fires accordingly: `src/weapon_firing.*` (rules as maths, CTest
  `weapon-firing-math`), `OpenWillowFiring`, `OpenWillowProjectile`, `OpenWillowStatusEffect`, and a smaller
  `OpenWillowWalker::FireWeapon` (one direction per pellet; trace, light bullet or rocket). No per-gun code.
- **Behaviour change.** Bullets now take time to arrive (light bullets, as in the game: damage when they reach the
  target); the card spread and pattern place the pellets (the old fixed figure-eight is gone; the Infinity fires along
  its own pattern lines); `AutomaticBurstCount` 1 guns (slice pistol and rifle) fire once per press; only
  `BSM_SpinUpToFullFireRate` ramps the fire interval (the former wait for the spin-up on other modes is gone). The
  quest suite's two shot checks now wait for the hit.
- **Confirmed in game** (three bl-sdk sessions, skills off, saves byte-identical): light bullets fly straight at their
  speed; `AirFriction` grows speed x(1 - F dt) per frame; rocket speed = `SpeedFormula` x projectile speed multiplier
  (the tool's first version left the multiplier out: fixed); weapon rockets do not fall, thrown guns fall at
  GravityScaling x 1,000; per-pellet damage = card on 4 of 5 guns; rocket and gyrojet splash has no falloff, the Tediore
  throw's follows the damage type's bands; the DOT clock (0.33 s, DPS x built-up time, final partial tick); the Tediore
  throw (card damage x rounds left, half direct, half splash).
- **UNVERIFIED / open.** The spread distribution (no impact points could be logged); the status proc rate (game 2-3 x the
  card chance on Maliwan pistols; the host uses the card chance); elemental impact explosions of elemental and Torgue
  bullets, element-vs-surface factors, slag, beams, ricochets, homing and kick are not modelled; host shot effects are
  additive stand-ins, not BL2's particle systems: matched-frame critic (Codex) 2.1, 3.9, 4.2 / 10 over three rounds,
  stalled; the rest needs the stock particle templates.
- **For other lanes.** The Dahl AR's card damage is 9.5 % below the live gun with the same parts (generation lane). A
  small `AOpenWillowShotFx::Spark` and an `Intensity` argument on `Flash` were added for impacts and explosions.
- **Checks.** See the PR: CTest, packages, Python tests, in-engine firing test, quest and action-skill HUD suites.

## 2026-11-02: merge of main into `t3/ai-melee-and-bleedout` (host firing, muzzle, turn-in loot)

AI-assisted (Claude). One merge commit; where both lines of work implemented the same host behaviour, main's implementation
is kept and the branch's callers adapted. Nothing here was re-checked in the running game. The paragraph as first written said
the in-engine suites were not run for this merge; that was before the hub ran them. Recorded afterwards (PR #59 body):
`tools/test_quest.ps1` at 5be3581 (the merge) and again at a8a3dd5 (after review fixes 1), each first run PASS 111/111 and
resume PASS 11/11 (host behaviour only, not parity with the original game); CTest 19/19 and `verify_packages` 9/9 at 5be3581,
with the UE5 module compiling. `tools/test_weapon_firing.ps1` was not run (no generated firing recipes in that worktree).
683a083 merges a8a3dd5 into an unmoved main, so it is the same tree.

- **Gun firing.** The branch's in-walker bullets (lane Hf, 2026-10-08: card `firing` block from `tools/weapon_refresh_firing.py`,
  `FOpenWillowFiringMode`, `AdvanceBullets`, `FShotRecord`) are replaced by main's firing from the parts (`OpenWillowFiring`,
  `OpenWillowProjectile`, `src/weapon_firing.*`, recipe-level `firing` from `tools/weapon_firing.py`). Kept from the branch: the
  level-up weapon damage scale (`ShotDamage`, script swap 9) now sets main's per-pellet damage for light bullets, rockets and
  traces. Not carried over: the per-bullet momentum (type `InstantHitMomentum` x 100; main passes none) and the branch's
  `TraceExtent` box sweep.
- **Quest suite, shot cases 14/15.** They keep the branch's structure (fire, wait for the landing, multi-pellet shotgun
  fixture) and now read main's tally (`OpenWillowFiring::Counts`) and the live `AOpenWillowProjectile` count. Re-expressed:
  `bullet_has_not_landed_in_the_pressing_frame` (trace guns exempt), `wrong_element_shot_...`, `multi_pellet_gun_fires_its_pellet_count_in_one_press`,
  `damage_called_once_per_pellet_that_hits`; `ammo_taken_once_per_press_not_per_pellet` unchanged. Dropped because they tested
  the replaced implementation's internals: `pellet_directions_inside_pattern_and_cone_bounds`, `shot_momentum_is_type_momentum_times_100`,
  `*_bullet_lands_after_its_flight_time`, `firing_pattern_lines_sequential_scaled_random_rules` (main's pattern rules and the flight
  step (speed, gravity, own-motion friction) are unit-tested by CTest `weapon-firing-math`; no in-engine check now compares a
  projectile's impact delay with distance and speed, so runtime flight timing is OPEN).
- **Muzzle point.** Main's view-model `MuzzleSocket` (`weapon_view.json`) first, then the branch's recipe `sockets` Muzzle
  (`GunSocketLocation`, WEAPON_VISUALS.md section 17, renumbered from 11), then main's camera-space fallback (logged once per gun).
  The base's Barrel + 27 cm estimate, which main had dropped, stays dropped. Whether the recipe socket's bone frame holds on
  main's assembled library meshes is UNVERIFIED.
- **Turn-in loot.** The branch's maintainer decision (2026-10-07: no item reward) wins over main's generator-rolled reward:
  the quest rolls and drops nothing; the generator itself and `-owweapongentest` are unchanged.
- **Lent weapon.** The branch's rule (grant to the backpack, not equipped; kept after the objective) is kept; main's
  `PreLendSlot` is dropped with it.
- **Notes.** The branch's `NATIVE_WEAPON_FIRING.md` (lane G13) is now `NATIVE_WEAPON_TRIGGER_AMMO.md`; main's note keeps the
  name. Links in NATIVE_INDEX, NATIVE_ACTION_SKILL, NATIVE_PLAYER_MOVEMENT and PROJECTILE_FIRING_PATTERNS_LIVE point to the new name.

## 2026-11-03: Regeneration gaps: one schema owner, the whole mission dialog group in the audio manifest, font library placed by the converter

AI-assisted (Claude Opus 5.5). Found by a from-scratch data regeneration on a second PC (main 683a083). Tool-side only; no
host code and no reader parsing changed.

- **Duplicate array-schema key.** `tools/weapon_recipe.py` gained `ValueResolverChain=ObjectProperty` in e767921;
  `tools/slice_values.py` and `tools/loot_pools.py` already declared it and append weapon_recipe's list, so ow-package
  rejected their schema ("duplicate array schema key") and `tools/prepare_slice_world.py` stopped. weapon_recipe owns the
  line (its own evaluator reads the chain); the two copies are removed, and `slice_values.SCHEMA_LINES` now starts from
  `weapon_recipe.SCHEMA_LINES` like the other tools. New CTest `tool-schema-lines` (`tests/schema_lines_test.py`) applies the
  reader's rule to every tool's schema, including the one `slice_values.build` writes at run time; it fails on the old files.
  `prepare_slice_world.py` without the regeneration agent's shim: 21/21 oracles.
- **Audio manifest covers the mission's dialog group, not one run.** The quest suite's `dialog_hook_finds_every_line_and_plays_nothing`
  failed (110/111) because `tools/audio_slice_chain.py` listed only the lines one fixed `--mission-run` reaches (01, 02, 03b
  at 95e48fb), while the host reaches 03a, 04 and 05 on some runs. The check is right (a line the host plays must resolve);
  the tool was too narrow. It now resolves every `DialogEvents` entry of the mission's `MissionDialogGroup` plus any group a
  run names: 35 events, all with an AkEvent, 47 manifest entries, each dialog entry marked `reached_in_scripted_run`. The
  group is shared by the Rock, Paper, Genocide series, so lines of the sibling missions are included (a superset by design).
  The null-`OutputAction` pairing rule stays UNVERIFIED as the engine's rule; the host's independently chosen AkEvent agreed
  with the manifest on every line it reached (`dialog_lines_name_the_manifest_ak_events`). New CTest
  `audio-slice-chain-synthetic`. The tool also makes the reader path absolute (a relative forward-slash path is not found by
  Windows `CreateProcess`, the first failure in the regeneration log).
- **Font library next to each movie.** `gfx_to_swf.py --inline-font-imports` resolves plain `gfxfontlib.swf` imports next to
  `--output`; UI_HUD and UI_StatusMenu use that form as well as `../gfxfontlib.swf`, so without a copy in the movie folder those
  imports were kept and their text fell back to other fonts. The converter now copies the library from the run root when it
  is missing beside the movie (never overwriting) and lists it in the report. Checked on the installed HUD movie: 5 of 5 font
  imports inlined, 2 of them only with the copy. `prepare_inventory_movie.py` stays a separate, documented step (it is a Ruffle
  workaround for one library, not part of conversion).
- **Review round 1 (GPT-6-Luna).** The library copy now happens only for an import named `gfxfontlib.swf` and only to a
  destination inside the movie's output folder; a `..` URL that leaves it is skipped with a warning (listed under
  `skipped_font_library_copies`). Manifest keys stay `dialog:<event tag>` (the host lookup is unchanged), so the tool now
  refuses to write a manifest when one tag occurs in two included groups, and `reached_in_scripted_run` is matched per
  (group, tag). The installed data has no shared tag (35 tags, one group). Both have synthetic tests.
- **Docs.** `docs/TOOLING.md` "Regenerating local data on a fresh machine" (order, UE-step ordering, the `MSYS2_ARG_CONV_EXCL=/Game`
  note), the UI conversion steps, and `tools/worktree-assets.md` (what the seed leaves out and how to fill it).
- **Checks.** CTest 21/21, `verify_packages` 9/9, UE module build; `tools/test_quest.ps1` run twice: first run 111/111 and
  resume 11/11 both times, 9 dialog lookups and 0 misses each. Both runs reached the same six mission lines (01, 02, 03b, 03a,
  04, 05), so the run-to-run variation reported by the regeneration was not reproduced here; the manifest no longer depends on it.
  Host behaviour only; nothing here was compared with the running game.

## 2026-11-03: per-bullet momentum and wide-bullet sweeps restored; status chance level scale; firing suite runs again (Weapons)

AI-assisted (Claude). Record and numbers: `docs/verification/WEAPON_PROJECTILES.md` sections 2-5.

- **Maintainer decision (2026-11-02).** Re-add per-bullet momentum and the wide-bullet hit sweep on top of main's
  part-driven firing (#56), dropped in the merge of the earlier branch.
- **What.** `tools/weapon_firing.py` writes `instant_hit_momentum` (weapon type, class default 10 as fallback), the splash
  `momentum` (`MomentumFormula`) and `status.level_chance_scales` (`GD_Globals.General.Globals.StatusEffectChanceScales`).
  The host multiplies the momentum by 100 (`weapon_firing::hitMomentum`), passes it with every direct, trace and splash hit
  (splash x the radius scale) and changes the dummy's velocity by momentum / `Mass` (`Default__Pawn` 100,
  `weapon_firing::momentumVelocity`); the dummy slides, slowed by `GroundFriction` 8 as v x (1 - 8 dt) per frame (a host
  stand-in). A firing mode's `TraceExtent` turns light-bullet ticks and traces into box sweeps (`OpenWillowFiring::SweepShot`).
  The status roll multiplies the card chance by the level-difference row (`weapon_firing::statusLevelScale`: Maya's level
  against the target's; `-owtargetlevel=N` sets the dummy's, without it the factor is 1).
- **Retired.** `tools/weapon_refresh_firing.py` (the L7 live-dump firing block), its test and its `add_test` line in
  `CMakeLists.txt`; the cooked firing block replaces it. `tools/weapon_firing_coverage.py` regenerates the firing test's
  coverage recipes and firing blocks on a fresh machine.
- **Firing test.** New `OWFIRING push` (momentum rule, every hit's velocity change, the dummy moved), `OWFIRING wide` (a pull
  beside the dummy misses as a line and hits with a host-chosen 24 uu box; single straight non-burst bullets only, its two
  pulls counted in the shots) and the restored flight-timing check (`timed` / `on_time`). `tools/check_weapon_firing.py`
  grades them, including each splash hit's momentum against the recipe x radius scale; the new lines are required unless
  `--legacy` is passed. A dead target stops sliding; a respawned one starts at rest.
- **Verified / UNVERIFIED.** The momentum values match live reads (pistol 8, shotgun 7, Torgue 10, rocket splash 10,000).
  The x100 factor is read from native code, the effect on a pawn and the friction are not measured: UNVERIFIED. No local
  recipe has a `TraceExtent`, so the sweep is checked with a host box only. The level scale is consistent with the live
  Maliwan counts (about 2 x the card chance at Maya 54 against a level-7 bullymong); a same-level live check is still to do:
  UNVERIFIED. All of it is host behaviour checked in the host, not parity with the running game.

## 2026-11-03: One Unreal run lock per machine

AI-assisted (Claude Opus 5.5). The run lock was `local/ue_run.lock` inside each checkout, so suites started from different
worktrees on one PC launched editors at the same time. `tools/ue_lock.ps1` now names one file per machine,
`%LOCALAPPDATA%\OpenWillow\ue_run.lock` (`OPENWILLOW_UE_LOCK` overrides it), and all 20 scripts that took the old lock
(suites, seed/import scripts, shot runners, `tools/real_game/realgame.ps1`) get the path from it. The take/release logic
and the wait while any `UnrealEditor` runs are unchanged; the optional `-SharedLock`/`-HubLock` parameters still work.
Branches older than this change still use the per-checkout file until rebased. Verification records that cite
`local/ue_run.lock` describe past runs and are left as written. Checks: CTest `ue-lock-machine-wide` (no script builds its
own lock path; the helper's default and override); live: with the lock held, `test_quest.ps1 -WaitForEditorSeconds 10`
gave up without touching it; a full `test_quest.ps1` passed 111/111 and 11/11 and left no lock behind.

Review round 1 (GPT-6-Luna) changed the rule set; `tools/ue_lock.ps1` is now a dot-sourced helper used by every caller:
- **Owner and staleness.** The lock file records the owner's PowerShell process id, its start time and the script name.
  A lock is stale only when that exact process is gone. The former rule in `test_inventory_actions.ps1` (older than 15
  minutes with no `UnrealEditor` or game process) could remove the lock of a long commandlet, since it ignored
  `UnrealEditor-Cmd`; it is gone. `Exit-OwUeLock` removes only the caller's own lock. Old-format locks are never removed
  automatically.
- **Every launch holds it.** Seven launchers started the editor or a commandlet without the lock (`run_ue_level`,
  `run_ue_character`, `run_ue_flash_hud`, `run_ue_probe`, `import_maya_combat_anims`, `seed_inventory_assets`,
  `test_ue_viewer`); each launch now runs under it until the process exits. Interactive launchers now wait for the editor
  window to close (behaviour change: the PowerShell call returns only then).
- **Lock then `try` at once.** `test_quest.ps1` and `run_quest.ps1` ran setup between taking the lock and entering
  `try/finally`, so a throw there stranded the lock; every caller now enters `try` on the next line.
- **Absolute override only.** A relative `OPENWILLOW_UE_LOCK` is rejected.
- **Own game only.** `tools/real_game/walking_capture.ps1` stopped the first `Borderlands2` process it found, which could
  be one the user started. It now keeps the process it launched and stops only that one, after checking its id, executable
  path, start time and command line (inline; the shared helper for this belongs to another open PR).
- Fail-fast scripts (`import_weapon_library`, `import_phaselock_target`, `seed_slice_*`, `run_weapon_assembly_shots`) keep
  failing at once when the lock is held (`-TimeoutSeconds 0`). `test_ambient` and `seed_ambient_npc_assets` still also wait
  for `Borderlands2` (`-AlsoWaitFor`).

Review round 2 (GPT-6-Luna):
- **Atomic stale recovery.** Two waiters could both judge the same dead owner stale; the second then deleted the lock the
  first had just created, and both ran Unreal. The staleness check, the removal and `CreateNew` (and the owner check in
  `Exit-OwUeLock`) now run under a machine-wide named mutex, and the owner is re-read inside it. Test: three PowerShell
  processes recover one stale lock at once, each pausing 1.5 s between its verdict and acting on it; exactly one owns
  it and two are refused (3 of 3 runs). With the mutex replaced by a pass-through the same test fails (all three remove
  the lock).
- **No test switch in the helper.** The process-name override variable is gone; the tests redefine
  `Get-OwUeBusyProcess` in their own PowerShell process.
- **Re-check after taking the lock.** If an Unreal process appeared in the meantime, the lock is given back and the wait
  continues.
- **Real game and Unreal never overlap.** `Enter-RunLock` (real-game helpers) and `run_phaselock_shots.ps1` also wait for
  `Borderlands2`; the CTest checks that every real-game script that starts the game takes the lock.

Review round 3 (GPT-6-Luna; resolved by the hub without a further round):
- **The gate fails closed.** The `Local\` fallback for the mutex is gone: a second, session-local gate would not
  serialize against the global one. If `Global\OpenWillowUeLockGate` cannot be opened or created, the script stops with an
  error that says how to fix it. No opt-out.
- **Known limitation, accepted.** The lock is per Windows user; two different accounts on one machine are not serialized. Set `OPENWILLOW_UE_LOCK` to the same absolute path for both if that's ever needed.

## 2026-11-03: Health 0 puts Maya down in the UE host; the HUD movie shows Fight For Your Life, the second wind and the mission tracker (HUD lane)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/HUD_PARITY.md` (element inventory; "Fight For Your Life, second wind and
respawn"); the down-state rules and their live numbers come from `docs/verification/COMBAT_DOWN_RESPAWN.md`.

- **What.** `AOpenWillowWalker` no longer respawns at health 0. It goes down (Injured): the bleed-out starts 2.28 s later and runs
  12 s, cut by 4 s per successive injury (30 s window, 1 s floor). Bleeding out is death, and the respawn follows 5.76 s later. A kill
  while down is a second wind (25 % of the maximum health). Damage while down is ignored. A health refill while Injured (a level-up)
  ends the down state, so down always means health 0. The game is reported to revive a player who levels up in Fight For Your Life,
  but that is not observed here (UNVERIFIED).
- **HUD.** `AOpenWillowMayaHUD` pushes `down`, `secondWind` and `tracker` to the HUD page, which drives the movie's `p1.dying1`,
  `p1.secondwind` and `p1.MissionNotifications.Module1-6`. `MissionSystem::trackerLines()` gives the active objectives with their
  package `ProgressMessage`, and `--mission-run` output now ends with a `"tracker"` array.
- **Weapon while down (UNVERIFIED, to confirm live with bl-sdk).** Going down puts the weapon away, as the injured special move does:
  it releases the trigger, drops owed burst shots and cancels a running reload. Fire and reload presses do nothing for the 2.28 s
  until `SwitchToSidearm`; after that the held weapon fires and reloads as usual (no sidearm switch). Checked in `-Down` (`down_weapon`).
- **Why.** Die and respawn is a Phase 4 gate piece, and BL2's loop goes through Fight For Your Life, not an instant respawn.
- **Checks.** `run_hud_shots.ps1 -Down` 10/10 and `-Tracker` 4/4 in UE, with `tests/check_hud_down_test.py` (20 synthetic cases)
  for their checker. The down check compares the title with the generated strings file and checks the bleed-out bar's pixels. The
  tracker check covers optional objectives, and `tests/mission_script_test.py` scenario T covers `trackerLines()` on a toy mission.
  `test_quest.ps1` waits out the bleed-out before its respawn checks and adds `player_death_goes_down_first`. On main 490661e it gives
  112/112 plus resume 11/11.
- **Not verified.** No real-game frame of these elements exists on this PC: bl-sdk and a Maya save are missing, so no critic score.
  Which `dying1` and `secondwind` fields the native fills is UNVERIFIED. Nothing in the UE host attacks Maya yet: the enemy attacks
  of PR #59 run in the C++ VM only.

## 2026-11-03: Missions log page from the StatusMenu movie and the slice mission (menus lane)

AI-assisted (Claude). Record: `docs/verification/MENUS_PARITY.md` (new; coverage table of the slice's menus and the mission
log's rules). Script and AS2 listings and the presentation data stay under ignored `local/menus/` and `local/ui/run/`.

- **What.** A third status page, `tools/hud_overlay/missions.html` (`missions.js`, pure rules in `mission_log.js`), runs the
  installed StatusMenu movie's mission log and fills it the way `StatusMenuExGFxMovie` does: list rows through the movie's
  `SetTextEntry` / `MoveHighlightBar` / `SetMoreBars` / `setActiveMission`, the summary through the paths
  `UpdateMissionDetails` writes, the level panel through `SetPlayerLevel` / `SetPlayerXP`. Presentation values come from the
  installed game (`tools/prepare_mission_log.py`: `StatusMenu_Definition` icon frames and colours, `GFxTextList_Missions`,
  `GlobalsDefinition` difficulty thresholds, localized strings and PC key names) into ignored `local/ui/run/missionlog.json`.
  The description follows `WillowPlayerController.GetMissionDescriptionForUI`, which is UnrealScript (read, own words in the
  record). Host: `EOpenWillowStatusPage`, a preloaded Missions page, `/__ow_tab_missions` and `/__ow_close_missions` routes,
  the Missions header tab on the Inventory and Skills pages, and the summary / turn-in description fields in
  `UOpenWillowQuest::MissionLogJson` (`MissionSystem::summary()` / `turnInDescription()` added; no parsing change).
- **Read, not seen.** No rule here was compared with the running game: bl-sdk is not installed on this machine. Native parts
  stay host choices and UNVERIFIED: row order and category rows (`GFxTextListContainer`, `UpdateMissionTextList`), the row
  markup, which strings `MissionTracker.GetMissionDescription` returns, `CanTrackHighlightedMission`.
- **Movie quirks kept or worked around.** The script's no-missions text goes to `missions.title_questname`, which this movie
  does not have, so the frame's own static text shows (kept). `gotoRegularMissions` targets a label the missions clip
  does not have, so after the no-missions frame the page returns the clip to frame 1 itself (found by the in-engine capture;
  the game rebuilds the screen on each open). In a headless replay a column of list scroll ticks still shows after that
  transition (open; not seen in the engine capture). Several of the movie's text fields embed font subsets without `/ : - , !`;
  the page re-sets them in the movie's own `$WillowBody` face, as `skills.js` does.
- **Checks.** `node tests/mission_log_test.js` 9/9 (synthetic); `tools/test_mission_log.ps1` (new, in engine with
  `-owquest`) 7/7: open, nothing listed before Accept, the mission under the in-progress category with the `inprogress` header on frame 1
  after Accept, Down, tabs to Inventory and back, Escape. Inventory suite 49/0/0, navigation 40/40, `OpenWillow.Skills`
  passes. Other numbers are in the PR.
- **Review rounds.** Round 1: no game values in tracked files (the script's markup constants are read into the ignored
  JSON), escaped mission text, path guards in `menus_probe.py` and `serve.py`, the Missions browser removed in `EndPlay`.
  Round 2: `prepare_mission_log.py` refuses an output outside the repository's `local/`; the host sends no credit or item
  reward (neither is evaluated; the page hides both panels, UNVERIFIED) instead of a hard-coded 0; the page uses the named
  `status` field instead of numeric status IDs, and the no-host sample and fixtures use invented values.
- **One objective view.** After the HUD lane's tracker landed (PR #63), the mission log reads the active objectives from the
  same `MissionSystem::trackerLines()` as the HUD tracker; the separate `activeObjectives()` is gone. `TrackerLine` gained the
  objective's `count` (ObjectiveCount), which the log prints progress against; the tracker does not use it. The launcher
  `tools/test_mission_log.ps1` takes the machine-wide UE run lock (`tools/ue_lock.ps1`, PR #66).
- **Inventory round-21 list.** VALUE thumbnails do not reproduce on round-21 code (they were a round-22 regression); the
  BRANDS order is the engine's unstable sort, not a missing tie-break. Details in `INVENTORY_MOVIE_PROTOTYPE.md`.

## 2026-11-03: HUD damage-direction arrows and level-up banner driven from host state (HUD lane 2, after PR #63)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/HUD_PARITY.md` ("Hit arrows and the level-up banner").

- **What.** `AOpenWillowWalker::TakeDamage` counts hits that took health and have a known source (a point hit's shot direction,
  else the causer or instigator). The HUD sends the last one's side of the view (90 degree quadrants) and a level-up count. The page
  plays `p1.dmg_<side>` from frame 2 on its `Health` frame, and `p1.levelup` from `go`. Both animations, and the banner text, are
  the movie's own frame scripts.
- **Rules after review round 1 (UNVERIFIED, to confirm live with bl-sdk).**
  - The arrow side is computed in view space, pitch included. A source near the view's up or down axis shows the top
    (above) or bottom (below) arrow.
  - One banner per level change: a multi-level jump plays one banner, since the HUD script keeps a single
    `QueuedLevelUpLevel`.
  - A zero point-hit direction falls back to the causer, then the instigator.
  - `-owhudfxshots` is editor-only.
  - Checks: `-Feedback` 7/7 with `fx_arrows_clear`; `fx_levelup` compares the page's banner count with the host's level
    rises.
- **Not done.** The reload prompt (its state and text are native), crosshair widening (the host has no bloom) and the grenade icon
  type (no delivery type on the item). The HUD_PARITY inventory says why for each.
- **Checks.** `run_hud_shots.ps1 -Feedback` 6/6 in UE; 4 synthetic checker cases; `hud_fields_test.js` extended.
- **Not verified.** No real-game frame of either element exists on this PC. The side split and the inner label the game uses are
  UNVERIFIED. Nothing in the UE host attacks Maya yet, so in play the arrows show only for the test hits.

## 2026-11-03: gun parity, host side: muzzle socket mirror removed, ADS refit, slice SMG ammo pool, reload clips (Weapons)

AI-assisted (Claude). Record and numbers: `docs/verification/WEAPON_VISUALS.md` (open list after section 14) and
`WEAPON_PROJECTILES.md` section 4. Host behaviour checked in the host only; nothing here was compared with new real-game frames.

- **Muzzle flash at the rear of the SMG and shotguns: fixed.** `WeaponSocketWorld` mirrored the cooked socket offset across Y;
  the decoded-socket path and the mesh-space reference do not. The gunprobe showed the mirrored SMG muzzle 33 cm behind the gun's
  root and the unmirrored point equal to the decoded socket (SMG 116.9 cm, shotgun 162.2 cm forward of the camera); the pistol and
  AR moved less than 0.3 cm. The mirror is removed. Firing suite after it: 14 PASS / 0 FAIL.
- **ADS refit (UNVERIFIED host rules).** The eye sockets went through the same mirror. The fit now projects the eye socket onto the
  plane through the gun's `Root` across the barrel axis (only the sight's height and side offset count), and converts the 18 px
  drop at the gun body's depth. SMG, shotgun and AR offsets change by at most 1 cm; the pistol's changes and its fit still does
  not converge (3.2 cm side error, as before the change): open.
- **Slice SMG ammo pool.** `UOpenWillowInventory::ResolveAmmoType` matched only the type `SMG`; the recipes carry the game's
  long SMG type label (the one `tools/weapon_stats.py` writes), so the slice SMG had no ammo pool and could not reload. One alias row; inventory self-test check added.
- **Reload clips.** Regenerated on a fresh machine with the existing tools (commands in WEAPON_VISUALS.md); the pistol, AR and SMG
  reloads now play with their arms clips in the host. The shotgun has no reload clip (no shotgun family in
  `tools/weapon_reload_anims.py`).
- **Tools.** `tools/run_weapon_assembly_shots.ps1` no longer defaults its hub lock to another machine's user folder
  (`OPENWILLOW_HUB_LOCK`, else none; a missing lock folder fails fast).
- **Waiting on real-game frames:** reload mid-poses, SMG hip size (the old comparison used a different SMG roll), ADS widths and the
  pistol ADS position. Elemental impact explosions wait on a maintainer decision (native read or live hook).

## 2026-11-03: inventory round 23: VALUE thumbnails, Inspect turn, card and plate fits (Inventory parity)

AI-assisted (Claude; Luna (Codex) critic). Record: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` "Round 23". Page behaviour
changes, checked in the UE host against the matched D20 real frames; no new real-game frames.

- **Thumbnails survive re-renders.** The movie's turn sways after some sorts and each step rebuilt every cell with a new image,
  which was still decoding when the VALUE frame was taken. Decoded thumbnails are now reused across renders. Confirmed on the host
  capture (D2e shows all four), and by a page log of the renders.
- **Single-card occlusion.** The single card hides equipped and gear thumbnails but not backpack ones (round 22 hid neither,
  which drew equipped thumbnails over the card text).
- **Inspect keeps the menu's 3D turn** (the movie sets 5 degrees there); the hint is larger and on one line. UNVERIFIED fit to D5.
- **Fits** (round-22 geometry without its sort tie change; card, compare card, focus panel, count and ammo plates, glass-sheet top
  edge) are measured on D20 only: UNVERIFIED elsewhere.
- **Not settled:** the number keys in the real backpack (the game would not start: Steam), the native BRANDS tie order, the zoom
  rounding in `src/weapon_gen.cpp`, the white Inspect weapon paint. One blind A/B critic round: final 7.99 against 7.84 for round 21.

## 2026-11-03: vending machines in Sanctuary, buy and sell, with the shop price confirmed live (Vendors PR 1)

AI-assisted (Claude; implementation sub-agents on Haiku). Record: `docs/verification/VENDORS.md`. One read-only real-game
dump (`tools/real_game/vendors_capture.ps1`, level-8 Maya in Sanctuary, saves blocked and restored byte-identical).

- **Placement (package data).** Machines spawn at population opportunity points whose population is a
  `PopulationFactoryVendingMachine`; `tools/prepare_vendors.py` writes them with their kind (shop type), pose (rotation in
  degrees like the slice manifests) and the expanded `Inventory` / `FeaturedItem` loot pools to ignored
  `local/slice/vendors.json`. Pools in the map package that point at Startup objects are resolved there. Live, seven
  machines and the Black Market sit exactly on these points; an eighth (a Dr. Zed machine) is not found yet.
- **Price (behaviour, confirmed live 79/79).** Buy price = floor(value x (item level x 0.14 + 4.1)) in single precision, the
  same for the featured item; 0.14 and 4.1 are the live values of the two markup attributes, which have no value in the
  packages. Stock sizes (weapons 7 + featured) and the 20-minute reset were read live too.
- **Host.** `UOpenWillowVendorSubsystem` spawns placeholder machines from `-owvendors=`; the use key opens the nearest within
  350 uu; the page `tools/hud_overlay/vending.html` (stock movie as frame art, HTML lists) buys, sells and buys back through
  `OWVEND` lines. Guns only for now; the sale price (the item value), buy-back, the stock roll and the layout are UNVERIFIED.
- **Tests.** `node tests/vending_test.js`, `tests/prepare_vendors_test.py`, `tests/check_vendors_test.py` (synthetic), the UE
  automation test `OpenWillow.Vendor.Stock`, and the in-engine suite `tools/test_vendors.ps1` (`-owvendortest`) for the hub.

## 2026-11-04: vendor currency and stock persistence; non-weapon item values confirmed live (Vendors PR 2a)

AI-assisted (Claude; sub-agents on Haiku and Opus). Record: `docs/verification/VENDORS.md` sections 4 and 6.

- **Item value (package data, confirmed live 63/63).** Ammo, Health Now!, grenade mods, shields and class mods share the
  gun formula: value = trunc(part_total x 1.12 ^ item level x base cost), with each kind's `Att_BaseCost_*` constant and
  the grade's part total (1 for ammo and health). Checked against the read-only live dump of PR 1; the two DLC class mods
  are not described by the packages read and stay UNVERIFIED. `tools/prepare_vendors.py` writes an `item` block per
  non-weapon leaf (kind, ammo pool and amount, heal fraction, value terms); the heal and the ammo-on-purchase behaviour
  are UNVERIFIED. The host does not stock these kinds yet (PR 2b).
- **Persistence (host behaviour).** `-owcurrencysave=<file under local/>` keeps money, eridium and each machine's restock
  count and remaining timer; the shelf is re-rolled from the machine's seed and restock count with the bought items
  removed. Buy-back is not kept, and the timer does not run while the game is closed: both host choices, UNVERIFIED.
  `-owmoney` / `-owerid` override the save. `OpenWillowInventory.h` gains `GetEridium` (one line).
- **Tests.** UE automation `OpenWillow.Vendor.Persist` (synthetic), a second launch in `tools/test_vendors.ps1` that checks
  money and shelf survive a restart, `tests/prepare_vendors_test.py` (15) and `tests/check_vendors_test.py` (35).

## 2026-11-04: the Ammo Dump and Dr. Zed sell; ammo purchases fill the pools (Vendors PR 2b)

AI-assisted (Claude; implementation sub-agent on Haiku). Record: `docs/verification/VENDORS.md` sections 4 and 6.

- **Stock (host behaviour, UNVERIFIED rules).** The Ammo Dump shows one row per ammo type, then grenade mods by weight,
  for 11 rows. Dr. Zed shows one row per health item, then shields by weight, for 8 rows. Both shelf sizes follow the live
  row counts. Rows come from the decoded item blocks (value confirmed live 63/63; the buy price uses the confirmed shop
  markup). They are seeded like the gun shelf, so the PR 2a save restores them; the manifest identity now covers the item
  blocks, so an older currency save is refused, not misapplied. Class mods are left out because their shop pools are
  empty in the packages.
- **Effects (UNVERIFIED in game).**
  - Ammo adds the item's rounds to the pool, capped at its maximum, and is refused with "Ammo full".
  - Health Now! heals 25 % of max health and is refused at full health.
  - Ammo and health rows stay on the shelf.
  - Shields and grenade mods enter the backpack as stand-in gear with no stats and rarity 1. They can be sold and bought
    back.
- **Tests.**
  - UE automation `OpenWillow.Vendor.Items`.
  - `node tests/vending_test.js` (26).
  - `tests/check_vendors_test.py` (ammo_buy, heal_buy, item_prices).
  - The `-owvendortest` sequence buys ammo at the Ammo Dump and a heal at Dr. Zed, after a test-only drop to half
    health.
## 2026-11-06: Sanctuary's lootables and pickups in the host (Vendors PR 3)

AI-assisted (Claude; implementation sub-agents on Haiku, data on Opus). Record: `docs/verification/VENDORS.md` section 7.
Package data only: nothing here has been seen in the running game, so the rules are UNVERIFIED.

- **Data.** `tools/prepare_interactives.py` writes 97 placed objects with their loot configurations, visuals, requirements and
  pickup data to ignored `local/slice/interactives.json`.
- **Host.** `UOpenWillowInteractiveSubsystem` (`-owinteractives=`) spawns the 95 supported objects; the golden and Bearer of Bad
  News chests are not spawned. The use key opens the nearest within 350 uu, in the walker's nearest-target rule. Guns and gear
  drop through the existing `AOpenWillowInventoryPickup`; `AOpenWillowLootPickup` handles cash, ammo, eridium and the
  health vial. Heals change current health only and are refused while down, as in PR #99.
- **Assets.** `tools/import_lootables.ps1` imported 21 of 36 assets. 15 clips were refused by the converter's mirror check: 2 and 3
  bone skeletons cannot determine the fitted map. Missing assets fall back to placeholders.
- **Tests.** UE automation `OpenWillow.Interactive.Loot`, `tests/check_interactives_test.py` and
  `tests/prepare_interactives_test.py` (synthetic), and the in-engine suite `tools/test_interactives.ps1`.

## 2026-11-06: Quick Change respec at the placed machine (Vendors)

AI-assisted (Claude). Record: `docs/verification/VENDORS.md` section 8. The use key near Sanctuary's `CharacterCustomizationMachine`
calls `UOpenWillowSkills::PurchaseReset` (#73) and charges the money. The menu is not built: two presses within 5 s stand in for
its dialog (host choice, UNVERIFIED). The cost formula is the skills lane's, UNVERIFIED in game. Checked by
`OpenWillow.Interactive.Loot` and check_interactives' `quick_change`; not yet run in-engine.

## 2026-11-06: lootable and pickup textures; pickup labels only on placeholders

AI-assisted (Claude). The imported lootable and pickup meshes rendered as plain pale surfaces. `tools/lootables_editor.py` gains a `materials` step: diffuse and normal from UModel's export, one instance per material of a minimal material, bound to each mesh's single slot. A mesh with no answer stays unbound with a reason; a similar name alone never binds a material. No parsing behaviour changed.

- **Verified:** synthetic tests of the selection rules; the plan resolves all 18 meshes on the local export, with `Skel_StorageLocker` taking `Mati_StorageLocker` from its own import, read from the package's import table and UModel's log. Editor binding and rendering are not yet run.
- **`UNVERIFIED`:** how the result looks (UModel's material output is heuristic, and this is not the game's material graph); the two meshes' slots in the running game; whether the game shows text on cash and ammo pickups. The host now hides the stand-in label when the real mesh loads.

## 2026-11-06: a lootable material that did not bind now fails the run

AI-assisted (Claude). The first editor run of the materials step bound only the 12 pickups. The skeletal path changed a copy of the slot, every lootable stayed grey, and both the import and `test_interactives` still read as a pass. Now `import_lootables.ps1` fails when a planned mesh did not bind, after writing the report, and the host logs the material each lootable type renders, which `check_interactives.py` reports as `INCOMPLETE` unless the materials step bound it. The slot is written back, and a rerun updates instances in place instead of deleting what meshes use.

- **Verified:** synthetic tests, including one the old bind fails; the new check exits 1 on the failed run's own report. No parsing behaviour changed.
- **`UNVERIFIED`:** that the fixed bind holds in the editor (not run yet), and how a bound material looks.

## 2026-11-06: the lootable material bind has run, 18 of 18

AI-assisted (Claude). With the slot written back, the editor run bound all 18 meshes through 10 instances, and `test_interactives.ps1` passed with its new `lootable_materials` row (`passed=11`). No code changed in this entry's commit.

- **Verified:** that the host binds and renders these materials. A passing suite is not parity.
- **`UNVERIFIED`, host frames only and not compared with the game:** how the materials look; the storage locker's hinge, which reads as correct from the frames; the strong box's hinge, which stays open until `tools/real_game/loot_capture.ps1`'s side frame and its real-game frames decide it. `docs/verification/VENDORS.md` names the frames nobody read and the untriaged blue streak in the existing world.

## 2026-11-06: lootable materials: a failed save fails; each type must render its planned instance

AI-assisted (Claude). From Luna's review of PR #120. A failed asset save in the materials step now fails that mesh, or every mesh for the folder, instead of reporting a bind that is not on disk. An instance's name now includes a hash of its textures, so two exports with one name and different textures do not share one. `check_interactives.py` compares the host's path with the exact instance planned for each type, by full balance, and neither the lid nor the materials row can pass without the content folder and plan that say what was expected. A rerun clears an instance's parameters first, and `import_lootables.ps1` drops the old `import_report.json` when it starts.

- **Verified:** synthetic tests, with the Unreal calls faked. No parsing behaviour changed.
- **`UNVERIFIED`:** these paths have not run in the editor. The mesh and clip saves of the other editor modes still ignore the result: they are outside the materials step and were not changed.

## 2026-11-06: lootable materials: nothing touches a mesh until its materials are saved; no lootables fails the suite

AI-assisted (Claude). From Luna's second review of PR #120. The materials step now saves textures, instances and this run's folders first, and only then binds and saves meshes; if a folder save fails, no mesh is touched. A mesh whose own save fails changed only in memory, and the commandlet exits without saving it. A texture the instance refuses fails the row. `test_interactives.ps1` no longer passes on a machine with no imported lootables unless told `-AllowNoLootables`, and the summary then says so. Frame names now carry the full balance, and the checker requires each frame on disk.

- **Verified:** synthetic tests, with the Unreal calls faked, looking at the fake meshes' slots. The current manifest has no two balances with one short name.
- **`UNVERIFIED`:** none of this has run in the editor yet.

## 2026-11-06: the material setter's result means nothing; read the value back

AI-assisted (Claude). The first editor run of the checked setter failed every row: 0 of 18 bound. In UE 5.8, `UMaterialEditingLibrary::SetMaterialInstanceTextureParameterValue` declares `bool bResult = false`, never sets it and returns it (read in the installed engine's `MaterialEditingLibrary.cpp`), so it returns false whether or not it worked. **The test's fake returned true, so the tests passed on a check that could never pass.** That fake assumed the API's meaning, and only a real run could show it. The step now checks that the parent has the parameter, then reads the value back and compares it; the fake returns false always, as the engine does.

- **Verified:** that the run touched no mesh: every mesh asset still references the earlier run's instance, with its time unchanged, and no new instance reached disk. Synthetic tests.
- **`UNVERIFIED`:** that the read-back holds in the editor, until the rerun.

Also from Luna's third review: each `test_interactives.ps1` run now has its own log and frames folder, and two balances sharing one frame name fail the check.
## 2026-11-06: the shop page's selection starts over on opening; refusals are shown in the stock labels

AI-assisted (Claude). From a playtest. The page kept its selection between machines; the host now counts openings and the page starts over on a new one. Refusals could go unseen: the host's one-second push wiped the page's own message, a press crossing that push was dropped as stale with no message, and the message was a small line under the lists. Now a refusal stays until the player moves, acts or the shop reopens, the timer push keeps the serial unless the stock changed, and a stale press says the stock changed.

- **Read from script, not from the running game (`UNVERIFIED`):** `VendingMachineExGFxMovie` shows one label for "cannot afford" (`INSUFFICIENT FUNDS`) and one for "cannot carry or use" (`FULL`), and the shop status that picks between them is native. So mapping a full ammo pool, full health and a full backpack to `FULL` is a reading. The label is plain page text, not the movie's feedback clip, and its sound is not played.
- **Verified:** synthetic page tests. The playtest's silent refusal was not reproduced, because the host path reads as sending its message; the causes above come from reading the code.

## 2026-11-06: the shop's serial moves only when what a request refers to changed

AI-assisted (Claude). From Luna's review of PR #127. A refusal bumped the serial, so a second press on a full pool looked stale and could say the stock changed when nothing had. The serial now moves on a success, a restock or an opening, and a refusal keeps it, so a repeat press gets the refusal again. A press that is really stale changes nothing: it keeps a refusal already showing, otherwise says "Try again", and does not move the serial, so a held key cannot keep missing. This replaces "The stock changed" from a stale press in the entry above; that text now only means a restock did happen.

- **Verified:** synthetic tests, including the real page code in Node with stub elements. `tools/test_vendors.ps1` gains a `stale_press` row through the host's own message path, which has not run yet.
- **`UNVERIFIED`:** real key input and how the text looks on screen.

## 2026-11-03: elemental impact explosions from the firing mode, the cooked muzzle flash and casing (Weapons parity 2)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` section 8, `WEAPON_VISUALS.md` section 18,
`NATIVE_WEAPON_TRIGGER_AMMO.md` (`ProcessInstantHitBullet` branches). The planned live session could not start on this PC
(Borderlands 2 stopped at a "Steam must be running" message box), so nothing here was checked against new live reads.

- **Impact explosions (behaviour change).** A trace or light-bullet firing mode whose own behaviours hold a
  `Behavior_Explode` explodes at every impact after the bullet's damage; `ExplosionOverrideDefinition` only picks the
  explosion definition (so the damage type) per element. Source: a clean-room native read (Ghidra, read-only; own-words
  note under ignored `local/`) and the cooked data. Only the Maliwan firing mode has one among the regenerated recipes:
  radius 106, 0.8 x weapon damage, momentum 1000, no falloff. It reproduces the one live number (45.30 x 0.8 = 36.24);
  the rest is UNVERIFIED in game. `tools/weapon_firing.py` writes `impact_explosion`; the host plays it as the gun's
  splash (light bullets and traces); shock, corrosive and slag explosions draw a small element-coloured burst.
- **Muzzle flash and casing (behaviour change, host look).** `tools/weapon_muzzle_fx.py` reads the type's muzzle-flash
  particle systems (per element) and casing system from the cooked data into `weapon_view.json`; the host draws each
  emitter as a soft quad with the cooked size, offset, life, colour and axis lock. The lookup-table layout is fitted and the
  quad fills are host fits (UNVERIFIED). Only the assault rifle type has a casing system; it now uses the cooked life,
  speed and gravity.
- **Pistol ADS fit.** The side term of the pistol's frame fit went from -3.2 to -0.6 cm (critic measurement); UNVERIFIED.
- **Tests and tools.** Checker: the impact explosion counts as the gun's splash; the firing test refills the dummy per
  segment; quest check `lent_pistol_shot_carries_its_damage_type_to_dummy` now expects the bullet hit first and, for an
  exploding gun, the explosion hit after it. New `tools/real_game/weapon_parity2_capture.ps1` and its probe (not yet run to
  completion).
- **Results.** Critic 6.3 -> 6.5 / 10 (firing 6.1 -> 6.5); target 8 not reached. Firing suite 14 PASS / 0 FAIL after review
  round 1 (the round-0 Tediore failure came from the test's per-segment dummy refill: a dummy that never died never respawned
  home and drifted; the test now returns it home before each gun). Shot effects have a per-frame cosmetic budget (96 actors;
  damage unaffected). `test_quest` 111/112: the failing check
  (`natural_level_up_scales_weapon_damage_for_duration`) needs the recipes' `damage_stack`, which this PC's local slice
  recipes lack (local data).
- **Open, for a live session:** world FOV 77.55 vs 90 (HUD-canvas projection), the equal-level status chance, the zoomed
  foreground FOV (AR/SMG ADS width), reload mid-poses, the particle values in the running game.

## 2026-11-03: Maya's shield and survival state in the host (maya lane, item 2)

AI-assisted (Claude). Record: `docs/verification/COMBAT_DOWN_RESPAWN.md` "The host's state". Host behaviour change; no new live reads.

- **New `FOpenWillowMayaState`** (`OpenWillowMayaState.{h,cpp}`): health, shield and Fight For Your Life move out of
  `OpenWillowWalker.cpp` unchanged, with every stat in one struct (`FOpenWillowMayaStats`) for the skills lane's modifiers.
- **Shield (behaviour change).** Hits take the shield first, then health; the recharge waits the delay after each hit, then rises at
  the rate. Numbers come from the equipped shield's card, else the live read of one shield (1.611 s, 25.88 per second) as a stand-in.
  No recharge while down, a swapped shield starts empty, the second wind sets the shield to 90 % (the last is live; the rest UNVERIFIED).
- **`OpenWillowApplyDamageToPlayer`** is the one damage entry point for enemies (range lane) and hazards.
- **Checks.** New automation test `OpenWillow.MayaState` (synthetic). In-engine results come from the hub's integration run.

## 2026-11-03: Maya's movement from the stock data in the host (maya lane, item 1)

AI-assisted (Claude). Record: `docs/verification/NATIVE_PLAYER_MOVEMENT.md` "Host implementation". Host behaviour change; no live reads.

- **Speeds and physics** move from estimates (450 run, 650 sprint, jump 420, 45-degree floors) to the cooked values: run 440, sprint 594,
  crouch 220, jump Z 630, air control 0.11, friction 8 with braking factor 2, floor Z 0.7. The predicted jump: a 318 uu apex and
  2.27 s in the air with the falling step's fluid friction 0.3 (396.9 and 2.52 s without it); logged at every landing, not yet
  compared with a live jump. In the air there is no horizontal speed cap (stock rule at air control 0.11).
- **Sprint** follows the script's rule (a 45-degree input cone, press-to-start, ended by crouch, firing, zoom or going down), with the
  sprint FOV (the flexible-FOV maths, eased at 10) and the sprint eye height. **Crouch** is new (hold, Left Ctrl or C).
- **Camera bob and eye height** follow the read eye-height and bob update; **falling damage** follows `TakeFallingDamage`.
- All UNVERIFIED in game. Synthetic automation test `OpenWillow.MayaMovement`.

## 2026-11-03: Phaselock round 22: the Adult bullymong, the palm orb's facets, the LDR cap (maya lane, item 3)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 22". Presentation change; no live reads.

- **Target archetype (tool data change).** `tools/prepare_phaselock_target.py` imports `GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult`
  (the "Adult Bullymong" of the real frames: brown `Mati_BullymongAdult`, 150 x 90 cylinder, no scale override), not the white
  `Pawn_PrimalBeast`. Checked with the tool's light steps (identity, extract, editor job: 4 UModel jobs, 0 failed).
- **Palm orb.** The star-burst layer's material alpha is stock again and its emitter gain is 0.5 (was about 3 % of stock).
  The hand-written effect stand-ins cap colour at 1 per channel, as the importer's own LDR rule does (was 4.0).
- **Capture switch** `-owtonecurve=<0..1>` for a tone-mapping A/B. The target's facing (0 or 180) is to be settled by an A/B capture.
- All UNVERIFIED until the hub's re-import, capture and critic round.

## 2026-11-03: the HUD shield bar follows Maya's shield pool (maya lane, item 4)

AI-assisted (Claude). Record: `docs/verification/HUD_PARITY.md`. Host behaviour change.

- The Flash HUD's `shield` fill and `shieldText`, and the canvas HUD's shield bar, now read `FOpenWillowMayaState` (drains on hits,
  recharges after the delay) instead of a full bar at the card's capacity. The text is the current pool rounded. The action-skill
  cooldown ring already read the Phaselock cooldown (`ActionSkillHudState`); unchanged.
- Test fixtures that meant "health to 0" or "half health" (`-owhuddownshots`, the quest's level-up fixture) add the shield, which now
  takes hits first.

## 2026-11-04: inventory number keys switch status-menu tabs (inventory lane, round 24)

AI-assisted (Claude). Record: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` "Round 24". Page behaviour change, read from
script bytecode (`StatusMenuExGFxMovie.HandleInputKey` -> `TrySwitchToScreen`); not yet seen in the running game (UNVERIFIED).

- Keys 1-5 in the inventory page switch to Missions, Map, Inventory, Skills, Challenges, as the status menu's script does; the
  pages the host lacks (Map, Challenges) do nothing. The host's own slot picker on 1-4 is removed: no inventory panel handles
  digits, and the transfer's slot is chosen with Up and Down.
- The inventory action suite follows: its digit presses became Up/Down in the transfer and one step was added (50 steps,
  50 PASS on a bounded set rebuilt on pc-hummer).
- Follow-up: Comma and Period are the script's previous and next tab (`GotoPrevTab` / `GotoNextTab`); from Inventory, Map
  (no host page: nothing happens) and Skills. UNVERIFIED in game.

## 2026-11-04: world pickup card from BL2's own card movie (inventory lane)

AI-assisted (Claude). Record: `docs/verification/PICKUP_CARD.md`. New host behaviour; read from script and installed data, not
observed in the running game.

- While Maya aims at a dropped item within pickup reach, the HUD shows BL2's pickup card movie (`UI_Inworld.Weapon_Card`,
  converted locally) with the compare arrows against the equipped analogue; it hides when she looks away and after pickup.
  Confirmed in host captures (`-owpickupcardshots`), not against the game.
- The item data is the inventory page's (`WeaponItemJson` / `GearItemJson`, moved out of `StateJson` unchanged) and the card
  is filled by `item_card.js`, the inventory page's card code (moved out of `inventory.js` unchanged).
- UNVERIFIED host choices: reach 220 uu (as `PickupNearby`), a 12-degree aim cone, card size 30 % of the screen height,
  placement over the item, frame style 'default', and showing and hiding at once. E still takes the nearest pickup (known gap).
- Follow-up (same day): the card's level-requirement strip shows only for a required level above 1, as
  `ItemCardGFxObject.SetItemCardEx` does; an item above the player's level turns the card red through the movie's own
  `SetLevelRequirement` (headless replays of both pages; the selected inventory card keeps a yellow outline, UNVERIFIED).


## 2026-11-04: gun look on the second PC: part library rebuilt, Inspect capture gets its own fitted light (gunlook)

AI-assisted (Claude Opus 5.5; Haiku 5.5 sub-agents; critic Luna). Record: `docs/verification/WEAPON_VISUALS.md` section 19.
No change to `src/`, `CMakeLists.txt` or package parsing.

- **Data step, no code:** the weapon part library was rebuilt on this PC with the documented chain (301/301 skins exported,
  300 imported, 6/6 meshes, 279 slots); 33 type x manufacturer guns assemble with no fallback (oracle 33/33 against the cooked
  prediction; no live check here). The 22 live-only Torgue glow colours are missing (no live census on this PC).
- **Behaviour change (host look):** the inventory preview capture (Inspect, thumbnails) uses its own light, ambient 3.2 /
  key 0.7 / fill 0.3 instead of the world look's 1.6 / 1.2 / 0.5, fitted on one gun (the slice Jakobs pistol) against the real
  Inspect frame: luma percentiles 25-75 within 6 of the real frame (were 21-28 darker). UNVERIFIED stand-in light; the
  world look is unchanged. `-owinspectlook` overrides it; `-owgunturn` adds turntable frames to `-owgunshots`.
- **Critic:** round 0 mean 7.6 (Inspect side 8.6), round 1 Inspect side 8.0 (different runs, not comparable); elemental glow
  parts and rarity skins read as correct (no real frames for them). Open: highlight roll-off, grip weave, the pale rim, a
  real-game reference session.

## 2026-11-04: Phaselock round 23: colour constants read as gamma-space values (maya lane)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 22 results" and "Round 23". Presentation change.

- Round 22 scored 5.8 / 10 (round 20 rescored 5.5) by a Luna critic on the hub's captures; target 8 not reached.
- The target's zone colours and the palm orb's blue constant are converted from gamma to linear (x^2.2): read as linear they showed
  pale olive fur and a pale cyan orb next to the game's tan fur and cobalt orb. UNVERIFIED until the next capture.
- The capture target faces Maya (+180). The critic preferred 0; the pre-lift frame and the FK check favour 180. Disputed, recorded.

## 2026-11-03: one hit-event API for every hit of Maya's shots (slice projectiles lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_PROJECTILES.md` section 9. Host behaviour change, no new game rule.

- Every hit (bullet, trace, rocket splash, impact explosion, thrown gun, status tick) goes through
  `AOpenWillowWalker::ApplyShotDamage` as an `FOpenWillowHitEvent` (who, where, damage, damage type, element, crit inputs,
  momentum), delivered as the target's `TakeDamage` event (`FOpenWillowDamageEvent`, a point damage event) and then on
  `OpenWillowHit::OnHit()`.
- Explosions and status rolls now reach every actor that implements `IOpenWillowHitTarget`, not only the host dummy class,
  so the range lane's enemies take splash. The dummy implements it with its existing geometry and level (no number changes).
- Crits stay the victim's decision as in BL2 (hit region, base bonus 2.0 confirmed live); the event carries the bonus, the
  target sets the result. Splash crits are taken as none (UNVERIFIED). In-engine confirmation: the hub's firing-suite run.

## 2026-11-03: accuracy pool, view kick and spread from live reads (slice projectiles lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_PROJECTILES.md` section 10. Behaviour change in the host and the firing
tool; one real-game session (hub heavy slot, saves restored and hash-identical).

- **Confirmed live** (one gun, the save's Jakobs shotgun, with Maya's skills on; rules only, not per-gun values): the accuracy
  pool rises by the impulse at each shot, holds 0.2 s, then falls linearly to its floor. The kick is impulse x 182.0444, times
  `WeaponKickZoomMultiplier` when fully zoomed. The kick targets decay linearly to 0 over `WeaponKickRecoveryTime` and the
  current amounts chase them at exactly `WeaponKickSpeed` per second. The controller rotation never moves, so the kick is a
  returning view offset. `AddSpread` draws yaw uniformly and pitch uniformly on the chord.
- **Host:** `src/weapon_firing` `AccuracyPool` and `Kick` (CTest). `tools/weapon_firing.py` writes `accuracy_pool` (the pool
  definition scaled by the weapon's external slots and effects; UNVERIFIED per gun) and the full kick fields. The walker draws
  the pool cone once per shot (x 1 - zoom), adds the impulse and kick after it and applies the kick as a view offset. The
  firing suite runs without it unless `-owaimmodel` is given.
- **Not settled:** the zoomed yaw offsets (view wander, suspected), the kick's horizontal sign, the Hyperion alternative kick,
  per-gun pool values. The equal-level status chance came out about x1.8 of the card chance with skills on (section 10), so the
  level-scale reading of the older x2 is now in doubt; a skills-off rerun is queued.
- **PR #84 review 1 (same day):** the kick chase keeps the live per-frame linear step for parity (the exponential form does
  not fit the 8.00 measurement) and substeps frames over 1/30 s; signed kicks; non-finite and inverted inputs are guarded; pool and
  kick are gated separately; the aim model is off in every scripted shot or test mode unless `-owaimmodel`.

## 2026-11-04: hit damage source, and explosions and rocket trails from the cooked particle systems (slice projectiles lane)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` sections 9 and 11. Host behaviour change and a new
data tool; no live session.

- **Hit source (skills lane request).** `FOpenWillowHitEvent::Source` (`EOpenWillowHitSource`) says what dealt the damage,
  separately from `Kind`. A rocket's explosion is Rocket with Kind Splash; a bullet's impact explosion is Explosion (the game
  sends it as a grenade source); the default is Unknown. The firing checker requires every firing path to set one.
- **Particle data.** `ExplosionDefinition.ExplosionScale` decodes as `ExplosionScaleData` radius bands, each naming its
  particle system and scale. `tools/weapon_projectile_fx.py` writes the tracer, trail and explosion layers into the recipes.
  The host draws the band for the explosion's radius, and the rocket trail, as camera-facing quads with the cooked values.
  The band choice and the drawing are UNVERIFIED; the critic round is pending.

## 2026-11-04: the status menu's Map screen (inventory lane)

AI-assisted (Claude). Record: `docs/verification/MAP_SCREEN.md`. New host page; read from script, installed class defaults
and the StatusMenu movie's own ActionScript, not observed in the running game.

- Key 2, Comma and the Map tab open `map.html`: BL2's StatusMenu movie on its Map frame with the level's tactical map movie,
  Maya's arrow, the stock icons and the quest's markers placed through the clip's own `addLandmark`, fed with the HUD
  minimap's state (`OpenWillowMapScreen`). Scale 135 on open, zoom clamped to 135-395 (class defaults); the hint bar is the
  movie's own `map_bar` text (`tools/prepare_map_screen.py`, local output only).
- UNVERIFIED host choices: centring on Maya, the arrow's rotation, one key press's zoom (x1.2) and pan (one PanIncrement),
  the header name; the custom waypoint, mouse drag and info box are not driven. No fast-travel list exists on this screen.

## 2026-11-04: the quest's on-screen text hides behind the status pages; mission log states checked (inventory lane)

AI-assisted (Claude). Record: `docs/verification/MENUS_PARITY.md` "Mission log". Host behaviour change.

- `UOpenWillowQuest::UpdateHints` no longer posts its tracker and talk lines (on-screen debug text) while a status page is
  open; they drew over the Inventory, Skills, Missions and Map pages under `-owquest`.
- The Missions page shows the Fire mission's four states (accepted, at the range, ready to turn in, turned in) in BL2's
  mission log with the expected category row and header (host captures from resumed saves; UNVERIFIED against the game).
- New capture switch `-owstatusshots=<page>[,<name>]` (`OpenWillowStatusShots`): opens a status page, captures it, quits.


## 2026-11-03: Skills menu: respec in the host, the page's spend rule as a tested module (Skills lane PR 1)

AI-assisted (Claude). Records: `docs/verification/NATIVE_SKILLS.md` section 3.1 "Host", `SKILL_TREE_NAVIGATION.md`
"Spend rule on the page".

- **Respec (behaviour change).** `UOpenWillowSkills::ResetTree` returns every point, the action skill's included, and keeps
  level and experience; `PurchaseReset` charges `ResetCost(level)` = 100 x 1.12^level credits (truncated), the formula of
  `GD_Globals.Skills.CostToResetSkillPoints` read from the install. The rule follows the native note and agrees with the
  local respec trace at level 45 (41 free points, no grades); the cost is UNVERIFIED in game. No caller yet: the Quick
  Change station is not built.
- **Page rule.** The tier/action-skill/max-grade/no-points rule the Skills page uses to show open tiers and the
  "Spend Point" hint is now `tools/hud_overlay/skill_rules.js`, tested by `node tests/skill_rules_test.js` (synthetic).
  Same rule as before; the host still decides each spend. Already in place before this change and unchanged: spending
  per tier with the 5-point gate, ranks, and progression in the quest save (`ProgressionJson` / `RestoreProgression`).
- **Checks.** `OpenWillow.Skills` gains respec and cost checks (UE automation; run by the hub's test runner).

## 2026-11-03: Skill effects in the host: Maya's skill modifiers from the stock SkillDefinitions (Skills lane PR 2)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILLS.md` section 4.8 (coverage table per stat and skill).

- **Data.** New `tools/prepare_skill_effects.py` writes ignored `local/ui/run/skilleffects_siren.json` (31 skills, 50
  effect rows, 34 constraints on the 2026-11-03 install); `UOpenWillowSkills::LoadTree` loads it beside the tree.
  Synthetic test: `python tests/skill_effects_test.py`.
- **Rules (behaviour change; native reads, UNVERIFIED in game).** `FOpenWillowSkillEffects` turns a trained skill into
  modifiers with the grade formula of NATIVE_SKILLS 4.3 (start grade, integer-divided interval, best bonus entry) and
  combines them with the stack formula of NATIVE_WEAPON_RULES 1. Passive skills need a point; ActionAugment skills count
  while Phaselock runs; kill skills not yet. Constraints are evaluated as data (attribute comparisons, And/Or, on foot,
  healthy); an attribute the host cannot answer makes its comparison false, as the native expression does.
- **API.** `OpenWillowSkillEffects::Stack / Apply / EffectValue / IsLoaded` (`OpenWillowSkillEffects.h`). One-line hooks
  in `OpenWillowWalker.cpp`: shot damage (Accelerate, Wreck), fire interval (Wreck), reload time (Foresight), Phaselock
  lock time (Suspension, Thoughtlock) and cooldown drain rate (Quicken). Shields, crits, status chance, clip size and
  max health are not hooked (no host stat, or another lane's code); listed in the record.
- **Checks.** New UE automation test `OpenWillow.SkillEffects` (invented manifest): grade formula, interval and bonus,
  scale-up/scale-down combination, conditions, augments, kill skills off, respec clears modifiers.


## 2026-11-03: Maya's behaviour skills, first set: kill-skill timer, Helios, Converge, Ruin, Sweet Release, regeneration (Skills lane PR 3)

AI-assisted (Claude; analyst sub-agent read the behaviour graphs and script, own-words note). Record:
`docs/verification/NATIVE_SKILL_BEHAVIOURS.md` (rules with confidence per skill; section 7 is the host status).
Nothing here was seen in the running game; native bodies were not read.

- **Kill skills (behaviour change).** A kill restarts a 7 s timer (KillSkillDurationsPerPlayers, one player); Kill skills'
  modifiers count while it runs. Refresh rather than add is a guess. Hook: one line in `NotifyKilledEnemy`.
- **Phaselock variants (behaviour change).** Helios, Converge and Ruin blasts on the lifted target at the graph's times,
  radii and damage formulas (PlayerSkillDamage = 8 x 1.13^level, PlayerSkillStatusEffectDamage = 1.13^level); Sweet
  Release heals 15 % of missing health per orb when the held target dies. Status effects, Converge's pull, the orb
  actors and the blocked-target chain are not done. The Phaselock code is read, not hooked.
- **Regeneration.** `tools/prepare_skill_effects.py` now writes balance formulas (Sustenance: 0.004 x grade x missing
  health) and Allies-with-self rows (Elated); the host heals Maya by `HealthPassiveRegenerationRate` per second. New
  `AOpenWillowWalker::Heal` (one header line).
- **Corrections found** (recorded in the note, section 5): the blocked-target damage is 10 x PlayerMeleeDamage (5 x for a
  player); `ActionSkill_Phaselock` sets `bAllowAutomaticActiveAbilityActivation`; `Att_EnemyIsPhaseLocked` comes from the
  cooldown-manager skill (selection to release). Read from data, UNVERIFIED in game.

## 2026-11-03: Maya's hit-driven skills: Life Tap, Cloud Kill, Immolate, Blight Phoenix (Skills lane PR 4)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md` section 7. Behaviour change, from the
behaviour graphs and effect data; UNVERIFIED in game.

- `UOpenWillowSkills` listens to `OpenWillowHit::OnHit()` (PR #75's hit events) for Maya's gun hits, so no other lane's
  file changes. Life Tap heals 0.012 x grade of the applied damage while the kill-skill timer runs; Cloud Kill leaves a
  5 s corrosive cloud (0.25 s ticks, radius 220, 0.25 x PlayerSkillDamage) with a 5.5 s cooldown; Immolate adds
  0.1 x grade x the hit as fire damage while Maya is down; Blight Phoenix pulses corrosive and fire around Maya every
  0.5 s while the kill-skill timer runs (radius 512, 0.02 x grade x PlayerMeleeDamage, melee buff taken as 0).
- Skill hits go through `ApplyShotDamage` with weapon id `skill:<name>`; the listener ignores them and status ticks.

## 2026-11-03: Recompense and Mind's Eye in the host (Skills lane PR 5)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md` section 7. Behaviour change; read from the
behaviour graph and effect data, UNVERIFIED in game.

- **Recompense.** When an enemy takes health from Maya, with chance 0.1 x grade the attacker takes the same damage back
  (DmgType_Normal, non-critical skill hit); not while Maya is down. Hook (hub-approved): one line in
  `AOpenWillowWalker::ApplyDamageToPlayer` (health damage only, after the shield); the reflected hit goes to the attacker only, never to Maya.
- **Mind's Eye.** For gun hits that can crit, the shared hit dispatcher multiplies the event's crit bonus (base 2.0, live)
  by `PlayerCriticalHitBonus` / 2.0 = 1 + 0.05 x grade, cached until a grade changes; skill hits are not scaled. Hook
  (hub-approved): one line in `AOpenWillowWalker::ApplyShotDamage`. It has no effect until a target applies the bonus
  (range PR #76 adds that for head hits).
- Recompense rolls from the skills component's own seeded random stream (`-owskillseed=<n>`), so tests replay.
- Open, recorded: Sub-Sequence, Thoughtlock, Res, Chain Reaction, Kinetic Reflection, Reaper, Scorn, Backdraft.

## 2026-11-04: Skill modifiers on Maya's survival stats through FOpenWillowMayaState::Effective (Skills lane PR 6)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILLS.md` section 4.8 "Survival stats". Behaviour change;
modifier rules are native reads, UNVERIFIED in game.

- `FOpenWillowMayaState` (maya lane's file) gains a `Modifiers` callback that `Effective()` applies after `Base`; the
  skills component installs it at BeginPlay and clears it at EndPlay. It sets max health (Restoration), health
  regeneration (Sustenance, Elated), shield capacity and recharge delay (Ward) and ground speed (Fleet) with the
  modifier stack formula. Evaluated on every read (the game refreshes every 0.5 s).
- Condition and formula attributes read the pools' current values and their `Base` maxima, so Fleet's shield-empty
  constraint now resolves, and nothing recurses into `Effective()`. Regeneration moved from the skills tick (`Heal`) to
  the pool's own `HealthRegenRate`.
- Review fixes: formulas on max health (Sustenance's missing health, Elated) read the effective maximum just computed;
  `Effective()` is computed once per frame (and on a `Base` change or a grade change), so reads within a frame agree; a
  level-up refill and a full new shield fill to the effective maxima; `Tick` clamps health and shield when a maximum
  drops (host rule, UNVERIFIED as stock behaviour). New `OpenWillow.MayaState` cases.

## 2026-11-04: skill hits carry Source = Skill; Cloud Kill and Immolate on bullet or rocket hits (Skills lane, after PR #88)

AI-assisted (Claude). With the hit-event `Source` field on main (PR #88), every skill hit (blasts, Recompense,
Immolate's extra hit) sets `Source = Skill`, the hit listener ignores Skill and Status sources, and Cloud Kill and
Immolate proc on `Source` Bullet or Rocket, the game's two damage sources for them (a rocket's explosion now counts; a
bullet's impact explosion and a Tediore throw do not). Record: `NATIVE_SKILL_BEHAVIOURS.md` section 7. UNVERIFIED in game.

## 2026-11-04: E takes the aimed pickup; no status page over an open shop (inventory lane)

AI-assisted (Claude). Records: `docs/verification/PICKUP_CARD.md`, `MAP_SCREEN.md`. Host behaviour change; UNVERIFIED in game.

- `PickupNearby` takes the pickup the world pickup card shows (`OpenWillowPickupCard::FindAimedPickup`: reach 220 uu, a 12-degree
  cone, line of sight) before the nearest-target rule for vending machines, doors and pickups (vendors #81). Decoy capture:
  the aimed gun is taken, the nearer un-aimed one stays. The nearest-pickup search uses the same sight test, so E takes nothing
  through a wall.
- The status pages (Inventory, Skills, Missions, Map) and the Walker's legacy inventory widget do not open while a vending shop
  is open: forced over a shop, closing the page left the shop without input or cursor.
- The quest's on-screen tracker text is not posted while a vending shop is open either (it already hid behind the status pages).

## 2026-11-03: a gun range outside Sanctuary with static dummies and moving enemies (range lane, PR1)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md).
- **New host feature, no parsing change.** `ow.Range` builds a test area and teleports Maya there: three Fire-mission stock
  dummies (engine shapes without the slice data) and moving enemies (`AOpenWillowEnemy`, a subclass of the combat target so
  every hit path reaches it) with a patrol. Toggles `ow.RangeMode normal|invulnerable|passive|both`, `ow.RangeLevel`.
- **Placement is a run-time scout** (rings 6,000 to 14,000 uu around the plaza; flat, open, near the plaza's height), because
  this lane cannot look at the map; a spot found good is pinned in `local/range/range.json`. Importing another map (Three Horns
  Divide) for the range stays a maintainer decision.
- **Stock numbers:** enemy max health per level from the VM's spawn of the stock pawn (`tools/prepare_range.py`, ignored
  manifest), critical x2.0 (class default). Host stand-ins: the head sphere as the critical region, shield-first order on enemies,
  ragdoll after 0.84 s, 8 s respawn, the health bar and the orange critical number. All UNVERIFIED in game.
- **AOpenWillowCombatTarget:** additions only, off by default (invulnerable flag, respawn of a stock pawn with ragdoll, damage and
  death hooks); the Fire dummy, the firing test and the Phaselock paths keep their old behaviour.
- **Open:** the VM's adult L5 maximum (614.08) against the recorded fight's first health (360.876); bandit and psycho not
  spawnable in the VM yet; AI (PR2) and loot (PR3) to follow.

## 2026-11-03: range enemy AI: notice, approach and melee from the live records (range lane, PR2)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md) "Enemy AI (PR2)".
- **New host behaviour, no parsing change.** Range enemies share one threat record (the range is one den). They notice
  Maya by sight (previous update's line of sight, front half-plane, within the den's ThreatRadius) or by proximity (768 minus
  the collision radius, within the PatrolRadius), and are provoked by her damage. ForgetTime is +10 s / +25 s, with the fall-back
  at the den's 1 s update. They run at her and swing: damage 0.41 s after the swing start if she is still in the fitted melee
  volume, one swing per 1.25 s. The rules come from AI_AWARENESS_LIVE and AI_MELEE_TARGETING_LIVE; the host's reproduction
  is UNVERIFIED in game.
- **Melee damage from the VM:** `tools/prepare_range.py` now also runs `maya:L spawn:<kind>:L swing:<n>:melee` per kind and
  level; its arrival and ratio rows reproduce every live melee row of COMBAT_MELEE (adult L5/6/7, regular L6, baby L6). The
  level scale beyond two levels above Maya is a stand-in line to 3.0.
- **Delivery to Maya** is one stub (`RangeDamagePlayer`, UE ApplyDamage) until the maya lane's
  `OpenWillowApplyDamageToPlayer` (PR #72) merges.
- **Not modelled:** momentum on Maya, leaps, the slinger's throw, the stock AI tree.

## 2026-11-04: range loot on death from the VM's death roll (range lane, PR3)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md) "Loot on death (PR3)".
- **New host behaviour, no parsing change.** `tools/prepare_range.py` kills each spawned pawn in the VM and keeps its death
  roll (pools and their evaluated chances per kind and level; they equal COMBAT_LOOT's table). The host rolls those chances in
  COMBAT_LOOT's order from a seeded stream. Weapon pools roll a gun with the weapon generator (`ow.RollGun`'s roll) and drop it
  through the inventory pickup's existing API. Money and ammo drop the range's own touch pickups. `ow.RangeLoot normal|always|off`.
- **Stand-ins (UNVERIFIED):** the money value per item (`round(4 x 1.13^level)`; the stock value is not decoded), the ammo rule
  (only below half the reserve, a magazine's worth), a pool pick that is not a gun drops nothing. Skins, eridium and health are
  not hosted.


## 2026-11-06: Phaselock round 24: the hand orb's added layers read as gamma-space colours (maya lane)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 23 results" and "Round 24". Presentation change.

- Round 23 scored 5.9 / 10 as a best-per-time mix of two facings (round 22 rescored 5.7); target 8 not reached. The target's fur now reads
  tan and brown. The orb's blue-constant conversion had no measurable effect (host 40 px mean 159, 208, 251 to 158, 208, 251; the game's orb is markedly darker in red and green).
- With the tone curve off the orb reads 10, 158, 253: most of its paleness is the film curve on an over-bright blue, not its own colour.
- The hand orb's ribbons, swirl and star-burst read their colour as gamma-space values (a `GammaOut` material parameter, set by the hand
  template's emitters only). UNVERIFIED until the capture, which also retests the tone curve off on the corrected target.
- Facing stays open: critics prefer 0 for the held frames, the default stays 180 as a choice, and the live probe now records the target's rotation.
- Result (last critic round, each setting scored on its own): offset 0 about 5.7, the default offset 180 5.1, tone curve off 5.3; target 8 not reached.
  `GammaOut` showed no gain over round 23.

## 2026-11-06: how the slice gate grades NOT_AVAILABLE and the pickup links (slice-gate lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_GATE.md`. Host behaviour only; a PASS says the host did this in a scripted run with named fixtures, not that it matches the game.

- **NOT_AVAILABLE is for a missing capability or precondition, never a miss.** `enemy_attacks_maya` in range mode, `loot_drop_on_kill` with the gun roll forced and a pickup link whose ids show an attempt are FAILs, in host and checker.
- **"Test setup gap" is its own case:** no roll source or no generator manifest means the link did not test the host. It stays NOT_AVAILABLE, owned by slice-gate and counted on its own line. The earlier "no ow.RollGun source" with a real drop was this lane's driver not reading the source on that path.
- **The checker reads the pickup ids itself:** aimed drop, card and picked-up item must be one item with an accepted pickup, and the decoy a different item.

## 2026-11-06: the Phaselock targetable-list rule applies only to the mission's dummy (range lane)

AI-assisted (Claude). The slice gate's `phaselock_enemy` link failed: `PreferredPhaselockTarget` skipped every stock pawn the quest had not registered whenever the quest was on
(written for the mission's dummy, 2026-10-02), so with `-owquest` and `ow.Range` no range pawn could be targeted. The rule now covers only the mission's dummy, the quest's one stock
pawn by reading the code, which keeps it. Other targets count as registered, as they already did with the quest off (host stand-in, UNVERIFIED in game). Host behaviour only;
no parsing change. Record: docs/verification/COMBAT_RANGE.md "Phaselock on range pawns".


## 2026-11-06: the Phaselock stock colours' source, GammaOut removed (maya lane)

AI-assisted (Claude), maintainer's decision. Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "The stock colour constants".

- **Option C:** `tools/build_phaselock_fx_constants.py` rebuilds the orb's blue and the tattoo glow from regenerable local data; the bubble twirls' violet stays
  hand-kept in the ignored `fx_constants.json`, because it exists only as a literal in the compiled shader. Without an existing value the tool stops. A tracked shader-map reader (option B) comes after the slice.
- **`GammaOut` removed:** round 24 showed no gain.
- **Probe:** the live read also records the target's mesh scale, draw scale and bounds radius; its rotation stays the priority.
- **Live read (one cast, one pawn):** the lifted target faces Maya, its rotation does not change, and its scale is 1.0. The default facing stays +180 (hub decision). The "too large" gap is neither scale nor a bubble value; its cause is open.

## 2026-11-06: Mind's Eye crit checked on a range enemy (Skills lane, after range landed)

AI-assisted (Claude). The range suite gains `crit_minds_eye`: a gun hit on an enemy's head through
`AOpenWillowWalker::ApplyShotDamage` does 2.0 x its damage untrained and 2.5 x at Mind's Eye 5; progression is restored in the
same tick. It may SKIP without the local skill effects file. This checks the host's rule (the event's bonus scaled by the
skill), not the game's crit damage, which stays UNVERIFIED until the live session. Test only; no behaviour change.

## 2026-11-06: the Phaselock target wears the Adult's six attachments, placed from a live read (maya lane)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "The Adult's worn attachments". Host and tool change, no parsing change.

- The tool reads the default-on static meshes from the Adult's body composition, the editor step imports them with the body's zone material, and the host attaches them to their bones. Without the local import the host keeps the bare body.
- The list and the body's material are confirmed live on one Adult (one cast). The pieces' own material is archetype data.
- Placement: the live bone matrices show the imported bone frames are the live ones, so each piece uses the component's own transform and not the citizens' correction. A piece's world matrix could not be read, so the composition is assumed. UNVERIFIED until the import runs.
- The attachments do not explain the width gap. The default facing stays yaw +180.
## 2026-11-06: Ruin and Helios damage from the live game; Mind's Eye joins the crit stack's Scale sum (Skills lane)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md`, "Third run". Read from the running game with bl-sdk hooks on
one level-8 Maya: every formula here is checked at level 8 only. Raw reads stay under ignored `local/realgame/`.

- **Behaviour change, corrected from a contradicted data reading.** Ruin's pulses do 1 x PlayerSkillDamage (the note had 7.2 x
  PlayerSkillStatusEffectDamage). Helios does 0.29 x grade x PlayerSkillDamage, linear at grades 1 and 5 (the note had 0.2). The factor 0.29
  is fitted to the live read and its reason is not known. The timing of both was confirmed.
- **Behaviour change, Mind's Eye.** Live, the crit bonus is (base 2.0 + PreAdd) x (1 + Scale sum), and the skill adds Scale 0.05 per grade:
  this fits the reads at grades 1, 3 and 5 to 4 decimals. The hook now adds the skill's part (2.0 x 0.05 x grade) to the
  event's bonus instead of multiplying it. With no other modifiers in the host, the result is unchanged: 2.5 at grade 5. The stack's
  other modifiers are not identified or modelled.
- **Unchanged and UNVERIFIED: the 7 s kill-skill timer.** In two runs, two gun kills with Life Tap 5 left the game's timer at 0 and no
  `UpdateKillSkills` call was recorded; the targets had been wounded by a direct `TakeDamage` first, so this is no verdict.
- **Not modelled yet, seen live:** status damage ticks about every 0.33 s; the target's resistances and scales change the health lost.
- **Tools.** The capture takes `-Steps`, runs the timer first with unwounded, controlled targets, and lowers Maya's health for Sweet
  Release's heal; `python tests/skills_live_probe_test.py`.

## 2026-11-06: skills live capture: verified backup and restored grades before the next run (Skills lane, #116 review)

AI-assisted (Claude). Tool safety only; no game behaviour. Every root is copied and hash-compared before the driver or the game starts; any
failure stops the run there and that root is never restored from. A root created during the run stays in place and makes the summary read
NOT IDENTICAL, naming it. A grade whose original cannot be read is not changed, a restore is read back, and a failed restore stops the later
steps. The capture also reads the running skills (`SkillEffectManager.ActiveSkills`), since a tree grade back at 0 can leave a skill running:
it deactivates that skill, a disagreement counts as not restored, and each step records whether another step's skill could be in its reading.
The session counts as complete, and exits 0, only when every requested step passed with no such doubt and cleanup is clean.
`python tests/skills_live_probe_test.py`. This is the smallest guard: the snapshot helpers from the Maya lane can replace it once they land.
## 2026-11-06: Maya throws the stock grenade mod on G (slice projectiles lane)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` sections 12 and 13. Host behaviour change and a new data
tool. **The stock frag has not been thrown live:** the one session's save carries the Sky Rocket mod, so its release delay, lockout
and spawn offset are used as UNVERIFIED stand-ins for the frag, and none of its fuse, damage or radius is recorded as frag values.

- `tools/grenade_mod.py` resolves the stock grenade mod's speed, gravity, bounces, radius, damage and explosion into ignored
  `local/items/grenades/`. The fuse is not settled by the data: the part's slot gives 0 and the projectile holds two timers.
- The host spends one grenade from the pool and launches a falling projectile with those values (hit source `Grenade`). The
  launch point, the lockout between throws, the falloff and the missing animation are stand-ins, labelled UNVERIFIED.
- The firing test throws one grenade at the dummy; the checker requires the result.
- **Status chance:** x1 at equal levels is consistent with a skills-off live run (8 of 42 against 23.4 %; small sample, different
  class). The mechanism behind the skills-on ~x2 stays UNVERIFIED (it is not in the five controller modifier fields), and the
  higher-level scale is unmeasured.


## 2026-11-06: The frag's contact, fuse and falloff come from its behaviour graph

AI-assisted (Claude). `tools/grenade_mod.py` now reads the projectile's behaviour graph by event, because counting behaviours gave the
wrong grenade: it reported two bounces and an impact detonation. The graph says a contact only bounces (0.4 off the world, 0.2 off a
damagable object), the first contact starts a 1.8 s timer once, the spawn starts a 6 s timer, a timer detonates, and the explosion has no
falloff. The host follows that and refuses a block without these rules.

Verified live (one session, 4 throws of the frag's definition with the Sky Rocket mod still equipped): no contact exploded, the explosion
came 1.80-1.88 s after the first contact (frame-sampled, consistent with but not separately proving the start at contact), bounces lost
about the graph's speed, gravity read 0.8 x 1000, and pawns at several distances took the same damage. 63.28 at `InExpLevel` 1 matches the
damage formula at level 1 only. `UNVERIFIED`: a throw with the stock mod equipped (level source, the -120 radius slot, so the block's level 8
damage and radius), the 6 s timer, the FuseTime slot's effect, and what the first contact touched. The 20 uu/s rest limit is a host stand-in.
Host suite results: pending the next integration batch. Details: `docs/verification/WEAPON_PROJECTILES.md` section 14.
## 2026-11-06: M opens the Map and L the Missions during play (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md). Host behaviour change, no parsing
change. The keys come from the stock keyboard table (`GD_Input.Devices.InputDevice_Keyboard`, OpenBLCMM dump), not from a press in
the running game: UNVERIFIED in game, as is what the stock menu refuses (`CanShowModalMenu` not read). The keys reuse the HUD's
existing page toggles, so a vending shop still refuses them. A passing `tools/test_controls.ps1` means the host did this.

## 2026-11-06: V swings Maya's melee, through the shots' damage path (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md), "Melee". Host behaviour change, no parsing
change. Numbers come from the game's data and script, not from a live swing: UNVERIFIED in game. The first-person clip is not imported
yet, so a swing lands with no visible arm for now. A passing `tools/test_controls.ps1` means the host did this.

## 2026-11-06: the HUD shows a use prompt for the use target (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md), "Use prompt". Host behaviour change, no
parsing change, and no change to what the use key does. It drives the movie's own prompt clip, so the placement is the movie's. The icon
per target and the key's form are host choices: UNVERIFIED in game until the real-game read.

## 2026-11-07: controls follow the first real-game read: 0.70 s swing, a page does not cancel it, no pages while down (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md). One live session with few samples, stated per claim there. Host behaviour change, no
parsing change. A swing holds the weapons for 0.70 s and its clip is looked for in the Siren's set. A status page no longer cancels a swing, as the one read showed;
a hit due after the page opened lands under it in the host, UNVERIFIED because the game pauses there. M and L refuse while Maya is down. M no longer closes the Map page and
goes to the Map from the Missions page. A passing `tools/test_controls.ps1` means the host did this.

## 2026-11-07: the second real-game read: the stock use prompt, and keys on open pages (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md). Host change, no parsing change. L stays on the Missions page and M and L route from
Skills, as read live; a page cannot route to the Map or Missions with Maya down. The prompt's frames and words for a shop, a pawn, a use machine and a chest held. The melee damage
rule and knockback held on one hit on a box. The read supports the view-ray note against the host's nearest-target rule; that rule is unchanged here and stays open.
## 2026-11-06: the host can play PCM WAV sounds; the Wwise decoder stays a maintainer decision (slice audio lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 5.

- **Added:** `OpenWillowAudio` plays role -> WAV through UE's `USoundWaveProcedural`, and `tools/prepare_slice_audio.py` writes the role map after a format and length check.
  One hook line each at the shot, the successful Phaselock lift and an item pickup; hits use the existing `OnHit` delegate. No new module, library or tool. Off unless `-owaudioplay=` is given.
- **Not decided:** the decoder. Nothing recorded in `THIRD_PARTY.md` decodes Wwise Vorbis, so the options went to the maintainer and no decoder was downloaded. Until then no game sound plays.
- **Evidence:** `fire` comes from the lent pistol's data and is played for every gun for now. `hit`, `phaselock_cast` and `pickup` have no sound assigned: they stay unresolved rather than guessed.
- **Not verified:** nothing has been heard. The synthetic tone test can only show the mixer pulled the bytes. No parsing behaviour changed.

## 2026-11-07: two sound roles from a live session, marked UNVERIFIED (slice audio lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 5; the session is in `docs/verification/SLICE_AUDIO_LIVE.md` on `feature/slice-audio-capture`.

- **Added:** manifest entries `role:phaselock_cast` and `role:hit`, and the prepare tool maps them. `pickup` stays unresolved: the session did not exercise it. `fire` still comes from the lent pistol's data, because the session's hooks saw no fire event.
- **Limit:** `hit` is a surface sound, not shown to be the enemy, with 96 sources. The host picks at random, so a hit can sound like any surface until the switch value is read.
- **Not stock:** one session on a modded install, Dialog Skipper included. No role here comes from a dialog event.
- **Not verified:** nothing has been heard, and no game sound can play until a decoder is approved. No parsing behaviour changed.
## 2026-11-06: no camera motion blur in the host (mission UI lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_VISUALS.md` section 1. `DefaultEngine.ini` sets `r.DefaultFeature.MotionBlur=False`; the engine's source documents 0 as "sets MotionBlurAmount to 0", and no
camera or volume in the host's code overrides that amount. The installed game's config has `MotionBlur=False`, a config read only. Host rendering only; no parsing change. In-engine frame: UNVERIFIED until the batch runs it.
## 2026-11-06: A missing overlay movie no longer covers the game (inventory lane)

AI-assisted (Claude). Record: `docs/verification/PICKUP_CARD.md`. Found by the scripted playtest on pc-hummer.

- With its movie absent, Ruffle draws an opaque error page over its whole player and `load()` still resolves. The pickup card's player fills
  the HUD, so aiming at a drop covered the game until the end of the run.
- `tools/hud_overlay/movie_guard.js` checks a page's movie files and their imports with HEAD before Ruffle sees them, and removes a player that
  shows the panel at any time in its life. Only a 404 counts as missing; other failures are asked again a few times. The HUD, Inventory, Skills,
  Missions, Map and pickup card use it; vending already had its own check. Each page shows its existing message or nothing, and logs
  `OpenWillow movie missing`. A HUD movie lost, before or after ready, sends the host back to its canvas HUD for good (`UNVERIFIED` in engine: synthetic tests only). The card asks again later.
- `tools/check_overlay_data.py` lists missing movies; the run script and the inventory suite print it, and stop only for the movie they
  cannot do without. Tested with synthetic fixtures and with real Ruffle in headless Edge (movie absent and present). Not yet run in engine.

## 2026-11-06: Marcus's mission screen is a page, and the host owns its keys (mission UI lane)

AI-assisted (Claude). Record: `docs/verification/MISSION_SCREEN.md`. With a page URL, E at Marcus now opens BL2's own movie under Ruffle (`FOpenWillowMissionPage`) instead of confirming by itself: Enter confirms on the offer, the turn-in and the reward
page, Escape leaves the first two, and E does nothing there. The accept and the reward page's order were observed in the live trace; the key rules are read from the installed script and stay UNVERIFIED until the live read. The host takes the keys itself so a page cannot
trap the player, and a missing movie gives a plain panel. Without a page URL or a ready page the old path runs, so `tools/test_quest.ps1` and the slice gate keep their flow. Host behaviour only; no parsing change.

## 2026-11-06: a shield without a capacity stat takes a level-scaled stand-in when equipped (maya lane)

AI-assisted (Claude). Record: `docs/verification/COMBAT_DOWN_RESPAWN.md` "Stand-in shield capacity". Host behaviour, no parsing change.

- Vendor and loot shields have no stats, so they gave Maya no protection. The walker now gives such a shield a capacity from a rule whose numbers are in ignored local data; a card's own capacity wins, and without the file nothing changes but a warning.
- The rule is UNVERIFIED, a stand-in until a live read of capacity against level. No vendor code changed.

## 2026-11-06: the psycho as a range enemy, melee numbers VM only (range lane)

AI-assisted (Claude). Record: docs/verification/COMBAT_RANGE.md "The psycho". `tools/prepare_range.py` adds the psycho kind (health, melee and loot chances per level from the VM's
spawn of the stock pawn) and `tools/prepare_phaselock_target.py` an import target for its mesh and clips. It joins the range's spawns only when its own mesh is imported, last in the list.
Its melee numbers have no live row and are labelled VM only, UNVERIFIED in the manifest; its collision cylinder and scale are the archetype's (no live read); its death clip is a choice
among what the set holds. It uses the range's brain, a host stand-in for its stock AI tree. No marauder or nomad: they shoot, and the brain only melees (a separate lane). New test step
`psycho_melee`, which may SKIP without the imported mesh. No parsing change.

## 2026-11-06: the range's fight after the first playtest: swings start at the stock range, the den starts unalerted, corpses limited (range lane)

AI-assisted (Claude). Record: docs/verification/COMBAT_RANGE.md "The first playtest's fight". Host and tool change, no parsing change. Automated checks only so far; not yet run in the
host, and no rule here is confirmed in the running game.

- A swing now starts inside the kind's stock attack range (`melee_start_range`, read by `tools/prepare_range.py` from the AI definition's melee actions); the fitted 640 volume only decides
  whether it lands. How that range is measured is UNVERIFIED. The host builds no range on a manifest without it, or with no manifest at all (the old engine-shape fallback for that case is gone), whichever tool started it, and any failed build leaves a reason on screen and for the slice gate.
- Critical hits from behind: the check now reaches the head from any side, and the shot's line must still cross the head sphere. A host rule, UNVERIFIED.
- Worn pieces are exported from a package that really holds them; a piece naming no material keeps what its import makes (the psycho's axe, UNVERIFIED look).
- Range enemies spawn facing away from the spot, so arriving does not trip the sight rule at once. A host layout, not a game rule. This addresses the first-update lockstep; it does not stagger the fight: once the den is alerted, members can still converge and land together. Damage and run speed unchanged; no stagger number invented.
- Ragdoll: depenetration limited, rise and sideways speed capped, each corpse measured and logged. Host stand-ins; the cause of the flung corpses is still UNVERIFIED.
- One way in: `tools/play_range.ps1` and `-owrangestart`, Sanctuary only. The name is not `-owrange`, which already passes the manifest path.
- `tools/real_game/cash_capture.ps1` gains a `den_read` stage with its own result, after the cash read. It has not run.

## 2026-11-07: the store's global fallback uses an index; the resolution order is unchanged (mission UI lane)

AI-assisted (Claude), design corrected by the hub's review. Record: `docs/verification/PACKAGE_GLOBAL_INDEX.md`. `PackageStore`'s fallback for an import whose root is not a file scanned the tree per path, 13 to 15 s each, and caused the first E's freeze. One pass now maps a path's first segment to the ordered files that could hold it, and a lookup runs the same `findExport` over those, in the same order, so the first match is the same file. **Parsing behaviour is meant to be unchanged:** no bounds check touched. Checked on the Fire step list with `OPENWILLOW_RESOLVE_CHECK`, which also runs the old scan: 5 lookups, 5 the same; `--slice-run` output identical before and after. That covers these runs' lookups, not every path; the argument for the rest is in the record. First use about 27.6 s to 5.5 s, setup about 4 s slower, core CLI on one PC; not measured in UE.
