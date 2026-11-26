## 2026-11-26: Maya's link 9 natives: SetDefaultStance, the shield hook and two statics

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-MAYA9-NATIVES (queue 0ak, link 1). Clean room: implemented from
docs/verification/NATIVE_MAYA9_AND_AFTER.md (#776, read at origin/docs/native-maya9-and-after b1655e4c) sections 3.1
to 3.4 and the hub's rulings alone; this lane did not read the executable. No session was run. Every rule is
UNVERIFIED in the running game.

- **What was decided** (Off only; Reference binds nothing and keeps the stub policy):
  - **`WillowPlayerPawn.SetDefaultStance()`** (new `ow-combat` module `stance`), section 3.1:
    - the source walk (`BodyClass`, then `AnimationParentBody` while `bUseParentStances`), the pick (injured, crouched,
      `bIsSprinting` not 0, else run) and the set-stance in the note's order;
    - the writes: `OverrideStance` for an override, `bNetDirty`, the current stance, `bCanCrouch` (the archetype's for
      one of the five, false for an override), `AimOffset` when its node name differs, and `SetSkelControlActive` on
      `LeftHandIK` with its propagation;
    - `OnExitStance` / `OnEnterStance` go through the kernel's event rule on the pawn's consumer handle. **The hub's
      ruling: an event on a provider the consumer never registered answers nothing** (UNVERIFIED, by fit; the kernel's
      own routine for it was not read). A handle no one owns: nothing. A crate-owned handle: `NotRead`;
    - the rotation branch (a `Controller`, the aim test passing before and failing after) is `NotRead`, before any
      write;
    - the preflight reads only: declarations (`VmError::Script`), residency (`NotRead`), the node lookup and the
      controls' plan come before the first write. A refusal after a write carries the partial-writes prefix;
    - **fitted, UNVERIFIED:** the current and override stance fields are named by their use, `CurrentStance` and
      `OverrideStance`;
  - **`WillowPawn.SetDefaultStance()`:** nothing. Both left go_down's placeholder list; `WillowAIPawn`'s stays
    `NotRead`;
  - **the `ShieldResourcePool` idle-reset hook** (`hit::pools`), section 3.2: the shield (the provider controller's
    pawn's first equipped item, a `WillowShield`) is planned before the stamp; after it, a changed `bPrimed` is written
    and `ShieldPrimedStateChanged` runs. The six mechanisms' table uses ordered float compares, so a NaN fails each.
    No `Definition`, no controller, pawn or shield: nothing after the stamp. A shield with `ShieldDef` None: `NotRead`
    before the stamp. The event's name is by fit (the routine finds it by a run-time name, not read);
  - **`FlagDefinition.ApplyFlagInitializationData(FlagsToSet, ContextSource)`** (new module `maya9`), section 3.3:
    an empty array or a None context does nothing; any entry is `NotRead` (the value resolver's set-up). **The hub's
    ruling:** the game's `rand()` draw is the VM's RNG stream, counted and UNVERIFIED; no draw is made while every
    entry refuses. **Gap:** the note's "a skipped array is `LayerAbsent`" is not decidable in a native that receives
    the array's value: a source array the store skipped arrives empty, so it answers as empty. Test 8's third case is
    not written;
  - **`BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects(Object)`**, section 3.4: a None
    argument, no `WorldInfo`, no GRI or a GRI of another class, or `CurrentPlaythrough` 1 or below: nothing. Above 1:
    `NotRead` (the level table and the effect applier). No world: the world layer's `LayerAbsent`. Reads only;
  - no other pack bound any of these keys (checked by grep).
- **Tests** (invented classes): `stance_tests.rs` covers the note's tests 1 to 6, 8 (first two cases) and 9; the
  shield's four tests in `hit/pools/tests.rs` cover test 7. Test 1 makes a kernel-registered consumer through
  `RegisterBehaviorConsumer` without the stance's provider (Maya's case). Test 4 calls the set-stance directly: the
  pick only ever selects one of the five, so an override cannot come from `SetDefaultStance` itself. Tests 10 and 11
  are the key-binding branch's (fix/is-player-pawn-and-unbound-keys).
- **The slice gate:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
