# `GearboxMind.GetAIComponent`: the mind's own `AIComponent` (link 9, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-GEARBOXMIND-GETAICOMPONENT, decision `docs/decisions/2026-11/2026-11-19-NNN-gearboxmind-getaicomponent-is-the-mind-s-aicomponent.md`.
Read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and a local read-only table reader, under
the machine lock). Field names come from the cooked packages (`tools/ghidra/class_layout.py`). It is written in our
own words: no decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here, and raw
output stays under ignored `local/analysis/`. **Every rule is UNVERIFIED in the running game.** The last section is
the bl-sdk check that would confirm it.

**Clean room.** The lane that wrote this note implements none of it. A fresh implementer codes the rule from this
note alone.

**What it answers.** NATIVE_AI_HOLDS.md's open row, "`GearboxMind.GetAIComponent`": that note found that the script
native returns what a virtual of an object held inside the mind returns, but not which field it reads, because the
layout tool stops on `GearboxMind`'s `MapProperty`. It is link 9's next stop after NATIVE_GEARBOXMIND_QUERYINTERFACE.md
(#589): `Flag_Skills_PhaseLockOnHold`'s AI component context resolver asks the target's mind for `IAIInterface`, gets
the mind, and calls `GetAIComponent()` on it.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| What it returns | the mind's own `AIComponent` property (a component property `GearboxMind` declares), as it is, None included | high | UNVERIFIED |
| `WillowMind` | the same: it keeps `GearboxMind`'s answer | high | UNVERIFIED |
| Arguments | none | high | UNVERIFIED |
| Writes, loads, draws | none: one field read | high | UNVERIFIED |

## 1. What it does

The script native `GetAIComponent()` takes no arguments. It calls the mind's native `IAIInterface` implementation
(the interface table every `GearboxMind` embeds), and that implementation returns one field of the mind: its
`AIComponent` property. Nothing is checked, created or cached. A mind whose `AIComponent` is None returns None.

`WillowMind` replaces `GearboxMind`'s `IAIInterface` table with its own, but the entry this native calls is the same
in both. So a `WillowMind` answers exactly as a `GearboxMind` does.

**How the field was placed** (structural checks, not a fit by eye):
- `GearboxMind`'s own fields were laid out from the packages with the size of its one `MapProperty` left unknown.
  Exactly one size, the engine's map size, makes the class end at the size its native class registration gives.
- With that size, the `IAIInterface` table pointer the native calls through falls exactly on the class's
  `VfTable_IIAIInterface` field. The field the implementation reads falls exactly on `AIComponent`, the class's last
  field.

**Writes, loads, draws.** None. It reads one field and returns it. It does not load the component, attach it, set its
`Owner` or create one when the field is None.

Two other classes have a script native of the same name, `PopulationOpportunityDen.GetAIComponent` and
`WillowGameInfo.GetAIComponent`. They are separate natives and were not read.

## 2. Link 9

The harness's target mind (lane IMPL-LINK9-TARGET-MIND) is a `WillowMind` spawned by the world's own spawn. On the
installed packages its `AIComponent` holds its own `WillowAIComponent`, so `GetAIComponent()` returns that component.
`Flag_Skills_PhaseLockOnHold`'s value resolver then asks that component for `IFlagProvider`. What a component answers
is NATIVE_PAWN_AI_INTERFACE.md section 6 (the plain answer, then its `Owner`), and section 4 below has what the probe
met there.

## 3. A proposed Off answer, for a separate implementer (UNVERIFIED)

**Where.**
- `ow-combat`'s `holds` pack binds `GearboxFramework.GearboxMind.GetAIComponent` in Off to a `NotYet { NotRead }`
  today (`crates/ow-combat/src/holds.rs`, "which object of the mind answers it is not resolved"). Replace that NotYet
  with this answer.
- The Reference stand-in in `crates/ow-combat/src/hit.rs` (the mind's `AIComponent`, decision 2026-11-16-83) already
  gives the same value. Keep it as it is: Reference is unchanged.
- Text and tests that pin the old NotYet, to update with it:
  - `crates/ow-combat/src/holds.rs`: the `GET_AI_COMPONENT` doc ("the note does not resolve") and the module doc;
  - `crates/ow-combat/src/hit.rs`: the Reference-only comment above the stand-ins ("GetAIComponent `NotYet`, the
    note leaves it unresolved");
  - `crates/ow-ai/src/awareness/holds.rs`: the doc comments that call `GetAIComponent` a `NotYet` (the module doc and
    the pack's doc);
  - `crates/ow-ai/src/awareness/holds/tests.rs`: `the_pack_binds_the_hold_kinds_in_off_only` pins
    `NotYet { NotRead }` naming NATIVE_AI_HOLDS.md for `GetAIComponent`. It must pin the answer instead.

**The answer.** Read the receiver's `AIComponent` by name, and return the value as it is: the component handle or
None. Nothing is loaded, written or drawn, and the returned handle is not loaded either.
- **The accessor.** A read that does not call `loaded`: `vm.space.prop(class, "AIComponent")` and then
  `vm.space.read(receiver, PropAddr::scalar(prop))`. Not `hit::access::get` or `required`: both call
  `access::loaded`, which runs `ensure_loaded` on an unloaded handle (a load). Reference's stand-in keeps `required`,
  unchanged.
- **An unloaded receiver.** If the receiver itself is an unloaded definition handle (`vm.defs.is_unloaded`, a read),
  the answer is `NotYet { NotRead }`: "GearboxMind.GetAIComponent: the mind `<path>` is an unloaded definition handle
  (reading its AIComponent would load it)". It is not loaded first. A mind that answers interface questions is a live
  actor in every route read so far, so this case means a harness has handed over a handle by mistake.
- An unloaded stand-in handle is returned as a handle. Loading it is the business of whatever uses it later, under
  that caller's own rule.
- The native never makes a component, so neither does the port.

**Refusals, all before any effect (nothing to undo).**

| Case | Outcome |
|---|---|
| Called with no object (`this` None) | `VmError::Script` |
| The receiver's class is not `GearboxMind` or a subclass | `VmError::Script` (the native belongs to `GearboxMind`) |
| The receiver's class does not declare `AIComponent`, or the field holds a value that is not an object | `VmError::Script` (a malformed class or value) |
| `AIComponent` names a class default's own template component (not an instance; the port's choice, not the native's) | `NotYet { NotRead }`: "the mind's AIComponent is its class default's template, not an instance". A hold written to it would land on the shared template, and the game always holds an instance here |

| The receiver is an unloaded definition handle | `NotYet { NotRead }` (above), nothing loaded |

No `LayerAbsent` or `NotPorted` case: the answer needs only the receiver and its class, which a call already has.

**The template test decodes nothing.** It reads only the heap, never the definition store. Take the returned
component's `outer` (`vm.space.heap.get(component)?.outer`), then that outer's record. It is a template when the outer
is a class default:
- its `origin` is `Origin::Default { .. }`, the heap's own mark for a class default object; or
- the outer's name starts with `Default__`, for a class default that came in as a package export.

Both are reads of objects already in the heap. It does not call `class_default` (`ow_vm::interp::load`, which decodes
through `vm.defs`, a load), and it does not ask `ow-defs`' crate-private `is_class_default`. A component with no
outer, or an outer that is neither, is not a template. An unloaded stand-in handle is judged by its own heap record
the same way, and it is not loaded.

**Where a NotYet from this answer lands.**
- **In a `Behavior_AIHold` run** (`ow_combat::holds::apply_hold_behavior`): the Off preflight
  (`preflight_hold_contexts`) judges every context before any script runs. The implementer adds the template test and
  the unloaded-receiver test to that preflight, for each context whose `QueryInterface(IAIInterface)` answer is a mind,
  so the hold refuses before any write. A NotYet raised later, inside the run (on context 2, after context 1's script
  ran), goes through the existing "stopped after partial writes" handling in `apply_hold_behavior`.
- **In link 9's cast** (the flag evaluator's AI component context resolver): the NotYet passes up through
  `FlagExpressionEvaluator.Evaluate`. The evaluator marks it "stopped after partial writes" when its counters moved,
  and the slice driver grades it as the cast's NotYet.

**Tests on invented classes** (synthetic fixtures, the existing `package()` / `tag()` helpers). Each checks the value,
and that heap committed writes, load transitions and generator draws are unchanged.
- A toy mind (a `GearboxMind` subclass) whose `AIComponent` holds its own component: returns that component.
- A mind whose `AIComponent` is None: returns None, and nothing is created.
- A `WillowMind`-like subclass: the same answer as its parent.
- A mind whose `AIComponent` is an unloaded stand-in handle: returned as the handle, and not loaded.
- A mind whose `AIComponent` is a template, outered to an object with `Origin::Default`, and one outered to an
  exported `Default__` object: `NotYet { NotRead }`, with load transitions unchanged (the test decodes nothing).
- A mind that is an unloaded definition handle: `NotYet { NotRead }`, and it is not loaded.
- A `Behavior_AIHold` run over two contexts whose second mind holds a template: the preflight refuses, and no
  context's script has run.
- Called with no object, on a class outside `GearboxMind`, and on a class with no `AIComponent` field:
  `VmError::Script` each.
- Reference: the `hit.rs` stand-in still answers, unchanged.

## 4. Local probe (never pushed)

The probe was a throwaway local merge:
- #535 (the flag evaluator), #579 (the target mind), #564, #548, #555, #560, #523 and the link 9 spawn branches;
- two probe-only patches: #589's answer for a mind asked `IAIInterface` (the mind itself), and this note's answer
  bound in link 9's set-up.

Installed Off strict `--slice-gate`, under the machine lock:
- **On that merge:** 1 ok / 3 NotYet / 10 errors. Link 9's `spawn` and `cast` both ran, and it is an error with "no
  stop, but no evidence": the cast took the blocked path. `GetAIComponent()` returned the mind's own
  `WillowAIComponent`. The component then answered no `IFlagProvider`, so `Flag_Skills_PhaseLockOnHold` read false and
  `CanLiftTargetIf` blocked the lift. In that world the component gives the plain answer: its `QueryInterface` rule
  (#524, `fix/rust-link11-query-interface`) is not in the stack.
- **The same merge with #524 added:** 1 ok / 4 NotYet / 9 errors. Link 9 is NotYet at its `cast` step, stopped after
  partial writes: "what a `WillowAIComponent` answers to `QueryInterface` is not read (asked for `IFlagProvider`)".
  That open answer is NATIVE_PAWN_AI_INTERFACE.md's, and it is link 9's next stop once this native and #589's answer
  are implemented.

## Open (each UNVERIFIED)

- Whether a mind's `AIComponent` can be None in the running game at the moment link 9's flags are read.
- `PopulationOpportunityDen.GetAIComponent` and `WillowGameInfo.GetAIComponent` (not read).

## Live confirmation (bl-sdk, the RTSE way; not run, no session granted)

On a spawned enemy's mind (a `WillowMind`), call `GetAIComponent()` and compare the result with the mind's
`AIComponent` property: expected the same object. Do the same on a mind whose `AIComponent` was cleared for the test
(expected None), and on a non-Willow `GearboxMind` if one is spawned in the map (expected its own `AIComponent`).
