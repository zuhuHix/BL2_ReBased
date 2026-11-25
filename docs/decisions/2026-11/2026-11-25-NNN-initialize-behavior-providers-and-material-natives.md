## 2026-11-25: The player pawn's InitializeBehaviorProviders and the material natives after it

AI-assisted (Claude Code)

Lane ANALYST-INITIALIZE-BEHAVIOR-PROVIDERS, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and
no bounds check changed.

- **Why:** NATIVE_LINK1_RBPHYSICS_AND_AFTER.md (#738) left `InitializeBehaviorProviders` and
  `GetInstancedMaterialInstanceConstant` in outline. They come on `maya:8` once its 2.A answers.
- **What:**
  - the new note `docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md`;
  - two NATIVE_INDEX rows and one GATES row;
  - a pointer under #738's 2.B and 2.E.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock: the execs, the pawn body, the kernel's provider routine,
    the material virtuals in their class tables, and the creation routine;
  - the layout tool;
  - `ow-package --def-dump` of Maya's body class, stances, definitions and providers.

  Raw output stays under ignored `local/analysis/ibp/` and `local/analysis/rbphys/`.
- **Findings** (UNVERIFIED in game):
  - with a handle of -1, the pawn registers, then hands to the kernel the definitions of its hit regions, scraped
    stances, instance-data hit regions, death and player events, in that order;
  - the kernel routine skips None definitions and definitions with no sequences. The script-facing
    `IntializeBehaviorProviderForConsumer` shares that routine, which is a side finding for the hub;
  - for Maya two providers pass, and `PlayerEvents_Siren` has conditional sequences;
  - `GetInstancedMaterialInstanceConstant` reuses a constant instance owned by the component, or makes one with the
    component as `Outer`, parents it and sets it;
  - the spawn-time `ApplyBodyClassStartingValues` has no `PlayerReplicationInfo`.
- **Proposed Off answer:**
  - per native, with a write-free and load-free preflight (the repeat and pass-2 refusals checked before the first
    write);
  - Maya's answer is the kernel's pass-2 `LayerAbsent` with nothing written;
  - tests listed.
- **Next** (a prediction, not a run): #738's 2.A is the stop, then this pass-2 `LayerAbsent`, then `SetDefaultStance`.
- **Clean room:** a different lane implements this note.
