## 2026-11-19: WillowWeapon.RecomputeAttributeBaseValues, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-RECOMPUTE-ATTRIBUTE-BASE-VALUES, slice link 8. Analyst note
only (clean room): docs/verification/NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md. It is written in own words from three
sources: the script (`research/script_disasm.py`), the recorded live evidence (AI_MELEE_TARGETING_LIVE.md section 6: the
live damage base and stack of the five slice guns), and local, read-only analysis of the executable with Ghidra under the machine lock, plus the
layout tool. Raw output stays under ignored `local/analysis/rabv/`. No live session was run.

- **Read (script):** the only callers are `WillowWeapon.InitializeInternal` and `WillowItem.InitializeInternal`. On the
  weapon the call comes after `CacheWeaponCard`, and the script then copies `ZoomedEndFOVBaseValue` into
  `ReplicatedZoomedEndFOVBaseValue`.
- **Read (native):** the native handles every stack-attribute property of the weapon's class, inherited ones included. It
  empties that property's modifier stack, then overwrites its base with a copy of its value: no recompute, no
  notification. Last it empties `WeaponAttributeModifiers`. It reads no definition, draws no random number and loads
  nothing.
- **Correction:** NATIVE_ATTRIBUTES.md section 8 had the copy the other way ("the value is reset to its base"). The
  note there now points here.
- **For the implementer:** an Off binding with a write-free schema preflight. It writes all 41 triples of a `WillowWeapon`
  (34 own, 7 from `Engine.Weapon`; stack emptied, base := value) in slot order, then the empty `WeaponAttributeModifiers`; there is no
  refusal. It must not go through `ow-attr`'s set-base, which recomputes and notifies.
- **Review r1 of #539:** the first version read the walk as the class's own fields only. The recorded live evidence
  contradicts that. The five slice guns' live `InstantHitDamageBaseValue` equals their card damage. `weapon_stats`,
  rerun on those recipes, shows a non-zero part damage term on all five. And the equipped gun's damage stack held only
  the Badass Rank modifier. So `Engine.Weapon`'s `InstantHitDamage` was baked, and the walk follows the class's whole
  property chain. The same record supports the copy's direction. The note also states the preflight's kind check as
  the same numeric kind (float, int or byte), and adds that set-base takes an `f32`. NATIVE_ATTRIBUTES.md's stale
  "reset" wording is corrected in three places.
- **Open:** the other inherited and own triples rest on the chain reading, not on a live read of each. Also open:
  `WillowItem`'s body and subclasses.
- **Link 8:** this removes the `RecomputeAttributeBaseValues` stop; the next stop is left to the gate run.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
