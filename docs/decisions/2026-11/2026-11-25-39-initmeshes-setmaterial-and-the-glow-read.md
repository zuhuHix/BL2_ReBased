## 2026-11-25: InitMeshes: SetMaterial and the glow read

AI-assisted (Claude Code)

Lane ANALYST-INITMESHES-SETMATERIAL-GLOW, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and
no bounds check changed.

- **Why:** after `BuildWeaponMaterial`, link 8's `InitMeshes` calls `MeshComponent.SetMaterial(0, …)` on both meshes,
  then the instance's `GetScalarParameterValue` for the glow. Neither is recorded or bound.
- **What:** `docs/verification/NATIVE_INITMESHES_MATERIAL_GLOW.md`, one NATIVE_INDEX row and one GATES row. They come
  from the script, a local read-only Ghidra reading under the machine lock, and the layout tool. Raw output stays
  under ignored `local/analysis/glow/`.
- **Findings** (UNVERIFIED in game):
  - `SetMaterial` does nothing for a negative index or the same material. Otherwise it grows and sets `Materials`, then
    runs the deferred reattach already recorded;
  - `GetScalarParameterValue` answers false on the base classes. A `MaterialInstanceConstant` looks in its own
    `ScalarParameterValues` by name, then asks its `Parent`, with a re-entry guard. A `Material` answers from its
    scalar-parameter expressions' `DefaultValue`. The out argument is written only on true.
- **Proposed Off answer:**
  - `SetMaterial` reuses the deferred-reattach preflight;
  - `GetScalarParameterValue` follows the class rules read-only, with a visited set instead of writing the flag;
  - unlisted classes and unloaded objects are a `NotRead`;
  - Reference unchanged; tests listed.
- **On link 8 (a prediction):** with the mesh's `Owner` set, `SetMaterial` meets `REATTACH_OWNER` first.
- **Review r1 (#705):**
  - `SetMaterial`'s preflight checks its declarations first, and its later writes go through `after_writes`;
  - the grown entries are UNVERIFIED (fitted) None, with the reason;
  - each of the two calls is its own native call, with tests for link 8's call shape;
  - None expressions are skipped, and the scalar expression class is checked as ready only when needed;
  - `ParameterValue` is in the structural check, and the GATES base is a3b548d3.
- **Clean room:** a different lane implements this note.
