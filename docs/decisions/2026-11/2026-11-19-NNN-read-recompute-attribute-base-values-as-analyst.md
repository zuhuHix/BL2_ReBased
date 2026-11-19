## 2026-11-19: WillowWeapon.RecomputeAttributeBaseValues, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-RECOMPUTE-ATTRIBUTE-BASE-VALUES, slice link 8. Analyst note
only (clean room): docs/verification/NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md. It is written in own words from three
sources: the script (`research/script_disasm.py`), a search of the recorded live evidence (none covers base values or
stacks after init), and local, read-only analysis of the executable with Ghidra under the machine lock, plus the
layout tool. Raw output stays under ignored `local/analysis/rabv/`. No live session was run.

- **Read (script):** the only callers are `WillowWeapon.InitializeInternal` and `WillowItem.InitializeInternal`. On the
  weapon the call comes after `CacheWeaponCard`, and the script then copies `ZoomedEndFOVBaseValue` into
  `ReplicatedZoomedEndFOVBaseValue`.
- **Read (native):** the native handles every stack-attribute property the weapon's runtime class declares itself. It
  empties that property's modifier stack, then overwrites its base with a copy of its value: no recompute, no
  notification. Last it empties `WeaponAttributeModifiers`. It reads no definition, draws no random number and loads
  nothing.
- **Correction:** NATIVE_ATTRIBUTES.md section 8 had the copy the other way ("the value is reset to its base"). The
  note there now points here.
- **For the implementer:** an Off binding with a write-free schema preflight. It writes the 34 `WillowWeapon` triples
  (stack emptied, base := value) in declaration order, then the empty `WeaponAttributeModifiers`; there is no
  refusal. It must not go through `ow-attr`'s set-base, which recomputes and notifies.
- **Open:** whether inherited triples (`Engine.Weapon`'s damage, fire rate and the rest) are covered; this reading says
  no. Also open: the direction of the copy, `WillowItem`'s body and subclasses.
- **Link 8:** this removes the `RecomputeAttributeBaseValues` stop; the next stop is left to the gate run.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
