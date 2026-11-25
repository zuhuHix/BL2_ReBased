## 2026-11-25: SetColorParameter updates one instance parameter

AI-assisted (Claude Code)

Lane ANALYST-PSC-SETCOLORPARAMETER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 8's `init` stops at the unbound `ParticleSystemComponent.SetColorParameter(name, color)`, called by
  `WillowWeapon.InitEffects` on the weapon's `TracerParameterTemplate`.
- **What:** the new note `docs/verification/NATIVE_PSC_SET_COLOR_PARAMETER.md`, plus one NATIVE_INDEX row and one GATES
  row.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool;
  - the `EParticleSysParamType` enum export;
  - `research/script_disasm.py`;
  - `ow-package --def-dump` of link 8's weapon type and `WillowWeapon`'s class default.

  Raw output stays under ignored `local/analysis/psccolor/`.
- **Findings** (UNVERIFIED in game):
  - a None name does nothing;
  - otherwise the first `InstanceParameters` entry with that name and type `PSPT_Color` (5) gets the colour, else
    one zeroed entry is appended with the name, the type and the colour;
  - nothing else changes, and no render-side state is touched.
- **Proposed Off answer:**
  - a read-only plan, then one `InstanceParameters` write;
  - a lazy receiver is `NotRead`;
  - malformed data is `VmError::Script`;
  - tests listed.
- **Clean room:** a different lane implements this note.
