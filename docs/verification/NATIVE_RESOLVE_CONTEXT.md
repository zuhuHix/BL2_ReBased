# `AttributeDefinitionBase.ResolveContext`: an attribute definition's context on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-RESOLVE-CONTEXT (decision 2026-11-19-NNN, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` (every function of the nine code packages searched for
  the call);
- the installed packages through `ow-package --def-dump` (the slice skill's effect and its attribute definition; names
  and counts only);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local probe of the installed slice gate on a merge that is never pushed (section 6).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/rc/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**What was already covered.**
- NATIVE_ATTRIBUTES.md section 1 describes the native: it reads the definition's `ContextResolverChain`, confidence
  high. Its section 5 lists the context resolvers.
- `ow-attr` implements the chain (`eval::resolve_context`) and the resolvers (`context_resolvers`).
- The native is bound in Off only by the equip run's pack (`ow_inventory::equip::natives`, `RESOLVE_CONTEXT`) and by
  the inventory harness (`ow-cli`). Link 9's world registers neither, so the cast meets it unbound (section 6).

This note re-reads the native's chain rule in the executable, places the call on link 9, and gives the Off binding
for a world that runs skill scripts.

## 1. Callers (script)

`native final function Object ResolveContext(Object ContextSource)`. It has **one script caller**,
`AttributeEffect.ApplyAttributeEffects` (two call sites), and both calls are **final** calls to
`AttributeDefinitionBase`'s native. So `FlagDefinition`'s own `ResolveContext` native (section 3) is never reached
from script.

`ApplyAttributeEffects(ContextSource, InAttributeEffects, out OutModifiers, OptionalOverrideContextSource)` does, for
each effect in order:
1. makes a new `AttributeModifier`, outered to `ContextSource` (**a write**). It sets the modifier's `Type` from the
   effect's `ModifierType`, and its `Value` from `AttributeInitializationDefinition.EvaluateInitializationData`;
2. sets the applied effect's `Context` to None and its `Attribute` to the effect's `AttributeToModify`;
3. with an override source: `Context = Attribute.ResolveContext(Override)`;
4. if `Context` is still None: `Context = Attribute.ResolveContext(ContextSource)`;
5. with a `Context`: `Attribute.AddAttributeModifier(Context, Modifier, ...)`, and on success the applied effect is
   appended to `OutModifiers`.

So this native is always reached **after** the script's own writes of the same effect.

**On link 9.** `LiftActionSkill.PhaseLockTarget` calls `ApplyAttributeEffects(NewTarget, PhaselockedAttributeEffects,
SavedPhaselockAttributeModifiers)` with no override. Its target is the cast's target pawn.

## 2. The rule (read from the executable)

The exec step reads its one object argument (`ContextSource`), runs the definition's chain walk, and returns its
answer. **The chain walk**, on the definition `D` and the source `S`:
1. the context so far starts as `S`, and the answer starts as None;
2. for each entry of `D.ContextResolverChain`, in order:
   - a None entry is skipped;
   - otherwise the resolver is asked for its context (its native `GetAttributeContext` virtual) with **the definition
     `D` and the context so far**;
   - a None answer **ends the walk with None**;
   - otherwise the answer becomes both the context so far and the answer;
3. the answer is returned.

**Edge cases that follow:**
- A chain with no non-None entry (empty, or only None entries) answers **None**, not `S`.
- A None `S` is handed to the first resolver as it is; what each resolver does with it is its own rule (NATIVE_ATTRIBUTES.md
  section 5).
- A None entry after a resolver that answered leaves that answer standing.

**What it touches.** The walk reads `ContextResolverChain` and nothing else of `D`. It writes nothing, loads nothing,
draws no random number and draws nothing on screen. What a resolver reads is the resolver's own rule.

**`ow-attr` agrees.** `eval::resolve_context` walks the chain the same way: None entries skipped, a None answer
returning None, no answering resolver returning None.

## 3. `FlagDefinition.ResolveContext`, for comparison (not on link 9's script path)

`FlagDefinition` registers its own `ResolveContext` native with a different walk:
- each resolver is asked with **no definition** (None) and the context so far;
- a None answer **keeps the context so far**;
- a chain with no answering resolver returns the source itself.

The same walk is reached natively from the flag natives (`SetValue`, `SetTrueTimed` and others) and from the flag
evaluator's lookups. Its "the context so far stays" agrees with NATIVE_FLAG_EXPRESSION_EVALUATE.md section 4.0, step 2;
the "no definition passed" detail is new here. It matters for the flag evaluator lane (#535), not for this native.

## 4. Link 9's call (names and counts only)

`ActionSkill_Phaselock.PhaselockedAttributeEffects` holds **one effect**: `GD_Siren_Skills.Attributes.IsPhaselocked`,
`MT_PostAdd`, base value constant 1. The definition:
- an `AttributeDefinition`, simple;
- its `ContextResolverChain` holds one entry, a `PawnAttributeContextResolver`;
- its `ValueResolverChain` holds one entry, a `NounAttributeValueResolver`.

With the source the target (a `WillowAIPawn`), the Pawn resolver answers the pawn itself. That is NATIVE_ATTRIBUTES.md
section 5's rule ("a pawn -> itself", confidence medium), implemented in `ow-attr`'s `context_resolvers::actors`; its
body was not re-read here. So the answer on link 9 is **the target pawn**, as the probe also gave (section 6).

## 5. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `AttributeDefinitionBase.ResolveContext(object)` in Off in the worlds that
run skill scripts, link 9's included. Use the same native the equip pack binds, and let a key bound first keep its
binding. Reference keeps the stub policy (or the C++ stand-in where one exists) and is unchanged.

**Preflight (write-free, load-free, RNG-free), in this order.** The first failure is the answer:
1. **The receiver** `this`: set, and its class `AttributeDefinitionBase` or a child, looked up with the class table's
   `by_path` only. Otherwise a `VmError::Script`.
2. **R1, an unloaded definition.** `this` is an unloaded definition handle (`vm.defs.is_unloaded`): `NotYet` with
   missing kind `NotRead`, "AttributeDefinitionBase.ResolveContext: the definition <path> is an unloaded handle (a
   load is a write)". `ow-attr`'s read path loads such a handle (`eval::ensure_loaded` through `property`), so it must
   not be reached with one.
