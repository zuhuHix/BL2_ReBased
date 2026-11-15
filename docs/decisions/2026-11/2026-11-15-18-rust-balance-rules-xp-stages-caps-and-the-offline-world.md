## 2026-11-15: Rust balance rules: XP, stages, caps and the offline world

AI-assisted (Claude Code). Lane W3-BALANCE of the Rust port fills `crates/ow-balance/src/{curve,stage,cap,playthrough,xp,offline}.rs`
(architecture section 3.6). No parsing behaviour changes. Every amount goes through the one evaluator (`ow-attr`)
from the data; no closed form is coded. This file records the choices a later lane should be able to find.

- **Edits outside the lane's own files** (each granted by the hub):
  - `BalanceError::Attr(AttrError)` in `types.rs`, additive and listed in section 3.6, with `From<AttrError>` (a
    `NotYet` stays `NotYet`) and `source()`.
  - `ow-attr`'s definition readers in `eval.rs` are now `pub`, unchanged in behaviour: `ensure_loaded`,
    `class_path`, `property`, `required`, `field`, `number`, `flag`, `object`, `elements` and `init_data`.
    `ow-balance` uses them instead of a copy. Only `is_a` and an optional `field_opt` stay in `offline/data.rs`.
  - The `ow-balance` block of `docs/rust-port/ARCHITECTURE.md` section 3.6 now holds the shipped signatures.
- **The shipped signatures** differ from the revision 3 sketch:
  - `region_game_stage(sys, cx, globals_definition, &mut RegionStages, &RegionQuery) -> RegionStage`.
    `RegionQuery` holds the region, the playthrough, the local player pawn's level (`None`: the minimum is used),
    the completed missions and the controller. `RegionStage` is the stage with its awesome level
    (`Option<AwesomeLevel>`). `RegionStages` is the caller's store of `RegionStage`s, because the controller field
    the game stores them in is not read.
  - Kill XP is split. `kill_xp_award(sys, cx, globals_definition, &KillEvent)` returns the whole party's award as an
    `f32`. `KillEvent` carries the native's five arguments: killed, killer, critical, the damage source (a
    `ClassId`, since `DamageSourceClass` is a class reference) and the damage type definition (an `ObjRef`).
    `kill_xp(sys, cx, globals_definition, &KillCredit)` gives one player's credit.
  - `xp_multiplier(sys, cx, kind, pt, player_level)`. The evaluator and the world are in the signature for the
    UVHM and level-50 branches, which read `PlayThroughBasedBalanceModifiers` (fields that may be
    `AttributeInitializationData`). Those branches still answer `NotYet`.
  - `mission_xp(sys, cx, curve, &MissionXp)`.
  - The `GlobalsDefinition` object is passed in (`globals_definition`): which `WillowGlobals` property holds it is
    not recorded (the C++ finds `GD_Globals.General.Globals` by path).
- **Where the C++ oracle refuses but the records read the rule, the port implements the rule** (hub decision). Each
  case is UNVERIFIED in the game:
  - `src/mission_script.cpp` `experienceReward` throws "the playthrough experience multiplier is not implemented"
    for any playthrough other than the first. The port uses the record's `m = 1.0` for the second playthrough
    below level 50.
  - The same function stops on any optional objective (`notImplemented`). The port adds a truncated term for each
    completed optional objective (NATIVE_PROGRESSION.md section 2).

  Where the C++ refuses and the record reads the rule only at medium confidence, the refusal is kept as
  `NotYet { NotPorted }`: a vehicle victim, a killer in a vehicle, and `DamageTypeExpModifiers` entries.
- **`tools/slice_values.py` stays.** Section 6 says it is deleted at this gate, but other tools still use it. Its
  deletion is W7-RETIRE's call.
- **XP curve.** `R(n) = max(0, trunc(f(n)) - trunc(f(1)))`, with the level put in global slot 0 and left there, as the
  game's routine leaves it. A NaN point or one outside the `i64` range is refused, not saturated.
- **Mission XP**: `trunc(float(span x pct x m))` per term, with optional objectives and the second playthrough as
  above.
