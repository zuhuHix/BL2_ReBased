## 2026-11-25: The owner-no-see gate's pawn and controller tests

AI-assisted (Claude Code)

Lane ANALYST-BODYCOMP-HOOK2-PAWN-TEST, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 1's `maya:8` stops at "the owner-no-see gate's pawn test (not identified)".
- **What:** section 6 of `docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md`, its NATIVE_INDEX row and a GATES row,
  from a local read-only Ghidra reading under the machine lock and the layout tool. Raw output stays under ignored
  `local/analysis/bcpawn/`.
- **Findings** (UNVERIFIED in game):
  - the pawn test is `Pawn.IsPlayerPawn`, identified by its exec's slot. `Pawn` and its Willow subclasses answer
    `Controller` set with `bIsPlayer`; `WillowPlayerPawn` always answers true;
  - the controller test is `PlayerController.UsingFirstPersonCamera`, also by its exec's slot. That corrects the
    earlier fit, `IsLocalPlayerController`. `WillowPlayerController` answers `bBehindView` and
    `bViewingThirdPersonMenu` both clear.
- **Proposed Off answer:** both tests by first native class, read-only, replacing the `PAWN_TEST` refusal and the
  `is_local_player` check. `Engine.PlayerController`'s camera test and unlisted classes are `NotRead`. Tests listed.
- **On link 1:** both tests pass for Maya with her controller's defaults. Hook 2 then does nothing, or reaches the
  setter's `REATTACH_OWNER`, depending on her `Mesh` template's `bOwnerNoSee`.
- **Clean room:** a different lane implements this note.
