# Native rules of the mission tracker's list build: MissionTracker.MissionList and MissionDefinition.MissionNumber (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-MISSIONLIST-BUILD (decision 2026-11-19-105, a placeholder), for
slice links 10 to 12 (the Fire mission's accept, objective and turn-in). Links 10 to 12 stop at
`MissionTracker.GetAllMissions`, which answers `NotYet` (layer absent) because the mission bridge holds one record and
not the game's list (NATIVE_MISSION_OBJECTIVES.md; NATIVE_MISSION_NUMBERS.md, "The port's case today";
NATIVE_PLAYTHROUGH_QUERY.md). This note records what builds that list in the game, when, what it reads and writes,
and what a headless Off world must answer at each step.

**How it was read.**
- No bl-sdk session was granted, and the recorded evidence has no capture of the tracker's list or of any
  `MissionNumber`.
- The script was searched first (`research/script_disasm.py`, `tools/ghidra/script_natives.py`). No UnrealScript
  function builds the list: the tracker's script has no `PreBeginPlay`, `PostBeginPlay` or list-filling function, and
  neither `WillowGameInfo` nor `WillowGameReplicationInfo` script spawns the tracker or touches its list. No placed
  `MissionTracker` stands in the Sanctuary packages, so the tracker is made at run time by code that was not found.
- The build was then read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and a local listing
  script, under the machine lock).
- Field names and declared types come from the cooked packages (`tools/ghidra/class_layout.py`).

Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets, call structure or
listing structure is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the
live observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays a named `NotYet` in Off until a fresh implementer
codes it from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The list build: one record per `MissionDefinition` in memory, kept ordered | native (a tracker virtual of the engine actor class) | high | UNVERIFIED |
| When the build runs | native; the engine step that calls it was not found | low | UNVERIFIED |
| The order key of `MissionList` | native (the name comparison) | high | UNVERIFIED |
| The mission numbering: `MissionNumber` of the main chain and of side missions | native, right after the build | high (main chain), medium (side missions) | UNVERIFIED |

## 1. When it runs

- The build is a native virtual method of `MissionTracker`. It overrides an engine `Actor` virtual whose base version
  sets the actor's `CollisionType` from its collision flags. The tracker's version builds the list and numbers the
  missions.
