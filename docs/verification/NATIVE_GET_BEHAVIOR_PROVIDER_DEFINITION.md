# Native rules of ActionSkill.GetBehaviorProviderDefinition (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-GET-BEHAVIOR-PROVIDER-DEFINITION (decision
2026-11-19-NNN, number pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the installed declarations and the Phaselock archetype, read through the port's definition store (`ow-package
  --def-dump`);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), plus
  a local read of one interface table's entries from the image.

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/gbpd/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** On a local merge with #523, #535 and the behavior-variable setters (#548), slice link 9's `cast`
stops at this native, unbound. It is an argument of the behavior event that `LiftActionSkill.OnSelectedTarget` fires.

## 1. The declaration

`GetBehaviorProviderDefinition()` returns a `BehaviorProviderDefinition` (an object reference). It is the method of the
behavior-provider interface (`GearboxFramework.IBehaviorProvider`), and 22 classes declare their own native version:
- `ActionSkill`, `SkillDefinition`, `ItemDefinition`, `ItemPartDefinition`, `WeaponPartDefinition`;
- `WeaponTypeDefinition`, `ProjectileDefinition`, `MissionDefinition`, `ChallengeDefinition`, and others.

Every declaration is native with no fixed native index: the engine binds it by name per class.

**`ActionSkill` itself declares a `BehaviorProviderDefinition` field** (an object reference). So do `SkillDefinition`
and the other definition classes that answer this method.

## 2. What it does

**The script-facing native** (high): it takes no arguments. It asks the receiver's behavior-provider interface for its
definition (the interface's own method, through the receiver's C++ interface table), and returns that answer as is.
It writes nothing. `ItemDefinition`'s and `BehaviorVolumeDefinition`'s natives have the same shape, each asking its own
receiver.

**What `ActionSkill`'s interface method answers** (medium):
- **The answer:** the receiver's own `BehaviorProviderDefinition` field, as it is. That includes None, with no
  fallback: no archetype lookup, no class default and no load.
- **How it was found:**
  - The interface table the native reaches sits at a fixed place in the object.
  - Among the C++ interface tables installed at that place, one related pair answers this method with a plain read
    of a single field at a fixed distance from the table's place.
  - That distance lands where `ActionSkill`'s `BehaviorProviderDefinition` would sit if its declared fields are laid
    out in order. The sizes of the three structs between them were not decoded.
- **What is not proven:** which C++ class that pair belongs to was not tied by name. So "it is `ActionSkill`'s" is the
  structural fit above, not a proof (rule 4).
- **No other path was seen:** no read of the archetype, the skill definition or the owner.
- **It agrees with an earlier note:** NATIVE_ACTION_SKILL.md (section 10) records the skill actor registering as a
  behavior consumer with its own provider, the archetype's `BehaviorProviderDefinition`. The events then fire against
  that same definition, which is what returning the field gives.

**The other declaring classes** (not read): their natives forward the same way, and each class has a field of the
same name. That each answers its own field is a likely pattern, not a read result.

## 3. Link 9's callers

- **The call shape** (high): `LiftActionSkill.OnSelectedTarget` fills two behavior variables (the instigator and the
  target, through `SetObjectBehaviorVariable`, #548). It then calls `BehaviorKernel.ActivateBehaviorEventFromScript`
  with five arguments:
  1. the skill's `ConsumerHandle`;
  2. **`GetBehaviorProviderDefinition()`**;
  3. the function's own name (`GetFuncName`), as the event name;
  4. an empty argument;
  5. the two parameters.
- **The same call shape in other functions:**
  - the 10 other `LiftActionSkill` events: `OnBeginLiftingTarget`, `OnTargetBlocked`, `OnTargetBecomesLocked`,
    `OnTargetIsAboutToBecomeUnlocked`, `OnReleasedTarget`, `OnKilledTarget`, `OnHealedTarget`, `OnLiftFailed`,
    `OnRuinImpact` and `OnCharmTarget`;
  - 8 `ExecuteActionSkill` events;
  - 2 behaviors (`Behavior_SkillCustomEvent`, `Behavior_ChangeSkillBehaviorSequenceState`, two calls each).

  In all, 24 script functions make 27 calls; 3 of those functions are outside the skills.
- **What link 9's receiver holds** (high, from the store):
  - The cast runs on the installed archetype `GD_Siren_Skills.Phaselock.ActionSkill_Phaselock` (a `LiftActionSkill`).
    The slice harness calls the script on that object directly (`slice_gate.rs`, `phaselock_cast`).
  - Its `BehaviorProviderDefinition` is its own subobject, `GD_Siren_Skills.Phaselock.ActionSkill_Phaselock.BehaviorProviderDefinition_0`.
  - On that receiver the answer is that subobject.
- **In the game,** the skill is an actor spawned from that archetype. Whether the spawned actor's field still names the
  archetype's subobject or a per-instance copy was not read (open question 2). The rule above answers whatever the
  field holds.

## 4. Proposed ParityMode Off answer (for a separate implementer)

**The rule, Off only** (Reference keeps the stub policy). Bind `WillowGame.ActionSkill.GetBehaviorProviderDefinition`
by path:
1. **The receiver** must be an object whose class declares `BehaviorProviderDefinition` as one object reference.
   Otherwise it is a data error (`VmError::Script`) naming the field. A None receiver is the VM's ordinary error.
2. **The answer** is the field's value as stored, None included. Read it without loading the object it names: a
   reference read is not a load. The consumer (`ActivateBehaviorEventFromScript`) loads or refuses when it reads the
   definition. The native writes nothing, draws nothing and loads nothing, so no preflight and no partial-write mark
   apply.
3. **The other 21 declaring classes stay unbound:** the VM's refusal, or a `NotYet { NotRead }` naming the class if
   the implementer binds a guard. Their bodies were not read (section 2).

**Typed refusals:** none beyond the data error. If the hub prefers to hold the medium-confidence step until the live
check, the alternative is `NotYet { NotRead }`, "which field ActionSkill's behavior-provider method answers". That
keeps link 9 at this stop.

**Tests the implementer can build** (synthetic packages):
- a skill whose field names a definition: that object is returned and it stays unloaded;
- a field holding None: None;
- a class without the field: the data error;
- Reference unbound;
- nothing written (heap, draws and load counters unchanged).

## 5. Link 9's next stop (not probed)

The local probe was not run: merging #535 onto main with #523 conflicts in code (`ow-defs` `store.rs`, `ow-object`
`heap.rs`, `ow-skills` `action_skill.rs`), so it was not cheap. The next stop is the behavior event itself,
`ActivateBehaviorEventFromScript` with this definition and the event name `OnSelectedTarget` (the kernel's own path,
`ow-behavior`). Whether it passes there was not seen.

## Open questions

1. **UNVERIFIED (medium):** that the C++ interface table described in section 2 is `ActionSkill`'s, and so that the
   answer is the field. The live check below settles it.
2. **UNVERIFIED:** whether a spawned action skill's `BehaviorProviderDefinition` names the archetype's subobject or its
   own copy.
3. **Not read:** what the other 21 declaring classes answer.

## Live checks that would confirm the rule (bl-sdk, the RTSE way)

1. With Maya in game, cast Phaselock. On the live skill actor, call `GetBehaviorProviderDefinition()` and read its
   `BehaviorProviderDefinition` field. Expect the same object.
2. Record that object's path name. Compare it with `GD_Siren_Skills.Phaselock.ActionSkill_Phaselock.BehaviorProviderDefinition_0`
   (answers open question 2).
3. Hook `ActivateBehaviorEventFromScript` during the cast. Expect the event name `OnSelectedTarget` and that same
   definition as its second argument.
