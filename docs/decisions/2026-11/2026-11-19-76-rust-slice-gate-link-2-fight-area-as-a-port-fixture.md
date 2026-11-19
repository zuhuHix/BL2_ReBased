## 2026-11-19: Rust slice gate link 2, the fight area as a port fixture

AI-assisted (Claude Code, Claude Opus 5.5). Lane LINK-2-FIGHT-AREA, implementer. Number 76 was given by the hub.
The branch is on `origin/main` 203c1aa, which holds #453 (decision 2026-11-19-66, link 2's headless Sanctuary walk).
This lane read no native code and opened no `local/analysis/`, Ghidra or REA output. There was no real-game session.
It adds no dependency, CLI flag or parsing change. Reference is unchanged, since `--slice-gate` runs in Off only.

- **Why.** Link 2 (`reach_fight_area`) passed H4's walk, then stopped at `fight_area` with a named `NotYet`: no fight
  area is placed in the map world. SLICE_RUN_PLAN.md section 3.2 calls the choice between Sanctuary's own population
  and a test fixture a scheduling one.
- **The hub's ruling.** The fight area is a labelled **PORT FIXTURE**, as the UE5 host's range was. It is a fixed
  spot that H4's walk reaches, with the target dummy placed there by the slice driver. It is not Sanctuary's own
  population, and the dens lane stays a later seam (decision 2026-11-16-59). This is a fixture, not parity. Every
  check is **UNVERIFIED** as a game rule: it checks only that the port's walk and the port's spawn path agree on one
  place.
- **The area** (`ow-cli` `cmd::slice_gate::fight_area`, one constant, `FIGHT_AREA`):
  - a vertical cylinder of radius 200 uu around an offset of (950, -50, 0) uu from the map's player start (the one
    the walk starts at);
  - the offset is H4's walk's end on the installed Sanctuary when the fixture was set (about 943 and -43 uu, on the
    start's floor height), rounded, so no map coordinate is written in the tree;
  - distances are horizontal: the area is a place on the floor, and the walk ends standing.
- **The step** (`fight_area`, after a passing walk):
  1. Maya's walked end must be inside the area. This is checked first, and a miss is an `error` at `fight_area`.
  2. Only then is the dummy placed, through `ow_world`'s spawn path (`ow_world::actors::spawn`), in the slice gate's
     Off world (the one `--inventory-move --equip-run` builds, with its `WorldInfo`). Placement details:
     - the template is the stock target dummy archetype, made from its export in `Sanctuary_Dynamic`, as the mission
       slice and `--combat-run` take it (the path alone names another file's object under the store's identity
       policy);
     - the spawner is the `WorldInfo`, and the location is the area's centre;
     - the spawn runs before play begins, so no begin-play script runs;
     - `Spawn`'s `bNoCollisionFail` is set, because a fixture stands where it is put. The free-spot search would need
       the collision hash the VM world does not model.
  3. The link passes when the spawn made an actor that is in the world's actor list and stands inside the area. A
     refusal of the spawn keeps its kind (a `NotYet` stays `notyet`), and a dummy outside the area is an `error`.
- **The checker.** `tools/check_slice_run_off.py` no longer lists `sanctuary-walk` among the drivers with no evidence
  check, so a link 2 row may be `ok`, but only when its last step is `fight_area` (`ARRIVAL_STEP`). That is the
  change #453's round 2 asked for once the driver checks arrival.
- **Verified (automated, local, desktop).**
  - **Synthetic tests:**
    - `fight_area::tests`, 5 tests on invented positions: the centre offset, inside the area (evident), outside it
      for Maya, the dummy or a NaN (an `error`), a spawn refusal keeping its kind, and no placement when Maya missed;
    - the link 2 test in `slice_gate_tests` now walks the invented flat floor, then passes in an area around the
      walk's end (`ok`) and fails in one just out of reach (`error` at `fight_area`);
    - `tests/check_slice_run_off_test.py`: an `ok` `sanctuary-walk` row is accepted after its fight-area step and
      refused when it ends at the walk, has no steps, or has steps that are not a list.
  - **Installed, Off, `--slice-gate --cooked <CookedPCConsole>` (strict), under the machine run lock:**
    - before (main 203c1aa): link 2 `notyet`, 3 of 4 steps passed (load, spawn, walk), stopped at `fight_area`:
      "needs a fight area placed in the map world";
    - after (this branch): link 2 `notyet`, 3 of 4 steps passed, still stopped at `fight_area`, with a different
      stop. Maya's walk ends 9.5 uu from the area's centre, which passes. The dummy's spawn then stops at the spawn
      path's own named `NotYet`: "a component of another class default object (inherited from a parent class's
      default): `SpecialMoveComponent0` (`GearboxFramework.SpecialMoveComponent`) of `Default__WillowAIPawn`"
      (decision 2026-11-19-44's open item);
    - the other 13 rows are byte-identical; 0 ok, 14 notyet, 0 error before and after.
- **What stops link 2 now.** The spawn's component instancing, for a component the template inherits from a parent
  class's default object, is not read. It is the same open item that stops every enemy spawn in Off (SLICE_RUN_PLAN.md,
  link 3). This lane does not invent it, so link 2 is `ok` only once that rule is read and implemented.
