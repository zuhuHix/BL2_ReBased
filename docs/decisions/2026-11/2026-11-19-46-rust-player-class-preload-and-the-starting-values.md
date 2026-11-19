## 2026-11-19: Maya's pawn archetype preloaded; the player controller's starting values are native and NotYet

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-PLAYER-CLASS-B (implementer; slice link 1), stacked on
decision 2026-11-19-41. It opened no raw analysis output, Ghidra or REA. The script was read with
`research/script_disasm.py` and `ow-package --disasm`.

- **The second harness input (hub decision).** In Off, before the class select of 2026-11-19-41, the `maya` step loads
  the class's own `PawnArchetypePath` (`GD_Siren_Streaming.Pawn_Siren`, read from `CharClass_Siren`) through the
  definition store. It stands in for the streaming load NATIVE_PLAYER_CLASS.md does not settle (section 3 step 4),
  UNVERIFIED, and is labelled like the class select. With it, the installed `SwitchPlayerClass` takes the note's
  found branch.
- **The starting values on a player controller: native, so the script cannot settle them.**
  - `WillowPlayerController.ApplyCharacterClassStartingValues` has no script: its declaration is native, as is the
    `Controller` base's.
  - NATIVE_COMBAT_GROUNDWORK.md section 1.10 step 2 says the override ends with "one parameterless script function on
    the controller (name not resolved)". That call is made from native code, so reading the installed script cannot
    name it. It needs an analyst read of the override.
  - So the hub's "route it through the script" does not apply. The override is now bound in Off as
    `NotYet { NotRead }` that names that open call, before anything changes. It used to be an unbound native.
  - Running the base and the three pools of step 2 first and then refusing would have left pools made and the call
    missing, so it is refused up front.
- **Reference.** The override stays unbound there (no C++ stand-in), and the preload runs in Off only.
- **Verified (automated, local).** One new `ow-combat` test: in Off the override is `NotYet { NotRead }`, the open
  call is named and no pool changes; in Reference it is unbound. The run fixture's toy `WillowPlayerController`
  gains the native declaration.
- **Installed runs, Off.**
  - Before (decision 2026-11-19-41): `--combat-run maya:8` and the slice gate's links 1, 3, 4, 5, 13 and 14 stopped at
    `FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren` (held, not loaded).
  - After: the archetype is in memory, so the switch runs `ClientNotifyClassChanged` and reaches the override. Those
    links now stop at `WillowPlayerController.ApplyCharacterClassStartingValues`'s NotYet (the unnamed script call of
    section 1.10 step 2). Links 10 to 12 are unchanged (`AIComponent.ActivateEvent`).
- **Review r1 of #426, folded in here.**
  - Both harness inputs now write a line to the VM log: `harness input: preload <path> (UNVERIFIED stand-in for the
    streaming load)` and `harness input: ServerPlayerSelectClass(<class path>)`. So a run's output names both (rule 7).
  - `GetDefinition`'s lookup in memory answers `NotYet` when a name match has no full path the port can name, before
    any store load (as `FindAlreadyLoadedObject`'s lookup does).
  - The module documentation says that the store's load runs before the class check, so a wrong-class `NotYet` leaves
    that definition live in the VM. Checking first is not cheap: the holder query gives no export's class.
  - `LoadPlayerPawnDataAsync` raises an error for an object argument of the wrong kind, instead of reading it as None.
- **Review r1 of #431, folded in here.**
  - **The class is loaded too, and `GetDefinition` now finds it in memory.** To read the class's `PawnArchetypePath`,
    the harness loads `GD_Siren.Character.CharClass_Siren` through the definition store before the class select. So
    the installed `GBXDefinition.GetDefinition` takes its in-memory branch, not decision 2026-11-19-41's load-by-name
    branch; both return the same object. That load is a third harness effect, and it is now logged as
    `harness input: load <class path> (to read its PawnArchetypePath)`.
  - The preload's line now says how it came out: `loaded`, or `found nothing`. A path that is missing, not a string or
    not an object path is logged as skipped. The class select runs in every case.
  - The two inputs sit in one helper (`apply_class_inputs` in `ow-cli`'s `combat` command), which checks Off itself.
    Three synthetic tests cover it: in Off the preload runs first, the class select second, with both lines written
    before the select's call and the archetype in memory by then; a preload that finds nothing, or a path that will
    not parse, is logged and the select still runs; Reference does neither.
  - `GetDefinition`'s lookup documentation no longer says an unnamed name match is skipped: it answers `NotYet`.
  - The override test calls `ApplyCharacterClassStartingValues` through the controller's method lookup, as the class
    switch does, and checks that the lookup reaches the player controller's override.
- **Review of #431 by a second model family, folded in here.**
  - **A class switch that stops is sticky (HIGH).** The installed switch writes the controller's `PlayerClass` and
    `CharacterClass` before the override answers `NotYet`. So a second `maya` step used to see a class and go on to
    the experience and health setup on a half-switched player. Now, in Off, the run keeps that stop, and every later
    step of the run answers with it. A rollback would need to know every write the switch makes before it stops
    (`ClientNotifyClassChanged`'s included), which is not read. A synthetic two-step test covers it: `maya:8 maya:8
    players:1` all stop the same way, the second `maya` does not select again, and no experience is set.
  - `LoadPlayerPawnDataAsync` of a bare name is now `NotYet`, even with an object of that name live. The note makes
    the no-package case a no-op only after a lookup miss, and the lookup's partial-path rule is not read.
  - `GetDefinition` checks the class from the store's holder records before it loads anything. The records name each
    export's class; a class the holder's own file exports is qualified with that file's package. A holder of another
    class, or a file that could not be searched, is `NotYet`, and nothing is loaded, so a refused call leaves no
    definition live for a later `FindAlreadyLoadedObject` to find. This corrects the earlier note above that checking
    first is not cheap.
  - Every harness attempt is labelled before it runs, `UNVERIFIED` included (the class load is a harness load too),
    and its outcome follows: `loaded`, `found nothing`, or `failed (<error>)` before the error is returned. The slice
    gate's rows gain a `harness` list with the `harness input:` lines their own steps logged, also when a step stops.
  - `LoadPlayerPawnDataAsync` checks the `Instigator` argument's kind like the others, and still does not pass it to
    the report.
- **Delta review of #431 (last round).**
  - `GetDefinition` no longer compares the class in holder records' path text. A new read-only
    `DefStore::holder_class` returns the class of the export the identity policy would pick: it loads that class's
    code, not the export. That class is compared with the requested one as a class. So a duplicate holder of another
    class that the policy does not pick no longer refuses the lookup, and a class exported under an outer group
    (metadata `Group.Class`, reflected as `File.Group.Class`) is recognized. A requested class with no package class to
    compare is `NotYet`. Two synthetic tests cover these.
  - After a latched class-switch stop, a pawn-indexed step takes no state snapshot either, because its getters are
    script. The two-step test now spawns an enemy first and checks with the VM's call census that the step after the
    stop runs no native or script call.
- **Next.** An analyst read of the override's last call (its name, and whether it is plain script), then an
  implementer.
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game.

Edits: docs/decisions/2026-11/2026-11-19-41-rust-player-class-through-the-script-route.md
Review r1 of #426: its first paragraph no longer says the note has to land first; NATIVE_PLAYER_CLASS.md is on main.
