# `GearboxMind`'s `QueryInterface` answer on link 9 (draft, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-GEARBOXMIND-QUERYINTERFACE (decision
`docs/decisions/2026-11/2026-11-19-NNN-gearboxmind-queryinterface-on-link-9.md`, number pending). Analyst only: a third
lane implements it (clean room). This lane's author implemented #602, the read-only interface chain, from another
lane's note.

**Sources:**
- the records: NATIVE_PAWN_AI_INTERFACE.md (the pawn's rule, section 2, and the component's, section 6, whose
  method this note repeats) and NATIVE_MIND_ROUTE.md (the `WillowMind` field fit);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries and table reads, one headless
  run at a time under the machine lock). The steps:
  - the class registrations of `Controller`, `AIController`, `GearboxMind` and `WillowMind`, and their constructors'
    primary table stores;
  - each class's entry for the virtual that `Object.QueryInterface` calls;
  - the engine's start-up name registration, for the two names the body compares;
- field names from `tools/ghidra/class_layout.py`;
- two local, uncommitted slice-gate probes (sections 5 and 6), never pushed.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/gmqi/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. Which body answers

`Object.QueryInterface`'s entry calls one virtual of the asked object (NATIVE_PAWN_AI_INTERFACE.md section 6.1). The
four classes' entries for that virtual compare like this:
- **`Controller`** holds the plain answer, behind a re-entry guard of its own: the object itself when its class or a
  parent lists the interface, else none.
- **`AIController`** holds its own body, section 2's rule.
- **`GearboxMind`** and **`WillowMind`** hold the same entry as `AIController`: neither overrides it.

**So "GearboxMind's own answer" is `AIController`'s rule.** It is the same for every class under `AIController` that
does not override the entry. Only these four classes were looked at.

**The pawn accessor.** The rule asks one more virtual of the controller, an accessor for "the pawn I control":
- `AIController` and `GearboxMind` answer the controller's `Pawn` field (`Controller.Pawn`, by the layout tool);
- `WillowMind` overrides it to answer a pawn field of its own. By meaning this is `MyWillowPawn`, the same fit
  NATIVE_MIND_ROUTE.md uses. The layout tool cannot place `WillowMind`'s fields, so the name is UNVERIFIED.

## 2. The rule

Asked for an interface class `I`, an `AIController` (and so a `GearboxMind` or a `WillowMind`) answers in this order:

0. **The guard.** The body keeps a list of the controllers it is answering for. A question that comes back to a
   controller already on the list gets none at once. The list is internal bookkeeping, as for the pawn and the
   component.
1. **Two names go straight to the pawn.** When `I`'s name is `IWorldBody` or `ISoundBehavior` and the pawn accessor
   gives a pawn, the answer is that pawn's answer, whatever it is (none included). Nothing else is asked. The test
   compares the class's name only, not its package. With no pawn, these two names take steps 2 to 4 like any other.
2. **The plain answer:** the controller itself, when its class or a parent lists `I`. This is `Controller`'s entry,
   with its own guard.
3. **The pawn:** when step 2 gives none and the pawn accessor gives a pawn, that pawn's answer, by its own rule.
4. **The player replication info:** when step 3 gives none (or there is no pawn) and the controller's
   `PlayerReplicationInfo` (by the layout tool) is set, that object's answer, by its own rule.
5. Otherwise: **none**.

An answer counts as given only when both of its parts are set, the object and its interface part, as in the pawn's
rule.

**What it reads:** the asked class's name, the pawn accessor (one field), `PlayerReplicationInfo`, and the guard lists.
**What it writes, loads or draws:** nothing in the body read. The pawn and replication-info answers it hands on are
those objects' own rules, with their own reads.

## 3. Link 9's question

`Flag_Skills_PhaseLockOnHold`'s AI component resolver asks the target pawn for `IAIInterface`. The pawn's rule (step 2)
hands the question to its `Controller`, the target's mind, a `WillowMind` (the harness input of decision
2026-11-19-183). By section 2:
- `IAIInterface` is not one of step 1's two names;
- step 2: `WillowMind`'s class lists `IAIInterface` (the packages list it for `GearboxMind` and `WillowMind`,
  NATIVE_PAWN_AI_INTERFACE.md section 1), so **the answer is the mind itself**. Steps 3 and 4 are not reached.

Had step 2 given none, step 3 would ask the pawn. The pawn is already answering this question, so its guard would give
none, and step 4 would ask the replication info. The expected answer is the mind, which agrees with
NATIVE_PAWN_AI_INTERFACE.md section 4's expectation (UNVERIFIED).

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.**
- **Bind in Off:** register section 2's rule as the interface answer of `Engine.AIController`, through the existing
  per-class rule registration (`ow_combat::pawn_interface::register`). `GearboxMind` and `WillowMind` take it as
  subclasses. Remove `GearboxFramework.GearboxMind` from `OPEN_ANSWERS`. A class that already has an answer keeps it.
  If #602's read-only chain has landed, add the rule's read-only form there too.
- **Reference** is unchanged (the current stand-in).
- **The pawn accessor, by class:**
  - `AIController`, `GearboxMind` and their script-only children: `Controller.Pawn`;
  - `WillowMind` and its script-only children: `MyWillowPawn`;
  - any other native class under `AIController`: a not-read refusal, "AIController.QueryInterface: the pawn accessor
    of <class> (its virtual was not read)", raised **only when a step needs the accessor** (step 1 for its two names,
    or step 3).

