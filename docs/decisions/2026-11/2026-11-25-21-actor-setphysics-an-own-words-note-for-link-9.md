## 2026-11-25: Actor.SetPhysics: an own-words note for link 9

AI-assisted (Claude Code)

Lane ANALYST-ACTOR-SETPHYSICS, analyst only, with Claude Opus 5.5. Numbers up to 20 for this date are taken on open
branches, so 21 is used. The maintainer may renumber it. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** the runner probe of main + #682 shows link 9's `cast` stopping at the unbound `Actor.SetPhysics(byte)`.
- **What:** `docs/verification/NATIVE_ACTOR_SET_PHYSICS.md`, one NATIVE_INDEX row and one GATES row. They come from the
  script, a local read-only Ghidra reading under the machine lock, the layout tool and the installed class defaults.
  Raw output stays under ignored `local/analysis/setphys/`.
- **Findings** (UNVERIFIED in game; event names by fit):
  - link 9's caller is `LiftActionSkill.LiftTarget`, which asks the target for `PHYS_Custom` (14) after its `Play` and
    `Queue`. That corrects NATIVE_SPECIAL_MOVE_QUEUE.md's inferred `Queue` caller;
  - the change is layered:
    - `WillowAIPawn`: a nav-mesh check, a rigid-body event below authority, and `bHardAttach` dropping the request;
    - `WillowPawn`: the falling and walking events, each behind its notify flag;
    - `Pawn`: `bForceFloorCheck` on landing;
    - `Actor`: nothing for the same mode; otherwise the mode write, a base search or the base cleared, velocity
      zeroed for None or Rotating, the volume's `PhysicsChangedFor`, and the rigid-body component calls.
- **On link 9:** with the target's class defaults, the call writes `Physics` = 14 and nothing else.
- **Proposed Off answer:** an Off binding with these layers by first native class. The parts not read (nav-mesh check,
  base search, base clearing, rigid body) are typed `NotYet`, decided in a write-free preflight. The writes and the
  script events are in the game's order. The hit pack's stand-in stays for Reference. Tests listed.
- **Clean room:** a different lane implements this note.
