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
- **Next.** An analyst read of the override's last call (its name, and whether it is plain script), then an
  implementer.
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game.

Edits: docs/decisions/2026-11/2026-11-19-41-rust-player-class-through-the-script-route.md
Review r1 of #426: its first paragraph no longer says the note has to land first; NATIVE_PLAYER_CLASS.md is on main.
