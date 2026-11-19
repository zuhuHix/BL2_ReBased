# How a pawn context answers `IAIInterface` (the `Behavior_AIHold` question, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-PAWN-AI-INTERFACE, decision 2026-11-19-51. The script
was read with `research/script_disasm.py`. The native part was read from the game executable with Ghidra
(`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock), and field names come from the cooked packages
(`tools/ghidra/class_layout.py`). It is written in our own words: no decompiler output, pseudo-code, addresses,
offsets or listing structure is reproduced here, and raw output stays under ignored `local/analysis/`. **Every rule
is UNVERIFIED in the running game.** The prepared bl-sdk session (last section) is the confirmation.

**Clean room.** The lane that wrote this note does not implement it. A fresh implementer codes the rules from this
note alone. Section 2 is the rule; the other records point at it rather than restating it. It is implemented in Off
by lane IMPL-PAWN-AI-INTERFACE (decision 2026-11-19-62, `ow-combat` `pawn_interface`), with every answer listed under
"Open" a named `NotYet`.

**What it answers.** NATIVE_AI_HOLDS.md's open row, "How a pawn context reaches `IAIInterface`". The class tables of
the packages list `IAIInterface` for `GearboxMind` and `WillowMind`, and not for `WillowAIPawn` or any of its
parents. COMBAT_GOLDEN_TRACE.md (question 8) records that the live Fire dummy became demigod inside its
`Behavior_AIHold_59` call at the first hit. Which object that call's context was is not recorded: that it was the
pawn, and that the pawn's mind then answered, are this note's **expectations (UNVERIFIED)**.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Which object answers `IAIInterface` for a pawn context | the first answer of section 2's steps: the pawn's own class, then its `Controller` (or, only with no `Controller`, a driven vehicle's controller), then its `Mesh` | high (the order) | UNVERIFIED |
| A possessed AI pawn | its `Controller`, the mind, is expected to answer | high (the rule), medium (that the mind answers) | UNVERIFIED |
| Which `AIComponent` gets the hold | the one the answering object's `GetAIComponent()` returns (NATIVE_AI_HOLDS.md, "`GearboxMind.GetAIComponent`") | as that note | UNVERIFIED |
| `Pawn`, `GamePawn`, `GearboxPawn`, `WillowPawn`, `WillowPlayerPawn` | one rule, `Pawn`'s; none of them adds its own | high | UNVERIFIED |
| `WillowAIPawn` | `Pawn`'s rule, then one extra answer for `IPlayerMaster` only | high | UNVERIFIED |

## 1. The script's question

`Behavior_AIHold.ApplyBehaviorToContext` asks its `ContextObject` for the interface with the `Object` native
`QueryInterface`, naming the class `IAIInterface`. It does not use an interface cast. With no answer it returns, and
nothing is held (NATIVE_AI_HOLDS.md, "The script that calls them"). The native passes the class to the object's own
answer for its class, so what a pawn answers is that pawn class's rule, below.

## 2. The rule every pawn class uses (`Pawn`'s)

Asked for an interface class `I`, a pawn tries these steps in order and gives the first answer that is not none:

1. **Its own class.** The pawn itself, when its own class lists `I` among its interfaces, its parents' included (the
   plain `Object` answer: the declared interfaces, as the package tables give them).
2. **One controller.**
   - If the pawn's `Controller` is set, the question goes to that controller, which answers by its own rule. The
     driven vehicle is **not** asked in this case, whatever the controller answers.
   - Only if the pawn's `Controller` is not set: if the pawn drives a vehicle (`DrivenVehicle`) whose `Controller` is
     set, the question goes to that controller instead.
3. **Its mesh.** If steps 1 and 2 gave none (a declining controller included), and the pawn's `Mesh` is set, the
   question goes to the mesh component.
4. Otherwise: none.

**A question asked twice (UNVERIFIED).** While an object is answering, a question that comes back to that same
object, further down the same chain, gets none at once, which ends a cycle. The object that sent that inner question
takes the none as its step's answer and goes on with its next step. So a pawn whose controller asks back about the
pawn goes on to its `Mesh`. The last section has a live check for it.

`GamePawn`, `GearboxPawn`, `WillowPawn` and `WillowPlayerPawn` do not change this rule. `Scout` and `WillowScout`
keep it too.

## 3. `WillowAIPawn`

It runs section 2's rule first. Only when that gives none, and only when the asked class is `IPlayerMaster`, it asks
the pawn's player master, the `WillowPlayerController` that masters the pawn, when one is set. The field is read as
`PlayerMaster` by its meaning; the layout tool does not place this class's fields exactly (UNVERIFIED). For
`IAIInterface` this changes nothing: a `WillowAIPawn` answers exactly as section 2 says.

## 4. The Fire dummy (slice link 11): expectations, UNVERIFIED

- The dummy is a `WillowAIPawn`, whose class and parents list no `IAIInterface`, so step 1 gives none.
- **Expected:** its `Controller` is set and is its mind, a `WillowMind`, whose class lists `IAIInterface`, so step 2
  gives the mind. That the dummy has a controller at its first hit, and that the mind answers yes, are not observed.
  The mind's own rule was not read beyond the plain answer (whether `GearboxMind` adds steps of its own).
- `Behavior_AIHold` then asks that mind for its AI component, and the component receives a demigod hold named `Safe`
  whose tag is the pawn, the context object, not the mind.
- **No controller and no driven vehicle:** step 3 asks the pawn's `Mesh`. A skeletal mesh component answers by
  `ActorComponent`'s rule (section 6, read since lane ANALYST-LINK11-QUERY-INTERFACE): for `IAIInterface` it gives
  none, so nothing is held. The port's dummy meets this case on slice link 11 (section 6.3).

## 5. The port's current assumption (`crates/ow-combat/src/hit.rs`, `ai_interface_for`)

The Reference stand-in answers a pawn context with the pawn's `MyWillowMind`, else the context itself. The reading
**agrees that the mind answers** for a possessed AI pawn, if its `Controller` is that mind. It differs here, and an
implementer follows section 2:

- the rule reads the pawn's **`Controller`** field, not `MyWillowMind`. Whether they can name different objects on a
  possessed AI pawn is not read (UNVERIFIED);
- the pawn's own class comes **first**: a pawn class that lists `IAIInterface` answers for itself;
- with a controller that declines, the next step is the **`Mesh`**, not a vehicle;
- with no controller, a driven vehicle's controller, then the **`Mesh`**, then **none**. The stand-in instead hands
  back the pawn itself;
- the rule is the same for every pawn class, player pawns included. A player pawn's question goes to its
  `WillowPlayerController`, whose answer for `IAIInterface` is not read (UNVERIFIED).

COMBAT_HIT_CHAIN.md's note ("the hold is applied to the pawn's mind") matches step 2 for a possessed AI pawn.

## 6. A component's answer (`ActorComponent`'s rule; lane ANALYST-LINK11-QUERY-INTERFACE, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-LINK11-QUERY-INTERFACE (decision 2026-11-19-NNN).
- **Sources:**
  - the game executable, read with Ghidra (`tools/ghidra/run.ps1` read-only queries and the local table reader, one
    headless run at a time under the machine lock);
  - field names from `tools/ghidra/class_layout.py`;
  - two local, uncommitted slice-gate probes (section 6.5).
- Raw output stays under ignored `local/analysis/link11qi/`. No live session was run: **every rule here is
  UNVERIFIED**. This lane implements nothing.

### 6.1 Which body answers

`Object.QueryInterface`'s entry calls one virtual of the asked object (section 1). The class tables were found through
each class's registration and constructor, and their entries for that virtual compared:
- **`Object`** holds the plain answer of section 2, step 1.
- **`ActorComponent`** holds its own body.
- **`SkeletalMeshComponent`** holds the same body as `ActorComponent`.
- The classes between them (`PrimitiveComponent`, `MeshComponent`) and every other component class were not looked at.

### 6.2 The rule

Asked for an interface class `I`, a component gives the first answer that is not none:
1. **The plain answer:** the component itself, when its own class or a parent lists `I` (section 2, step 1).
2. **Its owner:** when the component's `Owner` (the actor field of `ActorComponent`, by the layout tool) is set, the
   question goes to that actor, which answers by its own rule, the same dispatch as a direct question.
3. Otherwise: none.

**A question asked twice.** The body keeps its own list of the components that are answering. A question that comes
back to a component already in that list gets none at once, and the plain answer keeps such a list too. This is the
same guard section 2 describes for pawns, read here for the component's body, UNVERIFIED in the game.

**Writes:** none. The answering lists are internal bookkeeping, not script-visible state.

### 6.3 The inputs the slice reaches, and their answers

The Fire dummy of link 11 is a `WillowAIPawn` with no `Controller` and no `DrivenVehicle` in the port. The stop's own
text says so: it went to the `Mesh`. Its `Mesh` is a `SkeletalMeshComponent`.
- **The plain step:** no installed component class lists `IAIInterface` (section 1's tables list it only for
  `GearboxMind` and `WillowMind`), so none.
- **The owner step:**
  - **`Owner` is the dummy itself:** the dummy is still answering that same question (it asked its `Mesh` in section 2,
    step 3), so the pawn's guard gives none. The mesh answers none.
  - **`Owner` is None:** none.
  - **`Owner` is another actor:** that actor's own answer, by its rule. The slice was not seen to reach this case.
- So the dummy's answer is **none**, and `Behavior_AIHold` returns without a hold (section 1, NATIVE_AI_HOLDS.md): the
  dummy is **not** made demigod at that hit.

**This differs from the game's run.** COMBAT_GOLDEN_TRACE.md records the live dummy becoming demigod inside its
`Behavior_AIHold_59` call. Section 4 expects that answer to come from the dummy's mind through its `Controller`, which
the game's dummy has and the port's does not. So the "none" here is the game's rule applied to the port's input (a
pawn with no controller). It is not the game's outcome for the Fire dummy. The missing controller is the port's
spawn's gap, not this rule's, and is recorded as open (section 6.6).

### 6.4 Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight what is cheap and certain;
- a `NotYet` after a write is terminal and keeps its kind and record;
- grade by the `VmError` variant and the `NotYet` kind, never by text;
- **Off only; Reference is unchanged** (Reference keeps `crate::hit::ai_interface_for`'s stand-in).

1. **Register `ActorComponent`'s rule for exactly `Engine.ActorComponent` and `Engine.SkeletalMeshComponent`,** the
   two classes read. It replaces today's open answer for `Engine.SkeletalMeshComponent`, "what a skeletal mesh
   component answers to QueryInterface is not read" (`ow_combat::pawn_interface::OPEN_ANSWERS`, record this note).
2. **The rule:** the plain answer, then the `Owner` through the VM's own interface dispatch (`Vm::query_interface`),
   whose guard gives none for an object already answering. No step writes.
3. **`Owner` must be declared:** a component class that does not declare `Owner` is a `VmError::Script` naming it, as
   the pawn rule's fields are (`required`), never "not set".
4. **A `NotYet` from the owner's rule** passes through unchanged. The pawn rule's step prefix still names its own step
   ("goes to its Mesh, a Engine.SkeletalMeshComponent"). There is no `NotYet` of this rule's own. Nothing is written
   before or during it, so nothing needs preflighting.
5. **Any other component class between or beside them** (for example an object whose class is exactly
   `PrimitiveComponent` or `MeshComponent`, or another component subclass): its answer was not read. If such an object
   is asked, it is `NotYet { NotRead }`, "what a <class> answers to QueryInterface is not read (asked of <class> for
   <interface>)", record this note. This is the existing open-answer text, so it should be registered per class as the
   port reaches them, not inherited from `ActorComponent` by default.

### 6.5 Where link 11 stops next (local probes, never committed)

**The base.** A throwaway merge of #496 at 9b591ad and #497 at 7af3ab7 on main 7668d07. The probes applied
NATIVE_MISSION_TEXT_LIST.md's rulings R1 to R3, and this section's rule.
- **First probe:** link 11 reaches `hit:fire`, passes the hold (none, as above), and stops on the unbound
  `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)` (an `error` row).
  That native's record step ends in the same statistics submit routine as the events of NATIVE_MISSION_TEXT_LIST.md
  section 5. So hub ruling R3 covers it: nothing, with no write, for every player (a port answer, UNVERIFIED).
- **Second probe**, with that event answered by R3:

| # | Link | Result | Steps passed / driver steps | First stop |
|---|---|---|---|---|
| 10 | mission_accept | ok | 8 / 8 | none |
| 11 | mission_objective | ok | 8 / 8 | none |
| 12 | mission_turn_in | error | 1 / 6 | `turnin`: unbound native `WillowPlayerController.UnlockAchievementIfConditionsMet(byte,string,int,bool,bool,bool)` |

The whole gate under the second probe gave ok 2, notyet 2, error 10 of 14. The other links' stops were unchanged by
these answers.

### 6.6 Open (UNVERIFIED)

1. The rule of `PrimitiveComponent`, `MeshComponent` and other component classes (only `ActorComponent` and
   `SkeletalMeshComponent` were compared).
2. Why the port's Fire dummy has no `Controller` at its first hit, while the game's has its mind (section 4): the
   port's spawn of the dummy's mind. Until it does, link 11's hold is the port's, not the game's.
3. The `Owner` the port's dummy mesh holds (the dummy or None; both give none).

### 6.7 Live checks (bl-sdk, the RTSE way)

1. On the Fire dummy, call `QueryInterface(IAIInterface)` on its `Mesh` directly: expect None.
2. On a component whose `Owner` answers an interface the component's class does not list (for example a `WillowMind`'s
   pawn's mesh asked for `IAIInterface`, with the pawn's own answer hooked out of the way), expect the owner's answer.

## Vehicles (not on the slice path)

The `WillowVehicle` line answers `IAIInterface` and `IAnimBehavior` its own way. `IAnimBehavior` goes to the
vehicle's `Mesh`. `IAIInterface` goes to its controller, its own or its driven-by pawn's, only when that controller
is not a human player's. Otherwise it falls back to section 2. This is recorded for completeness only.

## Open (each UNVERIFIED)

- The rule for a question asked twice (section 2): that the inner question gets none and the asker goes on.
- `GearboxMind`'s own answer: whether it adds steps before or after its class's declared interfaces.
- `WillowPlayerController`'s answer for `IAIInterface`.
- What a skeletal mesh component answers: read in section 6 (`ActorComponent`'s rule). Still open there: the rule of
  the component classes between `ActorComponent` and `SkeletalMeshComponent`, and of every other component class.
- Whether `Controller` and `MyWillowMind` can name different objects on a possessed AI pawn.
- What `Controller` holds on a dead pawn, and on one not possessed yet (the rule reads only the field).
- `WillowAIPawn`'s `PlayerMaster` field identification.

## Live confirmation (bl-sdk, the prepared session)

**On the Fire dummy, in a hook of `Behavior_AIHold.ApplyBehaviorToContext`:**
- record the actual `ContextObject` and compare its identity with the pawn, the pawn's `Controller` and its
  `MyWillowMind`;
- call `QueryInterface(IAIInterface)` on the context, and directly on the pawn's `Controller`, on `MyWillowMind` and
  on the pawn's `Mesh`, and record each answer as the object or None: expected the mind from the pawn, the mind from
  the controller, and None from the mesh;
- read the answering object's `GetAIComponent()` and that component's `DemigodHolds` after the call: expected one
  entry, the name `Safe` with the pawn as its tag.

**The rule's branches, on spawned pawns:**
- **A present controller that declines, with a vehicle that would answer:** a player pawn (its `Controller` is its
  `WillowPlayerController`) that also has a `DrivenVehicle` whose own `Controller` answers `IAIInterface` (query that
  vehicle controller directly first, to show it would answer). Query the player controller directly for
  `IAIInterface` and record the object or None. Then query the pawn, with the pawn's `Controller`,
  the vehicle's controller and the `Mesh` hooked. Expected: the player controller declines, the vehicle's controller
  is not asked, the `Mesh` is asked, and the pawn's answer is the `Mesh`'s.
- **No controller, driving a vehicle:** a pawn with `Controller` cleared while `DrivenVehicle` has a controller:
  expected that controller's answer.
- **No controller, nothing driven:** a pawn with `Controller` cleared: expected the `Mesh`'s answer, and none if the
  mesh declines.
- **Dead and unpossessed:** a pawn just killed and one spawned but not yet possessed; record `Controller` on each and
  the query's answer.
- **A pawn class that declares the interface, and one that inherits it:** if any pawn class in the packages lists
  `IAIInterface`, query an instance of it and an instance of a subclass that inherits the interface **without
  declaring it again** (check the subclass's own interface list in the package): expected the pawn itself from both.
  If no pawn class lists it, record that, and the step stays unobserved.
- **`IPlayerMaster`:** on a `WillowAIPawn` whose base rule declines, query `IPlayerMaster` with `PlayerMaster` set and
  with it cleared: expected the player controller, then none.
- **A question asked twice:** a pawn whose `Controller`'s own answer asks the pawn back (if no stock class does,
  hook the controller's answer to ask the pawn). Record that the inner question to the pawn got None, the
  controller's final answer, and whether the pawn then asked its `Mesh`. Expected: None, the controller's own result
  without the pawn's, and the `Mesh` asked.
