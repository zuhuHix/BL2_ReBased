## 2026-11-26: Link 6's take runs in Off to Actor.SetBase

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-UPDATEAMMOCOUNTS, on main 0195ce94 (F's fixed #785). It implements
NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md sections 3.1 and 3.3 to 3.8, its gaps 4.1 and 4.2, and
NATIVE_PICKUP_SAVE_ROTATION.md section 5, with the hub's corrections on #785 and its two rulings. This lane did not
read the executable. No session was run. **Every rule is UNVERIFIED in the running game.** Reference binds nothing and
is unchanged.

- **What was decided (Off only):**
  - `ow-inventory` `pickup_take` binds five keys:
    - `UpdateAmmoCounts`:
      - the preflight checks the eight definitions' declarations and residency, and the pool lookup's own refusal
        (a vehicle pawn), against the state before the call;
      - the commit runs the lookup with creation, raises `ScriptAnnounceAmmoGain(kind, gain)` on a non-silent rise,
        then writes the cache;
      - a refusal after a write carries the prefix;
    - `GetSaveRotation`;
    - `IsDLCRequirementMet`, which requires the declarations of `DefinitionData`, `BalanceDefinition` and
      `DlcItemSet`;
    - `GetResourcePoolContext`;
    - `EvaluateExpressions`, which is `ow-attr`'s rule.

    `pickup_cost` drops its `UpdateAmmoCounts` `NotRead`.
  - `ow-world` `actor_take` binds four keys:
    - `ComponentList`;
    - `MakeNoise`, which does nothing;
    - `IsUsingVehicle`, taken over from `ow-combat` `go_down`'s `NotRead` list (checked by grep, no other binding);
    - `SetOwner`: `actors::set_owner`'s rule, then the owner-visibility refresh, then `bNetDirty`. The refresh is
      planned up front and planned again after `LostChild` and `GainedChild`. A difference is a prefixed `NotRead`.
- **A VM change:** `IteratorResult` can carry a guard, which `foreach` runs after each body. The VM's iterators hand
  over a list up front, but the game re-reads `Components` by index. So `ComponentList` keeps a copy and compares it
  after each body. A change is "ComponentList: the loop body changed Components" (`NotRead`, prefixed). Equal arrays
  give the game's answer.
- **The hub's ruling 1, the ammo definitions:** the pickup world's set-up loads the eight ammo definitions the globals
  definition lists. Nothing they reference is loaded. Each load is logged before it runs as "harness input: set-up
  preload: <path> (UNVERIFIED: an ammo definition the game has resident)". A resident definition is neither loaded nor
  logged. The native still refuses a definition the store has not loaded, and never loads one.
- **The hub's ruling 2, GetSaveRotation at rest:**
  - the default stays section 5's physics `LayerAbsent` for a rigid-body pickup with a `CollisionComponent`;
  - under the `AtRestBodyPose` harness setting, which only the slice's pickup world turns on (Off only, like
    `PhysXLevel`), that case answers the actor's `Rotation`;
  - it logs "a rigid-body pickup at rest, its body pose taken as the actor's Rotation (hub ruling; UNVERIFIED)".
- **Gap 4.1:** at the slice's pickup set-up, the controller and pawn take the pickup's `WorldInfo` when theirs is None.
  It is logged as an UNVERIFIED stand-in. The slice gate's pickup rows now also list the set-up's ammo and at-rest
  lines, and each step's own harness lines.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; main 0195ce94 and this change):
  - 6 ok / 14 on both, X = 7.167/14 = 51.2%;
  - link 6 moves from `UpdateAmmoCounts`' `NotRead` to "stopped after partial writes: Actor.SetBase: the class's own
    implementation is not resolved", which is #785's prediction;
  - seven ammo definitions were preloaded (one was already resident), and the at-rest line was logged at the key;
  - link 7's stop is unchanged; only its harness list gained the set-up lines. Every other row is unchanged.
- **Tests** (invented classes and one invented package):
  - `pickup_take` (14):
    - `UpdateAmmoCounts`: the order, gains, silence, None, fall and equal;
    - the preflight refusals, which leave the counters, `class_code_loads` and the name table unchanged;
    - the mistyped declarations, the lookup refusal's prefix, and the native through the globals;
    - `GetSaveRotation`'s cases and the ruling;
    - `IsDLCRequirementMet`, `GetResourcePoolContext`, `EvaluateExpressions`, and Reference;
  - `actor_take` (9):
    - `ComponentList`'s filter and its guard;
    - `MakeNoise` and `IsUsingVehicle`;
    - `SetOwner`'s refresh, its no-op, the re-own stop, the changed refresh and an up-front refusal;
    - Reference;
  - the VM guard (1);
  - the ammo preload's exact load list (1).
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game. That includes whether the slice's pickup is at rest when taken, and whether the game has these definitions
  resident.
- **Sensitive files:** none.
