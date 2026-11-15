## 2026-11-15: Rust balance rules: XP, stages, caps and the offline world

AI-assisted (Claude Code). Lane W3-BALANCE of the Rust port fills `crates/ow-balance/src/{curve,stage,cap,playthrough,xp,offline}.rs`
(architecture section 3.6). No parsing behaviour changes. Every amount goes through the one evaluator (`ow-attr`)
from the data; no closed form is coded. This file records the choices a later lane should be able to find.

- **Additive contract change:** `BalanceError::Attr(AttrError)` (listed in section 3.6, anticipated by
  `types.rs`), with `From<AttrError>` (a `NotYet` stays `NotYet`) and `source()`. That is the one edit outside the
  lane's own files.
- **XP curve.** `R(n) = max(0, trunc(f(n)) - trunc(f(1)))`, with the level put in global slot 0 and left there, as the
  game's routine leaves it. A NaN point or one outside the `i64` range is refused, not saturated.
- **Mission XP** goes further than the C++ oracle (`mission_script.cpp`), which refuses every playthrough but the
  first and any optional objective. Both are read in NATIVE_PROGRESSION.md section 2, so the port computes them:
  completed optional objectives add their own truncated terms, and the second playthrough below level 50 takes
  `m = 1.0`.
- **The XP multiplier helper** (`playthrough.rs`) answers 1.0 only in the first two playthroughs below level 50. UVHM,
  level 50 or more, and `playthrough_modifier` (which `PlayThroughBasedBalanceModifiers` entry serves a playthrough)
  are `NotYet { NotRead }`: the entry selection, the field types and the "level cap" input are open in the records.
- **Kill XP**: the award and one player's credit, as NATIVE_COMBAT_GROUNDWORK.md 5.1 and 5.2 read them, with the C++
  oracle's refusals kept (a vehicle victim, a killer in a vehicle, `DamageTypeExpModifiers` entries:
  `NotYet { NotPorted }`). The party weights for 2 to 4 players are read from the table as recorded (UNVERIFIED); the
  offline world itself refuses more than one player (D12).
- **Region stage.** It follows the record's step order. Four choices where the record is silent, each UNVERIFIED:
  - the table row is chosen by playthrough index, clamped (`tools/slice_values.py` matches `PlayThroughNumber`
    instead, which agrees on a table in playthrough order);
  - bounds and boosts are evaluated with the player controller as the source and truncated toward zero;
  - the clamp is UE3 `Clamp`;
  - a region no entry names keeps stage 1 and stores nothing.

  The stage is stored in `RegionStages`, per (region, playthrough), because the controller field is not read. It is
  never reset (question 14). UVHM is `NotYet { NeedsRealGame(2) }`, and a region with a DLC expansion is `NotRead`
  (the expansion's table field is not recorded). The awesome level is not read.
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
