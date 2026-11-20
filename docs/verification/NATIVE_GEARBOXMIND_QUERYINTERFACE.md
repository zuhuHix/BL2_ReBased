# How a mind answers `QueryInterface` (link 9's `IAIInterface` question, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-GEARBOXMIND-QUERYINTERFACE, decision 2026-11-19-201.
Read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and a local read-only table reader, under
the machine lock). Field names come from the cooked packages (`tools/ghidra/class_layout.py`). It is written in our
own words: no decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here, and raw
output stays under ignored `local/analysis/`. **Every rule is UNVERIFIED in the running game.** The last section is
the bl-sdk check that would confirm it.

**Clean room.** The lane that wrote this note implements none of it. A fresh implementer codes the rule from this
note alone.

**What it answers.** The open item of NATIVE_PAWN_AI_INTERFACE.md, "`GearboxMind`'s own answer: whether it adds steps
before or after its class's declared interfaces". It is link 9's stop since lane IMPL-LINK9-TARGET-MIND (#579): the
cast's target, a `WillowAIPawn` possessed by a `WillowMind`, hands `QueryInterface(IAIInterface)` to its
`Controller`, and the port answers that with a NotYet.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Whose answer a `GearboxMind` or `WillowMind` gives | `Controller`'s. `AIController`, `GearboxAIController`, `GearboxMind` and `WillowMind` add no answer of their own | high | UNVERIFIED |
| What `Controller`'s answer does | section 2: two named interfaces go to the controller's pawn first; otherwise its own declared interfaces, then its pawn, then its `PlayerReplicationInfo` | high (the order), medium (the two names' purpose) | UNVERIFIED |
| "Its pawn" | `Pawn` for a `Controller`; a `WillowMind` reads `MyWillowPawn` instead | high | UNVERIFIED |
| Link 9's question (`IAIInterface`, asked of the target's `WillowMind`) | the mind itself (its class lists `IAIInterface`, section 2 step 3) | high | UNVERIFIED |
| Writes, loads, draws | none: it only reads fields and asks other objects, which answer by their own rules | high | UNVERIFIED |
| `WillowPlayerController` | has its own answer, which uses `Controller`'s; still open (NATIVE_PAWN_AI_INTERFACE.md) | high (that it has one) | UNVERIFIED |

## 1. Which classes share it

The native answer is a per-class virtual. On the line from `Controller` down to the target's mind, it was read for
`Controller`, `AIController`, `GearboxAIController`, `GearboxMind` and `WillowMind`, and they all use one answer,
`Controller`'s. `Actor` (the class above `Controller`) has a different one: the plain `Object` answer behind a
repeat guard, which section 2 calls "the plain answer". `WillowPlayerController` has its own answer, which calls
`Controller`'s as part of its own steps. What it does before or after that was not read (it stays open).
`PlayerController` and `GamePlayerController` were not read.

## 2. `Controller`'s answer

Asked for an interface class `I`, a controller `C` answers as follows. "Its pawn" `P` is defined in section 3.

1. **A question asked twice.** If `C` is already answering a question by this same rule, further up the same chain,
   the answer is none at once, **whatever interface each question names**: this rule's list is keyed by the controller
   alone. Otherwise `C` is marked as answering until it returns. The port's dispatch guard keys by (object, interface)
   instead; section 5 covers the difference.
2. **Two named interfaces go to the pawn first.** If `I` is set and its name is `IWorldBody` or `ISoundBehavior`, and
   `P` is set, the answer is whatever `P` answers to `QueryInterface(I)`, none included. Nothing else is tried. If `P`
   is not set, `C` goes on to step 3.
3. **Its own class.** The plain answer: `C` itself, when its class or a parent lists `I` among its interfaces (the
   package tables). If that answers, it is the answer.
4. **Its pawn.** If `P` is not set, the answer is none. Otherwise `P` is asked `QueryInterface(I)`, and if it answers,
   that is the answer.
5. **Its player replication info.** If `P` declined and `C`'s `PlayerReplicationInfo` is set, the answer is whatever
   that object answers to `QueryInterface(I)`, none included. Otherwise none.

So `PlayerReplicationInfo` is asked only when the controller has a pawn and that pawn declined. A controller with no
pawn that does not list `I` answers none. With `I` not set (none), step 2 is skipped.

**The rule writes nothing.** It reads `C`'s class, `P` and `PlayerReplicationInfo`, and asks other objects, each of
which answers by its own rule. The only state it touches is the transient "answering" mark of step 1, which is gone
when it returns. It loads nothing and draws no random numbers. An object it asks might do more: that is that object's
own rule.

## 3. "Its pawn"

`Controller`'s answer reaches its pawn through another per-class virtual, not by a field read written into the
answer:
- for `Controller`, `AIController`, `GearboxAIController` and `GearboxMind`, it is the `Pawn` field;
- `WillowMind` overrides it to read `MyWillowPawn`. The `WillowMind` field layout fitted for this read ends exactly at
  the class size its native registration gives, a structural check of the fit.

Whether any class below `WillowMind` overrides it again was not read (UNVERIFIED). On a possessed AI pawn the two
fields name the same pawn (NATIVE_PAWN_AI_INTERFACE.md's open item about `Controller` and `MyWillowMind` is the
pawn-side twin of this one).

## 4. Link 9

The target `WillowAIPawn` is asked `IAIInterface` (through the AI component context resolver of
`Flag_Skills_PhaseLockOnHold`, NATIVE_FLAG_EXPRESSION_EVALUATE.md). Its class does not list it, so `Pawn`'s rule
hands the question to its `Controller`, the harness's `WillowMind` (decision of lane IMPL-LINK9-TARGET-MIND).
`IAIInterface` is neither of step 2's names. The mind's class lists `IAIInterface` (the package tables, as
NATIVE_PAWN_AI_INTERFACE.md records), so step 3 answers **the mind itself**. Steps 4 and 5 are not reached, and the
pawn is not asked back.

## 5. A proposed Off answer, for a separate implementer (UNVERIFIED)

Review r1 of #589 tightened this section. The rule must be **read-only along the whole chain it reaches**: no object
write, no class or code load, no cache fill, no random draw. Every step that cannot be answered that way is a typed
refusal, and the refusal itself changes nothing. Reference registers none of this and keeps its current path
(`crate::hit::ai_interface_for` and the plain answer), unchanged.

**Where.**
- Replace the `GearboxMind` open answer in `ow-combat`'s pawn-interface registration with this rule, registered for
  `Engine.Controller`, so `AIController`, `GearboxAIController`, `GearboxMind`, `WillowMind` and their subclasses use
  it.
- Keep `WillowPlayerController`'s own registered answer as it is: it is still open.
- Classes on the `PlayerController` line other than `WillowPlayerController` were not read. Register a NotYet
  (NotRead) answer for `Engine.PlayerController` that says so, so the `Controller` rule never answers for them by
  inheritance.

**Set-up, before any question (a write the harness logs, as link 9's set-up does).** Resolve:
- the interface-table entries of every class the rule can reach: the controller's class chain, and the classes of
  the objects it can ask (its pawn's, the pawn's `Controller`, `DrivenVehicle` and `Mesh` classes,
  `PlayerReplicationInfo`'s);
- the interface classes the rules compare by path (`IAIInterface`, `IPlayerMaster`, `IWorldBody`, `ISoundBehavior`).

Today `Vm::plain_interface` reaches `CodeIndex::implements`, which resolves a cold table entry and caches the target
(`ow-object` `reflect.rs`). That is a load and a write, even when the receiver and the interface are both loaded.

**A read-only plain answer (step 3).** It needs a lookup that reads only entries already resolved: yes when one
resolved entry is the interface or derives from it, no when every entry on the chain is resolved and none is. **An
unresolved entry on the chain is `NotYet { NotRead }`**, worded "the interface table of `<class>` has an entry not
resolved at set-up (resolving it is a load)". This must not fall back to the caching `implements`.

**Read-only dispatch down the chain (steps 2, 4 and 5).** Asking another object must not run a rule that can load.
Two of today's rules can:
- `Pawn`'s rule reads `DrivenVehicle` and then that vehicle's `Controller` with its field reader, which can load an
  unloaded vehicle;
- `WillowAIPawn`'s rule finds `IPlayerMaster` with `find_class_if_present`, which can load its code (`ow-combat`
  `pawn_interface.rs`).

So each registered rule needs a read-only form: field reads that refuse an unloaded object (`NotYet { NotRead }`
naming it) before they read it, and class lookups by path that read only classes resolved at set-up. **A delegate
whose class answers by a rule that has no read-only form yet is `NotYet { NotPorted }`** ("`<class>`'s QueryInterface
rule is not ported read-only"). A delegate that is an unloaded stand-in is `NotYet { NotRead }` before anything is
asked of it. A refusal from deep in the chain passes up with its kind and record kept, and the step that asked is
named ahead of it (as `pawn_interface`'s `ask` names its step today).

**Fields are checked only when a step reaches them.** A controller that answers itself at step 3 never reads `P` or
`PlayerReplicationInfo`, so a missing or malformed one changes nothing there.

| Case | Outcome |
|---|---|
| `I` is None | The script native returns None before any rule runs (`ow-vm` `natives_core.rs`, `Object.QueryInterface`), so the rule is never called with None. A direct call from Rust with None is `VmError::Script` (a caller error). |
| Step 2: `I`'s name read | A plain read of the interface class's name. A class not resolved at set-up is `NotYet { NotRead }` |
| Steps 2 and 4: "its pawn" field (`MyWillowPawn` or `Pawn`) not declared, or holding a value that is not an object | `VmError::Script` (a malformed class), only when the step reads it |
| Step 5: `PlayerReplicationInfo` not declared or malformed | `VmError::Script`, only when step 5 reads it |
| An object to ask is an unloaded stand-in | `NotYet { NotRead }`, before it is asked |
| A world with no code layer (no definition store), so the interface tables cannot be read at all | `NotYet { LayerAbsent }` |
| An interface-table entry not resolved at set-up | `NotYet { NotRead }` |
| A delegate rule with no read-only form | `NotYet { NotPorted }` |

**The repeat guard (step 1): an approximation, stated plainly.**
- In the game, the guard for this rule keeps a list of the controllers answering a question by it, keyed by the
  controller alone: while `C` is answering any interface, a question that reaches `C` again by this rule gets none.
- The port's dispatch guard (`Vm::query_interface`) keys by (object, interface) (`ow-vm` `api.rs`). A delegate that
  asks `C` a different interface therefore re-enters it in the port, and not in the game.
- Proposed: the `Controller` rule keeps its own guard keyed by the controller alone, on top of the dispatch guard, so
  it matches the game. If an implementer keeps only the dispatch guard instead, that must be documented as an
  approximation, and the cross-interface test below must pin the port's actual behaviour.

**Steps.** As section 2, in order. Step 3 is the read-only plain answer above, not a new dispatched question, so the
dispatch guard does not refuse `C`'s own plain step. Steps 2, 4 and 5 ask through the read-only dispatch.

**Writes.** None. A NotYet from an asked object passes up unchanged. It is marked "stopped after partial writes" only
by a caller whose own writes came first.

**Tests on invented classes** (synthetic fixtures). Each checks the answer, and that object state (heap
committed writes), class state (layouts, code loaded, the interface-table cache) and generator draws are unchanged.
- The branches: a controller whose class lists `I` (step 3); one that does not, with a pawn that lists it (step 4); one
  with no pawn (none, and `PlayerReplicationInfo` not read); a pawn that declines, with a `PlayerReplicationInfo` that
  answers (step 5); `IWorldBody` with a pawn that declines while the controller lists `IWorldBody` (still none: step 2
  does not fall through); the same with no pawn (step 3 answers); a mind subclass reading `MyWillowPawn` while `Pawn`
  names another object.
- Lazy fields: a controller that answers itself at step 3 while `PlayerReplicationInfo` is undeclared, and while it
  holds a malformed value (the answer is still the controller); the same field reached at step 5 (`VmError::Script`).
- A cold interface-table entry while both the controller's class and `I` are loaded: `NotYet { NotRead }`, with the
  entry still unresolved and nothing cached.
- A nested refusal: a loaded `WillowAIPawn` whose `DrivenVehicle` is an unloaded stand-in, reached through step 4,
  and a `WillowAIPawn` asked `IPlayerMaster` with that class not resolved at set-up. Each is a typed NotYet, with
  object state, class state and draws unchanged.
- A delegate whose class has a rule with no read-only form: `NotYet { NotPorted }`.
- No code layer: `NotYet { LayerAbsent }`.
- The guard: a question that comes back to `C` on the same interface (none); and a cross-interface cycle, where `C` is
  asked `I1`, its pawn asks `C` for `I2`, and `C` gets none by its own guard (or, under the documented approximation,
  the port's pinned behaviour).
- `None` passed directly: `VmError::Script`.

## 6. Local probe (never pushed)

The probe was a throwaway local merge of #535 (the flag evaluator), #579 (the target mind), #564, #548, #555, #560,
#523 and the link 9 spawn branches. On it, the open `GearboxMind` answer was changed, for `IAIInterface` only, to give
the plain answer, which is section 4's result. The installed Off strict `--slice-gate`, under the machine lock, gave 1
ok / 3 NotYet / 10 errors. Without the change it was 1 / 4 / 9. **In that probe's world**, link 9 is an error at its
`cast` step, at an unbound `GearboxMind.GetAIComponent()` (NATIVE_AI_HOLDS.md's open row). The error comes from the
probe's own registrations: link 9's world registers the pawn rules but not `ow_combat::holds`, which in Off already
binds `GetAIComponent` to a `NotYet { NotRead }` (`ow-combat` `holds.rs`). A world that registers that pack would
stop at that NotYet instead. Either way, `GetAIComponent` is the next stop once this answer is implemented, and it is
that native's own open work.

## Link 9's partial writes before this question (second pass, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5). A second analyst pass (lane ANALYST-GEARBOXMIND-QUERYINTERFACE again, on
main 4c4522d7 with #628's set-up). It re-read the interface virtual's entries for `AIController`, `GearboxMind` and
`WillowMind`, and the pawn accessor. It agrees with sections 1 to 3: one shared answer, the same two names, and
`WillowMind`'s own pawn field. It changes no rule here. Raw output stays under ignored `local/analysis/gmqi/`.
**UNVERIFIED** in the running game.

**Where the mark comes from.** On #628's head (77ff5e7c), link 9's `cast` stops at this note's question, marked
"stopped after partial writes". The mark is `FlagExpressionEvaluator.Evaluate`'s (`ow-skills`
`action_skill/flag_expression.rs`). The native takes a snapshot of #518's three counters when it starts and marks a
refusal when one has moved since.

**What moved.** A local probe, never pushed, logged the counters and each lazy definition load inside that one call
(installed Off `--slice-gate`, strict, under the machine lock). Between the evaluator's start and the refusal:
- **0 draws;**
- **8 definition loads:**
  - the skill's flag evaluator sub-object;
  - `Flag_Skills_CanPhaseLock` and its flag evaluator;
  - `Flag_Skills_DisablePhaseLock`, its `WillowMind` attribute context resolver and its rule-engine flag value
    resolver;
  - `Flag_Skills_PhaseLockOnHold` and its AI component attribute context resolver;
- **40 committed heap writes.** The probe did not separate the loads' own writes from any other write.

So the evaluator loads the definitions it reads as it walks them, and the refusal comes later. No earlier native
writes before a later one refuses: the loads are the evaluator's own. In the game these objects are presumably
resident with the skill's package (UNVERIFIED). Two ways out, for queue item PREFLIGHT-ORDER-LINK9-CAST:
- **(a)** load them at link 9's set-up as labelled harness inputs, as `IFlagProvider` and `IRuleEngineOwner` already
  are. That would remove the load transitions the probe saw. Whether the partial-write mark then goes stays
  UNVERIFIED until a run shows all three of #518's counters unchanged from the evaluator's start through the refusal:
  the probe did not attribute all 40 heap writes to the loads;
- **(b)** check residency first in the evaluator's preflight, so an unloaded definition refuses not-read before any
  load. Link 9 would then stop earlier.

The mark does not count writes made before the evaluator (the cast's own script), and the probe did not look at them.

**The probe with this note's answer stood in** (the open `GearboxMind` entry dropped, so the mind gets the plain
answer): ok 4, notyet 8, error 2 of 14. Link 9 passes 1 of 2 steps, and `cast` stops at the unbound
`GearboxMind.GetAIComponent()`, as section 6 found.

## Open (each UNVERIFIED)

- What `WillowPlayerController`'s own answer adds around `Controller`'s.
- `PlayerController` and `GamePlayerController`: whether they have their own answers.
- Whether a class below `WillowMind` overrides "its pawn" again.
- Why `IWorldBody` and `ISoundBehavior` go to the pawn first (the rule is read; the purpose is a guess: the pawn has
  the body and the sounds).
- The step 1 guard's exact scope. It is one list for this rule, keyed by the controller alone, so a question to `C`
  through a different rule (for example the plain answer's own guard) is not blocked by it. That reading is from the
  native; it is not observed.

## Live confirmation (bl-sdk, the RTSE way; not run, no session granted)

On a spawned enemy (a `WillowAIPawn` with its `WillowMind`), call `QueryInterface` on the mind and record each answer
as the object or None:
- `IAIInterface`: expected the mind itself;
- an interface the pawn lists and the mind does not (check both class tables first): expected the pawn;
- `IWorldBody` and `ISoundBehavior`: expected the pawn's own answer for each, also when that answer is None;
- the same three questions on a mind whose `MyWillowPawn` was cleared: expected the mind for `IAIInterface` and None
  for the pawn-only interface;
- and on the enemy pawn, `QueryInterface(IAIInterface)`: expected the mind (NATIVE_PAWN_AI_INTERFACE.md section 2).
