# How an AI component answers `QueryInterface`, and its `PhaseLockOnHold` (link 9, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-WILLOWAICOMPONENT-QUERYINTERFACE-IFLAGPROVIDER,
decision `docs/decisions/2026-11/2026-11-19-NNN-willowaicomponent-queryinterface-is-the-component-rule.md`. Read from
the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and a local read-only table reader, under the
machine lock). Field names come from the cooked packages (`tools/ghidra/class_layout.py`). The script was checked with
`research/script_disasm.py`. It is written in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure is reproduced here, and raw output stays under ignored `local/analysis/`. **Every rule is
UNVERIFIED in the running game.** The last section is the bl-sdk check that would confirm it.

**Clean room.** The lane that wrote this note implements none of it. A fresh implementer codes the rules from this
note alone.

**Where it fits.** This extends NATIVE_PAWN_AI_INTERFACE.md section 6 ("A component's answer"), which read the body
for `ActorComponent` and `SkeletalMeshComponent` and left every other component class open. It is a separate file
because it also covers `WillowAIComponent.PhaseLockOnHold`, which is not an interface question.

**What it answers.** Link 9's stop after NATIVE_GEARBOXMIND_GETAICOMPONENT.md (#594). `Flag_Skills_PhaseLockOnHold`
(NATIVE_FLAG_EXPRESSION_EVALUATE.md) resolves its context to the target mind's AI component, a `WillowAIComponent`.
Its `ObjectFunctionFlagValueResolver` then:
- asks that component `QueryInterface(IFlagProvider)` for a flag store;
- calls the script function its `FunctionCall` names, `PhaseLockOnHold`, on the component.

