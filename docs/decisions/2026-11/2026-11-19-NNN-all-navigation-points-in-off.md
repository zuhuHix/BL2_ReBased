## 2026-11-19: WorldInfo.AllNavigationPoints in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ALL-NAVIGATION-POINTS, implementer. The decision number is pending from
the hub (three digits); NNN is a placeholder.

- **Sources:**
  - the stock UE3 iterator as publicly documented;
  - the installed script, read with `research/script_disasm.py` (the two callers);
  - the serialized fields `WorldInfo.NavigationPointList` and `NavigationPoint.nextNavigationPoint`.

  The executable was not read for this native (the hub's brief), and there is no analyst note.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was relaxed.
- **The base:** `research/rust-is-split-screen` (#510, 368f3f0f), on #508 (13992c2f).

**Why.** With `Engine.IsSplitScreen()` bound false (c376's lane; a local, uncommitted binding for the probe here), slice
link 1 stops in `maya:8` at this iterator, unbound. It is reached from `GameInfo.FindPlayerStart`, which walks the points
for a `Teleporter` (only when the incoming portal name is not empty) and then calls `ChoosePlayerStart`.
`ChoosePlayerStart` walks them for a `PlayerStart` and rates each through `RatePlayerStart`.

**What changed** (`crates/ow-world/src/world/nav_points.rs`, bound by `World::new` outside Reference; Reference keeps
the VM's stub policy):
- **The rule:**
  - start at the receiver's `NavigationPointList` and follow each point's `nextNavigationPoint`;
  - yield each point whose class is `BaseClass` or a subclass of it, in list order;
  - the loop binds the out parameter (index 1), and the script body runs once per yield.
- **The list is taken once, when the loop starts.** The stock iterator follows `nextNavigationPoint` step by step. The
  two differ only if a loop body relinks the list, and neither caller's body writes a list field.
- **What the data and script do not settle** is each a `NotYet { NotRead }`, raised before the loop runs. The exact
  texts are the module's constants.
  1. `NONE_BASE`, a None `BaseClass`: "WorldInfo.AllNavigationPoints with a None BaseClass".
  2. `DELETED`, a destroyed or `bDeleteMe` list entry: "WorldInfo.AllNavigationPoints: whether a destroyed or bDeleteMe
     navigation point is visited".
  3. `LOOP`, a cyclic list (the cycle guard): "WorldInfo.AllNavigationPoints: a navigation list that loops back to a
     visited point".
  4. `NOT_A_POINT`, an entry that is not a `NavigationPoint`: "WorldInfo.AllNavigationPoints: a navigation list entry
     that is not a NavigationPoint".
  5. `UNPLACED`, an entry the level's actor list does not hold (dangling, from the port's view): "WorldInfo.AllNavigationPoints:
     a navigation list entry the level's actor list does not hold".
  6. `UNREACHED`, a placed, live navigation point the list does not reach: "WorldInfo.AllNavigationPoints: placed
     navigation points the WorldInfo's list does not reach (an empty or partial list; the streamed levels' lists)".
- **An empty list in a level that places no navigation point yields nothing.** Every reading agrees on that, and it is
  the slice's case: the drivers' worlds load no map, so their `WorldInfo` has no list and their level no points.
- **No preflight.** The iterator writes nothing, so nothing needs a preflight or a rollback. A caller that wrote first
  adds the "stopped after partial writes: " prefix, as `SetupInputDevices` does.

**Open questions for an analyst** (each is one of the `NotYet`s above):
1. Does the walk skip a point that is pending deletion (`bDeleteMe`), or visit it?
2. When a map loads, does the engine fill or rebuild `WorldInfo.NavigationPointList` (for example from each level's own
   navigation list), or does it use the serialized list as it is? In particular: what happens with an empty serialized
   list in a level that places points, and are the streamed levels' points joined onto the persistent list, and in what
   order?
3. What does the iterator do with a broken list: a cycle, an entry that is not a `NavigationPoint`, or a reference to an
   object the level does not hold?
4. What does it do with a None `BaseClass`: yield nothing, every point, or something else?

**Tests** (`nav_points_tests.rs`, an invented `Engine.upk` with a real script `foreach` over the iterator, in the shape
of `ChoosePlayerStart`'s loop):
- list order rather than placement order, and the subclass filter (the base class, `PlayerStart`, `Teleporter`);
- an empty list with no points yields nothing and writes nothing;
- placed points the list does not reach (an empty list and a partial one, with a destroyed point not counted);
- the cycle guard (a two-point loop and a self-link), and a deleted entry;
- an unplaced entry, a wrong-class entry and a None class;
- Reference unbound.

`world/tests.rs` checks that `World::new` binds the key in Off only.

**Installed Off `--slice-gate`** (release build, with `Engine.IsSplitScreen()` bound false locally only, uncommitted,
under the machine lock):
- 0 ok / 5 NotYet / 9 errors, unchanged;
- link 1 gets past `FindPlayerStart`'s walks (no list, no points: nothing yielded) and stops in `maya:8` at the unbound
  native `WillowPlayerPawn.InitializePawnMeshes()` (0 of 1 driver step);
- links 3, 4, 5, 13 and 14 stop at the same native, blocked behind link 1.
