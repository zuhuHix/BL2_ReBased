## 2026-11-26: GetPhysXLevel reads a harness setting on the engine object

AI-assisted (Claude Code, Opus 5.5). Lane SOCKETS-PUBLISH-AND-PHYSX-LEVEL, part 2 (queue 0af), on part 1. Clean room:
implemented from docs/verification/NATIVE_GETPHYSXLEVEL_AND_AFTER.md (#767, on main) alone; this lane did not read the
executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided**, in the new `ow-world` module `physx_level` (Off only):
  - **the harness setting:** at world set-up, `combat::world` (the world of links 1, 3, 4, 5, 13 and 14, the one with
    the engine object) writes the engine object's `PhysXLevel` from `HARNESS_PHYSX_LEVEL` = 0, the install's shipped
    default.
    - Only 0, 1 and 2 (the game's clamped range) are accepted, and `PhysXLevel` must be declared as an int. Both are
      checked before the write.
    - One line is logged: "harness input: the engine object's PhysXLevel = 0 (a harness setting; UNVERIFIED: the
      player's setting; shipped default 0)".
    - A world with no engine object sets nothing, and Reference refuses the setting;
  - **`Engine.GetPhysXLevel()`** reads the field. It writes, loads and interns nothing. No engine object is
    `NotYet { LayerAbsent }`, "Engine.GetPhysXLevel: the world has no engine object". An undeclared or non-int field is
    `VmError::Script`;
  - **`SkeletalMeshComponent.SetHasPhysicsAssetInstance(bool,bool)`**, reached only above level 0:
    - the two bools must be declared (`VmError::Script`) and the receiver resident (`NotRead`);
    - a request that matches the flag writes nothing;
    - false to true is the physics `LayerAbsent`, before any write;
    - true to false is the tear-down's `NotRead`;
  - **`NxForceFieldComponent.DoInitRBPhys()`**, reached only above level 0: the physics `LayerAbsent`, nothing written;
  - no pack bound any of the three keys before (checked by grep). The all-packs wiring test still passes.
- **At level 0** the installed `SetupSkeletalMeshPhysicsForPhysXEffects` returns after its level test and writes
  nothing. The installed run shows it: the links get past it.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; part 1, then this commit): 6 ok /
  8 not ok, 51.2% on both.
  - Links 1, 3, 4, 5, 13 and 14 pass `GetPhysXLevel` and `SetupSkeletalMeshPhysicsForPhysXEffects`, and
    `ApplyBodyClassStartingValues` assigns `Mass`. They then stop at **the unbound `Pawn.IsPlayerPawn()`**, not at
    `InitializeBehaviorProviders`' pass-2 `LayerAbsent` as #767 predicted.
  - **Why** (read with `research/script_disasm.py`; the listing stays under ignored `local/analysis/physx_after/`):
    the note's step 2 is not a plain copy. `bAlwaysRelevant` gets `bPawnIsAlwaysRelevant` **or** `IsPlayerPawn()`,
    and the second is asked only when the first is false.
  - Maya's body class has it false, so the native is reached before `CheckPainSounds`. **A correction for
    NATIVE_GETPHYSXLEVEL_AND_AFTER.md section 4, step 2** (for lane F, or the hub).
  - Every other row is unchanged.
- **Tests** (`physx_level_tests.rs`, invented classes):
  - the note's tests 1 to 5 and 8, and 2.C;
  - each call checks that nothing is written, drawn, loaded or interned.

  The note's tests 6 and 7 (the script's "greater than 0" gate) need a toy script with the core `>` operator declared,
  so they are not written; the installed run covers level 0. Mutation-checked: no range check, a constant answer and
  false to true answering nothing each fail a test.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, including the player's real level.
- **Sensitive files:** none.