Today the port answers the first with the open `NotYet { NotRead }` of `ow_combat::pawn_interface`'s component rule
(#524): "what a WillowAIComponent answers to QueryInterface is not read".

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Whose `QueryInterface` an `AIComponent` or `WillowAIComponent` gives | `ActorComponent`'s body, unchanged: the plain answer, then its `Owner`'s answer (NATIVE_PAWN_AI_INTERFACE.md section 6.2) | high | UNVERIFIED |
| What they answer for `IFlagProvider` | the component's class chain lists no `IFlagProvider`, so the answer is its `Owner`'s: for a mind's AI component, the mind itself, a flag store (NATIVE_GEARBOXMIND_QUERYINTERFACE.md section 2, step 3) | high (the rule), medium (that `Owner` is the mind) | UNVERIFIED |
| What `WillowAIComponent.PhaseLockOnHold()` returns | true when the component's `PhaseLockHolds` list is not empty, else false | high | UNVERIFIED |
| Writes, loads, draws | none, for both | high | UNVERIFIED |

## 1. The `QueryInterface` body

The class tables of `AIComponent` (its parent is `ActorComponent`) and `WillowAIComponent` (its parent is
`AIComponent`) were found through each class's native registration and constructor. Their entry for the
`QueryInterface` virtual is the same body that `ActorComponent` and `SkeletalMeshComponent` hold. So their rule is
NATIVE_PAWN_AI_INTERFACE.md section 6.2, unchanged:
1. **the plain answer:** the component itself, when its class or a parent lists `I`;
2. **its owner:** otherwise, when `Owner` (`ActorComponent`'s actor field) is set, the question goes to that actor,
   which answers by its own rule;
3. otherwise none, with section 6.2's repeat guard.

The same body was confirmed by the field it reads: its `Owner` read sits exactly on `ActorComponent.Owner`. The
`WillowAIComponent` layout ends at the size its native registration gives.

**`IFlagProvider` on a mind's AI component.**
- Step 1 declines: as the port decodes the package tables, neither class nor any parent lists `IFlagProvider`.
- Step 2 asks the `Owner`. For a mind's component in the game, that is expected to be the mind.
- The mind answers by `Controller`'s rule: its class lists `IFlagProvider`, so step 3 of that rule answers the mind
  itself. The flag store `ObjectFunctionFlagValueResolver` then uses is the mind's own (NATIVE_FLAG_EXPRESSION_EVALUATE.md
  section 4.2), the same store `Flag_Skills_DisablePhaseLock` reads.

**Other interfaces link 9 asks of the component.** None was seen. The cast reaches the component only through this
flag's resolver: one `QueryInterface(IFlagProvider)` and one `PhaseLockOnHold` call per evaluation.

## 2. `WillowAIComponent.PhaseLockOnHold()`

A native with no arguments, called from script (`FunctionCall` "PhaseLockOnHold", `CheckRate` 0.2). Its script entry
calls one virtual of the component. For `WillowAIComponent`, that virtual returns true when the component's
`PhaseLockHolds` array (a list of `AIComponent.AIHoldData`) has at least one entry, and false when it is empty. It
reads only the array's length. It writes, loads and draws nothing.
- The function resolver turns the answer into the flag: true sets it, false clears it (NATIVE_FLAG_EXPRESSION_EVALUATE.md
  section 4.1, step 4). `Flag_Skills_PhaseLockOnHold` is read with `IsFalse`, so a target with no Phaselock hold
  passes this test.
- Who adds to `PhaseLockHolds` (a Phaselock hold of `Behavior_AIHold`'s kind, NATIVE_AI_HOLDS.md) was not read. A
  freshly spawned target holds none.
- `AIComponent`'s own entry for this virtual was not read. The native is declared on `WillowAIComponent` only.

## 3. What the port does not do yet: the component's `Owner`

On the installed packages, the harness's target mind (lane IMPL-LINK9-TARGET-MIND, spawned by the world's own spawn)
holds its own `WillowAIComponent` instance, but that component's `Owner` is None. The port's spawn does not run the
component attach step that sets it; the encroachment check already refuses at that step with NotRead. In the game a
component's `Owner` is set when its actor attaches it (UNVERIFIED for this component). With `Owner` None, section 1's
rule answers none, `PhaseLockOnHold`'s flag has no store and reads false, and `CanLiftTargetIf` blocks the lift with no
stop at all.

## 4. A proposed Off answer, for a separate implementer (UNVERIFIED)

Reference registers none of this and is unchanged: `crate::hit::ai_interface_for` and the plain answer stay as they
are.

**The `QueryInterface` rule.**
- Add `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent` to the exact-class list of `ow-combat`'s
  component rule (`pawn_interface.rs`, `COMPONENT_RULE_CLASSES`), so both answer by NATIVE_PAWN_AI_INTERFACE.md
  section 6.4's contract, refusals included. Other subclasses keep the open NotRead: the exact-class test stays.
- **The unattached owner, a choice for the hub.** Section 3 makes "Owner None" ambiguous in the port: it cannot tell
  a component the game never attached from one whose attach step the port does not run.
  - (a) Faithful: Owner None answers none, as section 6.4 already says. Link 9 then fails with no evidence and no
    stop.
  - (b) **Recommended for Off:** when `Owner` is None and `bAttached` is false, answer
    `NotYet { NotRead }`, "the component `<path>` has no Owner and is not attached: the attach step that sets Owner is
    not ported (asked for `<interface>`)". It is read before any write and changes nothing. That matches the
    encroachment check, which refuses at the same step. A component that is attached with Owner None still answers
    none.
- Writes, loads, draws: none, as section 6.4.

**`WillowAIComponent.PhaseLockOnHold()`** (a new Off native in `ow-combat`'s holds pack, next to the hold kinds).
- **The answer.** Read the receiver's `PhaseLockHolds` by name, with a read that does not call `loaded`
  (`vm.space.prop` and `vm.space.read`). Return `Bool(length > 0)`. Nothing is loaded, written or drawn.

| Case | Outcome |
|---|---|
| Called with no object | `VmError::Script` |
| The receiver's class is not exactly `WillowAIComponent` (a subclass may replace the virtual; none was read) | `NotYet { NotRead }`, "PhaseLockOnHold on a `<class>` is not read" |
| `PhaseLockHolds` not declared, or not an array | `VmError::Script` |
| The receiver is an unloaded definition handle | `NotYet { NotRead }`, not loaded first |

No `LayerAbsent` or `NotPorted` case.

**Tests on invented classes** (synthetic fixtures, the existing `package()` / `tag()` helpers). Each checks the answer,
and that heap committed writes, load transitions and generator draws are unchanged.
- The component rule:
  - a toy `AIComponent` and a toy `WillowAIComponent` whose class lists `I` (the component itself);
  - one that does not, with an `Owner` that lists it (the owner);
  - an `Owner` that declines (none);
  - Owner None and attached (none);
  - Owner None and not attached (option b: NotRead; option a: none);
  - a subclass of `WillowAIComponent` (the open NotRead);
  - a question that comes back to the component (none).
- `PhaseLockOnHold`:
  - an empty `PhaseLockHolds` (false) and one with one entry (true);
  - a subclass receiver (NotRead);
  - no object, and a class without the field (`VmError::Script`);
  - an unloaded receiver handle (NotRead, not loaded).
- The two together, as link 9 reaches them: a toy mind whose class lists `IFlagProvider`, its attached component with
  `Owner` set to it, and empty holds. `QueryInterface(IFlagProvider)` on the component answers the mind, and
  `PhaseLockOnHold` answers false.

## 5. Local probe (never pushed)

A throwaway local merge:
- the link 9 stack (#535 at its current head, #579, #564, #548, #555, #560, the link 9 spawn branches), #524 (the
  component rule) and #523 (noun replication, merged by hand: its strategy-2 resolver over the newer main);
- probe-only stubs: #589's rule reduced to its plain step, #594's `GetAIComponent`, and this note's two answers (the
  two AI component classes added to the component rule's list, and `PhaseLockOnHold`).

Installed Off strict `--slice-gate`, under the machine lock:
- **On that merge:** 2 ok / 11 NotYet / 1 error. Link 9 ran both of the driver's steps (`spawn`, `cast`) and is the
  error, with "no stop, but no evidence": the cast was blocked. The component's `Owner` was None (section 3), so it
  answered no `IFlagProvider`, and `PhaseLockOnHold` was never called.
- **With one more probe-only stand-in**, setting the component's `Owner` to its mind, as the attach step would: still
  2 / 11 / 1. Link 9 ran both steps and is an error at the unbound native
  `AttributeDefinitionBase.ResolveContext(object)`, the next stop past this note.

## Open (each UNVERIFIED)

- That a mind's AI component has the mind as `Owner` in the game, and when it is set (the attach step, or the mind's
  own set-up).
- `AIComponent`'s own `PhaseLockOnHold` virtual, and any subclass's.
- Who adds to `PhaseLockHolds`, and under what name and tag.

## Live confirmation (bl-sdk, the RTSE way; not run, no session granted)

On a spawned enemy's mind (a `WillowMind`):
- read its `AIComponent`, that component's `Owner` (expected the mind) and `bAttached`;
- call `QueryInterface(IFlagProvider)` on the component (expected the mind) and `QueryInterface` for an interface no
  object on the chain lists (expected None);
- call `PhaseLockOnHold()` on the component with no Phaselock on the enemy (expected false), and while Maya holds it
  in Phaselock (expected true, if the lift adds a hold; record `PhaseLockHolds`' length either way).