**Preflight** (write-free, load-free and RNG-free). It is resident-only and uses the existing class and name lookups,
with no interning. The first failure is the answer:
1. **The receiver** is resident. An unloaded handle is a not-read refusal, "AIController.QueryInterface: the
   controller <path> is not loaded (a load is a write)".
2. **The asked class** is resident, and its name is read from the class table. A class that is not loaded is a
   not-read refusal, as #586 does for the flag evaluator's interface classes.
3. **The fields:** `Pawn` (or `MyWillowPawn`) and `PlayerReplicationInfo` must be declared as object fields. Another
   shape is a `VmError::Script`. A field holding an unloaded handle refuses as step 1 when the rule would ask it.

The rule writes nothing itself, so it never marks "stopped after partial writes". A refusal handed back by the pawn's
or the replication info's own rule passes through unchanged.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented):
1. a `WillowMind` stand-in whose class lists the interface: the mind itself, with the pawn not asked;
2. the guard: a question that comes back to the same controller gets none;
3. step 1, for each of the two names, with a pawn: the pawn's answer, including none, with neither the plain step nor
   the replication info asked; without a pawn: steps 2 to 4;
4. step 1 compares the name only: an invented `Other.IWorldBody` takes it too;
5. step 3: an interface the mind does not list but its pawn does gives the pawn; asked from that pawn first, the
   pawn's guard gives none and step 4 is reached;
6. step 4: the replication info's answer; then none when it is unset;
7. the accessor by class: `AIController` and `GearboxMind` read `Pawn`; `WillowMind` reads `MyWillowPawn` even when
   `Pawn` names another pawn; an invented native child of `AIController` refuses not-read only when a step needs the
   accessor;
8. preflight: an unloaded receiver, asked class or accessed field each refuse before any write, with #518's three
   counters unchanged; a non-object field is a `VmError::Script`;
9. registration: keep-existing, and under `ParityMode::Reference` nothing new is bound.

## 5. The partial writes before this stop (for queue item PREFLIGHT-ORDER-LINK9-CAST)

The mark "stopped after partial writes" on link 9's stop is added by `FlagExpressionEvaluator.Evaluate`
(`ow-skills` `action_skill/flag_expression.rs`). The native takes a snapshot of #518's three counters when it starts and
marks a refusal when one has moved since. A local probe logged the counters and every lazy definition load inside that
one call (installed Off `--slice-gate`, on #628's head 77ff5e7c). Between the evaluator's start and the refusal:
- **0 draws;**
- **8 definition loads:**
  - the skill's flag evaluator sub-object;
  - `Flag_Skills_CanPhaseLock` and its flag evaluator;
  - `Flag_Skills_DisablePhaseLock`, its `WillowMind` attribute context resolver and its rule-engine flag value
    resolver;
  - `Flag_Skills_PhaseLockOnHold` and its AI component attribute context resolver;
- **40 committed heap writes.** The probe did not separate the loads' own writes from any other write. Whether all 40
  come from the 8 loads is UNVERIFIED.

So the evaluator's walk loads the definitions it reads as it goes, and the refusal comes later. It is not a native
writing before another native refuses: the loads are the evaluator's own. In the game these objects are presumably
resident with the skill's package (UNVERIFIED). The choice is the hub's:
- **(a)** load them at link 9's set-up as labelled harness inputs, as `IFlagProvider` and `IRuleEngineOwner` already
  are (the stop's mark would then go);
- **(b)** check residency first in the evaluator's preflight, so an unloaded definition refuses not-read before any
  load. Link 9 would then stop earlier, at the first unloaded definition.

The probe did not look at writes before the evaluator (the cast's own script), because the mark does not count them.

## 6. The probe (local, never pushed)

A local tree at #628's head (77ff5e7c), with two probe-only changes:
- the counter and load logging of section 5;
- **this lane's stand-in:** `GearboxMind` dropped from `OPEN_ANSWERS`, so the mind gets the plain class-table answer,
  section 2's step 2 for link 9's question.

**Installed Off `--slice-gate`, strict:**
- **without the stand-in:** ok 4, notyet 9, error 1 of 14. Link 9 passes 1 of its 2 driver steps (`spawn`), and
  `cast` stops at this note's question, marked "stopped after partial writes";
- **with it:** ok 4, notyet 8, error 2 of 14. Link 9 passes 1 of 2 steps, and **`cast` stops with an error at the
  unbound native `GearboxMind.GetAIComponent()`**. IMPL-GEARBOXMIND-GETAICOMPONENT codes that native on its own branch,
  not on main.

So, under the stand-in, link 9's next stop is `GearboxMind.GetAIComponent()`.

## Open questions

1. **Not read:**
   - the entries of classes under `AIController` other than `GearboxMind` and `WillowMind`, for the interface virtual
     and for the pawn accessor;
   - why the two names go straight to the pawn. The rule is recorded as read, not explained.
2. **UNVERIFIED:** that `WillowMind`'s accessor field is `MyWillowPawn` (a fit), and that `PlayerReplicationInfo` is
   the field step 4 reads (the layout tool's placement for `Controller`).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a spawned enemy, call `QueryInterface(IAIInterface)` on its pawn and on its `Controller`. Expect the mind both
   times.
2. On the same mind, ask for `IWorldBody`. Expect the pawn's answer.
