# EffectCollectionDefinition.GetParticleEffect (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-GET-PARTICLE-EFFECT (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (every script function of the nine code packages searched for
  callers);
- the installed packages through `ow-package --def-dump` (the slice gun's collection: names and counts only), and
  `tools/ghidra/class_layout.py` for the field names;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local, uncommitted probe of the installed slice gate (section 5).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/gpe/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With #558 (`RecomputeAttributeBaseValues`) on #534 and #552, link 8 is 1 of 5 at `init`, stopped
at the unbound `EffectCollectionDefinition.GetParticleEffect(object)`.

## 1. Where it is called (question a)

- **Callers:** `WillowWeapon.ClientInitializeFromWeaponTypeDefinition`, twice, and no other script function in the
  nine code packages.
- **On link 8:** the weapon's `InitializeInternal` calls `ClientInitializeFromWeaponTypeDefinition`
  (NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md section 2). That function first registers the weapon as a behavior
  consumer and initialises the type's and the parts' behavior providers. Then, for the muzzle flash:
  1. `MuzzleFlashPSTemplate` = None;
  2. if the barrel part is set and its `MuzzleFlashPSTemplates` (an `EffectCollectionDefinition`) is set:
     `MuzzleFlashPSTemplate` = that collection's `GetParticleEffect(Self)`;
  3. if `MuzzleFlashPSTemplate` is still None and the weapon type's `MuzzleFlashPSTemplates` is set:
     `MuzzleFlashPSTemplate` = that collection's `GetParticleEffect(Self)`.

  The argument is always the weapon itself.
- **The script then reads it:** `MuzzleFlashPSTemplate` is a field of the weapon. Its readers (the muzzle-flash
  set-up) were not searched.

## 2. What it does (question b)

`EffectCollectionDefinition` holds one field, `ParticleEffects`, an array of `ConditionalParticleEffectData`. Each
entry is an `Expression` (an `ExpressionEvaluator`) and a `ParticleEffect` (a `ParticleSystem`).

The native walks `ParticleEffects` **in array order** and returns the `ParticleEffect` of the **first** entry that
passes:
- an entry whose `Expression` is None passes at once;
- otherwise, the entry's `Expression` is evaluated with the argument as its context. This is the evaluator's own
  `Evaluate`, a per-class virtual. The entry passes when that answers true.

When no entry passes, or the array is empty, it returns None. The returned `ParticleEffect` may itself be None, if an
entry that passes has none.

**What it reads, and what it does not:**
- **Selection** is by the evaluators only, in array order: no class, name, material or tag test of its own. Whatever
  the argument means to an entry comes from that entry's evaluator.
- **The native writes nothing**, draws nothing and loads nothing of its own. The `ParticleSystem` is returned as a
  reference. The native does not touch the particle system itself.
- **The evaluators** may do anything their class does: read attributes of the context, resolve them, and so on.
  That is outside this native.
- **A None argument** is passed to the evaluators as it is. What they do with it is theirs.

## 3. The slice gun's data (names and counts only)

On link 8's weapon (`EQUIP_WEAPON`, the Bandit assault rifle):
- the barrel part's `MuzzleFlashPSTemplates` is None, so step 2 of section 1 is skipped;
- the weapon type's is `GD_Weap_AssaultRifle.Effects.AR_Default_MuzzleFlashes`, with **6 entries**:
  - entries 1 to 5 each have an `AttributeExpressionEvaluator`, testing in turn `WeaponDamage_Is_Corrosive`,
    `_Explosive`, `_Incendiary`, `_Shock` and `_Amp` (the E-tech flash): each with operand 1 an attribute,
    comparison 0 and constant 1;
  - entry 6 has no `Expression` and the default flash.

So for this gun, the answer is the first of the five element flashes whose test passes, else the default flash.

## 4. Proposed Off answer (question c; for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `EffectCollectionDefinition.GetParticleEffect(object)` in Off. Reference keeps
the stub policy.

**The load question.** The local probe (section 5) shows that **the collection arrives as an unloaded handle**: the
script's context call does not load it. Each of its five evaluators is an unloaded handle too. Reading the array, or
calling an evaluator, therefore needs a load. Under #518 a lazy load counts as a write.

**Preflight (write-free, loads nothing):**
1. The collection (`this`) is an unloaded handle: refuse `NotYet { NotRead }`, "GetParticleEffect: the collection is
   not loaded (a load is a write)". That is the pattern of `ow-inventory/src/globals.rs`.

   **For the hub:** this refusal is Maya's link 8 stop as long as the store keeps the weapon type's collection lazy.
   The alternative is a ruling that the weapon's set-up preloads the collections a weapon definition names, as #476's
   set-up preloads the globals definition, or that this native may load and is marked.
2. `ParticleEffects` is declared as an array of a struct with `Expression` and `ParticleEffect`. Otherwise a
   `VmError::Script` naming the field.
3. Before the walk, every entry's `Expression` that is an unloaded handle refuses the same way: `NotYet { NotRead }`,
   "GetParticleEffect: an entry's evaluator is not loaded". The walk could stop at an earlier entry, but checking
   them all keeps the preflight free of evaluator calls.

**The walk** (only when the preflight passes): in array order, an entry with no `Expression` returns its
`ParticleEffect`. Otherwise its `Evaluate` is called through the VM with the argument (the evaluator class's bound
native or script):
- true returns the entry's `ParticleEffect`;
- false goes on to the next entry;
- an error, including an unbound evaluator, passes through unchanged, kind and record kept.

No entry passing returns None.

**The counters.** The native itself writes nothing. A `NotYet` raised by an evaluator after `committed_writes()`,
`draws()` or `load_transitions()` moved takes the "stopped after partial writes: " prefix (#518). The preflight's
refusals come before any of that, so they never do.

**Not a stand-in.** The port must not return the default (the entry with no `Expression`) without evaluating the
earlier entries. On a gun whose element test passes, that would be the wrong flash.

## 5. What link 8 meets after it (question d; local probe, uncommitted)

The probe ran on a local, unpushed merge of #558 (3a598e4c), #534 and #552, with a throwaway binding of this native
by section 4's walk. The installed Off `--slice-gate` strict totals were 1 ok, 4 notyet and 9 error, the same as
without the probe. Link 8 stays 1 of 5 at `init`. It showed, in order:
1. the collection was an unloaded handle on entry, and so were all 5 evaluators (section 4's load question);
2. with both loaded by the probe, the first evaluator call stops at the unbound
   **`AttributeExpressionEvaluator.Evaluate(object)`**. `ow-skills` binds that evaluator for its own packs, but the
   equip run's world does not register them. Whether that binding serves this use (an attribute of the weapon as
   context) is the implementer's question;
3. with the five evaluators treated as false by the probe, the default flash is returned, and link 8 then stops at
   the unbound **`InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`**. The script calls that
   right after the muzzle flash.

So after this native, link 8's next stops are, in order:
1. the load question (section 4, rule 1);
2. `AttributeExpressionEvaluator.Evaluate` in the equip run, with its attributes `WeaponDamage_Is_*` on the weapon;
3. `GetExpLevelFromManufacturerData`.

## Open questions

1. **UNVERIFIED:** that `Evaluate` here is the same per-class virtual the script's `ExpressionEvaluator.Evaluate`
   reaches. Every evaluator class's script `Evaluate` goes through one shared native thunk, so by fit it is.
2. **Not searched:** the readers of `MuzzleFlashPSTemplate`.
3. **For the hub:** the load question of section 4, rule 1.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `EffectCollectionDefinition.GetParticleEffect` (pre and post) while a Bandit assault rifle is built. Expect
   one call on the weapon type's collection with the weapon as argument, and the default flash as the result for a
   non-elemental gun.
2. The same with a corrosive (or shock) variant of the gun: expect that element's flash, and expect the hook on
   `AttributeExpressionEvaluator.Evaluate` to fire once per entry up to the first that passes, and no more.
3. Read the weapon's `MuzzleFlashPSTemplate` after the build, and compare.
