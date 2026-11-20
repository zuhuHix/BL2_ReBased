# Native rules of ActionSkill.GetBehaviorProviderDefinition (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-GET-BEHAVIOR-PROVIDER-DEFINITION (decision
2026-11-19-164, number pending). Analyst only: a different lane implements it (clean room).

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
behavior-provider interface (`GearboxFramework.IBehaviorProvider`), and 22 classes declare their own native version
(a census of the installed packages):
- `ActionSkill`, `AIDefinition`, `BehaviorVolumeDefinition`, `BlackMarketUpgradeDefinition`,
  `BodyClassDeathDefinition`, `BodyHitRegionDefinition`, `ChallengeDefinition`, `GearboxAnimDefinition`;
- `InteractiveObjectDefinition`, `InterpTrackBehaviors`, `ItemDefinition`, `ItemPartDefinition`, `MissionDefinition`,
  `PerchDefinition`, `PlayerEventProviderDefinition`, `ProjectileDefinition`;
- `SkillDefinition`, `StanceTypeDefinition`, `VehicleClassDefinition`, `WeaponPartDefinition`, `WeaponTypeDefinition`,
  `WillowCharacterClassDefinition`.

**No subclass of `ActionSkill` is among them:** `LiftActionSkill` and `ExecuteActionSkill` declare no copy of their
own, so a call on either reaches `ActionSkill`'s.

Every declaration is native with no fixed native index: the engine binds it by name per class.

**`ActionSkill` itself declares a `BehaviorProviderDefinition` field** (an object reference). So do `SkillDefinition`
and the other definition classes that answer this method.

## 2. What it does

**The script-facing native** (high): it takes no arguments. It asks the receiver's behavior-provider interface for its
definition (the interface's own method, through the receiver's C++ interface table), and returns that answer as is.
It writes nothing. `ItemDefinition`'s and `BehaviorVolumeDefinition`'s natives have the same shape, each asking its own
receiver.

**What `ActionSkill`'s interface method answers** (low: a fitted step, **UNVERIFIED**):
- **The answer:** the receiver's own `BehaviorProviderDefinition` field, as it is. That includes None, with no
  fallback: no archetype lookup, no class default and no load.
- **How it was found:**
  - The interface table the native reaches sits at a fixed place in the object.
  - Among the C++ interface tables installed at that place, one related pair answers this method with a plain read
    of a single field at a fixed distance from the table's place.
  - That distance was compared with where `ActionSkill`'s `BehaviorProviderDefinition` sits under its declared field
    order. Three struct fields lie between the interface table and that field. Their sizes were not decoded and not
    taken from the store, so the distance is only consistent with a range of sizes for those structs: it fits, it
    does not pin.
- **What is not proven:** which C++ class that pair belongs to was not tied by name, and the distance is fitted, not
  measured. So "it is `ActionSkill`'s" is a fit, not a proof (rule 4).
- **What carries the rule instead:** the script's call pattern (section 3: every caller hands the answer straight to
  the behavior kernel as the definition its own events belong to) and the consumer registration below. Both point at
  the skill's own definition; neither reads the native.
- **No other path was seen:** no read of the archetype, the skill definition or the owner.
- **It agrees with an earlier note, on one condition:** NATIVE_ACTION_SKILL.md (section 6) records the skill actor
  registering as a behavior consumer with its own provider, the archetype's `BehaviorProviderDefinition`. Returning
  the field gives that same definition only if the spawned actor's field still names the archetype's subobject. If
  spawning copies the subobject per instance, the two differ and the agreement does not hold (open question 2).

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
   Otherwise it is a data error (`VmError::Script`) naming the field. Through the path binding the receiver is an
   `ActionSkill` or a subclass, and `ActionSkill` declares the field, so this can only come from broken data (a class
   table that lost the field). A None receiver is the VM's ordinary error.
2. **The receiver is not loaded by the native.** In this port a property read on the VM path (`ow-vm`
   `interp/load.rs`, `loaded_for_read`) loads an unloaded store handle, and a lazy load is a write under #518. So the
   native does not take that path:
   - an unloaded receiver (the store's `is_unloaded`), or a stand-in with no slots, refuses `NotYet { NotRead }`
     naming the receiver, "GetBehaviorProviderDefinition on an unloaded receiver", with nothing loaded. On link 9
     this should not arise, since the harness runs the receiver's own script first, but the native must not rely on
     that;
   - otherwise the field is read straight from the heap (`space.heap.read`), with no load.
3. **The answer** is the field's value as stored, None included. The object it names is not loaded either: a
   reference read is not a load. The consumer (`ActivateBehaviorEventFromScript`) loads or refuses when it reads the
   definition. The native writes nothing, draws nothing and loads nothing, so no preflight and no partial-write mark
   apply.
4. **The other 21 declaring classes stay unbound:** the VM's refusal, or a `NotYet { NotRead }` naming the class if
   the implementer binds a guard. Their bodies were not read (section 2).

**Typed refusals:** the unloaded receiver (rule 2) and the data error (rule 1). The hub's ruling on #553 r1 keeps the
binding as proposed, with no hold-back on the fitted step; the live checks below settle it.

**Tests the implementer can build** (synthetic packages):
- a skill whose field names a definition: that object is returned and it stays unloaded;
- a field holding None: None;
- an unloaded receiver: the `NotYet` refusal, with the load counter (`load_transitions`) and the heap unchanged;
- a class without the field: the data error. The path binding only reaches `ActionSkill` and its subclasses, so the
  test needs a hand-built class table in which that class lacks the field;
- Reference unbound;
- nothing written (heap, draws and load counters unchanged).

## 5. Link 9's next stop (not probed)

The local probe was not run: merging #535 onto main with #523 conflicts in code (`ow-defs` `store.rs`, `ow-object`
`heap.rs`, `ow-skills` `action_skill.rs`), so it was not cheap. The next stop is the behavior event itself,
`ActivateBehaviorEventFromScript` with this definition and the event name `OnSelectedTarget` (the kernel's own path,
`ow-behavior`). Whether it passes there was not seen.

## Open questions

1. **UNVERIFIED (low, fitted):** that the C++ interface table described in section 2 is `ActionSkill`'s, and so that
   the answer is the field. The live check below settles it.
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