- **The XP multiplier helper** (`playthrough.rs`) answers 1.0 only in the first two playthroughs below level 50. UVHM,
  level 50 or more, and `playthrough_modifier` (which `PlayThroughBasedBalanceModifiers` entry serves a playthrough)
  are `NotYet { NotRead }`: the entry selection, the field types and the "level cap" input are open in the records.
- **Kill XP**: the award and one player's credit, as NATIVE_COMBAT_GROUNDWORK.md 5.1 and 5.2 read them, with the C++
  oracle's refusals kept (a vehicle victim, a killer in a vehicle, `DamageTypeExpModifiers` entries:
  `NotYet { NotPorted }`; the damage source and type reach the award, but the entries' match is not ported). The
  party weights for 2 to 4 players are read from the table as recorded (UNVERIFIED); the offline world itself
  refuses more than one player (D12). `PlayerCount` cannot hold the game info's class default 0, which the game
  treats as outside the table (bonus 0, weight 1). A caller that passes 1 for it gets the same share only because
  this data's first row is (0, 1): that mapping is the caller's choice, UNVERIFIED for any other table.
- **Region stage.** It follows the record's step order. Four choices where the record is silent, each UNVERIFIED:
  - the table row is chosen by playthrough index, clamped (`tools/slice_values.py` matches `PlayThroughNumber`
    instead, which agrees on a table in playthrough order);
  - bounds and boosts are evaluated with the player controller as the source and truncated toward zero;
  - the clamp is UE3 `Clamp`: with the bounds out of order (`min > max`), an `x` below `min` gives `min` and any
    other `x` gives `max`;
  - a region no entry names keeps stage 1 and stores nothing.

  The stage is stored in `RegionStages`, per (region, playthrough), because the controller field is not read. It is
  never reset (question 14). UVHM is `NotYet { NeedsRealGame(2) }`, and a region with a DLC expansion is `NotRead`
  (the expansion's table field is not recorded). **Awesome level:** the step-1 default (awesome level 1) comes back
  with the default stage. A computed stage carries `None`, because the types of `DefaultAwesomeLevel` and an
  override's `AwesomeLevel` are not read (open item).
- **Caps.**
  - `max_exp_level`: 50 plus the licensed increments (11, 11, 8), in every playthrough.
  - `item_stage_cap`: 50 outside UVHM; in UVHM with all three packs, 80 plus the OP level. UVHM with fewer packs is
    `NotYet { NotRead }` (the record leaves the intermediate values open).
- **Skill points**: `int(GeneralSkillPointsPerLevelUp)` after each increment, using the VM's float-to-int.
  `OfflineWorld::skill_points_earned` sums the level-ups.
- **OfflineWorld** builds the globals object, the game replication info (`CurrentPlaythrough`), the replication info
  (`ExpLevel`, `Owner`) and the player controller (`PlayerReplicationInfo`) from the space's classes. It answers
  `NotYet` for:
  - a game info (it answers `Ok(None)`);
  - engine queries other than the controller's own player controller;
  - change notifications;
  - a random source it was not given;
  - finding a definition without a store.
- **Rounding mode 1** (decision 2026-11-15-11's open item): no rule here rounds by itself. Rounding inside a definition
  is the evaluator's form (`floor(x + 0.5)`), where `progression.cpp` uses `std::round`; no test data here has a
  negative half.
- **Verified** against recorded numbers on synthetic worlds: the curve for levels 1..59 (both pow forms), the health
  base at 2/8/17/70, Fire XP 395 at stage 8, the six live kill rows of COMBAT_KILL_XP.md (the seventh repeats a
  row), and skill points `max(0, L - 4)`. **UNVERIFIED:** every other branch, and the real-package run (the `DefStore`
  body is not on main, so every lookup answers `NotYet`; open item for the gate).
- **Doc follow-up for the hub:** `curve.rs` keeps a local `trunc_i64` that refuses NaN and out-of-range values
  (one range-guarded `as`). `ow_core::num` has no refusing float-to-`i64` form; moving one there is the hub's
  change.
