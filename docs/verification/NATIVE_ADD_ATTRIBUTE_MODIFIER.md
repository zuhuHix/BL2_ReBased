# `AttributeDefinitionBase.AddAttributeModifier` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-ADD-ATTRIBUTE-MODIFIER (decision
`docs/decisions/2026-11/2026-11-19-216-add-attribute-modifier-on-link-9.md`, number pending). Analyst only: a different
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
- a local probe of the installed slice gate (section 4), never pushed;
- NATIVE_RESOLVE_CONTEXT.md section 5 at 871f3ccc, whose binding rulings section 3 adapts.

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
- **Loads and draws:** an existing entry and a property add load and draw nothing of their own. A **missing noun
  entry** is created from its `DefaultValue`, and that evaluation can load, run resolver script, make other noun
  entries and draw `AttributeVariance` (NATIVE_NOUN_ATTRIBUTES.md section 5: the entry is created from `d` first).
  The storage lookups also load an unloaded context or resolver handle on the port's read path (section 3).

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
2. **No preflight.** `storage_at` loads each resolver handle (`eval::ensure_loaded`) before it writes, and both
   storage lookups load the context. A lazy load counts as a write under the current rules. `check_modifier_chain`,
   the read-only chain check, also loads, on an older ruling: the resolver, the context (`entry_with`), and on a
   missing noun entry the default's definitions (`preflight_init`, `preflight_read`). So every one of those must
   refuse before anything changes.
3. **The noun write's own refusals** (NotPorted or NotRead from the replication table) come from `Storage::Noun` and
   pass through as they are. On a chain of more than one entry, one of them could follow an earlier entry's write.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** The hub's rulings on review r1 of #607 are folded in. The binding follows #604's
final rulings (NATIVE_RESOLVE_CONTEXT.md section 5 at 871f3ccc).

**Prerequisite: #523** (noun replication; accepted, not yet on main). On main, `NounAttributeValueResolver` refuses
replication strategies 1 and 2 on a provider with `NotYet { NotPorted }` (`ow-attr` `value_resolvers/noun.rs`, the
`replication_not_read` refusal of its strategy match). `IsPhaselocked` is replicated, so link 9's add needs #523's
mirror. Without it the probe below reports `NotPorted` (counted in the write, below).

