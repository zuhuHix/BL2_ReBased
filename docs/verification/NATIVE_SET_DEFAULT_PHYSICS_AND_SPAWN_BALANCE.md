# SetDefaultPhysics, the spawn's balance state, and links 3 and 4 after the spawn (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SETDEFAULTPHYSICS-AND-SPAWN-BALANCE (decision 2026-11-26-46), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/sdp/` and, for review r1 of #788, `local/analysis/l34/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_TARGETABLE_RESOLVER.md (#783) predicts that, once the resolver is bound, links 3 and 4's spawn stops at
`WillowPawn.SetDefaultPhysics` past its gates. That is NATIVE_MIND_ROUTE.md section 1's open part: two unnamed pawn
queries and an unnamed navigation-handle query. #783 also found that the port's spawn never fills the pawn's
`BalanceDefinitionState`, which the game's spawn does.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor`;
  - `Pawn.InitializeBalanceDefinitionState` and `ApplyBalanceDefinitionCustomizations`;
  - `WillowPawn.TakeDamage`, and the callers below.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs and the class-table
  slots named below, in `WillowAIPawn`'s, `WillowMind`'s, `WillowNavigationHandle`'s and `DamagePipeline`'s primary
  class tables (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method).
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field below lands exactly where the routine
  reads it, except in `WillowAIPawn`'s own fields. There the routines read `AIClass` and `MyWillowMind` one field after
  the tool's places, the layout gap NATIVE_MIND_ROUTE.md already records; they are named by use.
- **A local probe** (temporary code, reverted, never committed). It ran an installed Off `--combat-run` on main 785c2a4,
  under the machine lock, with a call trace. The steps were links 3 and 4's own, after a `player:8` step in place of
  `maya:8` (link 1 stops earlier): `spawn:baby:6`, `hit:0:9999::Bullet`, `spawn:adult:7`, `swing:1:melee`.

  The probe stubbed, one at a time:
  - the targetable resolver (0.0, #783);
  - `SetDefaultPhysics` past its gates (nothing): **so its `SetPhysics` call never ran**;
  - in `RecalculateAttributeInitializedState`, **the mind part's refusal only**. The rest of the port's native ran as
    the port has it: the controller part's class-level check (`refuse_class_parts`), the skill refresh and the pools'
    rebase. The mind part itself then wrote nothing;
  - each later stop below.

  The stubs are not answers and wrote nothing, except where a stub made the field read the note describes.
- **A second probe for review r1 of #788**, on main d3be3e5, the same steps and lock. It stubbed the targetable
  resolver's step-0 refusal (0.0; #787's walk load is not implemented yet) and printed, read-only, what
  `SetDefaultPhysics` and `RecalculateAttributeInitializedState` see when called. Its stops match the first probe's.
- **`ow-package --def-dump`** of the two archetypes, their AI classes and the 35 `AIPawnBalanceDefinition` exports of
  the archetypes' package (section 3).

## 2. `WillowPawn.SetDefaultPhysics()`: the three queries named

NATIVE_MIND_ROUTE.md section 1's order stands. Its open items are now read:

1. **The gates** (unchanged): `bDeleteMe` or `bIsDead`, the archetype not a `Pawn`, no `SMComponent`, or a current
   special move that is a `SpecialMove_Spawned`: nothing. The dead test is the pawn's own virtual. For `WillowAIPawn` it
   is `bDeleteMe` or `bIsDead`, exactly.
