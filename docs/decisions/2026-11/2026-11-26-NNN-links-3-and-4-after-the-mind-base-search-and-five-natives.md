## 2026-11-26: Links 3 and 4 after the mind: the base search and five natives

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK34-AFTER-MIND, on main 54c9cf0 with #792
(fix/rust-setdefaultphysics @ 08f8dae) merged in: links 3 and 4 need it to reach these stops. Clean room: implemented
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
  - **`WillowPlayerPawn`** has `WillowPawn`'s three layers in `SetPhysics` (it keeps `WillowPawn`'s body), replacing
    its `NotRead`;
  - **`GameInfo.IsFriendlyFire`** (both keys): the parameters declared as an object and an interface, else
    `VmError::Script`; a `WillowCoopGameInfo` receiver `NotRead` ("four parts are not named"); otherwise false;
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
    (else `NotRead`); another surface 1.0, the base class 0.0;
  - **`Pawn.IsVulnerableToFriendlyFire`** (both keys): a `WillowPlayerPawn` whose
    `FriendlyFireDamageAcceptedBaseValue` is above 0.0001 (UNVERIFIED: the note's layout reading); any other pawn
    false.
- **Tests** (invented packages): `ow-world` `set_physics_tests.rs` the note's tests 1 and 2 (the base search's four
  answers and the commits under the stand-in; a `WillowPlayerPawn`'s change and its nav-mesh request with no AI step);
  `ow-combat` `link34_tests.rs` tests 3 to 7, each checked write-free (test 8). **Not tested:** `IsFriendlyFire`'s
  mistyped argument. Four older tests that pinned the old base-search or player-pawn refusal assert the new answers.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
