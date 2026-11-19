# How a pawn context answers `IAIInterface` (the `Behavior_AIHold` question, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-PAWN-AI-INTERFACE, decision 2026-11-19-51. The script
was read with `research/script_disasm.py`. The native part was read from the game executable with Ghidra
(`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock), and field names come from the cooked packages
(`tools/ghidra/class_layout.py`). It is written in our own words: no decompiler output, pseudo-code, addresses,
offsets or listing structure is reproduced here, and raw output stays under ignored `local/analysis/`. **Every rule
is UNVERIFIED in the running game.** The prepared bl-sdk session (last section) is the confirmation.

**Clean room.** The lane that wrote this note does not implement it. A fresh implementer codes the rules from this
note alone.

**What it answers.** NATIVE_AI_HOLDS.md's open row, "How a pawn context reaches `IAIInterface`". The class tables of
the packages list `IAIInterface` for `GearboxMind` and `WillowMind`, and not for `WillowAIPawn` or any of its
parents. Yet the live Fire dummy became demigod in its `Behavior_AIHold_59`, whose context is the pawn.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Which object answers `IAIInterface` for a pawn context | the pawn's `Controller` (for an AI pawn, its mind), not the pawn | high | UNVERIFIED |
| With no controller | the controller of the vehicle the pawn drives, if any; else the pawn's `Mesh` is asked; else none | high (order), medium (`Mesh`'s answer) | UNVERIFIED |
| Which `AIComponent` gets the hold | the one the answering mind's `GetAIComponent()` returns (NATIVE_AI_HOLDS.md, "`GearboxMind.GetAIComponent`") | as that note | UNVERIFIED |
| `Pawn`, `GamePawn`, `GearboxPawn`, `WillowPawn`, `WillowPlayerPawn` | one rule, `Pawn`'s; none of them adds its own | high | UNVERIFIED |
| `WillowAIPawn` | `Pawn`'s rule, plus one extra answer for `IPlayerMaster` only | high | UNVERIFIED |

## 1. The script's question

`Behavior_AIHold.ApplyBehaviorToContext` asks its `ContextObject` for the interface with the `Object` native
`QueryInterface(IAIInterface)`. It does not use an interface cast. With no answer it returns, and nothing is held
(NATIVE_AI_HOLDS.md, "The script that calls them"). The native passes the class to the object's own answer for its
class, so what a pawn answers is that pawn class's rule, below.

## 2. The rule every pawn class uses (`Pawn`'s)

Asked for an interface class `I`, a pawn answers by the first of these that gives a result:

1. **The pawn itself**, when its own class lists `I` among its interfaces (the plain `Object` answer: the class's
   declared interfaces, as the package tables give them).
2. **Its controller.** If the pawn's `Controller` is set, the same question goes to that controller, which answers by
   its own rule. If the pawn has no `Controller` but drives a vehicle (`DrivenVehicle`) that has a `Controller`, the
   question goes to that vehicle's controller instead.
3. **Its mesh.** If there is still no answer and the pawn's `Mesh` is set, the question goes to the mesh component.
4. Otherwise: none.

Each object refuses a question that is already being asked of it further up the same chain, so a cycle (a controller
that asks back) ends with no answer.

`GamePawn`, `GearboxPawn`, `WillowPawn` and `WillowPlayerPawn` do not change this rule. `Scout` and `WillowScout`
keep it too.

## 3. `WillowAIPawn`

It runs `Pawn`'s rule first. Only when that gives no answer, and only for the interface class `IPlayerMaster`
(declared in the `Engine` package), it asks the pawn's player master: the `WillowPlayerController` that masters the
pawn. The field is read as `PlayerMaster` by its meaning; the layout tool does not place this class's fields exactly.
For `IAIInterface` this changes nothing: a `WillowAIPawn` answers exactly as section 2 says.

## 4. What this means for the Fire dummy (slice link 11)

- The dummy is a `WillowAIPawn`, whose class lists no `IAIInterface`, so step 1 gives nothing.
- Its `Controller` is its mind (a `WillowMind`, which possesses it). The mind's class lists `IAIInterface`, so the
  mind answers. The mind's own answer was not read beyond that: whether `GearboxMind` adds a rule of its own was not
  resolved, but the plain answer already gives the mind.
- `Behavior_AIHold` then calls that mind's `GetAIComponent()` and applies `HoldDemigod('Safe', <the pawn>)` to the
  component it returns. The tag is still the pawn (the context object), not the mind.
- **A dummy with no controller** (not possessed yet, or unpossessed), driving nothing, asks its `Mesh`. A skeletal
  mesh component is not expected to answer `IAIInterface` (its own answer was not read), so the hold does nothing.

## 5. The port's current assumption (`crates/ow-combat/src/hit.rs`, `ai_interface_for`)

The Reference stand-in answers a pawn context with the pawn's `MyWillowMind`, else the context itself. The reading
**confirms the mind answers** for a possessed AI pawn, with these differences for an implementer:

- the rule goes through the pawn's **`Controller`** field, not `MyWillowMind`. For a possessed AI pawn they are
  expected to name the same mind, but that is not read here;
- with no controller it asks the **driven vehicle's controller**, then the **`Mesh`**, and otherwise answers **none**,
  so the hold does nothing. The stand-in instead hands back the pawn, which has no `IAIInterface`;
- the pawn itself comes **first**: a pawn class that did list `IAIInterface` would answer for itself;
- the same rule holds for every pawn class, player pawns included. A player pawn's question goes to its
  `WillowPlayerController`, whose own answer for `IAIInterface` was not read.

COMBAT_HIT_CHAIN.md's note ("the hold is applied to the pawn's mind") matches this rule for a possessed AI pawn.

## Vehicles (not on the slice path)

The `WillowVehicle` line answers `IAIInterface` and `IAnimBehavior` its own way. `IAnimBehavior` goes to the
vehicle's `Mesh`. `IAIInterface` goes to its controller, its own or its driven-by pawn's, only when that controller
is not a human player's. Otherwise it falls back to `Pawn`'s rule. This is recorded for completeness only.

## Not read

- `GearboxMind`'s own answer (whether it adds a rule before or after its class's declared interfaces).
- `WillowPlayerController`'s answer for `IAIInterface`.
- What a skeletal mesh component answers.
- Whether `Controller` and `MyWillowMind` can ever name different objects on a possessed AI pawn.

## Live confirmation (bl-sdk, the prepared session)

On the Fire dummy, in a hook of `Behavior_AIHold.ApplyBehaviorToContext`:
- log the class of `ContextObject`, its `Controller` and `MyWillowMind`;
- log what `QueryInterface(IAIInterface)` returns on the pawn: expected the mind, the same object as `Controller`;
- read the mind's `GetAIComponent()` and that component's `DemigodHolds` after the call: expected one entry,
  (`Safe`, the pawn).

With a spawned AI pawn whose controller is cleared, the same query is expected to return none.
