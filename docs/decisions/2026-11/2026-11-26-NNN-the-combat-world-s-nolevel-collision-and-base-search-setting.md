## 2026-11-26: The combat world's NoLevel collision and base-search setting

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COMBAT-WORLD-COLLISION, a harness lane on the hub's ruling. The branch is
stacked on fix/rust-bounding-cylinder @ 47dc7bea, itself on main 92c87b4 through fix/rust-links-13-14. No native code
was read, and no rule of the game changes. **Both settings are UNVERIFIED stand-ins.** Reference is unchanged.

- **What was decided (Off only, the slice gate's combat set-up):**
  - the slice's combat world without `--map` installs link 9's `NoLevel` collision service, logged as a set-up harness
    input (`slice_gate::COMBAT_COLLISION_SETUP`);
  - it then turns on #798's base-search setting (`set_physics::enable_no_level_base_search`), which logs its own
    set-up line;
  - both are in `slice_gate::combat_world_collision`, which refuses in Reference before writing anything;
  - the `--map` route gets neither, because it loads a level, and "no level geometry" would be false there;
  - four harness strings had lost a continuation backslash and carried a run of spaces: the base-search set-up line
    (`set_physics.rs`), the trace-miss set-up line (`slice_gate.rs`), the adopt's `CharacterClass` line (`hit.rs`) and
    the player set-up's `Pawn` line (`run/player.rs`). They are single-spaced again, with no other change of text, and
    a test keeps the set-up lines so.
- **Verified (host only):**
  - two new `ow-cli` tests: the two settings in order with their lines, a second call refused, and Reference refused
    with nothing installed or logged; the set-up lines are single-spaced;
  - the installed Off strict `--slice-gate` (release build, under the machine lock): 7 ok / 14, X = 8.167/14 = 58.3%,
    the same score as the base 47dc7bea;
  - links 3, 4, 5, 13 and 14 move from the floor trace's `LayerAbsent` ("the world has no collision service") to its
    `NotRead` ("its trace flags are not decoded"). Every other row's stop is unchanged.
- **Why the links do not get past the floor trace (local probe, reverted):**
  - the stand-in of #798 gives no floor only when no other live actor holds collision (#798's R2 rule);
  - in both combat worlds (5 and 6 live actors), the AI pawn's search finds Maya's `WillowPlayerPawn` with its
    `CylinderComponent` set;
  - the AI pawns and Maya all stand at the origin, because the combat world has no map spawn. So the AI's 8-unit
    downward line starts inside Maya's cylinder, and a rule that only clears actors out of the line's reach would not
    clear it either;
  - getting past needs one of: the floor trace's flags (whether the trace can hit a pawn), or spawn positions that keep
    the actors apart. Both are for the hub.
- **UNVERIFIED:** whether the game's base search would hit Maya's cylinder here; both settings are harness inputs, not
  observations of the game.
