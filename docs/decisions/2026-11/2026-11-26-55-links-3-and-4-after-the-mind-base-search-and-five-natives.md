## 2026-11-26: Links 3 and 4 after the mind: the base search and five natives

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK34-AFTER-MIND, on main 7a67b9f (#792 landed there). Clean room: implemented
from docs/verification/NATIVE_LINK34_AFTER_MIND.md (#793 as landed) alone; this lane did not read the executable. No
session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference unchanged):
  - **`Actor.SetPhysics`' base search** (`ow-world` `set_physics`, step 3 of the preflight, replacing "the base
    search for a walking-type mode"): into `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider`, no collision
    service is `LayerAbsent` ("the base search's floor trace", layer "collision (the world has no collision
    service)"); a service with geometry, or another live actor with a `CollisionComponent`, is `NotRead` (the trace's
    flags are not decoded). **The stand-in** (the hub's ruling on #793): a service that holds no geometry (`NoLevel`,
    a new `SpawnCollision::holds_no_geometry`, true for it only) and no other actor with collision gives no floor;
    with `Base` None nothing changes past the mode write, logged once per call as a harness input; with a `Base` set
    it is `NotRead`, "the base search's base setter", until the base setter is read (lane E's SetBase note). All
    decided before the mode write, so none carries the partial-writes prefix. With the stand-in the transition
    commits are reached: `bForceFloorCheck` into `PHYS_Walking` (the `Pawn` layer, before the mode) and `Velocity` /
    `Acceleration` zeroed into `PHYS_None` or `PHYS_Rotating` (after it);
  - **The hub ruling on review of #798, R1** (closes the ruling decision 2026-11-26-51 left open): the `NoLevel`
    stand-in is allowed only behind an Off-only harness setting, `set_physics::NoLevelBaseSearch`
    (`enable_no_level_base_search`, a registration error outside Off or twice), which the slice's two `NoLevel`
    worlds turn on, as they do `PhysXLevel` and `AtRestBodyPose`; it logs a set-up harness input and stays
    UNVERIFIED. Without the setting a `NoLevel` world keeps the trace flags' `NotRead`;
  - **The hub ruling on review of #798, R2**: the stand-in's actor walk skips the actor's own `CollisionComponent`.
    UNVERIFIED, on the common convention that an engine trace ignores its source; labelled so in the code;
  - **`WillowPlayerPawn`** has `WillowPawn`'s three layers in `SetPhysics` (it keeps `WillowPawn`'s body), replacing
    its `NotRead`;
  - **`GameInfo.IsFriendlyFire`** (both keys): the parameters declared as an object and an interface, and the values
    an object and an interface (or None; review of #798), else `VmError::Script`; a `WillowCoopGameInfo` receiver `NotRead` ("four parts are not named"); otherwise false;
  - **`Pawn.InGodMode`** and the `Controller`, `GearboxMind` and `WillowMind` keys, load-free: the pawn's `Controller`,
    else its `DrivenVehicle`'s; the AI-god flag (VM-wide, initially false; `GetAIGod` / `SetAIGod` bound), then
    `bGodMode` / `bNonCheatGodMode`, then on a `GearboxMind` its own `Pawn`'s `bCanBeInjured` (false) or its
    `AIComponent`'s god holds. Every object read must be resident, else `NotRead`;
  - **`DamageTypeDefinition.IsHealingDamageType`**: a `WillowDamageTypeDefinition` whose `DamageType` is the enum's
    `DAMAGE_TYPE_Healing` (6 when the byte has no enum); the base class false;
  - **`WillowPawn.AdjustMomentumForSurfaceType`**: no damage type, the magnitude; else times the momentum scale for the
    victim's surface (a live shield pool with value 3; else the hit region by bone, then its physical material's
    `WillowPhysicalMaterialProperty`, else its `DefaultDamageSurfaceType`; no helper, body class or region 0), the
    scale's init data evaluated with `InstigatedBy` as the context source after a residency check of its definitions
    (else `NotRead`); another surface 1.0, the base class 0.0. Review of #798: the surface, the residency check and
    the evaluation run in `ow_attr::eval::load_free`, so a nested unloaded handle the evaluation reaches (the
    formula's, the conditional's, the variance's or the range's terms) is `NotRead` and loads nothing (finding 1); a
    `HitInfo` that is not a struct is `VmError::Script` (finding 7); the shield pool's manager is checked resident
    before its slots are read, here and in `default_physics`' shield read (finding 8). The float reads accept a plain
    `Float` (the pool's `CurrentValue` and an attribute's base side are plain floats in the packages) or a
    `FloatAttribute` value side (finding 7);
  - **`Pawn.IsVulnerableToFriendlyFire`** (both keys): a `WillowPlayerPawn` whose
    `FriendlyFireDamageAcceptedBaseValue` is above 0.0001 (UNVERIFIED: the note's layout reading); any other pawn
    false.
- **Tests** (invented packages): `ow-world` `set_physics_tests.rs` the note's tests 1 and 2 (the base search's four
  answers and the commits under the stand-in; a `WillowPlayerPawn`'s change and its nav-mesh request with no AI step);
  `ow-combat` `link34_tests.rs` tests 3 to 7, each checked write-free (test 8). Review of #798 added: `NoLevel`
  without the setting refused, the setting Off only and once, the actor's own collision skipped, another placed
  actor's collision refused with nothing written, `Acceleration` zeroed with `Velocity`, `PHYS_None` to
  `PHYS_Falling` sending `BeganFalling(0)`; `IsFriendlyFire`'s two mistyped arguments; a lazy `DrivenVehicle`; a live
  shield pool's surface 3, an empty pool's 0 and an unloaded manager's `NotRead`; a non-struct `HitInfo`; and a
  nested unloaded attribute under a resident definition answering `NotRead` with the counters unchanged (removing
  the scope makes it load, mutation-checked). Four older tests that pinned the old base-search or player-pawn refusal assert the new answers.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