- **Open.**
  - For an analyst: how a spawn instances a component the template inherits from a parent class's default object
    (decision 2026-11-19-44). That clears both this fixture's dummy and the enemy spawns.
  - The fight area stays a port fixture until Sanctuary's own population is ported (the dens seam, decision
    2026-11-16-59). A real-game reading of where the slice's fight takes place would replace the offset.
- **Review r1 of #470** (on the hub's rebase, aaa83b7):
  - **Typed grading.** The placement no longer turns a `VmError` into text. A `VmError::NotYet` is `notyet` through
    the slice gate's `typed_stop`; every other VM error, and every failed check of the step's own, is an `error`,
    whatever its text says. A synthetic regression gives a `Decode` error the exact text of a `NotYet`: it stays an
    `error`.
  - **The placement is checked against the loaded Sanctuary collision** before anything is spawned (port checks,
    UNVERIFIED as game rules):
    - the dummy's cylinder is read from its archetype's `CylinderComponent`; where it cannot be read, the checks are
      not available and the step is a `NotYet`;
    - the floor: the cylinder is swept down from 160 uu above the centre (the mover's `SPAWN_LIFT`) to 2000 uu below
      it (the walk's `MAX_DROP`), both in `FIGHT_AREA`. It must land on a floor Maya could walk on (normal Z at least
      her `WalkableFloorZ`), from a start not in solid, and not stand in solid there. The dummy is spawned at that
      spot, no longer at the player start's height;
    - Maya's reach: her feet and the dummy's within her `MaxStepHeight` (the same floor), and her cylinder, raised by
      that step, sweeping straight from her walked end to the dummy's spot without a hit. That is a conservative
      check of the port's own, not a path search;
    - after the spawn, the dummy must stand where it was put (within 1 uu, the script float's rounding).
  - **The location vector** is built, with its fields checked, before the template is made.
  - **The checker:** an `ok` `reach_fight_area` row must be driven by `sanctuary-walk`, with exactly the steps load,
    spawn, walk and fight_area in that order.
  - **Installed, Off, under the machine run lock** (desktop, this branch on aaa83b7): link 2 `notyet`, 3 of 4 steps
    passed, stop at `fight_area`. The floor and reach checks pass on the installed Sanctuary, and the spawn then stops
    at decision 2026-11-19-44's inherited-component `NotYet`, as before. 0 ok, 14 notyet, 0 error. Row 9 differs from
    the 203c1aa run above only through main's own LINK-9-CAST-SPAWN changes.
