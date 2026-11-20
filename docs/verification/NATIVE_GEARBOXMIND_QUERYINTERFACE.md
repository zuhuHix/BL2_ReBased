# How a mind answers `QueryInterface` (link 9's `IAIInterface` question, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-GEARBOXMIND-QUERYINTERFACE, decision 2026-11-19-NNN.
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
   the answer is none at once. Otherwise `C` is marked as answering until it returns. This is the same guard as the
   pawn's, NATIVE_PAWN_AI_INTERFACE.md section 2.
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

- **Where.** Replace the `GearboxMind` open answer in `ow-combat`'s pawn-interface registration with this rule,
  registered for `Engine.Controller`, so `AIController`, `GearboxAIController`, `GearboxMind`, `WillowMind` and their
  subclasses use it. Keep `WillowPlayerController`'s own registered answer as it is: it is still open. Classes on the
  `PlayerController` line other than `WillowPlayerController` were not read, so register a NotYet (NotRead) answer for
  `Engine.PlayerController` naming that, so the `Controller` rule never answers for them by inheritance. Reference
  registers none of this, as now.
- **"Its pawn".** `MyWillowPawn` when the controller's class is `WillowGame.WillowMind` or a subclass, else `Pawn`.
  Both read by name; a class that does not declare the field it needs is an error, never "not set".
- **Preflight, before any step and with nothing written or loaded.** Read `I`'s name, `P` and
  `PlayerReplicationInfo` as plain reads. Then:
  - an interface class that is declared but not loaded is `NotYet { NotRead }` ("the interface class `<path>` is not
    loaded (a load is a write)"), as the hub ruled for the flag evaluator's `implements` (lane
    FIX-FLAG-IMPLEMENTS-NOTYET);
  - if a step the rule will reach must ask `P` or `PlayerReplicationInfo`, and that object is an unloaded stand-in,
    the answer is `NotYet { NotRead }` naming it, because asking it could load;
  - the controller's own class tables are read as loaded; a class that is not ready is `NotYet { NotRead }` as above.
- **Steps.** As section 2, in order. The plain answer of step 3 is `Vm::plain_interface`, not a new dispatched
  question, so the port's object-level repeat guard (`Vm::query_interface`) does not refuse `C`'s own plain step.
  Steps 2, 4 and 5 ask through `Vm::query_interface`, so the asked object answers by its own rule and a question that
  comes back to `C` gets none.
- **Writes.** None. A NotYet from an asked object passes up unchanged. It is marked "stopped after partial writes"
  only by a caller whose own writes came first.
- **Tests on invented classes**, one for each branch:
  - a controller whose class lists `I` (step 3);
  - one that does not, with a pawn that lists it (step 4);
  - one with no pawn (none, and `PlayerReplicationInfo` not asked);
  - a pawn that declines, with a `PlayerReplicationInfo` that answers (step 5);
  - `IWorldBody` with a pawn that declines, while the controller lists `IWorldBody` (still none: step 2 does not fall
    through);
  - the same with no pawn (step 3 answers);
  - a mind subclass reading `MyWillowPawn` while `Pawn` names another object;
  - a question that comes back to the controller;
  - the unloaded-interface and unloaded-pawn refusals, with no write and no load.

## 6. Local probe (never pushed)

The probe was a throwaway local merge of #535 (the flag evaluator), #579 (the target mind), #564, #548, #555, #560,
#523 and the link 9 spawn branches. On it, the open `GearboxMind` answer was changed, for `IAIInterface` only, to give
the plain answer, which is section 4's result. The installed Off strict `--slice-gate`, under the machine lock, gave 1
ok / 3 NotYet / 10 errors. Without the change it was 1 / 4 / 9. Link 9 is now an **error** at its `cast` step, at the
unbound native `GearboxMind.GetAIComponent()`, NATIVE_AI_HOLDS.md's open row ("not resolved"). So once this answer is
implemented, link 9's next stop is that native. Its NotYet becomes a hard error because nothing binds it in Off, which
is that note's open work and not this one.

## Open (each UNVERIFIED)

- What `WillowPlayerController`'s own answer adds around `Controller`'s.
- `PlayerController` and `GamePlayerController`: whether they have their own answers.
- Whether a class below `WillowMind` overrides "its pawn" again.
- Why `IWorldBody` and `ISoundBehavior` go to the pawn first (the rule is read; the purpose is a guess: the pawn has
  the body and the sounds).
- The exact scope of the step 1 guard. It is one list for this rule, so a question to `C` through a different rule
  (for example the plain answer's own guard) is not blocked by it. The port's per-object guard is close to this, but
  that is not shown.

## Live confirmation (bl-sdk, the RTSE way; not run, no session granted)

On a spawned enemy (a `WillowAIPawn` with its `WillowMind`), call `QueryInterface` on the mind and record each answer
as the object or None:
- `IAIInterface`: expected the mind itself;
- an interface the pawn lists and the mind does not (check both class tables first): expected the pawn;
- `IWorldBody` and `ISoundBehavior`: expected the pawn's own answer for each, also when that answer is None;
- the same three questions on a mind whose `MyWillowPawn` was cleared: expected the mind for `IAIInterface` and None
  for the pawn-only interface;
- and on the enemy pawn, `QueryInterface(IAIInterface)`: expected the mind (NATIVE_PAWN_AI_INTERFACE.md section 2).
