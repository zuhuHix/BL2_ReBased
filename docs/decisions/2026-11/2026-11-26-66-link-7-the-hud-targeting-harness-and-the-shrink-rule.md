## 2026-11-26: Link 7: the HUD targeting's harness inputs and the shrink rule

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK7-HUD-TARGETING, on main d118d2b, which has the note (#806 with the hub's rulings
and their review r1). Clean room: implemented
from docs/verification/NATIVE_LINK7_HUD_TARGETING.md alone; this lane did not read the executable. No session was
run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference unchanged), each harness input logged:
  - **the shrink step** (`ow-inventory` `pickup_targeting`, replacing the pass's shrink `NotRead`; the note's
    section 7): enabled when the HUD has a `WorldInfo` whose `NetMode` is not 3 and the owner is not a split-screen
    player (a `Player` of None: no; a set `Player` is `NotRead`, since the engine's local-player count is not wired,
    the `--map` route's case included). The counted pickups whose `LifeSpan` is above the `WillowPickup` default's
    `PickupShrinkDuration` count, and the smallest such `LifeSpan` is tracked. When the count is above the HUD's
    `MaxNumPickups` (by fit), `BeginShrinking` is raised on it: resolved and checked in the plan, raised after the
    other writes. `MaxNumPickups`, `NetMode` and `Player` are read with their kinds;
  - **ruling 1, the harness HUD** (`ow-inventory` `hud_harness`, under the Off-only setting `HudTargetingHarness`,
    which the pickup world's set-up turns on): at `make`, before any begin play (the pickup world never begins play),
    the world's `Game.HUDType` is spawned owned by the controller into `myHUD`, then `PlayerOwner`, `WPlayerOwner`
    and `HUDDef` (the globals definition's `HUDDefinition`, preloaded at set-up and logged) are set. No movie native
    is called. `Game` None, a `HUDType` that is not a `WillowHUD`, an unloaded definition and a second set-up are each
    `NotRead` before any write;
  - **ruling 2, the registration** (same setting): the note's first form, begun play at set-up, was run on the
    installed gate and broke link 6. Its `make` stopped at the spawn's "the base search after PostBeginPlay", the
    refusal the note's review r1 said to check for, because the templates spawned after begun play need a base
    search. So this lane uses the note's fallback: each dropped pickup is registered with the bound
    `WillowGlobals.AddPickup` right after its spawn, as its `PostBeginPlay` would;
  - **ruling 3, the view** (same setting): the owner's pawn's `Location` + (0, 0, `BaseEyeHeight`), with the
    controller's `Rotation` (its offsets dropped). With no pawn, the camera `LayerAbsent` stays;
  - **ruling 4, the line check** (its own Off-only setting, `pickup_targeting::NoLevelLineCheck`, which the slice's
    two `NoLevel` worlds turn on): under a `NoLevel` service the line check answers no hit, passing over the owner's
    pawn, the target pickup and its `Base`. Any other live actor holding collision is `NotRead`, unless the
    segment's box is disjoint from its bounds box by the bounding-cylinder rule. The refusal names the actor, its
    component and the two boxes. A world with geometry is `NotRead`; one with no collision service keeps the
    headless `LayerAbsent`.
- **On the installed gate, link 7 stops at ruling 4:** the decoy pickup's bounds (its skeletal mesh, radius 118.5 by
  the rule) meet the line's box. Under the first wording of ruling 4 (no hit outright) link 7 passed all four steps.
  Whether the decoy should be cleared, or the rule's box narrowed, is the hub's call.
- **Tests** (invented packages): `hud_harness_tests.rs` (the harness HUD and its five refusals, made before begun play;
  the registration on and off; the eye-point view on, off and with no pawn; the line check off, with no service, cleared
  boxes, the three passed-over actors, an overlapping cylinder and an overlapping mesh actor, the setting twice;
  Reference refusing both settings and the HUD) and `pickup_targeting_tests.rs` (the shrink step: `BeginShrinking` on
  the 18 of 20 / 18 / 5 with a limit of 1, nothing at 2, nothing in a client world or with no `WorldInfo`, a
  split-screen owner `NotRead` before any write). **Not tested:** link 7's look on a toy fixture end to end (the
  installed gate runs it); the toy actors' bounds are points (their own body reads components the toy does not
  attach).
- **Review of #810, r2:**
  - `Vm::check_call`'s entry check now decodes a script body without storing the decode, as `Vm::check_prepared`
    does, so a plan that checks a cold handler and then refuses (for example at the split-screen check) leaves the
    cache cold. The call itself still decodes and caches. `Vm::is_decoded` is a read for tests;
  - `globals_definition_resident`'s class lookups are read-only. A class in the table is used; a package file that is
    not open is the cold `NotRead` (opening it to look is a write); "class not found" stays the hard error when no
    package file of that name exists or the open package's export table lacks the class;
  - the shrink step keeps its place in the plan, but reads the pickup default and each candidate's `LifeSpan` only
    after its enable test, so a client HUD or one with no `WorldInfo` needs no built default;
  - a pawn `Location` missing an axis is `VmError::Script`, as `Rotation` is;
  - tests: the decode cache left cold by both checks and filled by the call; the cold class with no package opened,
    no code loaded and no reflection entry; a disabled shrink with an unbuilt default (with and without pickups); a
    `Location` with no axes.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
