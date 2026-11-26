## 2026-11-26: the slice gate's combat rows carry the world set-up's harness lines

AI-assisted (Claude Code, Claude Opus 5.5). Lane GATE-SETUP-HARNESS-LINES, a harness fix on main 37a1f3e (first written on 08472592). No rule
changed, and no row's result changed.

**What was missing.** `set_physx_level` logs "harness input: the engine object's PhysXLevel = 0 …" when
`combat::world` sets up the combat world. The gate's combat rows kept only the `harness input:` lines their own steps
logged (`slice_gate.rs`, `run_combat`), so that line, and every other line of the world's set-up, never reached a row.
A row depended on harness stand-ins it did not name.

**What changed.**
- `run_combat` takes the world's `harness input:` lines once the combat world is built, before any link runs
  (`setup_harness`). It puts them at the front of every combat row that runs, each marked `world set-up: `. All combat
  links run on that one world. On the `--map` route, the first combat row carries the route's notes in full, first;
  they are left out of the set-up lines, so they are not repeated (review of #777). Each later combat row, which runs
  on the same world built with those stand-ins, carries one line in their place, "world set-up: route stand-ins as
  above (the first combat row's route notes)" (the hub's ruling on review r2 of #777), so every row shows what it
  depends on.
- `set_physx_level` used to return early, with nothing logged, when the world had no engine object. It now logs
  "harness input: no engine object; PhysXLevel not set" (`NO_ENGINE_LINE`), and still writes nothing.

**Rows that do not run** gain nothing. A row `blocked_by` an earlier link (links 3, 4, 5, 13 and 14 behind link 1)
runs no step on the world and carries the earlier link's stop.

**Measured** (installed Off strict `--slice-gate`, release build, under the machine lock): 6 ok, X 51.2%, every result
and stop unchanged from main plus #770 and #773. Link 1's row gains its two set-up lines:
- "world set-up: harness input: an empty scene layer (the port has no scene: components attach with their
  script-visible writes only; UNVERIFIED stand-in for the game's scene)";
- "world set-up: harness input: the engine object's PhysXLevel = 0 (a harness setting; UNVERIFIED: the player's
  setting; shipped default 0)".

The blocked combat rows stay empty.
