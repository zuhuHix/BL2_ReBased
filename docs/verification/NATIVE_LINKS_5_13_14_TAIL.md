# Links 5, 13 and 14 past the spawn: the death roll, the go-down and the bleed (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINKS-5-13-14-TAIL (decision 2026-11-26-76), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/l51314/`. This note
is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** Lane D is implementing NATIVE_SPAWN_SETBASEVALUE.md's set-up preload. With it, the spawns of links 3 and 4
complete, and links 5 (`loot_drop_on_kill`, on link 3's kill), 13 (`go_down`, three more swings of link 4's adult) and
14 (`bleed_out_respawn`, on link 13's down) go on. NATIVE_SPAWN_SETBASEVALUE.md section 5 and NATIVE_LINKS_3_4_TAIL.md
read links 3 and 4; NATIVE_LINKS_13_14.md (#796) the class inputs of 13 and 14. This note reads on from there.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `WillowPawn.DropLootOnDeath`, `WillowPawn.PlayInjured`,
  `StatusEffectsComponent.OwnerDied`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - `AmmoDropWeightAttributeValueResolver`'s get (its class table's override of the resolver virtual), the controller
    virtual it calls in the `Controller`, `PlayerController`, `GamePlayerController`, `WillowPlayerController`,
    `AIController`, `GearboxMind` and `WillowMind` tables, and the resource-pool provider's pool getter;
  - `WillowDamagePipeline.AdjustDamage`'s damage-source stage, where it reads `AttackInjuredMeleeDamageModifier`;
  - `StatusEffectsComponent.ClearDenStats` (the exec and its class virtual);
  - `WillowPawn.GetBalancedActorTypeIdentifier` (the exec, and the `IBalancedActor` slot of the `WillowPawn`,
    `WillowAIPawn` and `WillowPlayerPawn` tables);
  - `WillowLeviathanService.RecordCharacterDownEventForPlayer`, outlined.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`; every field named below landed where read
  (`WorldInfo.ControllerList`, `Controller.NextController`, `Controller.ResourcePoolManager`, the
  `IResourcePoolProvider` interface slot of `Controller`, `WillowDamagePipeline.DamageInstigatorWPC` and its
  neighbours, `WillowPlayerController.AttackInjuredMeleeDamageModifier`, `StatusEffectsComponent.ActiveStatusEffects`,
  `WillowPawn.BalanceDefinitionState`, `Object.ObjectArchetype`).
- **A local probe** (temporary code, reverted, never committed): installed Off strict `--slice-gate` runs under the
  machine lock, on main f80b68e7 with lane D's `fix/rust-spawn-setbasevalue` and `fix/rust-floor-trace-rulings`
  merged locally. Each stop already read was stood in by its note's rule, each new one by the rule read here, in
  turn. The substitutions of each run are recorded in the ignored `local/analysis/l51314/SUBSTITUTIONS.txt`.

## 2. The probe's sequence

With the preload in, both spawns complete. Every stop NATIVE_LINKS_3_4_TAIL.md lists was stood in by its rule (the
pipeline getters, `IsPlayingAny`, `IsFriendlyFire`'s object argument, the hit-region cap, the shield and protection
stages, `GetDisplayNameAtGrade`, `NotifyKilledEnemy`, `IsActionSkillRunning`, the force-feedback order, and the dialog
globals' read path). Then **link 4 passes**, and:

**Link 3, and so link 5** (link 5's loot rows come from link 3's kill):

| # | Stop | This note |
|---|---|---|
| 1 | `WillowPawn.DropLootOnDeath` evaluates each item pool's `PoolProbability`; the ammo and health pools' `AmmoDropWeightAttributeValueResolver` asks the world's controller list, which no world answers | **section 3** |
| 2 | unbound `StatusEffectsComponent.ClearDenStats()` (`StatusEffectsComponent.OwnerDied`, in `Died`) | **section 4** |
| 3 | `BodyClassDeathDefinition.OnKilledBy`: "MountDeathNotify raised on a running behavior provider whose run went on after the raise", the port's order tripwire (`NotPorted`) | section 5 |

**Link 13** (three swings; Maya goes down on the third):

| # | Stop | This note |
|---|---|---|
| 1 | the attribute `WillowMind.AttackInjuredMeleeDamageModifier`: "the VM object's class does not declare it" (NATIVE_SPAWN_SETBASEVALUE.md section 5) | **section 6** |
| 2 | `WillowPawn.PlayInjured`: the killer's `QueryInterface(IBalancedActor)`, "the interface table of WillowGame.WillowMind has an entry not resolved at set-up" | section 7 |
| 3 | unbound `WillowPawn.GetBalancedActorTypeIdentifier()` | **section 7** |
| 4 | unbound `WillowLeviathanService.RecordCharacterDownEventForPlayer(...)` | **section 7** |
| 5 | `injured.BeginState`: unbound `SkillEffectManager.NotifySkillEvent` (read: NATIVE_ACTION_SKILL.md) | section 8 |
| 6 | `injured.BeginState`'s `AttributeEffect.ApplyAttributeEffects`: unbound `AttributeDefinitionBase.ResolveContext` (read and implemented: NATIVE_RESOLVE_CONTEXT.md) | section 8 |

**Link 14** needs link 13's down, so it is blocked behind row 6. The probe stopped there.

## 3. The death roll's drop weights: `AmmoDropWeightAttributeValueResolver` (links 3 and 5)

NATIVE_ATTR_RESOLVERS.md section 4 reads the resolver but leaves two steps open: the controller virtual, and the
provider's method. Both are read now.
- **The controller list:** `WorldInfo.ControllerList`, then each controller's `NextController`, in order. The port
  already walks it (`ow_world` `world/all_controllers.rs`).
- **The controller virtual:** it answers **None** on `Controller`, `AIController`, `GearboxMind` and `WillowMind` (a
  shared stub), and **the controller itself** on `PlayerController` and its subclasses (`GamePlayerController`,
  `WillowPlayerController`; a shared "return yourself" body). So **only player controllers count**; every AI mind is
  skipped.
- **The provider:** the controller's own `IResourcePoolProvider` (one body in the three controller tables): with no
  `ResourcePoolManager`, an invalid reference; otherwise the manager's pool for `Resource`, resolved as the pools rule
  resolves it (slot below 16, the slot pool's `PoolGUID` matching). A failed manager lookup falls back to a further
  provider (by fit, the vehicle's); the port's `pools::controller_pool` already reads this with
  `VEHICLE_POOL_LOOKUP`'s `NotRead`.

**Proposed Off answer:** answer the two engine queries in the combat world, load-free and write-free:
- the world-controller query (its argument the list index): the controller at that place in the chain, `Ok(None)`
  past its end; an unloaded link a typed `NotRead` (as `all_controllers.rs` does);
- the controller-pool query (its argument the resource definition, asked of one controller): `Ok(None)` unless the
  controller is a `PlayerController` (or a subclass); then **the read-only `pools::existing_pool` path** on the
  controller's `ResourcePoolManager`, not `pools::controller_pool` (review r1 of #821: that one writes
  `ResourcePoolManager` on its existing-manager path even with creation off). No manager, or an invalid reference:
  `Ok(None)`. A missing or mistyped declaration (`ResourcePoolManager`, `ResourcePools`, `PoolGUID`, the pool's
  `Definition` and `Resource`) is `VmError::Script`; an unloaded manager, slot pool or definition, and the unread
  vehicle provider fallback, are typed `NotRead`s.

On the slice the only player controller is Maya's. Her pools decide the weights; the probe stood the whole resolver
in with `NoPoolWeight`, so the death roll it ran is not the rule's. **A later probe** (on main d90df41e with lane D's
#815 and links 3-4 rows merged locally, the two queries answered by this rule) found the combat world's
`WorldInfo.ControllerList` **empty**: the walk ended at index 0, so the rule also gives `NoPoolWeight` there, and link 3
went on to section 4. Why the port's combat world links no controller into that list is not read (open question 5).

## 4. `StatusEffectsComponent.ClearDenStats()` (links 3 and 5)

The exec calls one class virtual. Its body, **by fit** (the per-type meaning is inferred from the field and type
names): for each of the seven status-effect types, when any entry of `ActiveStatusEffects` holds a definition of that
type, a further virtual runs with that type (not read). **With `ActiveStatusEffects` empty it does nothing.**

**Proposed Off answer:** load-free; `ActiveStatusEffects` declared as an array, else `VmError::Script`; empty: nothing;
not empty: `NotRead`, "ClearDenStats: the per-type clear". On the slice the bullet kill leaves the list empty.

## 5. The `MountDeathNotify` order tripwire (links 3 and 5)

`BodyClassDeathDefinition.OnKilledBy` runs the dead pawn's behaviour provider, which raises `MountDeathNotify` while
the provider is still running. The C++ reference runs the raised event inside the raise; the port defers it past the
rest of the run, and in Off refuses when the run went on after the raise (`ow-combat` `run.rs`, `order_tripwire`,
decision 2026-11-16-54). This is the port's design gap, not an unread native. **For the hub:** a lane to run a raised
event at the raise (re-entrantly), as the C++ does.

## 6. `AttackInjuredMeleeDamageModifier` (link 13)

In `AdjustDamage`'s damage-source stage (NATIVE_DAMAGE_DEATH.md section 1, stage 2) the three instigator factors
(`TargetMinHealthDamageBoostPercent` and `TargetOverMinHealthGunDamageMultiplier` for bullets,
`AttackInjuredMeleeDamageModifier` for melee) are read from the pipeline's **`DamageInstigatorWPC`**: the instigator as
a `WillowPlayerController`, None otherwise.
- **melee:** `AttackInjuredMeleeDamageModifier` of `DamageInstigatorWPC` when it is set **and** the damaged pawn's
  health is below 30% of its maximum; otherwise 1;
- **bullets:** with no `DamageInstigatorWPC` the gun factor is skipped (the stage's bullet branch needs it).

So **an AI's swing never reads either**, and the port's read of the attribute on the AI's `WillowMind`
(`ow-combat` `damage.rs`, stage 3 of its numbering) is the error behind the refusal.

**Proposed Off answer:** take the three factors only when the instigator controller is a `WillowPlayerController`
(the `DamageInstigatorWPC` rule); otherwise 1, reading nothing. On link 13 the factor is 1 and Maya goes down on the
third swing (the probe). **Its contract** (review r1 of #821):
- the class check is load-free: the controller's resident class against `WillowGame.WillowPlayerController` through
  the class table's lookup only (`by_path`, `is_child_of`), never a `find_class` that may load;
- the reads are of resident objects only: an unloaded instigator controller or victim pawn is a typed `NotRead`,
  never loaded;
- the fields are typed: `AttackInjuredMeleeDamageModifier`, `TargetMinHealthDamageBoostPercent` and
  `TargetOverMinHealthGunDamageMultiplier` as float attributes on the controller, and the victim's health values as
  floats; a missing or mistyped declaration is `VmError::Script`, not the attribute helper's `NotRead`.

## 7. `PlayInjured`'s telemetry call (link 13)

`WillowPawn.PlayInjured`, for a player pawn with a killer:
1. `QueryInterface(IBalancedActor)` on the killer (the AI's mind). The port refuses: `WillowMind`'s interface table has
   an entry not resolved at set-up (`ow-vm` `interface_read_only.rs`). **Off:** the combat world's set-up resolves the
   AI mind class chain's interface tables (`Vm::resolve_interface_tables`, a logged load), as the combat owner set-up
   already does for a pawn's chain (`ow-cli` `cmd/combat/owner_setup.rs`). The
   mind's `QueryInterface` rule (NATIVE_GEARBOXMIND_QUERYINTERFACE.md) then answers the AI pawn;
2. **`GetBalancedActorTypeIdentifier()`** on the answer, read: the `IBalancedActor` slot (one body on `WillowPawn`
   and `WillowAIPawn`) gives the **name of `BalanceDefinitionState.BalanceDefinition`**: None for none, and the text
   "<uninitialized>" for an object the engine has not registered. `WillowPlayerPawn`'s own body gives its
   `ObjectArchetype`'s name the same way. **Off:** that read, load-free. Declared with their kinds, else
   `VmError::Script` (review r1 of #821): for the balance branch, `BalanceDefinitionState` as a struct whose first
   field,
   `BalanceDefinition`, is an object; for the player branch, `ObjectArchetype` as an object. An unloaded definition or
   archetype whose name decides the answer is a typed `NotRead`;
3. **`WillowLeviathanService.RecordCharacterDownEventForPlayer(controller, level, identifier, location)`** (static),
   outlined: it builds a telemetry record and hands it to the online telemetry service. By fit, nothing the slice
   reads changes. **For the hub:** an Off-only no-op stand-in, logged as a harness input (no telemetry service in the
   port); otherwise `NotRead`.

## 8. The `injured` state's set-up (link 13)

`WillowPawn.injured.BeginState`:
- **`SkillEffectManager.NotifySkillEvent`** (event 37 in the probe), read in NATIVE_ACTION_SKILL.md section 8: it
  clears a "dirty" bit, fans the event to the controller's active skills, then runs a trailing bookkeeping call. An
  empty skill list is therefore **not shown to be a no-op** (review r1 of #821). **Off:** the supported path is a
  manager with no active skill whose instigator is the controller **and** no work the trailing call would do; its
  preflight, write-free and load-free, checks the active-skill list (declared, else `VmError::Script`) and decides
  before the dirty-bit write: any matching skill (the event-37 handler is not read) or the trailing bookkeeping (not
  read) is a typed `NotRead`, "NotifySkillEvent: the skill handlers and the trailing bookkeeping". So until the
  trailing call is read, the stop is that `NotRead`, decided first;
- **`AttributeEffect.ApplyAttributeEffects`** then reaches `AttributeDefinitionBase.ResolveContext`, read and
  implemented (NATIVE_RESOLVE_CONTEXT.md) but not bound in the combat world. **Off:** bind it there. The binding adds
  the partial-writes prefix only when one of #518's write marks really changed between entry and the refusal, and
  keeps the refusal's kind (review r1 of #821);
- **then `AddAttributeModifier`:** when `ResolveContext` returns a context, `ApplyAttributeEffects` calls
  `AttributeDefinitionBase.AddAttributeModifier`, read (NATIVE_ADD_ATTRIBUTE_MODIFIER.md) but unbound in the combat
  world. The slice's injured effects were not shown to resolve no context, so that call is the predicted stop.

Past these the probe did not go. **Link 14's bleed is not reached**; its natives are NATIVE_GO_DOWN_PATH.md section
4's.

## 9. Predicted stops

- **With the preload alone** (lane D; #815 measured it): links 3, 4 and 13 at the unbound
  `DamagePipeline.GetBarrelSourceTime()`, read in NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 4.2 (a plain
  read of the pipeline's `BarrelSourceTime`) with the other four plain reads there; links 5 and 14 behind them.
- **With NATIVE_LINKS_3_4_TAIL.md's rows in:** link 4 passes; link 3 (and 5) at section 3's controller list; link 13
  at section 6's attribute.
- **With this note's sections 3, 4, 6, 7 and 8 in:** link 3 and link 5 at section 5's tripwire; link 13 at
  `NotifySkillEvent`'s typed `NotRead` (its trailing bookkeeping is not read), and with that read, at the unbound
  `AttributeDefinitionBase.AddAttributeModifier` in `ApplyAttributeEffects` (review r1 of #821); link 14 behind it.

## 10. Tests (synthetic fixtures)

1. **The drop weight:** a toy world with an AI mind and a player controller in the list: the mind is skipped; the
   player controller with no manager is skipped; with a pool of the resource, it counts; an unloaded list link:
   `NotRead`; the query writes nothing (the controller's `ResourcePoolManager` unchanged) and a mistyped
   `ResourcePools`: `VmError::Script` (review r1 of #821); an empty `ControllerList`: `NoPoolWeight`.
2. **`ClearDenStats`:** an empty `ActiveStatusEffects`: nothing; one entry: `NotRead`; a mistyped field:
   `VmError::Script`.
3. **The injured melee factor:** an AI instigator against a victim at 10% health: factor 1, nothing read; a player
   instigator: its `AttackInjuredMeleeDamageModifier`; at 40% health: 1. **The gun factors:** an AI instigator: none.
   (review r1 of #821) An unloaded player controller: `NotRead`, nothing loaded (the class-code load counter
   unchanged); a mistyped `AttackInjuredMeleeDamageModifier`: `VmError::Script`.
4. **`GetBalancedActorTypeIdentifier`:** a pawn with a balance definition: its name; none: None; a player pawn: its
   archetype's name; a mistyped `ObjectArchetype` or `BalanceDefinitionState`: `VmError::Script`.
4a. (review r1 of #821) **`NotifySkillEvent`:** any call: the typed `NotRead`, the dirty bit unchanged.
   **`ResolveContext`'s binding:** a refusal with no write mark changed carries no prefix, and its kind is kept;
   after a write it carries the prefix once.
5. **The interface tables:** after the set-up resolution, the mind's `QueryInterface(IBalancedActor)` answers the
   pawn, with nothing loaded at the call.
6. **The telemetry call:** with the stand-in: nothing written, one log line; without: `NotRead`.
7. Each refusal: nothing written by its own native or stage; Reference unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **For the hub:** the `MountDeathNotify` order (section 5); the telemetry stand-in (section 7).
3. **Not read:** `ClearDenStats`' per-type clear; the telemetry service; the vehicle pool fallback.
4. **By fit:** the provider fallback's identity; "no game state changes" for the telemetry call.
5. **Not read:** why the port's combat world links no controller into `WorldInfo.ControllerList` (the later probe,
   section 3); `NotifySkillEvent`'s trailing bookkeeping and its event-37 handler (section 8).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Let an AI melee Maya below 30% health and read the hit's damage against one above 30%: equal (no injured factor for
   an AI).
2. Kill a primal beast and read the ammo drop weights with Maya's ammo full and empty.
3. Read `GetBalancedActorTypeIdentifier` on a spawned enemy: its balance definition's name.
