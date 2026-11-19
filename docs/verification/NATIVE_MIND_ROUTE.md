# Native: the three natives on an AI mind's starting-values route (2026-11-19)

AI-assisted (Claude Code). Lane MIND-ROUTE-NATIVES, analyst only (clean room, docs/LANE_BRIEF.md; decision 2026-11-19-25).
Read locally from `Borderlands2.exe` with the project's Ghidra tooling (read-only queries and a local read-only table
reader; raw output only under ignored `local/analysis/`), in our own words. Field names come from
`tools/ghidra/class_layout.py`, corrected where noted. Nothing here was observed in the running game: **UNVERIFIED** until it
is. The lane that wrote this note does not implement it; every rule stays `NotYet` in Off until a fresh implementer codes it.

The route is NATIVE_COMBAT_GROUNDWORK.md section 1.11 (decision 2026-11-19-17): `WillowMind.ApplyCharacterClassDefaults`
reaches these three natives in this order.

## Summary

| Native | Parameters (from the package's exports) | Used on the route by | Confidence | Status |
|---|---|---|---|---|
| `WillowPawn.SetDefaultPhysics` | none, no result | `WillowMind.ApplyCharacterClassDefaults` | high for the gates and the collision copy; medium for the AI pawn's default physics; low for two unnamed queries | UNVERIFIED |
| `WillowPawn.GetExpLevel` / `WillowAIPawn.GetExpLevel` | none; returns an int (one native serves both declarations) | the passive-skill grade | high for the routing, medium-high for the field | UNVERIFIED |
| `WillowMind.RecalculateAttributeInitializedState` | none, no result | after the pawn's `ApplyAttributeStartingValues` | high for the structure, high for the field names | UNVERIFIED |

## A layout correction that applies to all three

`tools/ghidra/class_layout.py` does not place every `WillowPawn` and `WillowAIPawn` field where the game reads it, so
the fields this note names on those classes (`GameStage`, `ExpLevel`, `AwesomeLevel` and `WillowAIPawn`'s own) are
identified by meaning and by name, not by the tool's layout (UNVERIFIED). `WillowMind`'s `AIClass` and `MyWillowPawn`
are named the same way (a fit). Correcting the tool is a separate lane; the evidence stays under ignored
`local/analysis/`.

## 1. `WillowPawn.SetDefaultPhysics()`

The script native calls one routine on the pawn; no arguments, no result. In order:

1. **Gates.** It does nothing when the pawn is gone (a pawn query that answers `bDeleteMe` or `bIsDead`), when the pawn's
   `ObjectArchetype` is not a `Pawn`, when the pawn has no `SMComponent`, or when that component's current special move
   (`CurrentSMData.SMD`) is a `SpecialMove_Spawned` (a pawn still in its spawn move keeps its physics).
2. **Physics.** The pawn's physics becomes its **default physics** (an AI pawn's rule below), through the pawn's physics
   setter (on `WillowAIPawn` an override with an extra step when changing into `PHYS_NavMeshWalking`: the mind's navigation
   handle is re-anchored; not read further).
3. **`bCollideWorld`** is copied from the archetype.
4. **Collision.** Unless two pawn queries both answer true (not named: they read `WillowPawn` fields inside the unplaced range
   above, one of them through the pawn's damageable interface; low), the pawn's collision is set from the archetype's
   `bCollideActors`, `bBlockActors` and `bIgnoreEncroachers`.

**An AI pawn's default physics** (`WillowAIPawn`'s own virtual; values named by the package's `EPhysics`):
- no `AIClass`: `PHYS_Falling`;
- the class's `bIsTurret`: `PHYS_None`;
- the pawn has no `MyWillowMind`, or the mind has no `WillowNav` (its navigation handle): the class's `Physics`;
- otherwise, when one query of the navigation handle answers false (not named) and the class's `Physics` is not
  `PHYS_Flying`: `PHYS_NavMeshWalking`; else `PHYS_Flying`.

What script sees: the pawn's `Physics`, `bCollideWorld`, `bCollideActors`, `bBlockActors` and `bIgnoreEncroachers`. The
stock-UE3 meaning of the physics setter (contacts, floor) is not part of this reading.

## 2. `GetExpLevel()` on a pawn

- The script native (declared on `WillowPawn` and again on `WillowAIPawn`; one native routine serves both) returns what the
  pawn's own `IBalancedActor` implementation answers: it calls that interface's `GetExpLevel` slot through the interface
  table embedded in the pawn.
- **On a `WillowAIPawn` the answer is the pawn's own `ExpLevel` int** (medium-high). The interface table's slots follow the
  interface's declared order (`GetGameStage`, `GetAwesomeLevel`, `GetExpLevel`, `GetExpLevelForEquip`, `SetGameStage`,
  `SetAwesomeLevel`, `SetExpLevel`, `GetBalancedActorTypeIdentifier`); each getter and its setter touch the same field, and
  the three fields are `GameStage`, `ExpLevel` and `AwesomeLevel` after the layout correction above. It reads no other
  object: not the mind, the AI class or the world.
- Nothing is computed: the value is whatever was stored (by the spawn's balance code or `SetExpLevel`).

## 3. `WillowMind.RecalculateAttributeInitializedState()`

The shared script native (NATIVE_LEVEL_UP_ATTRIBUTES.md section 1: one thunk for `Controller`, `WillowPlayerController` and
`WillowMind`) reaches `WillowMind`'s own override. In order:

1. **Skill refresh.** When the world's game info is a `WillowGameInfo`: its skill manager is fetched with the routine behind
   `WillowGameInfo.GetSkillManager` (lane FIX-SKILLMGR-A's note: made lazily on the authority when absent). Unless the
   manager's `bUpdatingSkills` is set, that flag is set; every `ActiveSkills` entry whose `SkillInstigator` is this mind gets
   `bForceRefreshModifiersNextTick` set and one call on the skill (not read further); then `bUpdatingSkills` is cleared.
2. **The controller-class part** (the same routine as the `Controller` behaviour of NATIVE_LEVEL_UP_ATTRIBUTES.md section 1
   step 1 and 2): with a pawn that is not excluded and a `CharacterClass`, the pawn's `EncumbranceResistance` value from the
   class and the damage-modifier attributes re-initialised (base from the class's init data, value recomputed from the
   stack); then, always, every pool of the controller's `ResourcePoolManager` is rebased. This pass counted 13
   base-and-recompute writes where that note lists 12: not reconciled here.
3. **The mind's own part.** When the mind has both `AIClass` and `MyWillowPawn`: the pawn's `CommerceMarkup` value is set to
   the evaluation of the class's `BaseCommerceMarkup` (the value only); and, when the pawn has a `StatusEffectComp`, ten
   status-effect resistance attributes of that component are re-initialised from the class's matching init data (base set
   to the evaluation, value recomputed from the attribute's modifier stack, no change notification): `Ignite`, `Shock`,
   `Corrosive`, `Slow` and `Amp` `ChanceResistanceModifier`, then the same five `DurationResistanceModifier`, from
   `Base<Name>`.

What script sees: the pawn's damage-modifier and status-effect resistance attributes, `CommerceMarkup`,
`EncumbranceResistance`, the pools' rebased maxima, and the refreshed skills of this mind.

## Not read

- The two collision-gate queries of section 1, the navigation handle's query, the physics setter's navigation step.
- The skill call of section 3 step 1.
- Which `WillowPawn` field the layout tool misplaces.

## Live confirmation (bl-sdk)

On a fresh bullymong: `Physics` and the collision flags right after the spawn; `GetExpLevel()` against `ExpLevel`; the
status-effect resistances and `CommerceMarkup` on its `StatusEffectComp` and pawn against the AI class's init data.
