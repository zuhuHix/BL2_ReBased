## 2026-11-25: GetFiringModeDefinition takes the barrel's mode, then the elemental part's, then the type's

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GETFIRINGMODEDEFINITION, analyst only, on main 5fb7e097: another lane
implements it. Raw output stays under ignored `local/analysis/firingmode/`. No session was run. Every rule is
UNVERIFIED in the running game, except the barrel and type steps, which lane L7 observed.

- **What was decided:** the own-words note NATIVE_WEAPON_GET_FIRING_MODE.md records `WillowWeapon.GetFiringModeDefinition`:
  1. the barrel part's `CustomFiringModeDefinition` when both are set;
  2. else the elemental part's, when both are set;
  3. else the weapon type's `DefaultFiringModeDefinition` (None included);
  4. else None.

  It reads only the weapon's `DefinitionData` and those three objects' fields, writes nothing and keeps no cache. The
  same implementation serves `WillowWeapon` and its three native subclasses.
- **A disagreement settled:** NATIVE_WEAPON_TRIGGER_AMMO.md section 3 had the elemental step, and NATIVE_WEAPON_FIRING.md
  section 1.1 did not. The executable has it. NATIVE_WEAPON_FIRING.md's line now points to the new note, and the index
  row points to the new note first.
- **The slice:** link 8's Bandit AR (barrel `AR_Barrel_Bandit` with no mode, no elemental part) gets the type's
  `Bullet_Assault_Default`, a Bullet mode with no explosion override. So `InitializeInternal` skips its explosion block
  and returns.
- **The proposed Off answer:** load-free, class-code loads included. An unloaded or failed object on the rule's path
  is a typed `NotRead` naming it; an object off the path is not checked; a malformed declaration is `VmError::Script`;
  the returned mode is handed back as it is. Twelve tests are listed, link 8's call shape among them.
- **Evidence:** PROJECTILE_FIRING_PATTERNS_LIVE.md (OBSERVED); Ghidra 12.1.4 headless read-only under the machine lock
  (the class table found from each class's registration and constructor, with a throwaway helper kept under ignored
  `local/`); `tools/ghidra/class_layout.py` for the field names (by fit); `ow-package --def-dump` and `--exports`, and
  `research/script_disasm.py`.
- **Sensitive files:** none. No code changed.
- **Verified:** the barrel and type steps in the running game (lane L7); nothing else.