2. **The default physics** (`WillowAIPawn`'s rule, as section 1 says), with its last query named. **The navigation
   handle is "flying"** when its `FlightMode` is neither 0 nor 4 (two values of `EFlightMode`). With both a mind and a
   `WillowNav`:
   - not flying, and the class's `Physics` not `PHYS_Flying`: `PHYS_NavMeshWalking`;
   - else `PHYS_Flying`.
   
   The other cases stand: no `AIClass` gives `PHYS_Falling`; `bIsTurret` gives `PHYS_None`; no mind or no `WillowNav`
   gives the class's `Physics`.
3. **The physics set** through the pawn's physics setter (`Actor.SetPhysics`, NATIVE_ACTOR_SET_PHYSICS.md), with that
   mode.
4. **`bCollideWorld`** copied from the archetype.
5. **The collision copy is skipped only when both queries hold:**
   - **query 1: the pawn's `bCloaked`** (a byte on `WillowPawn`, true when not 0). It is asked first, so an uncloaked
     pawn never asks query 2;
   - **query 2: "the cloak's shield requirement is met":**
     - false when `WillowPawn`'s static can-cloak flag is clear. That flag is the process-wide value behind
       `WillowPawn.StaticCanCloak()` and `StaticSetCanCloak(bool)`; its initial value is **true**;
     - false with no `BodyClass`;
     - otherwise the cloak's body class: the `BodyClass`, then its `AnimationParentBody` for as long as the class has
       `bCloakUseParentData` and a parent;
     - when that class has `bCloakingRequiresShields`, the answer is false when the pawn's maximum shield is above 0
       and its current shield is below 0.0001;
     - otherwise true.
     
     The shield values come through the pawn's damageable interface. The current shield is the `ShieldArmor` pool's
     `CurrentValue` when the reference names a live pool, else `ShieldVar`. The maximum is the pool's maximum, else
     `ShieldMaxVar`.
6. **Otherwise the collision is set** from the archetype's `bCollideActors`, `bBlockActors` and `bIgnoreEncroachers`.
   This is the native `SetCollision`, the same body as `Actor.SetCollision`.

**On the slice** (the second probe and `--def-dump`; review r1 of #788):
- **the mode each spawn asks for.** Both AI classes (`CharClass_PrimalBeastBaby` and `CharClass_PrimalBeastAdult`)
  have `bIsTurret` false and `Physics` 0 (`PHYS_None`) as the store reads them. When `SetDefaultPhysics` runs, inside
  the mind's `ApplyCharacterClassDefaults`, the pawn has its `AIClass` and its `MyWillowMind`, but the mind's
  `WillowNav` is None. So both spawns ask for **the class's `Physics`, `PHYS_None`**. The pawn's `Physics` is then
  `PHYS_Walking` (1);
- **so step 3 refuses.** A change from `PHYS_Walking` into `PHYS_None` meets `Actor.SetPhysics`' transition refusal
  "the base search for a walking-type mode" (NATIVE_ACTOR_SET_PHYSICS.md, preflight step 3, `NotRead`). It is decided
  before `SetDefaultPhysics` writes anything;
- **had the mind a `WillowNav`** (the game's order of the nav handle's creation was not observed), the mode would be
  `PHYS_NavMeshWalking`, which `SetPhysics` always refuses on a `WillowAIPawn` ("a WillowAIPawn's nav-mesh walking
  check", preflight step 1). So either way the spawn stops in `SetPhysics`;
- **the collision copy** (reached only past `SetPhysics`): the enemies are uncloaked, so it runs. The port makes the
  pawn as a copy of its archetype, so the three flags already equal the archetype's. `SetCollision` with no change does
  nothing (NATIVE_GO_DOWN_PATH.md 4.1).

**Proposed Off answer** (replacing the past-the-gates `NotRead` in `ow-combat` `hit/pools.rs`):
- **preflight** (write-free and load-free; each field declared with its kind, else `VmError::Script`):
  - the gates as today;
  - the archetype: the object the spawn copied. **This is new port state** (review r1 of #788): the port does not
    model `ObjectArchetype` (`hit/pools.rs`, the gate's comment). The implementer adds it at `hit::adopt`: a per-pawn
    record of the archetype object the spawn copied, kept for the pawn's life and read here only. Without one it is
    `NotRead` ("SetDefaultPhysics: the pawn's archetype");
  - the default physics by the rule above, reading `AIClass`, `bIsTurret`, the class's `Physics`, `MyWillowMind`,
    `WillowNav` and `FlightMode`. Each object read must be resident, else `NotRead`;
  - `Actor.SetPhysics`' own preflight for that mode (review r1 of #788). On a `WillowAIPawn` it refuses
    `PHYS_NavMeshWalking` always, even when the pawn is already in it, and a change into `PHYS_None`, `PHYS_Walking`,
    `PHYS_Rotating` or `PHYS_Spider` with the base-search `NotRead`. The slice's spawns meet the second (above);
  - `bCloaked`; and only when it is set, query 2's reads (the can-cloak flag, the body-class walk, the shield fields
    and pool). `ShieldArmor` is read through the pool handle rule the port already has (NATIVE_COMBAT_GROUNDWORK.md);
  - the collision copy: when the three flags already equal the archetype's, nothing. Otherwise `Actor.SetCollision`'s
    `NotRead` (NATIVE_GO_DOWN_PATH.md 4.1), decided here;
- **commit:**
  - the physics set (its own rule; a change of mode is its writes);
  - then `bCollideWorld`;
  - a refusal from `SetPhysics`' commit carries the partial-writes prefix only when it comes after `SetPhysics`' own
    mode write (review r1 of #788). One decided before that write carries none, since `SetDefaultPhysics` writes
    nothing before the physics set.
- **the can-cloak flag:** a VM-wide value, initially true. `StaticCanCloak` and `StaticSetCanCloak` are bound in Off to
  read and write it. Nothing on the slice calls them.

## 3. `BalanceDefinitionState`: where the game's spawn fills it

**The game:** the population factory's script `PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor` does the
spawned pawn's balance set-up, in this order:
1. `SetGameStage`, then `SetAwesomeLevel`;
2. the exp level, from the balanced instigator or the factory's level data (at least 1), then `SetExpLevel`;
3. `SetGameStageForSpawnedInventory`, from the balance's item game-stage rule;
4. the mind's `InitializeCharacterClass`;
5. the factory's `ApplyAttributeStartingValues(pawn)`;
6. **`pawn.InitializeBalanceDefinitionState(PawnBalanceDefinition, -1)`**. Its script sets
   `BalanceDefinitionState.BalanceDefinition` := the factory's `PawnBalanceDefinition`, `GradeIndex` := -1 and
   `bGradeCustomizationsApplied` := false, then runs `ApplyBalanceDefinitionCustomizations`. That marks the
   customizations applied once the balance's own customization step has run, when a balance is set and they were not
   applied yet;
7. for a pawn hostile to the player: `BalanceModifierDefinition.UpdateSpawnedPlayerEnemyAIPawn(pawn, mind)`;
8. the mind's `RecalculateAttributeInitializedState`;
9. the balance's `SetupPawnItemPoolList`, the inventory (saved or default), `SetFlagDefinitions`, the spawn point, and
   the clan.

So `BalanceDefinitionState` is filled after the mind's class step and the attribute starting values, and before the
mind's recalculation.

**The port:** `ow-combat`'s `hit::adopt` starts from an archetype alone (the slice's `spawn:<baby|adult>`). It writes
`ExpLevel` and `GameStage` directly, makes the mind and runs `InitializeCharacterClass`. It has no population factory,
so no balance definition exists to fill the state with.

**Proposed** (the hub's ruling on #788: yes, as a logged, UNVERIFIED harness input):
- **Off only; Reference unchanged;**
- **the input:** the archetype's pawn balance, an `AIPawnBalanceDefinition`. In the game the population factory names
  it, and the balance points at the archetype (`AIPawnArchetype`), not the reverse. So the harness finds it by that
  pointer: among the `AIPawnBalanceDefinition` exports of the archetype's own package, the one whose `AIPawnArchetype`
  is the spawned archetype. Exactly one must match; none, or more than one, is `NotRead` naming the archetype. On the
  slice (`--def-dump` of all 35 balances in that package, each read):
  - `spawn:baby` (`GD_PrimalBeastBaby.Character.Pawn_PrimalBeastBaby`):
    `GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast_Monglet`;
  - `spawn:adult` (`GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult`):
    `GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeastAdult`.

  Which balance the game's own population picks for these spawns was not observed: UNVERIFIED;
- **logged on every spawn that uses it**, as a harness-input note naming the archetype and the balance;
- **placed** after the port's attribute starting-values step (`ApplyAttributeStartingValues`, which the probe's trace
  shows running before the recalculation) and before the mind's `RecalculateAttributeInitializedState`, as steps 5, 6
  and 8 order them. There the spawn runs the installed script `InitializeBalanceDefinitionState(balance, -1)` on the
  pawn;
- **refusals, decided before the step's first write:** a missing balance (no match) or one the store has not loaded
  is `NotRead`, with nothing written by this step;
- the balance's customization step is then the next read, or its own `NotRead`;
- steps 7 to 9 of the factory script stay outside the port's spawn, as today.

## 4. Links 3 and 4 after the spawn (from the probe)

### 4.1 `WillowMind.RecalculateAttributeInitializedState()`: the key, and the mind's own part

Both spawns stop first at the **unbound** `WillowMind.RecalculateAttributeInitializedState()`. `ow-world`'s level-up
pack binds the `Controller` and `WillowPlayerController` keys only, though the three share one exec and the rule
already has the mind's branch (`levelup.rs`). **Proposed:** bind the `WillowMind` key to the same rule.

That branch's own part is a `NotRead` ("the context they are evaluated with is not named"). It is now read.

**The order inside the mind's override** (review r1 of #788; read again for it): the mind's class-table routine does
1. the skill refresh (NATIVE_MIND_ROUTE.md section 3 step 1);
2. then a direct call of the `Controller` routine, the shared controller part: the class-level part when a
   `CharacterClass` is set and the pawn is not a vehicle, then the pools' rebase. It is the same routine
   NATIVE_LEVEL_UP_ATTRIBUTES.md section 1 reads, called from here and from the player controller's override;
3. then the mind's own part, below.

That is NATIVE_MIND_ROUTE.md section 3's order, which stands. **This corrects this note's first version**, which put
the mind's part before the controller part. **The port's order differs** (`ow-world` `levelup.rs`, `mind_override`):
the mind part's refusal, then the controller part's class-level refusal, then the skill refresh, then the shared
controller part. Refusals may stay decided up front, but the writes must follow the game's order: the implementer
fixes it.

**The mind's own part:**
- **with an `AIClass` and a `MyWillowPawn`:**
  - **`CommerceMarkup`** on the pawn := `AIClass.BaseCommerceMarkup` evaluated with **the mind itself** as the context
    source and no override;
  - **with the pawn's `StatusEffectComp`:** ten resistances, in this order: `IgniteChance`, `ShockChance`,
    `CorrosiveChance`, `SlowChance` and `AmpChance`, then the same five `...Duration`. For each:
    - the component's `<name>ResistanceModifierBaseValue` := `AIClass.Base<name>ResistanceModifier` evaluated with the
      mind as context;
    - its `<name>ResistanceModifier` := that base with the component's `<name>ResistanceModifierModifierStack` applied.
      This is now read (review r1 of #788): the same shared stack routine the controller part and the player's
      resistance step call, the attribute rule of a value from its base and its modifier stack. In the port that is
      `ow-attr`'s stack fold (`storage.rs`, `stack_value`). Its rounding form is not settled here: the game computes the
      sum and factor wider than a float and stores one float at the end, closest to the port's one-rounding form
      (UNVERIFIED).

**The class-level part on the slice** (review r1 of #788): in the second probe the mind's `CharacterClass` is None when
the recalculation runs, so `refuse_class_parts`' class-level `NotRead` is not met. No step read here sets it: the
mind's `InitializeCharacterClass` script sets `bCharacterClassInitialized` and passes `AIClass` to the starting values
and the class defaults, and the starting-values base makes pools only (NATIVE_CLASS_STARTING_VALUES.md section 1).
Whether any other step of the game sets a mind's `CharacterClass` was not searched: UNVERIFIED.

**Proposed Off answer:** that rule, with the existing evaluator (ow-attr) on the mind as context, after the controller
part:
- **preflight, write-free and load-free** (review r1 of #788):
  - each of the eleven init-data attribute definitions (`BaseCommerceMarkup` and the ten `Base<name>` entries) and the
    resolvers of its chains must already be resident, else `NotRead` naming it. A load counts as a write, so this is
    decided before the first write;
  - each field declared with its kind, else `VmError::Script`: the pawn's `CommerceMarkup` and `StatusEffectComp`, and
    the component's twenty resistance fields (each `...BaseValue` and value) with their modifier stacks;
- **writes** in the order above;
- **prefixes:** an evaluator refusal before the native's first write carries no prefix; one after it does. Since the
  skill refresh and the controller part come first, the mind part's own refusals follow their writes when those wrote.

With the mind part's refusal stubbed, both spawns complete in the probe: the health pools, the loot set-up, and the
rows.

### 4.2 The hit and the swing

These are among the getters NATIVE_GO_DOWN_PATH.md section 5 lists as not read (link 4's swing path, with
`GameInfo.IsFriendlyFire`), and among the stubs COMBAT_DEATH.md's "Still stubs on the chain" names ("the pipeline
getters"). **Now read here:** `GetBarrelSourceTime`, `GetPlantSourceTime`, `GetCanInflictFriendlyFire`,
`IsUsingWalkingPhysics` and `IsPlayingAny`. Still not read: the rest of that list, `GameInfo.IsFriendlyFire` included.

Past the spawns, `WillowPawn.TakeDamage` (both links) and the swing path meet, in order:
- **`DamagePipeline.GetBarrelSourceTime()` and `GetPlantSourceTime()`:** the pipeline's own `BarrelSourceTime` and
  `PlantSourceTime` floats. These are the class's virtual, the same body for `DamagePipeline` and
  `WillowDamagePipeline`. **Off:** that read;
- **link 3, `Pawn.IsUsingWalkingPhysics()`:** true when `Physics` is `PHYS_Walking` or `PHYS_NavMeshWalking`. It is a
  direct read, not a virtual. **Off:** that read;
- **link 4, `SpecialMoveComponent.IsPlayingAny()`:** true when the component's `CurrentSMData.SMD` is set (the field
  `SetDefaultPhysics`' gate reads). **Off:** that read;
- **link 3, `DamagePipeline.GetCanInflictFriendlyFire()`:** the pipeline's `bCanHitFriendly`, a direct read. **Off:**
  that read.

Each read is declared with its kind, else `VmError::Script`, and writes nothing.

## 5. Predicted stops (from the probe, each earlier stop stubbed)

- **Links 3 and 4's spawns** (`spawn:baby:6`, `spawn:adult:7`), once the targetable resolver (#783, #787) is
  answered:
  1. **first, `SetDefaultPhysics`' `SetPhysics` refusal** (review r1 of #788): both spawns ask for `PHYS_None` from
     `PHYS_Walking`, "the base search for a walking-type mode" (section 2). It comes after the spawn's earlier writes
     (the health pool), so the step reads "stopped after partial writes";
  2. then, with that answered, the unbound `WillowMind.RecalculateAttributeInitializedState()` (4.1). Binding it moves
     the stop to its own part's `NotRead`, until 4.1's rule is in. The controller part's class-level `NotRead` is not
     met: the mind's `CharacterClass` is None (4.1).
- **Link 3's `hit`**, past 4.1 and 4.2: **the unbound `GameInfo.IsFriendlyFire(object, interface)`**. It is not read
  in any record (NATIVE_DAMAGE_DEATH.md lists it as not read; COMBAT_DEATH.md calls it a stub with a game-side
  script). **No script body overrides it:** `research/script_disasm.py` finds none in `Engine`, `GameFramework`,
  `GearboxFramework` or `WillowGame`. The native key is declared on `GameInfo` and again on `WillowCoopGameInfo`,
  both on the same exec; `WillowGameInfo` declares none (review r1 of #788).
- **Link 4's `swing`**, past 4.2: **`Actor.SetPhysics` on a `WillowPlayerPawn`**, the existing `NotRead` ("the physics
  change of WillowGame.WillowPlayerPawn", NATIVE_ACTOR_SET_PHYSICS.md). In the probe the player was `player:8`'s pawn;
  in the gate it is Maya's.
- **Link 5** stops with link 3. **Links 13 and 14** spawn an adult and swing it, so link 4's stop is expected for them
  (not probed).

In the slice gate these links start where link 1 ends (NATIVE_MAYA9_AND_AFTER.md section 5).

## 6. Tests (synthetic fixtures)

1. `SetDefaultPhysics` on a toy `WillowAIPawn`:
   - no `AIClass`: `PHYS_Falling`;
   - `bIsTurret`: `PHYS_None`;
   - no mind: the class's `Physics`;
   - a `WillowNav` with `FlightMode` 0 and a walking class: `PHYS_NavMeshWalking`;
   - `FlightMode` 2: `PHYS_Flying`;
   - the class's `Physics` `PHYS_Flying` with `FlightMode` 0: `PHYS_Flying`;
   - (review r1 of #788) the slice's case: a mind with no `WillowNav`, the class's `Physics` `PHYS_None`, the pawn in
     `PHYS_Walking`: `SetPhysics`' base-search `NotRead`, nothing written by `SetDefaultPhysics`;
   - a `WillowNav` with `FlightMode` 0 and the pawn already in `PHYS_NavMeshWalking`: `SetPhysics`' nav-mesh
     `NotRead`, nothing written.
2. `bCollideWorld` copied from the archetype in each case.
3. The collision copy:
   - uncloaked, flags equal to the archetype's: nothing, and query 2 not read;
   - uncloaked, a flag differs: `SetCollision`'s `NotRead`, nothing written;
   - cloaked with the can-cloak flag clear: copied (the same two outcomes);
   - cloaked, can-cloak set, `bCloakingRequiresShields` with max shield 50 and current 0: copied;
   - the same with current 10: skipped;
   - `bCloakUseParentData` walking to a parent body class.
4. No archetype recorded: `NotRead`, nothing written.
5. `InitializeBalanceDefinitionState` run by the spawn with a toy balance: `BalanceDefinition` set, `GradeIndex` -1,
   `bGradeCustomizationsApplied` as the customization step leaves it. Also (review r1 of #788):
   - the harness pick: one toy balance pointing at the archetype is picked and logged; none, or two, is `NotRead`;
   - a missing or unloaded balance: `NotRead`, nothing written by the step;
   - Reference: unchanged.
6. 4.1: a toy mind with an `AIClass` and a pawn with a `StatusEffectComp`:
   - the order: the skill refresh, the controller part's pools' rebase, then the mind part (a toy skill and a toy pool
     show the order of writes);
   - an unloaded init-data definition, or an unloaded resolver of its chains: `NotRead` before any write of the mind
     part;
   - a mistyped resistance field: `VmError::Script`;
   - `CommerceMarkup` and the ten base values from constant init data;
   - each value with a one-entry modifier stack;
   - the context passed is the mind;
   - no `StatusEffectComp`: only `CommerceMarkup`.
7. 4.2: each read on a toy pipeline, pawn and component; a mistyped field: `VmError::Script`.
8. The `WillowMind` key through the VM.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - `WillowAIPawn`'s physics-setter step when changing into `PHYS_NavMeshWalking` (the navigation re-anchor);
   - the balance's customization step (`ApplyBalanceDefinitionCustomizations`' call on the balance);
   - `GameInfo.IsFriendlyFire`;
   - `Actor.SetPhysics` for `WillowPlayerPawn`;
   - when the game creates a mind's `WillowNav`, relative to `SetDefaultPhysics` (section 2);
   - whether anything sets a mind's `CharacterClass` (4.1).
3. **Fitted:** `AIClass` and `MyWillowMind` places on `WillowAIPawn` (the layout gap); the stack fold's rounding form
   (4.1).
4. **Ruled by the hub on #788:** the spawn's balance input (section 3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After an enemy spawns, read its `Physics`, `BalanceDefinitionState` (expect its population balance and
   `GradeIndex` -1) and `CommerceMarkup`.
2. Read its `StatusEffectComp`'s resistance values and bases.
3. On a cloaking enemy with shields, read `bCollideActors` while cloaked with shields up and down.
