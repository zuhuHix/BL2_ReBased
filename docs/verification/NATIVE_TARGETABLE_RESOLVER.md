# TargetableAttributeValueResolver: the champion test, and links 3 and 4 after it (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-TARGETABLE-RESOLVER (decision 2026-11-26-41), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/targetable/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** NATIVE_MAYA9_AND_AFTER.md predicts that once link 1 passes, links 3, 4, 13 and 14 reach their own spawn steps
and stop at `TargetableAttributeValueResolver`'s Off get, which no record had read (NATIVE_ATTRIBUTES.md, "Not read
yet"). Link 5 stops with link 3. This note reads that get, what it answers for the slice's enemies, and what links 3 and
4 meet after it.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The natives** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the resolver's get and get-base slots (the value-resolver slots of the ow-attr records);
  - the interface cast helper the get uses;
  - `IsChampion`'s exec on `Pawn`, `WillowProjectile` and `WillowInteractiveObject`. `WillowAIPawn` and `WillowVehicle`
    share `Pawn`'s;
  - the `ITargetable` table that each of `Pawn`, `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn` installs. Each was
    found from the class's registration routine and its default constructor, by the documented method
    (`docs/NATIVE_ANALYSIS.md`, "When `virtual:` refuses a class"), with a throwaway helper kept under
    `local/analysis/targetable/scripts/` (not tracked). The constructors' object sizes match the registration's sizes;
  - `IsChampion`'s body in those tables;
  - the balance definition's champion virtual on `BaseBalanceDefinition`, `InventoryBalanceDefinition` and
    `AIPawnBalanceDefinition`. The base names the virtual in its own error text: `DoesSpawnChampion`.
- **Field places** (the local copy of `tools/ghidra/class_layout.py`). Every field the routines read lands exactly on a
  declared property, with no shift:
  - the resolver's `ValueToResolve`;
  - `Pawn`'s `ITargetable` table slot and its `BalanceDefinitionState`;
  - `AIPawnBalanceDefinition`'s `Champion` bool.
- **The packages** (`ow-package`, local, names only):
  - `TargetableAttributeValueResolver.ETargetableValue` has two values, `TARGETABLE_IsChampion` and `TARGETABLE_MAX`;
  - `D_Attributes.Targetable.Targetable_Is_Champion` has one context resolver, a `NoContextNeededAttributeContextResolver`,
    and this resolver as its only value resolver;
  - only three classes carry a native `ITargetable` table: `Engine.Pawn` (and so every pawn class),
    `WillowGame.WillowInteractiveObject` and `WillowGame.WillowProjectile`. No controller or mind class does;
  - the slice's two enemy balances, `PawnBalance_PrimalBeast_Monglet` (link 3) and `PawnBalance_PrimalBeastAdult`
    (link 4), are `AIPawnBalanceDefinition`s with `Champion` false.
- **A local probe** (temporary code, reverted, never committed). The installed Off `--combat-run` ran on main 5dd2906,
  under the machine lock, with link 3's steps and link 4's steps, each after a `player:<level>` step. Both spawns stop
  at today's `NotRead`. Then the get was stubbed to log its context's class and answer 0.0, to see what comes next.

## 2. What the get does

**The get** (the value) and **the get-base** (the base value) answer the same: the get-base only calls the get. With
the context the resolver is asked about:
1. **the cast:** the context is cast to `Engine.ITargetable` through the engine's native interface lookup. A None
   context, or an object whose class has no native `ITargetable` table (a class that implements the interface only in
   script counts as none), fails the cast;
2. **the field:** the resolver's `ValueToResolve` must be `TARGETABLE_IsChampion` (0). Any other value answers 0;
3. **the question:** the targetable's `IsChampion()`, through its native `ITargetable` table;
4. **the answer:** **1.0 when the cast succeeds, the field is 0 and `IsChampion` answers true; else 0.0.**

It writes nothing, loads nothing and raises no event. Its add, remove and set-base were not read.

## 3. `IsChampion`, by class

