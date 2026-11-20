# Native AI holds: the movement, demigod, god and dialog kinds, and how a hold reaches the AI (draft, 2026-11-16)

AI-assisted (Claude), analyst lane FIX-AIHOLD. Read from the game executable with Ghidra (`tools/ghidra/run.ps1`,
read-only project), written in our own words; no decompiler output, pseudo-code, addresses or listing structure is
reproduced here. Field names come from the cooked packages (`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED
in the running game**; each section ends with the live observation that would confirm it.

**Clean room.** The lane that wrote this note does not implement it. In the Rust port these natives answer `NotYet` in
Off until a fresh implementer codes them from this note alone (decision 2026-11-16-74).

## Summary

| Native | Script signature (from the package) | Used by | Confidence | Status |
|---|---|---|---|---|
| `AIComponent.HoldMovement` / `ReleaseMovement` / `MovementOnHold` | `native function HoldMovement(name HoldName, Object Tag)` etc. | `Behavior_AIHold` Type 2 | high (lists), medium (the side effect) | UNVERIFIED |
| `AIComponent.HoldDemigod` / `ReleaseDemigod` / `DemigodOnHold` | the same shape | `Behavior_AIHold` Type 3: the Fire dummy's `Behavior_AIHold_59` | high | UNVERIFIED |
| `AIComponent.HoldGod` / `ReleaseGod` / `GodOnHold` | the same shape | `Behavior_AIHold` Type 4 | high | UNVERIFIED |
| `AIComponent.HoldDialog` / `ReleaseDialog` / `DialogOnHold` | the same shape | `Behavior_AIHold` Type 5 | high | UNVERIFIED |
| `GearboxMind.GetAIComponent` | `native function AIComponent GetAIComponent()` (`IAIInterface`) | every `Behavior_AIHold` call | low | not resolved |
| How a pawn context reaches `IAIInterface` | (the `QueryInterface` of a pawn) | `Behavior_AIHold` on a pawn's provider | high | read 2026-11-19: [NATIVE_PAWN_AI_INTERFACE.md](NATIVE_PAWN_AI_INTERFACE.md) (the pawn's `Controller` answers); implemented in Off (decision 2026-11-19-62), the controller's own answer still `NotYet`; UNVERIFIED |

The base kinds (`Hold` / `Release` / `OnHold`, `HoldTarget` / `ReleaseTarget` / `TargetOnHold`) are in
NATIVE_AI_AWARENESS.md ("Holds, OnHold, ClearState, InitAI"); this note follows that wording.

## The script that calls them (for context)

`Behavior_AIHold.ApplyBehaviorToContext` is UnrealScript (read with `research/script_disasm.py`): it asks its
`ContextObject` for `IAIInterface`; with none, or with a `Reason` of None, it returns. Otherwise `Action` 0 holds and any
other value releases, and `Type` picks the kind: 0 base, 1 target, 2 movement, 3 demigod, 4 god, 5 dialog. The hold name
is the behavior's `Reason`, the tag is the context object, and the component is the interface's `GetAIComponent()`.

So `Behavior_AIHold_59` of the Fire dummy (reason `Safe`, action 0, type 3) is `HoldDemigod('Safe', <its context>)`.
COMBAT_GOLDEN_TRACE.md question 8 read type 3 as the bits `Target | Movement`; the script's switch makes it the demigod
kind instead, which fits the demigod flip recorded there.

## The four kinds: one rule, four lists

Each kind keeps its own list on the component, an array of `AIHoldData` (`HoldName`, `Tag`): `MovementHolds`,
`DemigodHolds`, `GodHolds`, `DialogHolds` (the base and target kinds use `Holds` and `TargetHolds`).

- **`Hold<Kind>(HoldName, Tag)`:** a None `Tag` does nothing. Otherwise the list is searched for an entry with the same
  name and the same tag; when there is one, nothing changes; when there is none, the pair is appended at the end.
- **`Release<Kind>(HoldName, Tag)`:** a None `Tag` does nothing. Otherwise the entry with the same name and tag, if any,
  is removed (whether the rest keep their order was not read); without one, nothing changes.
- **`<Kind>OnHold()`:** true when the list has at least one entry.
- Each script native only unpacks its two arguments and calls a virtual function of the component, so a subclass can
  replace any of these. For the base class the virtuals are the rule above.

**Side effect of `HoldMovement` only.** When the pair is newly added (not when it was already there), and the component
has an `Owner` whose controller (asked through a virtual of the owner; read as its controller, not proven) has a `Pawn`,
that pawn's `Velocity` and `Acceleration` are both set to one engine vector constant, read as the zero vector (not
proven). `ReleaseMovement` has no side effect, and neither do the demigod, god and dialog kinds in the base class.

**What a demigod or god hold does to damage is not in these natives.** They only keep the lists. The Fire dummy's
`IsDemigodMode()` became true in the same call as `HoldDemigod` (COMBAT_GOLDEN_TRACE.md question 8), so something reads
`DemigodHolds` (or `DemigodOnHold`) when damage is taken; that reader was not looked for here.

**The Willow component.** `WillowAIComponent` declares its own native `Hold`, `Release` and `OnHold` (other native keys
in the packages, covered in NATIVE_AI_AWARENESS.md). It declares none of the four kinds, so the base script natives
serve it, but whether it replaces their virtual functions was not read: the tooling did not resolve that class's C++
function table.

*Read 2026-11-19 (lane LINK-1-NEXT, decision 2026-11-19-31; analyst only, the rules stay `NotYet` until implemented):*
**`WillowAIComponent` uses the same hold behaviour as `AIComponent` for the target, movement, demigod, god and dialog
kinds (all three natives of each)** (high); only the base kind (`Hold`, `Release`, `OnHold`), which it declares itself,
behaves differently, and that behaviour was not read. So on a bullymong's or the dummy's component the demigod, god, movement
and dialog lists follow the rule above unchanged, and the hit's `DemigodOnHold` is the base answer (any entry in
`DemigodHolds`). The target kind (Type 1) follows NATIVE_AI_AWARENESS.md's rule; only Type 0 (base) stays `NotYet` on a
`WillowAIComponent`.

- **Live confirmation:** with bl-sdk, hook `Behavior_AIHold.ApplyBehaviorToContext` on the Fire dummy and read the
  dummy's component `DemigodHolds` before and after `Behavior_AIHold_59` (one entry, `Safe` and the context, expected);
  call `HoldMovement` on a moving AI and read its pawn's `Velocity` and `Acceleration` on the next frame; call each
  `Release<Kind>` with the same pair and read the list empty.

## `GearboxMind.GetAIComponent`

The script native returns what a virtual function of an object held inside the mind returns (the C++ side of
`IAIInterface`). Which field that object is was not resolved: the layout tool stops on `GearboxMind` (it cannot lay out
a `MapProperty`). Reading it as "the mind's `AIComponent` property", as `ow-combat` binds it, is plausible and
unproven.

**Read 2026-11-20** in [NATIVE_GEARBOXMIND_GETAICOMPONENT.md](NATIVE_GEARBOXMIND_GETAICOMPONENT.md): it is the mind's
own `AIComponent` property, as it is (placed by structural checks on the layout). Still UNVERIFIED in game.

- **Live confirmation:** call `GetAIComponent()` on a bullymong's mind and compare with its `AIComponent` property.

## Not read

- **How a pawn context reaches `IAIInterface`.** *Read 2026-11-19 in [NATIVE_PAWN_AI_INTERFACE.md](NATIVE_PAWN_AI_INTERFACE.md):
  a pawn answers by its own class first; then its `Controller`, or, only when it has no `Controller`, the controller of a vehicle it drives; then its `Mesh`; else none (that note's section 2 is the rule). For a possessed AI pawn the mind is expected to
  answer (UNVERIFIED). Implemented in Off by lane IMPL-PAWN-AI-INTERFACE (decision 2026-11-19-62): the controller,
  vehicle controller and mesh steps stay `NotYet` until their own answers are read (`GearboxMind`'s for the Fire
  dummy). The text below is the question as it stood.* The package class tables list `IAIInterface` for
  `GearboxMind` and `WillowMind`, and not for `WillowAIPawn` or any of its parents, so by the tables `QueryInterface` on the Fire dummy's
  pawn answers None and the hold script does nothing. Yet the live dummy became demigod in its `Behavior_AIHold_59`
  call. Either that behavior's context was not the pawn (for example its mind), or a pawn answers the interface in a way
  the tables do not show. Neither was read. **Live confirmation:** in the same hook, log the class of `ContextObject` and
  whether `QueryInterface(IAIInterface)` on it is None.
- The reader of `DemigodHolds` and `GodHolds` on the damage path.
- `WillowAIComponent`'s replacement of the base kind (`Hold`, `Release`, `OnHold`); it keeps the other kinds' routines
  (read 2026-11-19, above).
- The removal's effect on the order of the remaining entries.
