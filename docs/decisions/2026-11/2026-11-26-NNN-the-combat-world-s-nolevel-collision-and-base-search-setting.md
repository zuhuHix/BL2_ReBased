## 2026-11-26: The combat world's NoLevel collision, base-search setting and spawn positions

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COMBAT-WORLD-COLLISION, a harness lane on the hub's rulings. The branch
is stacked on #804 @ 88191026, itself on #803 @ dc0e9ef9 and main 21a668a7. No native code was read. **Every setting
here is an UNVERIFIED stand-in.** Reference is unchanged.

- **What was decided (Off only, the slice gate's combat set-up without `--map`):**
  - the world installs link 9's `NoLevel` collision service, logged as a set-up harness input
    (`slice_gate::COMBAT_COLLISION_SETUP`);
  - it turns on #798's base-search setting (`set_physics::enable_no_level_base_search`), which logs its own line;
  - **the spawn positions** (the hub's ruling on the lane's open question): `ow_combat::spawn_positions`, turned on
    there too, writes the n-th spawned pawn's `Location` as (1000 n, 0, 0), right after the copy and before the rest of
    the spawn, logging the set-up line "the combat run's spawn positions, apart from every other actor (stand-in for
    the population spawn points; UNVERIFIED)" and one line per placed pawn. The spacing of 1000 is far larger than
    every collision cylinder in the slice's combat world: Maya's has radius 42 and half-height 80, the AI pawns'
    radius 64 and half-height 48, and radius 150 and half-height 90;
  - **the base search's clearance by geometry** (needed for the ruling to have its effect): #798's stand-in refused
    whenever any other live actor held a `CollisionComponent`, wherever it stood. Another actor now no longer blocks
    when its component is an exact `Engine.CylinderComponent` whose box the bounding-cylinder rule answers, and the
    8-unit line down from the actor's `Location` runs more than the radius from its axis, or wholly above or below it.
    Touching counts as a hit. This is geometry alone, so it holds whatever the trace's flags are. Any other component,
    or an actor whose own collision component holds a translation (the note fits the start offset to it), keeps the
    trace flags' `NotRead`. The stand-in's logged line now says "no other actor's collision cylinder within the line's
    reach";
  - all three settings are in `slice_gate::combat_world_collision`, which refuses in Reference before writing
    anything. The `--map` route gets none of them, because it loads a level and has its own spawn points;
  - two harness strings that had lost a continuation backslash are single-spaced again: the base-search set-up line
    (`set_physics.rs`) and the trace-miss set-up line (`slice_gate.rs`); a test keeps the set-up lines so.
- **Verified (host only):**
  - `ow-world` set_physics (1 new): a cylinder at the origin, at 30 and at 42 units, and overlapping from below or
    above refuses; at 43 units, at 1000, and wholly above or below clears; an Absolute flag, the subject's own
    translated component, and a non-cylinder component refuse; nothing is written on a refusal;
  - `ow-combat` spawn_positions (2): off by default; the n-th pawn at (1000 n, 0, 0) with its lines; a second enable
    and Reference refused; a class without `Location` a script error;
  - `ow-cli` (2): the three settings in order with their lines, refused twice and in Reference; single-spaced lines;
  - the installed Off strict `--slice-gate` (release build, under the machine lock), base 7b9daea9 against this head:
    both **7 ok / 14, X = 8.167/14 = 58.3%**. Links 3, 4, 5, 13 and 14 now get past the floor trace (each spawn placed,
    at 1000 and 2000, and the stand-in logged) and stop at the spawn's
    `AttributeInitializationDefinition.SetBaseValue`: "a definition it names is not loaded (a load is a write)".
    Every other row is as on the base;
  - before the spawn positions (this lane's first commit), links 3, 4, 5, 13 and 14 moved from the floor trace's
    `LayerAbsent` (no collision service) to its `NotRead`. A local probe (reverted) showed why: every actor stood at
    the origin, and the AI pawn's trace started inside Maya's cylinder.
- **Not in this change:** the floor trace's flags (lane F, queue 0ar), against which the geometric clearance can be
  checked; the definition the spawn's `SetBaseValue` names.
- **UNVERIFIED:** whether the game's base search would hit another actor's cylinder; the spawn positions stand in for
  the population spawn points and are not observations of the game.
