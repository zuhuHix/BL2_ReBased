# Link 1 after ApplyBodyClassStartingValues: the rest of maya:8, and players:1 (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-MAYA9-AND-AFTER (decision 2026-11-26-34), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/maya9/`. This note is
in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** NATIVE_BEHAVIOR_KERNEL_PASS2.md (#771) predicts that `InitializeBehaviorProviders` completes and that the rest
of `ApplyBodyClassStartingValues` is bound on main. The hub asked what link 1 (`spawn_maya`) meets after that, through
its two steps, and whether links 3, 4, 5, 13 and 14 then reach their own steps.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The port** on main 3b8df87:
  - the slice gate's link list (`ow-cli` `slice_gate.rs`). Link 1's steps are `maya:8` then `players:1`. Every combat
    link runs in one run, in list order, so links 3, 4, 13 and 14 start where link 1 ends, and link 5 reads link 3's
    rows;
  - the combat run's steps (`ow-combat` `run.rs`). `maya:8` does, in order:
    - the class select (`ServerPlayerSelectClass`, the game's script route);
    - the experience set;
    - the health set-up (the controller's installed `ApplyCharacterClassDefaults` on the bridge's pawn);
    - the bridge pawn's death preparation.
    
    `players:1` writes `EffectiveNumPlayers` on the world's game.
- **The script** (`research/script_disasm.py`): `WillowPlayerController.ApplyCharacterClassDefaults`,
  `WillowPawn.BodySelectClass`, both `ApplyBodyClassStartingValues`, and each script function named below.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs and the class-table
  slots of the four natives in section 3, each slot in its class's primary class table (the
  NATIVE_COMPONENT_SKIP_FLAG.md section 6 method). The shared material execs were read in the native table.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field below lands where the routine reads
  it, under the usual shifts (none for actors and definitions), except the two stance fields of 3.1 (fitted).
- **A local probe** (temporary code, reverted, never committed). It ran the installed Off strict `--slice-gate` on main
  3b8df87, under the machine lock, with a call trace of script-to-script calls.

  The probe stubbed, one at a time, each stop it met, so the next could be seen:
  - `InitializeRBPhysics` (lane E's) and `GetPhysXLevel` (answering 0);
  - pass 2's conditions (#771);
  - the rest of section 2's stops, in order.

  The stubs are not answers. They only show the order.

## 2. What link 1 meets, in order (measured by the probe, with each earlier stop stubbed)

Today's stop is inside `maya:8`'s **health set-up**, not in the class select. The bridge's `RestartPlayer` makes no
pawn (NATIVE_MAYA_PAWN_SPAWN_SOURCE.md). So the set-up's call of `WillowPlayerController.ApplyCharacterClassDefaults`
is the one that reaches `BodySelectClass` → `ApplyBodyClassStartingValues`. In the game, `SetPlayerDefaults` makes
that call from `RestartPlayer`.

| # | Where | Native or step | On main | This note |
|---|---|---|---|---|
| 1 | `SetupSkeletalMeshPhysics` | `InitializeRBPhysics`, then `Engine.GetPhysXLevel` | lane E, in progress (#763, #767) | — |
| 2 | `ApplyBodyClassStartingValues`, `bAlwaysRelevant` | `Pawn.IsPlayerPawn()` (virtual; reached only when `bPawnIsAlwaysRelevant` is false) | **the rule exists** (`render_hooks`' pawn test); the native key is **unbound** | 4.1 |
| 3 | `CheckPainSounds`, `CrouchHeight` | script only | runs | — |
| 4 | `InitializeBehaviorProviders`, preflight | `RunStance0` (`GD_Siren_Streaming.BodyClass_Siren:RunStance0`), a lazy handle: `NotRead` | the native is bound; the stance objects are **not loaded** | 4.2 |
| 5 | the same, pass 2 | the conditions | #771 | — |
| 6 | `SetDeathDefinition` | the direct provider native with `Death_Siren` (empty) | #771 section 5 | — |
| 7 | the player part | `WillowPlayerPawn.SetDefaultStance` | bound to `NotRead` (NATIVE_GO_DOWN_PATH.md 3.2) | **3.1** |
| 8 | the arms | `Actor.AttachComponent` | **the rule exists** (`ow-inventory` `attach_mesh`); **unbound in the combat world** | 4.3 |
| 9 | the arms' materials | `MaterialInstanceConstant.SetParent` and `.SetVectorParameterValue` | **the rules exist** under the `MaterialInstance` keys; the `MaterialInstanceConstant` keys are **unbound** | 4.4 |
| 10 | `ApplyCharacterClassDefaults` → `SetShieldResourcePoolDefinition` → the pool's `ResetIdleTimer` | the `ShieldResourcePool` idle-reset hook | `NotRead`, after the stamp (NATIVE_RESOURCE_POOL_IDLE.md) | **3.2** |
| 11 | `SetPlayerFOVFromProfile`, `SetSplitDirectionFromProfile` | script and bound natives | runs | — |
| 12 | `ApplyAttributeStartingValues` → `GetAttributeStartingValuesFromCharacterClass` | `FlagDefinition.ApplyFlagInitializationData(array, object)` | **unbound** | **3.3** |
| 13 | `SetUpTradeInteraction` | script, the proxy spawn | runs | — |
| 14 | below the client's net mode | `BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects(Self)` | **unbound** | **3.4** |
| 15 | `RecalculateAttributeInitializedState`, the rest of `maya:8`, `players:1` | bound | runs | — |

With rows 1 to 14 stubbed, **link 1 completes both steps**. Section 5 gives the caveats.

## 3. The natives read here

### 3.1 `WillowPlayerPawn.SetDefaultStance()` (and `WillowPawn`'s)

The exec only calls the pawn's virtual:
- **`WillowPawn`'s body:** nothing;
- **`WillowAIPawn`'s:** not read; it keeps `NotRead`;
- **`WillowPlayerPawn`'s body:**
  1. **the stance source:** the pawn's `BodyClass`, then its `AnimationParentBody` for as long as the source has
     `bUseParentStances` set and a parent. With no `BodyClass`: nothing;
  2. **the pick:**
     - `bIsInjured` set: the source's `InjuredStance`;
     - else `bIsCrouched` set: its `CrouchStance`;
     - else `bIsSprinting` (an int attribute) not 0: its `SprintStance`;
     - else its `RunStance`;
  3. **the set-stance** (the pawn's virtual, below), with that stance.

**The set-stance**, in order:
1. **nothing** when `Mesh` is None, the stance is None, `Role` is not the authority's, or the stance is already the
   pawn's current stance;
2. **the kind:** whether the stance is one of the source's five stances, by identity. Run, Patrol, Sprint, Injured and
   Crouch count; any other stance is an **override**;
3. **nothing** when an override is already held and the new stance is one of the five;
4. **the aim test, before the change:** `Physics` is not `PHYS_Ladder`, `AimOffset` is set, and the current stance is
   None or has `bCanUseAimOffsets`;
5. **an override** is held from now on;
6. **with a current stance:** the current stance's provider gets the event `OnExitStance` on the pawn's consumer.
   This goes through the kernel's event activation, the rule `ActivateBehaviorEventFromScript` already uses;
7. **the writes:**
   - `bNetDirty` set;
   - the current stance := the stance;
   - then `OnEnterStance` fired the same way;
8. **`bCanCrouch`:** one of the five gives the pawn archetype's `bCanCrouch`; an override gives false;
9. **the rotation branch:** with a `Controller`, when the aim test passed before and fails now:
   - `DesiredRotation` := `Rotation`;
   - the controller's rotation is set from it (that call was not read);
10. **the aim-offset refresh:**
    - the name is the stance's `AimOffsetName`, or the source's `AimOffsetNodeName` when that is None;
    - when `AimOffset` is unset or its node name differs, the node of that name is looked up on `Mesh`, through the
      mesh's anim-behaviour interface, and kept in `AimOffset` as a `WillowAnimNodeAimOffset` (None when absent or of
      another class). By fit, this is the lookup NATIVE_ANIM_NODE_LOOKUP.md describes;
11. **the left-hand IK step**, when `LeftHandIK` is set:
    - wanted: the current stance is None or has `bAllowLeftHandIK`, and `LeftHandIKHolds` is empty;
    - active: its `StrengthTarget` is above 0.5;
    - when they differ, `SkelControlBase.SetSkelControlActive(wanted)` (below).

**`SkelControlBase.SetSkelControlActive(bool)`** (read; the stock engine rule):
- `StrengthTarget` := 1 or 0;
- `BlendTimeToGo` := |`StrengthTarget` − `ControlStrength`| × `BlendInTime` when activating, or `BlendOutTime` when
  not;
- when that is not above 0: `BlendTimeToGo` := 0 and `ControlStrength` := `StrengthTarget`;
- then the same on `NextControl`, for as long as the control just set has `bPropagateSetActive`.

**Fitted, UNVERIFIED:**
- the current and override stance fields sit 4 bytes after the places the layout tool gives `CurrentStance` and
  `OverrideStance`. They are named by their use: the field compared, written and passed to the exit event is the
  current one;
- `bIsSprinting` is read at its own place exactly.

**Maya (data, `ow-package --def-dump`):**
- the source is `BodyClass_Siren` itself (`bUseParentStances` false), so the stance is `RunStance0`;
- `RunStance0` has `bCanUseAimOffsets` and `bAllowLeftHandIK` set and `AimOffsetName` None, so the node name is the
  body class's `AimOffset`;
- `Pawn_Siren`'s `bCanCrouch` is true;
- `RunStance0`'s provider has no sequences, so it was never registered (#740). `OnEnterStance` then reaches no
  registered provider, and the kernel's answer for that (by fit: nothing) was not read.

**Proposed Off answer:**
- **preflight** (write-free and load-free), each else `VmError::Script`:
  - the pawn's `BodyClass`, `bIsInjured`, `bIsCrouched`, `bIsSprinting`, `Mesh`, `Role`, `Physics`, `AimOffset`,
    `LeftHandIK`, `LeftHandIKHolds`, `Controller`, `bCanCrouch` and the two stance fields declared;
  - the source's five stance fields, `AnimationParentBody`, `bUseParentStances` and `AimOffsetNodeName` declared;
  - each stance's `AimOffsetName`, `bCanUseAimOffsets` and `bAllowLeftHandIK` declared;
- every object read must be resident (the body classes, the stance, the current stance, the archetype), else
  `NotYet { NotRead }`;
- the rotation branch, when it would run: `NotYet { NotRead }` ("the controller's rotation set");
- the event firings by the kernel rule, with its own refusals;
- the node lookup and `SetSkelControlActive` by the rules above, planned before the first write.

All refusals come before the first write. A refusal from a behavior the events run comes after writes and carries the
"stopped after partial writes: " prefix.

### 3.2 The `ShieldResourcePool` idle-reset hook

It runs after the stamp, inside `ResetIdleTimer`. It replaces NATIVE_RESOURCE_POOL_IDLE.md's "not read":
1. with no `Definition`: nothing;
2. `AssociatedProvider` as a `Controller` (a class test), its `Pawn` as a `WillowPawn`, that pawn's `EquippedItems[0]`
   as a `WillowShield`. Any of them missing or of another class: nothing;
3. the shield's `bPrimed` is recomputed from `ShieldDef.ShieldPrimeMechanism`. The game reads `ShieldDef` unchecked.
   Here C is the pool's `CurrentValue`, Min its `MinValue`, Max its `MaxValue`, and F its
   `bHasPoolBeenFullSinceLastBeingDepleted`. By the mechanism's value:

   | Value | Primed when |
   |---|---|
   | 1 | C equals Max |
   | 2 | C is at most Min, or F is clear |
   | 3 | C is at most Min |
   | 4 | always |
   | 5 | C is above Min |
   | 6 | C is above Min and F is set |
   | any other | never |

   The compares are ordered float compares: a NaN fails each of them;
4. when `bPrimed` changed: the shield's script event `ShieldPrimedStateChanged`. By fit, the routine finds that
   function by a name kept at run time, which was not read.

**Maya:** at the health set-up she has no shield (`EquippedItems` empty), so the hook does nothing.

**Proposed Off answer:**
- the hook is planned **before the stamp**. It reads nothing the stamp writes, so a refusal comes before any write;
- `Definition`, `AssociatedProvider`, `Pawn`, `EquippedItems`, `ShieldDef`, `ShieldPrimeMechanism`, `bPrimed` and the
  four pool values declared with their kinds, else `VmError::Script`;
- each object on the walk resident, else `NotYet { NotRead }`;
- a shield with `ShieldDef` None: `NotYet { NotRead }` ("a shield without a definition: the game reads it unchecked");
- then the stamp, `bPrimed`, and the event through the VM (its own refusals carry the prefix).

### 3.3 `FlagDefinition.ApplyFlagInitializationData(array FlagInitData, object Context)` (static)

1. with an empty array or a None `Context`: nothing;
2. **each entry:** the entry's flag, a chance and a timer duration (two attribute-initialization values; the struct's
   field names were not read, so the names are by fit):
   - skipped when the flag is None, its `ContextResolverChain` is empty, or its `ValueResolver` is None;
   - **the context:** the flag's own walk, NATIVE_RESOLVE_CONTEXT.md section 3;
   - with a context:
     1. the value resolver's set-up step for the flag and the context (a virtual, not read);
     2. the chance and the duration are evaluated with ow-attr's evaluator for that context;
     3. a draw: the C runtime's `rand()` scaled by 1/32767. This is **not** one of the game's seeded streams;
     4. chance below the draw: the flag set false. Else a duration above 0.0001: set true with that timer. Else: set
        true.
     
     The setters are the value resolver's, NATIVE_FLAG_EXPRESSION_EVALUATE.md's flag-store rules, by fit.

**Maya:** `CharClass_Siren`'s `DesignerFlagStartingValues` is empty, both in the store's decode (no skip logged) and
in the probe (`len 0`). So nothing happens.

**Proposed Off answer:**
- the first argument declared as an array of a struct, else `VmError::Script`;
- an array the store marked skipped (not decoded): `NotYet { LayerAbsent }`, the reader's skipped-array refusal;
- empty: nothing;
- any entry: `NotYet { NotRead }`, "flag initialization: the value resolver's set-up and the runtime random draw are not
  read". The draw needs a hub ruling on which stream stands in for the C runtime's.

### 3.4 `BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects(object)` (static)

1. the world's `WorldInfo`, its `GRI` as a `WillowGameReplicationInfo`. Missing: nothing;
2. **the playthrough:** the GRI's virtual returns `CurrentPlaythrough` **as stored** (0-based). This resolves
   NATIVE_ATTRIBUTES.md's "the exact value tested was not confirmed";
3. **at 1 or below** (normal and the second playthrough), or with a None argument: nothing;
4. **above 1:** a table entry is chosen for the player's level, and its attribute effects are applied to the argument.
   The level number and the effect applier were only partly read.

**Maya:** the slice's playthrough is 0, so nothing happens.

**Proposed Off answer:**
- read-only through the world's game replication info, as `PlayThroughCountAttributeValueResolver` already reads it
  (ow-attr);
- no info, or one not a `WillowGameReplicationInfo`: nothing;
- `CurrentPlaythrough` an int, else `VmError::Script`;
- 1 or below: nothing;
- above 1: `NotYet { NotRead }` naming the table lookup.

## 4. The wiring the port lacks (rules on main)

### 4.1 `Pawn.IsPlayerPawn()`

`render_hooks`' pawn test (NATIVE_BODYCOMP_RENDER_HOOKS.md section 6) is this virtual's rule by first native class:
- `WillowPlayerPawn`: true;
- `Pawn`, `GearboxPawn`, `WillowPawn` and `WillowAIPawn`: a resident `Controller` with `bIsPlayer`;
- any other class: undecided.

**Proposed:** bind the native key `Pawn.IsPlayerPawn()` in Off to that one rule. The undecided case is its
`NotYet { NotRead }`, and a lazy `Controller` is its `NotRead`. No second copy.

### 4.2 The body class's stance objects

`InitializeBehaviorProviders`' preflight meets `RunStance0` as an unloaded handle. In the game, the body class's
subobjects arrive with `GD_Siren_Streaming`.

**Proposed** (the hub's call, like the mesh and anim-set preloads): a labelled harness preload at the class select's
set-up. It loads the body class and its same-package objects; the probe's walk visited 156. It is an UNVERIFIED
stand-in for the streaming load. The natives keep loading nothing.

### 4.3 `Actor.AttachComponent` in the combat world

The combat run already installs an empty scene layer, but does not bind `ow-inventory`'s `Actor.AttachComponent` rule.
**Proposed:** the combat world binds it through `bind_engine_attach`, as the cast's worlds do (Off only,
keep-existing).

### 4.4 The `MaterialInstanceConstant` keys

`MaterialInstanceConstant` declares its own `SetParent` and `SetVectorParameterValue`. They are separate native keys
with **the same exec** as `MaterialInstance`'s (`MaterialInstanceTimeVarying`'s too), by the native table.
**Proposed:** bind `MaterialInstanceConstant.SetParent(object)` and
`MaterialInstanceConstant.SetVectorParameterValue(name,linearcolor)` to the existing rules. The receiver tests stay as
they are; `MaterialInstanceTimeVarying`'s keys stay unbound until something reaches them.

## 5. Predicted stops (from the probe, with each earlier stop stubbed)

- **Link 1:**
  - first lane E's two stops (row 1);
  - then row 2's unbound `Pawn.IsPlayerPawn()`;
  - then rows 4, 7, 8, 9, 10, 12 and 14 in that order, as each is answered;
  - with all answered, link 1 is `ok`.
  
  Caveats:
  - the probe skipped pass 2's conditions, so a condition's or a linked objective's residency (#771, open question 2)
    may stop it at row 5;
  - the probe's `IsPlayerPawn` stub answered true for the player pawn, as the pawn test does.
- **Links 3, 4, 13 and 14:**
  - they then reach **their own first step**: `spawn:baby:6` for link 3, `spawn:adult:7` for the others;
  - they stop at `TargetableAttributeValueResolver`'s Off `get` (NATIVE_ATTRIBUTES.md, "Not read yet"), a `NotRead`
    after the spawn's writes, so prefixed;
  - **link 5** has no steps of its own and stops with link 3.
- Links 6 to 12 are unchanged.

## 6. Tests (synthetic fixtures)

1. `SetDefaultStance` on a toy player pawn, with no current stance, Run selected:
   - the current stance is set, `bNetDirty` set;
   - `bCanCrouch` from the archetype;
   - `OnEnterStance` fired once, and no `OnExitStance`.
2. The pick: injured, then crouched, then sprinting (`bIsSprinting` 1), each selecting its stance;
   `bUseParentStances` with a parent reads the parent's stances.
3. The same stance again: nothing written. An override held, then a Run stance: nothing written.
4. An override stance: it becomes the override, and `bCanCrouch` false.
5. The aim test turning false with a `Controller`: `NotRead`, nothing written.
6. `LeftHandIK` with `StrengthTarget` 0, a stance with `bAllowLeftHandIK`, no holds: `SetSkelControlActive(true)`:
   - `StrengthTarget` 1, `BlendTimeToGo` = (1 − strength) × `BlendInTime`;
   - with `BlendInTime` 0: `ControlStrength` 1;
   - the propagation to `NextControl` only with `bPropagateSetActive`.
7. The shield hook:
   - no `Definition`: nothing;
   - no shield on the provider's pawn: nothing;
   - each of the six mechanism values with C, Min, Max and F around their edges, a NaN among them: `bPrimed`, and the
     event only on a change;
   - `ShieldDef` None: `NotRead` before the stamp.
8. `ApplyFlagInitializationData`:
   - an empty array: nothing;
   - one entry: `NotRead`, nothing written;
   - a skipped array: `LayerAbsent`.
9. `ApplyPlayThroughBasedPlayerAttributeEffects`:
   - `CurrentPlaythrough` 0 and 1: nothing;
   - 2: `NotRead`;
   - no GRI: nothing.
10. `Pawn.IsPlayerPawn()` through the VM on a `WillowPlayerPawn` (true), on a `WillowPawn` with and without a player
    controller, and on another class (`NotRead`).
11. The two `MaterialInstanceConstant` keys through the VM: the same results as the `MaterialInstance` keys.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the controller's rotation set (3.1 step 9);
   - the kernel's answer for an event on an unregistered provider;
   - the run-time name of 3.2's event;
   - the flag value resolver's set-up step (3.3);
   - the level number and effect applier of 3.4;
   - `WillowAIPawn`'s `SetDefaultStance`.
3. **Fitted:** the two stance field places (3.1); the flag entry's field names (3.3).
4. **The hub's calls:**
   - the body class preload (4.2);
   - which stream stands in for the C runtime's `rand()` (3.3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her pawn's current stance (expect `RunStance0`), `bCanCrouch` (true) and `AimOffset` (the
   `AimOffset` node).
2. Crouch, then uncrouch, and read the current stance each time.
3. With a shield of each prime mechanism, read `bPrimed` as the shield drains and refills.
