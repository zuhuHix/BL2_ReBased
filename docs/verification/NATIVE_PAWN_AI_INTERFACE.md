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
- **Steps 1 and 2 give none** (no controller, a declining one, or the vehicle branch): step 3 asks the pawn's `Mesh`.
  A skeletal mesh component answers by `ActorComponent`'s rule (section 6, read since lane
  ANALYST-LINK11-QUERY-INTERFACE).
  - Asked from the pawn, the mesh's owner is the pawn, already answering, so the mesh gives none and nothing is
    held.
  - Asked directly, the mesh's owner can answer through its own controller (section 6.3).
  - The port's dummy meets the pawn-first case on slice link 11. Which way its step 2 gave none is not observed.

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

**What the port's stop proves.** The stop's text, "goes to its Mesh, a Engine.SkeletalMeshComponent", proves only
that section 2's steps 1 and 2 gave no answer for the port's Fire dummy, and that its `Mesh` is a
`SkeletalMeshComponent`. It does not prove which way step 2 gave none:
- a `Controller` that is None;
- a set `Controller` that declined;
- no `Controller`, and a `DrivenVehicle` that is None, has no `Controller`, or whose controller declined.

No field of the port's dummy was observed directly. **The game's dummy** has a mind and goes demigod at its first hit
(COMBAT_GOLDEN_TRACE.md question 8). Its `Controller` value at that hit, and the route its answer takes, are not
observed (section 4). Both stay UNVERIFIED here.