**One shared function, two bindings** (adapted from #604):
- **The function.** One public function holds the preflight below and the add. Its logic goes in `ow-attr`, as an
  `AttributeSystem` method on an `EvalCx`. The native wrapper is a public function in `ow_inventory::equip::natives`
  (today's `add_modifier`), which both packs can reach.
- **Bound twice, either order.** `ow_skills::action_skill::Pack::register` binds it next to `ResolveContext`, through
  the keep-existing bind (`ow_combat::hit::bind_fn`, decision 2026-11-19-127). The equip pack binds it through the same
  keep-existing rule. `ow-inventory` sits below `ow-combat`, so it checks `vm.is_bound` itself; today its `register`
  binds without that check. Whichever pack registers first binds it, and the other does not fail.
- **Reference** keeps today's equip native and is unchanged.
- **`RemoveAttributeModifier`** (hub ruling): it stays unbound in skill worlds and out of scope (open question 4).

**The equip run changes in Off.** It now gets this preflight as well. A context, definition or resolver handle that the
equip run's add used to load on the way is now refused `NotYet { NotRead }`. The implementer runs the equip-run gates
and reports what moved:
- `cargo test -p ow-inventory` (the equip natives' tests) and `cargo test -p ow-cli` (the equip run);
- the installed equip run (`ow-package --inventory-move ... --equip-run`);
- the installed Off strict `--slice-gate` row of link 8 (`equip_gun`).

**The preflight** (write-free, load-free, RNG-free), in this order. The first failure is the answer:
1. **The receiver** is set, and its class is `AttributeDefinitionBase` or a child (the class table's `by_path` only).
   Otherwise a `VmError::Script`.
2. **The None cases, before the definition is read** (hub ruling, today's behaviour): `Modifier` None or `Context`
   None answers **false** at once, with nothing read or written. `bSuppressNotify` that is not a bool is treated as
   false, as today; it is not an error. `Context` or `Modifier` of another shape than an object is a
   `VmError::Script`.
3. **A1, an unloaded definition:** `NotYet { NotRead }`, "AttributeDefinitionBase.AddAttributeModifier: the definition
   <path> is an unloaded handle (a load is a write)".
4. **A0, an unloaded context:** the `Context` is an unloaded handle (`vm.defs.is_unloaded`): `NotYet { NotRead }`,
   "AttributeDefinitionBase.AddAttributeModifier: the context <path> is an unloaded handle (a load is a write)". Both
   value resolvers' storage lookups load the context (`entry_with` in the noun resolver, `target` in the property
   resolver), so neither may be reached with one.
5. **The chain.** `ValueResolverChain` is declared as an array of objects, otherwise a `VmError::Script` naming the
   field. An empty chain answers false. Only the entries before the first None entry are checked, since the add stops
   there with false.
6. **A2, an unloaded resolver, for every entry first:** the first unloaded handle refuses `NotYet { NotRead }`,
   "AttributeDefinitionBase.AddAttributeModifier: the value resolver <path> is an unloaded handle (a load is a
   write)".
7. **A3, an unregistered resolver class, for every entry after that:** the registry's existing `NotYet { NotRead }`,
   "value resolver class <path>". So in a chain of an unregistered loaded resolver followed by an unloaded one, the
   answer is A2.
8. **The storage probes, for every entry last:** `check_modifier_chain`'s read-only check for an add, made load-free
   throughout. Today it can load in three places, and each must refuse where it would load:
   - the resolver and context handles (A2 and A0 above);
   - `entry_with`;
   - on a missing noun entry, `preflight_init` on `DefaultValue`, which loads its `InitializationDefinition` and,
     through `preflight_read`, each `BaseValueAttribute`. A handle there that is not loaded refuses `NotYet { NotRead
     }`, "AttributeDefinitionBase.AddAttributeModifier: a default value's <field> <path> is an unloaded handle (a load
     is a write)".

   The probes' own refusals pass through unchanged, the noun replication table's among them.
9. Then the write: `ow-attr`'s `add_modifier`, and the answer is `succeeded()`.

**What the write may still do** (why the next rule is needed): on a **missing noun entry** the write evaluates
`DefaultValue` (noun.rs, the entry-creation path). That evaluation can:
- run resolver script;
- make other noun entries;
- draw `AttributeVariance` from the generator.

An existing entry and a property add load and draw nothing once the preflight has passed.

**Refusals once the write starts are terminal** (#604's rule, word for word): once the write starts, **any `NotYet` is
terminal**. It goes through `after_partial_writes` (its text prefixed "stopped after partial writes: " once, its kind
kept), and nothing is put back. Any other error is returned unchanged. The preflight's own refusals (A0 to A3, the
probes' refusals and the `VmError::Script` cases) come before the write and are never prefixed inside this native. On
link 9 the script made the new modifier first, so the caller's counters mark those refusals anyway.

**Tests** (invented classes and definitions, rule 9). Each refusal before the write also checks #518's three counters
unchanged (heap committed writes, store load transitions, generator draws):
1. **A property attribute:** the modifier joins the stack and the value is recomputed. The notification is raised,
   and not raised with `bSuppressNotify` set.
2. **A noun attribute on a pawn,** split:
   - **2a, strategy 0** (works on main): the entry is made, the modifier joins its stack, and no notification is raised;
   - **2b, a replicated strategy** (after #523): the value is mirrored into the replicated slot with `bNetDirty`.
3. **The same modifier twice:** false the second time, nothing changed. A None modifier or a None context: false, with
   zero reads of the definition (an unloaded definition does not refuse). A non-bool flag is treated as false.
4. **A constant-resolver attribute:** false, nothing changed.
5. **An empty chain, and a None entry first:** false.
6. **A0, A1, A2 and A3,** each before any write: the counters unchanged, and the handle still unloaded afterwards.
   - The mixed chain: an unregistered loaded resolver, then an unloaded one, gives A2.
7. **A missing noun entry whose default's `InitializationDefinition` is an unloaded handle:** refused in the probe,
   nothing loaded.
8. **A missing noun entry whose default has variance:** the add succeeds, and the generator's draw is counted in the
   write, not in the preflight (the counters before and after the preflight are equal).
9. **A two-entry chain whose second entry refuses in the storage probe:** refused before the first entry writes.
10. **Both registration orders:** the skill pack first, then the equip pack, and the reverse. Each binds once and
    neither fails; a key bound first keeps its binding. Reference binds nothing new.

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
   They may need the same refusals.
4. **`RemoveAttributeModifier`** (hub ruling on #607): unbound in skill worlds and out of scope. When a link reaches
   it, it needs its own preflight (`ProbeOp::Remove`), on this note's pattern.
