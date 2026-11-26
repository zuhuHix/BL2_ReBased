## 2026-11-26: Links 3 and 4 after the mind: the base search and the damage path

AI-assisted (Claude Code)

Lane ANALYST-LINK34-AFTER-MIND, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** #788 predicts link 3's hit stops at `GameInfo.IsFriendlyFire` and link 4's swing at `SetPhysics` on the
  player pawn; its review r1 found the spawns stop first at `SetPhysics`' base search. The hub asked for those and the
  rest of links 3, 4 and 5 as far as possible.
- **What:**
  - the new note `docs/verification/NATIVE_LINK34_AFTER_MIND.md`;
  - six NATIVE_INDEX rows and a pointer on the `Actor.SetPhysics` row;
  - two pointers in NATIVE_ACTOR_SET_PHYSICS.md;
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (execs and class-table slots of the classes named in the
    note);
  - the layout tool;
  - `ow-package --def-dump`;
  - three float constants from the executable;
  - a local probe: an installed Off `--combat-run` on main d3be3e5 with a call trace, each stop stubbed in turn. The
    temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/l34/`.
- **Findings** (UNVERIFIED in game):
  - the base search is a downward floor trace of 8 units, then the base setter when the floor differs; its trace flags
    are not decoded;
  - `WillowPlayerPawn` keeps `WillowPawn`'s physics change;
  - `IsFriendlyFire` is false on `GameInfo` and `WillowGameInfo`. `WillowCoopGameInfo` has its own rule, outlined, with
    four parts not named;
  - read with Off answers:
    - `InGodMode` (the controller classes, with `GodOnHold` and the AI-god static);
    - `IsHealingDamageType`;
    - `AdjustMomentumForSurfaceType`, with the pawn's damage surface for a hit;
    - `IsVulnerableToFriendlyFire`.
- **Next** (from the probe):
  - link 3: `AdjustDamage`'s hit-region cap, an existing `NotPorted` whose consumer is only outlined;
  - link 4: `AdjustDamage` stage 11's existing surface `NotRead`.
- **Needs the hub:**
  - whether the base search may use a `NoLevel` stand-in;
  - the combat world's `WillowGameInfo` stand-in, where the installed game uses `WillowCoopGameInfo`.
- **Clean room:** a different lane implements this note.