- It is not an UnrealScript event: no script function of the tracker or its parents has this role.
- **Not found:** which engine step calls that virtual (the actor's spawn, its load, or both), and whether it can run
  more than once in a session. A scan of the engine's calls through that method found only callers on other classes.
  So the moment is not settled: `NotYet` (not read), "when the engine runs the tracker's list build".
- Each run starts the list again from the definitions in memory at that moment (section 2), so a second run would
  re-number every mission and drop every record's progress. Whether that happens in the game is part of the same
  open question.

## 2. The list build

**Reads:** every `MissionDefinition` object in memory at that moment (an iteration over all loaded objects of that
class, not over any configured list or package). **Writes:** the tracker's `MissionList`, and three fields of each
definition.

In order, for each mission definition in memory:
1. **Resets on the definition object itself** (shared data, not a per-tracker copy):
   - `bGameStageLocked` is cleared;
   - `MissionNumber` is set to -1;
   - `GameStage` is set to 0.
2. **Finds the insert place** in `MissionList` by binary search on the order key below.
3. **Inserts one new record** there: every field zero or None (so `Status` 0, not started; empty `ObjectivesProgress`;
   no `ActiveObjectiveSet`; empty `SubObjectiveSets`; flags clear), then its `MissionDef` set to the definition.

After the loop the list's storage is trimmed to its length (no behaviour).

There is no duplicate check (each object is visited once) and no filter: plot-critical, side, DLC and seasonal
missions all get a record.

**The order key.** The list is ordered by the definition's object **name**, compared as the engine's name handles:
first by the name's entry number in the engine's global name table, then by the name's instance number. It is
**not** an alphabetical order of the text. The entry number is a run-time quantity: it depends on the order in which
names entered the name table since the engine started (load order).
- **Equal keys.** A new definition whose key equals an existing record's goes **before** the existing ones (read).
  So among definitions with the same object name (in two packages), the one visited last comes first. The visit
  order is the engine's global object order.
- **Correction:** NATIVE_MISSION_OBJECTIVES.md ("The record lookup the first one shares") reads the lookup's
  comparison as "the engine's ordinary name ordering" without opening the comparison. The lookup and the build use the
  same key, so the lookup's order is this name-table order too. That note is fixed in place with a pointer here.

## 3. The numbering

Right after the build, on the same list, the tracker numbers the missions. It writes only `MissionNumber`, on the
definition objects.

1. **Groups.** The missions are grouped by `DlcExpansion` (None is the base game's group). Within a group the missions
   keep their list order.
2. **Each group counts on its own**, from its expansion's `MissionNumberBase` (`DownloadableExpansionDefinition`), or
   from 0 for the base game. The order in which groups are visited does not change any number.
3. **The main chain.** In list order, the first mission of the group that is plot-critical (`bPlotCritical`) and has
   an empty `Dependencies` array starts the chain. It and each `NextMissionInChain` after it, until None, get the
   counter's next value (the start gets the base). Only one chain per group is numbered; the chain members after the
   first are not tested for `bPlotCritical`. A plot-critical mission off that chain keeps -1.
4. **Side missions are filed under a chain number.** For each mission that is not plot-critical, its anchor is the
   largest `MissionNumber` among the plot-critical missions it depends on, directly or through other side missions'
   `Dependencies` (a None dependency is skipped; a plot-critical mission is its own anchor and is not searched past).
   With no plot-critical mission behind it the anchor is -1.
   - One bucket is kept for each number from 0 up to the counter's value after the main chain. A side mission whose
     anchor is in that range goes into that bucket; an anchor of -1 (or out of range) leaves the mission unnumbered,
     at -1.
5. **The place inside a bucket**, for each side mission in list order:
   - For each of its side-mission dependencies, the tracker walks from that dependency along *first* dependencies
     (each mission's `Dependencies[0]`), side missions only, until it finds one already in the bucket. The new mission
     goes after the furthest such find over all its dependencies.
   - With no find, it goes at the end, unless a mission already in the bucket depends on it (directly, or through its
     own chain of first dependencies, side missions only); then it goes just before the first such mission.
6. **A second pass per bucket**, for missions with two or more dependencies (taken in bucket order at the start of
   the pass): each of the mission's dependencies that is in the bucket and that no other mission in the bucket depends
   on is moved to stand right before the mission, the one furthest down the bucket first, so they end up in their old
   relative order just ahead of it.
   - **Edge, as read:** when such a dependency stands *after* its mission, the move as read writes a copy of the
     dependency before the mission and removes a different record (the one just before the dependency's old place).
     That reads like a defect of the game and was not confirmed.
7. **Numbers.** Buckets are numbered from bucket 0 upwards, each bucket's missions in order, continuing the group's
   counter from where the main chain stopped.

So, for the base game: the main story chain is numbered 0, 1, 2, ... and the side missions follow after the last
story number, grouped by the story mission they come after.

**What the order changes.**
- The main chain's numbers do not depend on the list order as long as a group has one plot-critical mission with no
  dependencies. With more than one, the first in name-table order wins.
- Side-mission numbers do depend on it: the bucket placement visits side missions in list order.
- Every consumer that walks `GetAllMissions` "in list order" (NATIVE_MISSION_NUMBERS.md, section 1 step 3 and the
  first live check) inherits the name-table order.

## 4. What a headless Off world answers, step by step

For an implementer. The hub rulings of 2026-11-19 apply: preflight before writes, a lazy definition load counts as a
write, no Off rollback, and grading by typed `VmError` variant. Reference is unchanged. The hub ruled on this note's
round-1 review (#493 r1); the rules below follow that ruling.

1. **The moment.** The list is built **once, at world set-up**, before any query reads it. The port makes its tracker
   there. This placement is a port rule, labelled UNVERIFIED, because the engine step that runs the build was not found
   (section 1).
   - A second build in one world is `NotYet` (not read), "a second run of the tracker's list build".
2. **The set.** It is every `MissionDefinition` export of the packages the set-up loaded.
   - No declared set is needed.
   - Whether this equals the game's in-memory set is UNVERIFIED until live check 1.
3. **The world's own definitions.** The resets and numbers are written to the world's own definition objects (the
   store is per world), so nothing leaks from one world to another.
4. **Preflight, before the first write:**
   - The tracker's `MissionList` must be declared as a dynamic array of `IMission.MissionData`.
   - Each definition must be a `MissionDefinition` with readable `bPlotCritical`, `bGameStageLocked`, `MissionNumber`,
     `GameStage`, `Dependencies`, `NextMissionInChain` and `DlcExpansion`. Each expansion must have a readable
     `MissionNumberBase`. Bad data is a script error naming the field.
   - **Reference targets.** Every `Dependencies` entry, `NextMissionInChain` and `DlcExpansion` must already be
     loaded, else `NotYet` (not read), "a mission reference into a package the set-up did not load".
     - Such a target sits in a package the set-up did not load. In the game, a loaded object's references load with
       it, so the target would be in memory and in the list (UNVERIFIED).
     - The port does not load it lazily (that would be a write). Adding its package to the set-up's load list cures
       the stop.
   - **Cycles.** A dependency cycle among side missions, or a `NextMissionInChain` loop, is bad data found before any
     write. The game's walk has no guard against either, and what it does then (endless recursion) is not behaviour to
     copy.
   - **Equal keys** (section 2, "Equal keys"): two definitions with the same object name and instance number are
     `NotYet` (not read), "the visit order of equal mission names". The insert side was read, but the visit order it
     depends on is the engine's object order, which the port does not model.
   - **A dry run of the numbering**, on the planned list and without writing, finds the second-pass edge (section 3,
     step 6) before any write: `NotYet` (not read), "a dependency that stands after its mission in the tracker's
     reorder pass".
5. **The writes, in the game's order:** for each definition, the three resets, then its record; then the numbering.
   - Anything that still stops after the first write is terminal: the "stopped after partial writes: " prefix, its
     kind and record kept (decision 2026-11-19-80's rule).
6. **The order key** (section 2) is approximated, UNVERIFIED, by the order in which names first enter the port's name
   table:
   1. the engine's own names;
   2. then the packages in set-up load order;
   3. within each package, its name-table order, counted only where a name enters for the first time;
   4. then the instance number.
   - Every loaded package counts, including packages with no missions, because they also add names.
   - The key is the object **name**, not its path.
   - This approximation is a ruling, not a `NotYet`: every order-dependent answer (`GetAllMissions`'s order,
     side-mission numbers, "the first in-progress plot-critical mission in list order") uses it. Live check 2 would
     confirm or correct it.
7. **A `NotYet` at world set-up** (any of the above) is recorded with its kind and record, and surfaces at the first
   read of the list. `GetAllMissions` then answers with that `NotYet`. Its kind is never changed.

**What this changes for links 10 to 12 (reading at `origin/fix/rust-link10-mission-list` 3bb3f6de).**
- Today, link 10 drives 3 of its 8 steps and stops at the accept, on the bridge's `GetAllMissions` `NotYet` (layer
  absent). Links 11 and 12 are blocked behind it.
- The next stop is implementing this build. After that, links 10 to 12 stop at whatever the turn-in's playthrough walk
  over the list reaches next.

## Open questions

1. Which engine step calls the tracker's build (spawn, load, or both), and whether it runs more than once.
2. Which `MissionDefinition` objects are in memory when it runs in Sanctuary (and so how many records the list has).
3. The game's name-table order of the mission names, which fixes the list order and side-mission numbers (the port's
   approximation is section 4, step 6).
4. The second pass's edge with a dependency after its mission (section 3, step 6).
5. What the tracker is spawned by (no script or placed actor makes it).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The set.** After Sanctuary loads, read `MissionTracker.MissionList` (its length and each `MissionDef`) and list
   every `MissionDefinition` object in memory. Expect one record per object, no more and no fewer.
2. **The order.** In the same read, record the list order. Expect it not to be alphabetical in general; it tells the
   name-table order apart from a text order.
3. **The numbers.** Read `MissionNumber` of every definition. Expect the main story chain at 0, 1, 2, ... in
   `NextMissionInChain` order; each DLC's chain from its `MissionNumberBase`; side missions after the last story
   number, grouped by the story mission they depend on; -1 for side missions with no plot-critical mission behind them.
4. **The resets.** Read `bGameStageLocked` and `GameStage` of a few definitions right after the load. Expect false
   and 0 before anything else sets them.
5. **The moment.** Hook the tracker's spawn and the level's begin play, and read the list's length before and after
   each. Expect the list filled once, at one of them.
