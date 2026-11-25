## 2026-11-25: WillowWeapon.OnCreate is the kernel broadcast

AI-assisted (Claude Code)

Lane ANALYST-LINK8-NEXT, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** a look-ahead for link 8 past `SetColorParameter` (#719). The first native on its path with no note is
  `WillowWeapon.OnCreate`.
- **What:** the new note `docs/verification/NATIVE_WEAPON_ON_CREATE.md` (the path and the native), plus one NATIVE_INDEX
  row and one GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - `ow-package --def-dump` and `--behavior-dump` of the fixed weapon's type, parts and providers;
  - a local read-only Ghidra reading under the machine lock.

  Raw output stays under ignored `local/analysis/link8next/`.
- **Findings** (UNVERIFIED in game):
  - `OnCreate` fires the behavior event `OnCreate` on the weapon's consumer, with filter -1, no payload and, with the
    argument omitted, every provider;
  - this is the same kernel step as `BroadcastBehaviorEventFromScript`;
  - the Bandit assault rifle's nine providers have no sequences, so nothing fires.
- **Proposed Off answer:**
  - bind it beside the broadcast;
  - a supplied `Providers` array is `NotRead`;
  - tests listed.
- **Prediction** (not a run): link 8's next stop is the unbound `WillowWeapon.GetFiringModeDefinition`.
- **Clean room:** a different lane implements this note.
