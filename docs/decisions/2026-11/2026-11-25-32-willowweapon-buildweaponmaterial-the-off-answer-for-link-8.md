## 2026-11-25: WillowWeapon.BuildWeaponMaterial: the Off answer for link 8

AI-assisted (Claude Code)

Lane ANALYST-BUILDWEAPONMATERIAL, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** with #678, link 8's `init` stops with an error at the unbound `WillowWeapon.BuildWeaponMaterial`.
- **What:** section 1.1 of `docs/verification/NATIVE_WEAPON_VISUALS.md`, one NATIVE_INDEX row and a GATES row. Section
  1 (2026-10-04) already read the behaviour. This lane re-read it, locally and read-only in Ghidra under the machine
  lock, for the edge cases. Raw output stays under ignored `local/analysis/bwm/`.
- **Findings** (UNVERIFIED in game), all agreeing with section 1:
  - no type definition gives None;
  - the parent is the Material part's `Material`, else the mesh component's material 0, else None;
  - a new instance of `MaterialClass`, outer the weapon, is constructed without checking the class;
  - its parent is set, then each part applies its own parameters.
- **Proposed Off answer:** a write-free, loaded-only preflight that refuses with a typed `NotRead` for the mesh's
  material lookup, a missing or unexpected `MaterialClass`, and any part vector parameter (the setter is not read).
  Then one object is created with its `Parent`, and that is the answer. Reference unchanged; tests listed.
- **On link 8:** with a Material part and no elemental part, the native answers a new instance, and `SetMaterial` is
  the next candidate stop.
- **Review r1 (#698):**
  - `MaterialClass` and `MaterialInstanceConstant` must be ready classes, with a resident default object;
  - a None weapon is the default-outer `NotRead`;
  - the `Parent` write is labelled UNVERIFIED by name, with why it is safe;
  - the setter's overwrite reading is behaviour only, and the nine walked slots are named;
  - tests for link 8's call shape, a None `Material`, name parts, a cold class and a None weapon.
- **Clean room:** a different lane implements this note.