The script `ITargetable.IsChampion` of each implementer is a native whose exec asks the same table slot the get asks.
The bodies in the tables:
- **`Pawn`, `WillowPawn` and `WillowPlayerPawn`:** false. The slot holds the engine's shared "return 0" body;
- **`WillowAIPawn`:**
  - its `BalanceDefinitionState.BalanceDefinition` None: false;
  - otherwise that definition's `DoesSpawnChampion()`, a virtual of the definition's class;
- **`DoesSpawnChampion`:**
  - **`AIPawnBalanceDefinition`:** its `Champion` bool;
  - `BaseBalanceDefinition` and `InventoryBalanceDefinition`: the base, which logs "pure virtual not implemented" and
    answers false;
  - any other balance class: not read;
- **`WillowVehicle`, `WillowProjectile`, `WillowInteractiveObject`** and the other pawn classes with their own table
  (`GearboxPawn` among them): not read.

So for an AI pawn, "champion" is the champion flag of the balance it was spawned from.

## 4. The slice's case (links 3 and 4)

The probe shows the context that reaches the get: **the enemy's `WillowGame.WillowMind`**, not its pawn. The
attribute's context chain is a single `NoContextNeeded` resolver, which passes the evaluation's source through, and the
spawn evaluates it on the mind. The caller was not identified.

`WillowMind` has no native `ITargetable` table, so the cast fails and **the get answers 0.0** for both links. That
holds whatever the balance says. Both balances' `Champion` is false anyway, so on a pawn context the answer would be 0.0
too.

**The port's spawn never fills `BalanceDefinitionState`.** No crate writes it, though in the game the spawn's balance
code does. A pawn context would therefore always read a None balance and answer false here. That is right for these two
balances, but only by their data.

## 5. Proposed Off answer (for the implementer)

In `ow-attr`'s `value_resolvers::targetable`, for the value and the base value alike. Reference keeps its C++ stand-in.

**Preflight**, write-free and load-free; the first refusal met is the answer:
0. a resolver the store has not loaded: `NotRead`, "TargetableAttributeValueResolver: a resolver the store has not
   loaded (a load is a write)". The hub's ruling (review of #783): this note did not read whether the evaluator loads
   the resolver first, so the get does not load it. **Section 5.1 now reads that** (lane
   ANALYST-EVALUATOR-LOADS-RESOLVER): with the walk's load it proposes (review r1 of #787), this step is a guard that the evaluator's
   path no longer meets;
1. `ValueToResolve` declared as a byte on the resolver's class, else `VmError::Script`; not 0: **0.0**;
2. a None context: **0.0**;
3. a context the store has not loaded (a handle, or a stand-in without its slots): `NotRead`,
   "TargetableAttributeValueResolver: a context the store has not loaded (a load is a write)";
4. **the cast, by the context's class chain** (the class table only, nothing loaded). The chain holds none of
   `Engine.Pawn`, `WillowGame.WillowInteractiveObject` and `WillowGame.WillowProjectile`: **0.0**;
5. **`IsChampion`, by the context's first native class:**
   - `Engine.Pawn`, `WillowGame.WillowPawn`, `WillowGame.WillowPlayerPawn`: **0.0**;
   - `WillowGame.WillowAIPawn`:
     - `BalanceDefinitionState`, a struct with an object `BalanceDefinition`, declared, else `VmError::Script`;
     - `BalanceDefinition` None: **0.0**;
     - a balance the store has not loaded: `NotRead`, "TargetableAttributeValueResolver: a balance definition
       the store has not loaded";
     - a balance whose first native class is `WillowGame.AIPawnBalanceDefinition`: its `Champion` (declared as a bool,
       else `VmError::Script`), **1.0 or 0.0**;
     - any other balance class: `NotRead`, "TargetableAttributeValueResolver: DoesSpawnChampion of <class>".
       The base's logged-error answer is not taken as a rule;
   - any other first native class: `NotRead`, "TargetableAttributeValueResolver: IsChampion of <class>".

