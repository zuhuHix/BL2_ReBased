## 2026-11-19: AI mind route natives: SetDefaultPhysics, GetExpLevel, the mind's attribute recalculation (analyst)

AI-assisted (Claude Code, Opus 5.5). Lane MIND-ROUTE-NATIVES, **analyst only** (clean room, docs/LANE_BRIEF.md). No crate code
changed. The rules stay `NotYet` in Off until a different thread implements them from the note. There was no real-game
session: everything is **UNVERIFIED** in the running game.

**Why.** Decision 2026-11-19-17 (NATIVE_COMBAT_GROUNDWORK.md section 1.11, PR #402) found three natives without a note on an
AI mind's starting-values route. FIX-OFF-STOCK's adopt patch will reach them in this order: `SetDefaultPhysics`,
`GetExpLevel`, `WillowMind.RecalculateAttributeInitializedState`.

**What changed (docs only).**

- `docs/verification/NATIVE_MIND_ROUTE.md`, new. It is a separate note so as not to collide with PR #402's section 1.11,
  which it points to.
  - **`SetDefaultPhysics`.**
    - Gates: the pawn is not gone; the archetype is a `Pawn`; there is an `SMComponent`; the current special move is not
      `SpecialMove_Spawned`.
    - The pawn's physics becomes its default physics. An AI pawn's default physics is read from its `AIClass`
      (`bIsTurret`, `Physics`) and the mind's `WillowNav`, with values named from the package's `EPhysics`.
    - `bCollideWorld` is copied from the archetype, and collision from the archetype's `bCollideActors`, `bBlockActors`
      and `bIgnoreEncroachers`, unless two unnamed pawn queries both answer true.
  - **`GetExpLevel` on an AI pawn** returns the pawn's own `ExpLevel`, through its `IBalancedActor` implementation. It
    reads no other object.
  - **`WillowMind.RecalculateAttributeInitializedState`** does three things:
    - a refresh of this mind's active skills through the skill manager;
    - the shared controller-class part (NATIVE_LEVEL_UP_ATTRIBUTES.md), ending in the pool rebase;
    - the mind's own part: the pawn's `CommerceMarkup` value, and ten status-effect resistance attributes of its
      `StatusEffectComp` re-initialised from the AI class.
- `docs/verification/NATIVE_INDEX.md`: three rows.

**A finding for every user of `tools/ghidra/class_layout.py`.**

- The tool does not place every `WillowPawn` and `WillowAIPawn` field where the game reads it. The notes name those
  fields by meaning, not by the tool's layout (UNVERIFIED); the evidence stays under ignored `local/analysis/`, and
  correcting the tool is a separate lane.
- `WillowMind`'s layout needs a size for its parent's map property. The size fitted against the registered object size also
  places the fields the code reads (labelled a fit).

**Tools.** Ghidra 12.1.4 read-only, under the machine lock, one run at a time, waiting whenever the project was held: 6
`tools/ghidra/run.ps1` query runs and 4 runs of the local read-only table reader (untracked, under `local/analysis/`). The
layouts come from `class_layout.py` (with the correction above) and `EPhysics` from the package's name table. Raw output only
under ignored `local/analysis/mind-route/`.

**Open:**

- the unnamed queries listed in the note's "Not read";
- the 13 against 12 count of the controller-class part's re-initialisations, against NATIVE_LEVEL_UP_ATTRIBUTES.md;
- the missing `WillowPawn` field;
- bl-sdk confirmation on a fresh bullymong.
