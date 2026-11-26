## 2026-11-26: SetDefaultPhysics, the mind's recalculation and the spawn balance

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC, on main 7494393. Clean room:
implemented from docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md (#788 as landed) and
NATIVE_MIND_ROUTE.md sections 1 and 3 alone; this lane did not read the executable. It read the installed script
`WillowMind.ApplyCharacterClassDefaults` with `research/script_disasm.py` for the call order (output under ignored
`local/analysis/`). No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference unchanged):
  - **`WillowPawn.SetDefaultPhysics()` past its gates** (`ow-combat` `default_physics`): the default physics by
    `WillowAIPawn`'s rule with the navigation handle's query named (flying when `FlightMode` is neither 0 nor 4); the
    modes named by the pawn's declared `EPhysics`; another pawn class's default physics `NotRead`. Then, all before
    the first write: `Actor.SetPhysics`' own preflight for that mode (a new read-only
    `ow_world::world::set_physics::preflight_check`), `bCloaked` and, only when set, the cloak's shield requirement
    (the can-cloak flag, the body-class walk, the shield through the pool handle rule else `ShieldVar` /
    `ShieldMaxVar`), and the collision copy (equal flags nothing, else `SetCollision`'s `NotRead`). The commit is the
    bound `SetPhysics`, then `bCollideWorld` from the archetype;
  - **the archetype record** (`hit::spawn_enemy`): the store does not load pawn archetype exports, so a second copy of
    the export, made the same way and never written, stands for the archetype the spawn copied, kept per pawn. No
    record is `NotRead` ("SetDefaultPhysics: the pawn's archetype");
  - **the can-cloak flag**, VM-wide and initially true, read and written by `StaticCanCloak` / `StaticSetCanCloak`;
  - **`WillowMind.RecalculateAttributeInitializedState`** (`ow-world` `levelup`): the key bound to the shared rule;
    the writes in the game's order, the skill refresh, the controller part (its pools' rebase) and then the mind's own
    part, which **corrects the port's order** (it ran the mind part's refusal first and the skill refresh before the
    class-level refusal). Refusals stay up front: the controller part's class-level step, then the mind part's plan
    (each of the eleven init data's definitions resident, with its chains' resolvers, else `NotRead`; each field
    declared, else `VmError::Script`). The mind part sets `CommerceMarkup` and, with a `StatusEffectComp`, the ten
    resistances by their base (the value recomputed from the modifier stack, `ow-attr`'s one-rounding stack fold,
    UNVERIFIED as the game's rounding), each evaluated with the mind as the context source. A refusal after a write is
    prefixed;
  - **the spawn's balance** (the hub's ruling on #788; a logged, UNVERIFIED harness input): at the spawn, the
    `AIPawnBalanceDefinition` export of the archetype's package file whose `AIPawnArchetype` is the archetype (by
    path; exactly one, else the step's `NotRead` naming it), logged. The step runs the pawn's installed
    `InitializeBalanceDefinitionState(balance, -1)` at the start of the mind's recalculation: in the port the
    attribute starting values run inside the mind's class-defaults script just before that call
    (`ApplyCharacterClassDefaults`: `SetDefaultPhysics`, …, `ApplyAttributeStartingValues`,
    `RecalculateAttributeInitializedState`), so that is where the factory's order puts it.
- **Tests** (invented packages): `default_physics_tests.rs` covers the note's tests 1 to 4, the can-cloak natives and
  test 5's pick (one match picked and logged, none and two refused); `ow-world` `pools/tests.rs` `mind_part` covers
  test 5's step, test 6 (the markup and the ten resistances after the rebase, no status component, an unloaded
  definition and a mistyped field before any write) and test 8 (the `WillowMind` key). **Not tested:** a one-entry
  modifier stack and the context being the mind (the toy system has no resolvers). Two older tests changed with the
  rule: the mind part's blanket `NotRead`, and a toy class that now declares its init data.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