Nothing is written, so no answer carries the partial-writes prefix. The spawn's own writes come before, so a refusal
here reaches the slice as "stopped after partial writes", as today's does. Add, remove and set-base keep their
`NotRead`.

### 5.1 Step 0: is the resolver resident with its attribute? (addendum, 2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-EVALUATOR-LOADS-RESOLVER (decision 2026-11-26-45). Read with Ghidra
12.1.4, headless and read-only, under the machine lock; raw output under ignored `local/analysis/resid/`. UNVERIFIED in
the running game.

**What the game's evaluator does.** The attribute-initialization evaluator reaches an attribute's value through two
walks on the attribute definition (the field places land exactly on `AttributeDefinitionBase`):
- the context walk over `ContextResolverChain`;
- the value walk over `ValueResolverChain`.

Each walk takes every non-None entry and calls its resolver's own virtual directly. **Neither walk loads anything,
and neither checks that an entry is loaded.** The `ResolveContext` and `GetValueFromContext` execs reach the same two
walks.

**Why that is safe in the game.** The engine has no lazy object handle. The resolver is an object of the attribute's
own package (here a subobject of `Targetable_Is_Champion`, in `Startup.upk`). It is referenced from the definition's
chain, so the engine's package load creates and serializes it with the definition, before the definition can be read.
That load order is the engine's standard package load, by fit, not read in this build. In addition, `Startup.upk` is
loaded whole at start-up. **The resolver is resident whenever its attribute is**, so the game never meets step 0's
case.

