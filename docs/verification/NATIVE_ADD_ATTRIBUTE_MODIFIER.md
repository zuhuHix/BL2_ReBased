# `AttributeDefinitionBase.AddAttributeModifier` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-ADD-ATTRIBUTE-MODIFIER (decision
`docs/decisions/2026-11/2026-11-19-NNN-add-attribute-modifier-on-link-9.md`, number pending). Analyst only: a different
lane implements it (clean room).

**A short note: the records already settle the rule.** No new reading of the executable was needed:
- NATIVE_ATTRIBUTES.md section 1 reads the native, and section 3 reads the property-level add (confidence high);
- NATIVE_NOUN_ATTRIBUTES.md sections 5 and 10 read the noun add, which link 9 reaches;
- NATIVE_RESOLVE_CONTEXT.md (#604) places the call on link 9.

This note says what those records already give, what the port's code does with them, what link 9 needs on top, and
where link 9 goes next.

**Sources:**
- those three records;
- `ow-attr` (`effects.rs`: `add_modifier`, `add_chain`, `storage_at`, `check_modifier_chain`);
- the equip pack's binding (`crates/ow-inventory/src/equip/natives.rs`);
- a local probe of the installed slice gate (section 4), never pushed.

There is no bl-sdk session. **Every rule here is UNVERIFIED** in the running game.

## 1. What the native does (the records, in short)

`native final function bool AddAttributeModifier(Object Context, AttributeModifier Modifier, optional bool
bSuppressNotify)`:
- **`Context`** is the context **already resolved** (`ResolveContext`'s answer), not a source. None answers false.
- **The list it adds to** is decided by each entry of the definition's `ValueResolverChain`, in order. Each entry is
  asked only while the ones before it succeeded. A None entry counts as a failure, and so does an empty chain. Per
  entry:
  - an `ObjectPropertyAttributeValueResolver`: the property's own modifier stack on the context. The value is then
    recomputed (NATIVE_ATTRIBUTES.md section 3);
  - a `NounAttributeValueResolver`: the noun entry's `ModifierStack` on the provider. The entry is created on a miss
    where the table says so; the value is recomputed and mirrored into the replicated slot, which sets `bNetDirty`
    (NATIVE_NOUN_ATTRIBUTES.md sections 5, 10.3 and 10.5). This **supersedes** NATIVE_ATTRIBUTES.md section 1's "every
    other resolver class answers false" for the noun resolver;
  - every other resolver class: false, and nothing changes.
- **The same modifier object twice:** false, nothing changes (identity, not equal values). A None modifier: false,
  nothing looked up.
- **`bSuppressNotify`** suppresses only the attribute-change notification of NATIVE_ATTRIBUTES.md section 4, which a
  property change raises. A noun change raises none, whatever the flag.
- **The answer** is the chain's success. An earlier entry's change is not rolled back when a later entry fails.
- **Loads and draws:** none of its own.

## 2. The port's code, and link 9

**`ow-attr`'s `add_modifier`** is that rule: `add_chain` over the value chain, `Storage::Stack` for properties and
`Storage::Noun` for nouns.

**The equip pack** binds it (`equip::natives::ADD_MODIFIER`): a None modifier answers false, and the bool is
`succeeded()`. That fits link 9's call, which is:
- in `ApplyAttributeEffects`, from `LiftActionSkill.PhaseLockTarget`;
- on `GD_Siren_Skills.Attributes.IsPhaselocked`, whose one value resolver is a `NounAttributeValueResolver`;
- with the target `WillowAIPawn` as the context and a new `MT_PostAdd` modifier;
- with `bSuppressNotify` false.

**What it lacks for link 9:**
1. **The binding's place.** The equip pack registers only in the equip worlds. The cast's world does not bind it, so
   link 9 meets an unbound native, as it did for `ResolveContext` (#604).
2. **No preflight.** `storage_at` loads each resolver handle (`eval::ensure_loaded`) before it writes. A lazy load
   counts as a write under the current rules. `check_modifier_chain`, the read-only chain check, also loads an unloaded
   resolver, on an older ruling. So an unloaded resolver must refuse before anything changes.
3. **The noun write's own refusals** (NotPorted or NotRead from the replication table) come from `Storage::Noun` and
   pass through as they are. On a chain of more than one entry, one of them could follow an earlier entry's write.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** In Off, bind `AttributeDefinitionBase.AddAttributeModifier(object,object,bool)`
with the equip pack's body, in the worlds that run skill scripts. Put it next to `ResolveContext` (#604), and bind
`RemoveAttributeModifier` with it. A key bound first keeps its binding. Reference keeps the stub policy, unchanged.

**Preflight** (write-free, load-free, RNG-free), in this order. The first failure is the answer:
1. **The receiver** is set, and its class is `AttributeDefinitionBase` or a child (the class table's `by_path` only).
   Otherwise a `VmError::Script`.
2. **A1, an unloaded definition:** `NotYet { NotRead }`, "AttributeDefinitionBase.AddAttributeModifier: the definition
   <path> is an unloaded handle (a load is a write)".
3. **The arguments.** `Context` and `Modifier` are each an object or None, and `bSuppressNotify` is a bool when given.
   Another shape is a `VmError::Script`.
   - `Modifier` None or `Context` None: answer **false** at once, with nothing checked or written (the record).
4. **The chain.** `ValueResolverChain` is declared as an array of objects, otherwise a `VmError::Script` naming the
   field. An empty chain: answer false.
5. **Per entry, up to the first None entry** (the add stops there with false):
   - **A2, an unloaded resolver:** `NotYet { NotRead }`, "AttributeDefinitionBase.AddAttributeModifier: the value
     resolver <path> is an unloaded handle (a load is a write)";
   - **A3:** a resolver class `ow-attr` does not implement is the registry's existing `NotYet { NotRead }`, "value
     resolver class <path>";
   - **the storage probe:** `check_modifier_chain`'s read-only check for an add, once it is made load-free (A2 first).
     Its refusals pass through unchanged, the noun replication table's among them.
6. Then the write: `ow-attr`'s `add_modifier`, and the answer is `succeeded()`.

**The counters:** the preflight's refusals come before anything is written, so they are unmarked inside the native. A
`NotYet` raised during the write, after the first change, is marked "stopped after partial writes: " by #518's
counters. On link 9 the script made the modifier first, so the cast's own counters mark any refusal there.

**Tests** (invented classes and definitions, rule 9):
1. A property attribute: the modifier joins the stack and the value is recomputed. The notification is raised, and not
   raised with `bSuppressNotify` set.
2. A noun attribute on a pawn: the entry is made, the modifier joins its stack and the value is mirrored with
   `bNetDirty`. No notification.
3. The same modifier twice: false the second time, nothing changed. A None modifier or None context: false, with
   zero writes.
4. A constant-resolver attribute: false, nothing changed.
5. An empty chain, and a None entry first: false.
6. A1, A2 and A3, each before any write: #518's three counters unchanged, and the handle still unloaded afterwards.
7. A two-entry chain whose second entry refuses in the storage probe: refused before the first entry writes.
8. A key bound first keeps its binding, and Reference binds nothing.

## 4. The probe (local, never pushed)

A local tree at the resolve-context lane's probe commit, which merges `origin/main` 3ad58b0 with #579, #586, #564,
#548, #555, #560, #523 (counter conflicts on main's side, except its noun resolver), #595, #601 and
`fix/rust-gearboxmind-queryinterface`.

**Probe-only stand-ins:**
- `GearboxMind.GetAIComponent` bound where link 9 runs;
- the AI components' `QueryInterface` answered by main's component rule;
- the mind's AI component `Owner` set to the mind;
- `WillowAIComponent.PhaseLockOnHold` answering false;
- `ResolveContext` as `ow-attr`'s `resolve_context` (#604's stand-in);
- **this lane's:** `AddAttributeModifier` as the equip pack's body, logged.

**Installed Off `--slice-gate`, strict:** ok 2, notyet 9, error 3 of 14.
- **Link 9** passes 1 of its 2 driver steps (`spawn`) and stops at `cast` with an `error`: the unbound native
  `WillowAIPawn.IsStaggered()`.
- The one add was:
  - on `IsPhaselocked`, with the target `WillowAIPawn` as context, a `Core.AttributeModifier`, and `bSuppressNotify`
    false;
  - answered **true**;
  - 3 committed heap writes (the noun entry, its stack and the mirror, by the noun note), no load and no draw.

So link 9's next stop is `WillowAIPawn.IsStaggered()`.

## Open questions

1. **Not re-read here:** the property and noun bodies (their records' confidence stands).
2. **UNVERIFIED:** that the cast's later script reads the noun's mirrored value as the probe's port does.
3. **To check (implementer):** making `check_modifier_chain` load-free changes its other callers (the internal slots).
   They may need the same A2 refusal.