**The answers, by call path and owner.** In every case the plain step gives none for `IAIInterface`: no installed
component class lists it (section 1's tables list it only for `GearboxMind` and `WillowMind`). The owner step then
depends on how the mesh was reached.

1. **Pawn first** (the slice's path: `Behavior_AIHold` asks the pawn, and the pawn's step 3 asks its mesh). The pawn is
   still answering this question.
   - `Owner` is that pawn: the guard gives none, so the mesh answers none.
   - `Owner` is None: none.
   - `Owner` is another actor: that actor's own answer, by its rule (not seen on the slice).

   So when the pawn's steps 1 and 2 gave none, the pawn's answer is **none**, and `Behavior_AIHold` returns without a
   hold (section 1, NATIVE_AI_HOLDS.md): the dummy is not made demigod at that hit.
2. **A direct mesh query** (the mesh asked first, with nothing answering yet). `Owner` gets the question with no cycle,
   and answers by its own rule:
   - a pawn owner runs section 2 from step 1. A possessed pawn can then answer through its `Controller`, the mind,
     before any cycle, so the mesh would answer the mind;
   - the cycle guard ends the question only if it comes back to the mesh, through the pawn's step 3.

**This differs from the game's run, as far as the slice goes.** The game's dummy goes demigod; the port's gets no
hold. The "none" of case 1 is the game's rule applied to the port's input, a pawn whose steps 1 and 2 give none. It
is not the game's outcome for the Fire dummy. Why the port's step 2 gives none is open (section 6.6).

### 6.4 Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight what is cheap and certain;
- a `NotYet` after a write is terminal and keeps its kind and record;
- grade by the `VmError` variant and the `NotYet` kind, never by text;
- **Off only; Reference is unchanged** (Reference keeps `crate::hit::ai_interface_for`'s stand-in).

**Hub ruling (2026-11-19, review r1 of #512): the classes.** The rule applies to exactly `Engine.ActorComponent` and
`Engine.SkeletalMeshComponent`. The native checks this by an exact-class test on the receiver, inside the native.
- Every other component class keeps the open `NotRead` (step 5). That includes the classes between them and every
  subclass of `SkeletalMeshComponent`.
- A registration on `ActorComponent` through the VM's interface rules would be inherited by every subclass without a
  nearer rule, so the rule must not rely on that inheritance alone.
- There is no VM-wide dispatch change: the VM's other interface rules keep their inheritance.

**Hub ruling (2026-11-19, review r1 of #512): the contract.**
1. **Which class answers.**
   - When the receiver's class is exactly one of the two, the rule below runs.
   - When it is any other class that reaches this rule through inheritance, the answer is
     `NotYet { NotRead }`, "what a <class path> answers to QueryInterface is not read (asked of <class path> for
     <interface path>)", record `docs/verification/NATIVE_PAWN_AI_INTERFACE.md`. This is the existing open-answer
     text of `ow_combat::pawn_interface`.
   - The rule replaces today's open answer for `Engine.SkeletalMeshComponent`, which is that same text with "a
     skeletal mesh component".
2. **The plain answer first** (`Vm::plain_interface`):
   - an object is the answer;
   - a `NotYet` or any other error passes through unchanged;
   - only a successful None goes on to step 3.
3. **The `Owner` field.**
   - **A class with no `Owner` declaration:** `VmError::Script`, "ActorComponent.QueryInterface: <class path>
     declares no Owner".
   - **`Owner` declared with another type** (not an object reference to `Engine.Actor` or a subclass):
     `VmError::Script`, "ActorComponent.QueryInterface: <class path>.Owner is not an object reference to
     Engine.Actor".
   - **No lazy load.** In the port, loading a definition handle is a store write (the hub's rule for lazy loads), so
     the rule loads nothing.
     - If the receiver is an unloaded definition handle, the answer is `NotYet { NotRead }`,
       "ActorComponent.QueryInterface: the component <object path> is an unloaded definition handle", record this
       note.
     - If `Owner` holds such a handle, the answer is `NotYet { NotRead }`, "ActorComponent.QueryInterface: the Owner
       <object path> of <object path> is an unloaded definition handle", record this note.
   - **`Owner` None:** the answer is None.
4. **The owner's answer.**
   - `Owner` set: the question goes to it through the VM's own dispatch (`Vm::query_interface`), with the VM's existing
     cycle guard and depth limit unchanged. An object already answering gets None, and the depth limit keeps its
     existing error.
   - The owner's answer is the answer: an object, None, a `NotYet` or an error, each passed through unchanged.
5. **What the rule adds.** It has no `NotYet` of its own beyond steps 1 and 3. The pawn rule's step prefix still names
   the pawn's own step ("goes to its Mesh, a Engine.SkeletalMeshComponent"). No step writes, so nothing needs
   preflighting, and nothing is marked.

**Tests the implementer owes** (both paths of section 6.3):
- **Pawn first, the cycle:** a pawn with no `Controller` and no `DrivenVehicle`, whose `Mesh` is a
  `SkeletalMeshComponent` with `Owner` = that pawn. Ask the pawn for `IAIInterface`: expect None, with the mesh asked
  once and the pawn's second question ended by the guard.
- **A direct mesh query, owner forwarding:** a mesh whose `Owner` is a pawn whose `Controller` is an object that
  answers `IAIInterface` (an invented class listing the interface). Ask the mesh: expect that controller.
- **The exact-class test:** an invented subclass of `SkeletalMeshComponent` gets the open `NotRead`, not the rule.
- **`Owner` None, `Owner` missing, `Owner` mistyped and an unloaded `Owner`:** None, the two `VmError::Script` texts,
  and the `NotRead`, respectively.

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

1. The rule of `PrimitiveComponent`, `MeshComponent`, subclasses of `SkeletalMeshComponent` and other component classes
   (only `ActorComponent` and `SkeletalMeshComponent` were compared).
2. Why the port's step 2 gives none for its Fire dummy: a None `Controller`, a declining one, or the vehicle branch
   (section 6.3). No field was observed directly.
3. The game's dummy's `Controller` at its first hit, and the route of its answer (section 4). Until both are known,
   link 11's hold in the port is the port's, not the game's.
4. The `Owner` the port's dummy mesh holds. On the pawn-first path, the dummy and None both give none.

### 6.7 Live checks (bl-sdk, the RTSE way)

1. **Pawn first:** on the Fire dummy, hook its `Mesh`'s answer and ask the dummy for `IAIInterface`. Record the
   dummy's `Controller`, whether the mesh is asked, and the dummy's answer.
2. **A direct mesh query:** call `QueryInterface(IAIInterface)` on the dummy's `Mesh` directly, and record the mesh's
   `Owner`. A possessed owner is expected to answer through its mind, so the mesh answers the mind (owner forwarding).
3. **A component with no owner:** on a component whose `Owner` is None, expect None.

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