This claim covers only a resolver in the attribute's own package. A chain entry imported from another package is
resident only if that package is loaded, which this reading did not check: the claim is UNVERIFIED there (review r1 of
#787). The rule below loads such an entry all the same.

**Why the port meets step 0 today.** The port's definition store is lazy: loading one definition never loads what it
references, a subobject included, and loads never nest. So the attribute can be resident while its chain still holds
an unloaded handle for the resolver. That is the state #786 refuses at step 0 (review r1 of #787). The store's rule
stays as it is; the fix belongs in the evaluator's walk.

**Proposed Off answer** (the port's evaluator treats it so; hub ruling on #787):
- **the walks load each entry before they dispatch it.** `ow-attr`'s two chain walks on the attribute definition:
  - the context walk (`resolve_context`);
  - the value walk (`fold_chain`), and `preflight_read`'s walk over the same chains.

  For each non-None entry, the walk calls `ow-attr`'s follow-on `ensure_loaded` on the entry before it reads or
  dispatches it. That load goes through the existing `load_free_refusal` check, as the other loads of the evaluator
  do. This does not depend on how the attribute became resident: `ow-attr`'s own `ensure_loaded`, the VM's
  `loaded_for_read`, the context resolvers' `ensure_loaded`, a direct `DefStore` call or a harness preload all leave
  the same state, and the walk covers each of them;
- **in a load-free scope** (`preflight_read_resident`, `load_free`), nothing changes. An unloaded entry refuses through
  `load_free_refusal` (`NotRead` naming it), as today;
- **a resolver whose load fails** (hub ruling on #787): the failed handle is left in place, with no new refusal point.
  The walk's existing residency check then reports the store's `BadData` where the entry is reached;
- **step 0 stays** as a guard for a get reached outside the walks. On the links' path it is no longer met, since the
  walk loads the resolver before the get;
- no harness preload is needed;
- **the count of loads changes.** A get through `ow-combat`'s `get_value`, which runs `preflight_read` first (its doc
  comment in `crates/ow-attr/src/eval.rs`), now loads the definition and each of its N unloaded chain entries: 1+N
  loads where it was 1. #518's `load_transitions` counter rises by the same amount (test 5 below).

**Test sketch** (synthetic, with `package()` / `tag()`):
1. A toy attribute definition whose `ValueResolverChain` holds a toy `TargetableAttributeValueResolver` export of the
   same package, both unloaded:
   - an evaluation loads the definition, then the resolver in the value walk;
   - the get answers by section 5 (0.0 on a mind context);
   - no `NotRead` for step 0.
2. The same with a `ContextResolverChain` entry: loaded by the context walk.
3. The attribute resident, the resolver a handle:
   - in a load-free scope: the existing `NotRead`, nothing loaded;
   - the attribute loaded by a direct `DefStore::ensure_loaded` (standing in for the VM's `loaded_for_read`), then an
     evaluation outside a load-free scope: the walk still loads the resolver, and the get answers.
4. A None chain entry: skipped by the walk's load, as the game's walks skip it.
5. #518's counters: a `get_value` on the toy of test 1 counts 1+N `load_transitions` (here 2), in the scope that
   allows the loads.
6. A resolver whose load fails (a toy export whose body is cut short): the handle stays, and the walk's residency check
   answers the store's `BadData` where the entry is reached. No refusal before it.

## 6. Tests (synthetic fixtures, for the implementing lane)

1. A toy `WillowAIPawn` context with an `AIPawnBalanceDefinition` balance:
   - `Champion` true: 1.0;
   - false: 0.0;
   - no balance: 0.0.
2. `WillowPlayerPawn`, `WillowPawn` and plain `Pawn` contexts: 0.0.
3. A toy `WillowMind` (a controller class with no native `ITargetable`): 0.0. A None context: 0.0.
4. `ValueToResolve` 1 (`TARGETABLE_MAX`) on a champion AI pawn: 0.0.
5. A `WillowVehicle`, `WillowProjectile` and `WillowInteractiveObject` context: the `IsChampion` `NotRead` naming the
   class.
6. Not loaded or not read:
   - a balance of another class (`InventoryBalanceDefinition`): the `DoesSpawnChampion` `NotRead`;
   - a lazy balance: its not-loaded `NotRead`;
   - a lazy context: its not-loaded `NotRead`.
7. The base value equals the value in each case.

   Every case leaves #518's three counters, the class-code loads and the name table unchanged.
8. Reference: unchanged (the C++ stand-in's 0.0 for `Targetable_Is_Champion`).

## 7. Predicted stops (from the probe)

- **Links 3 and 4**, with section 5 bound: the get answers 0.0 on the enemy's mind. The spawn then stops at
  **`WillowPawn.SetDefaultPhysics` "past its gates"**, the port's `NotRead`. That is NATIVE_MIND_ROUTE.md
  section 1's open collision branch: the two unnamed pawn queries, and the navigation-handle query of an AI pawn's
  default physics. It is after the spawn's writes, so prefixed.
  - Link 3's `hit` and link 4's `swing` are not reached. In the probe their errors were only the harness failing to
    name the spawned pawn after the failed spawn.
- **Link 5** stops with link 3. **Links 13 and 14** spawn an adult, as link 4 does, so the same stop is expected; not
  probed.
- In the slice gate's own run these links start where link 1 ends (NATIVE_MAYA9_AND_AFTER.md section 5). This holds once
  link 1 passes.
- **The next read:** those three unnamed queries of `SetDefaultPhysics`.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - `IsChampion` on `WillowVehicle`, `WillowProjectile`, `WillowInteractiveObject` and the other pawn classes with their
     own table;
   - `DoesSpawnChampion` on the other balance classes;
   - the resolver's add, remove and set-base;
   - which spawn step evaluates the attribute on the mind.
3. **For the hub:** the port's spawn does not fill `BalanceDefinitionState`. A later lane may need it for any rule that
   reads a pawn's balance.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Spawn a Primal Beast and read `D_Attributes.Targetable.Targetable_Is_Champion` on its mind and on its pawn: expect 0
   on both.
2. With a champion balance (one whose `Champion` is set), read it on the pawn: expect 1. On the mind: expect 0.
3. Read the spawned pawn's `BalanceDefinitionState.BalanceDefinition`: expect its population balance.
4. Right after start-up, before any spawn, find `D_Attributes.Targetable.Targetable_Is_Champion`'s
   `TargetableAttributeValueResolver` subobject (section 5.1): expect it found, resident with its attribute (review r1
   of #787).