3. **The argument** `ContextSource`: an object or None. Any other shape is a `VmError::Script`.
4. **The chain.** `ContextResolverChain` must be declared as an array of objects. Otherwise a `VmError::Script` naming
   the field.
5. **R2, an unloaded resolver.** Each non-None entry must be resident: an unloaded handle refuses `NotYet` with missing
   kind `NotRead`, "AttributeDefinitionBase.ResolveContext: the context resolver <path> is an unloaded handle (a load
   is a write)".
6. **R3, a resolver class not implemented.** Each non-None entry's class must be one `ow-attr` implements. Otherwise it
   is the registry's existing `NotYet { NotRead }`, "context resolver class <path>" (NATIVE_ATTRIBUTES.md section 5).
   The same check is `preflight_read`'s context half.

**The answer:** `ow-attr`'s `resolve_context` (section 2's walk), returned as the object or None. Once the preflight
has passed, nothing it touches needs a load.

**The counters.** The native's refusals come before anything it could write, so none carries the "stopped after
partial writes: " prefix from inside it. On link 9 the script made the new modifier first (section 1, step 1), so a
refusal there is marked by the caller's counters, as every NotYet of the cast is.

**Not in scope here:** `AttributeDefinitionBase.AddAttributeModifier`, the next stop on link 9 (section 6). It is bound
by the same equip pack, from NATIVE_ATTRIBUTES.md section 1. The same world-level binding question applies to it and
to `RemoveAttributeModifier`, so an implementer may bind the three together; their rules are not changed here.

**Tests for the implementer** (synthetic, the existing `package()` / `tag()` helpers; every value invented). Each
refusal case also checks #518's counters unchanged (heap committed writes, store load transitions, generator draws):
1. a definition with one Pawn resolver and a pawn source: the pawn;
2. a controller source through the same chain: the controller's `Pawn` (the resolver's rule, unchanged);
3. a two-resolver chain where the second answers None: None;
4. an empty chain and a chain of None entries: None, even with a source;
5. a None entry after an answering resolver: that answer;
6. an unloaded definition handle: R1, still unloaded afterwards;
7. an unloaded resolver entry: R2, still unloaded afterwards;
8. a resolver class `ow-attr` does not implement: R3;
9. no receiver, a receiver outside `AttributeDefinitionBase`, and a non-object argument: `VmError::Script` each;
10. under `ParityMode::Reference`: nothing new bound.

## 6. The probe (local, never pushed)

A local merge of `origin/main` with:
- #579, #586, #564, #548, #555, #560 and #523 (link 9's stack, with #523's counter conflicts kept on main's side
  except its noun resolver);
- #595, #601 and `fix/rust-gearboxmind-queryinterface`.

Probe-only stand-ins:
- `GearboxMind.GetAIComponent` bound where link 9 runs;
- `WillowAIComponent` and `AIComponent` answering by main's component rule (NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md's
  proposal);
- the target mind's AI component `Owner` set to the mind;
- `WillowAIComponent.PhaseLockOnHold` answering false.

**Installed Off `--slice-gate` strict.** Link 9 passes 1 of its driver's 2 steps (`spawn`) and stops at `cast`:
- **without a `ResolveContext` binding:** 2 ok / 9 NotYet / 3 errors. Link 9 is an error at the unbound
  `AttributeDefinitionBase.ResolveContext(object)`;
- **with a stand-in for it** (the definition and source logged, then `ow-attr`'s `resolve_context`): the one call is
  `IsPhaselocked` (already loaded) with the target `WillowAIPawn` as its source, and it answers that pawn. Link 9 then
  stops at the unbound `AttributeDefinitionBase.AddAttributeModifier(object,object,bool)` (still 2 / 9 / 3).

## Open questions

1. **Not re-read:** the Pawn resolver's own body (NATIVE_ATTRIBUTES.md section 5, medium), and the other resolvers'
   bodies.
2. **Not read:** which other callers reach `FlagDefinition`'s walk, and what each of those natives does with the
   answer (section 3).
3. **UNVERIFIED:** that no other script path on the slice reaches this native with a definition whose chain needs a
   resolver `ow-attr` does not implement.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. During a Phaselock on an enemy, hook `ResolveContext` (pre and post) on `IsPhaselocked`: expect the target pawn in
   and the same pawn out.
2. Call `ResolveContext` on a definition whose chain is empty, with a pawn: expect None.
3. Call it on a definition whose second resolver answers None: expect None.
