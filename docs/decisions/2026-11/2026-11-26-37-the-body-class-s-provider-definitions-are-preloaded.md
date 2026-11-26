## 2026-11-26: The body class's provider definitions are preloaded

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-BODYCLASS-PRELOAD (queue 0aj), on main 0847259. It implements the hub's
ruling on NATIVE_MAYA9_AND_AFTER.md section 4.2 (#776, read at 84672ed2): in the game, the body class's referenced
definitions (stances, behavior providers) stream in with the character. This lane did not read the executable. No
session was run. UNVERIFIED in the running game.

- **What was decided**, in `ow-cli`'s new `combat::body_class_preload`. The `maya` step's Off harness runs it at the
  class select's set-up, after the anim-set preload (#754's pattern). It is **narrow**: exactly what
  `InitializeBehaviorProviders`' preflight reads from the body class (`ow-behavior`
  `kernel::natives::pawn_providers`, steps 6 (a) to (f)), not a transitive closure. In order, each once:
  1. the body class (the archetype's `BodyClass`);
  2. the holders:
     - (a) the `DefaultHitRegion`, only when `HitRegionList` holds it;
     - (b) each `HitRegionList` entry, then its `OverrideDeathDef`;
     - (c) each `ScrapedStances` entry;
     - (e) `DeathDef`;
     - (f) `PlayerEventDef`;
  3. after each holder, its `BehaviorProviderDefinition`, whose `BehaviorSequences` the kernel's gate reads.
- **What is not loaded:**
  - step (d), the pawn's instance-data hit regions, adds nothing: the body composition refuses a component with a hit
    region;
  - `ApplyBodyClassStartingValues` is script, and its other bound native (`SetDeathDefinition`) reads the `DeathDef`
    already listed;
  - a definition's sequences' own references (conditions, behaviors) are not loaded: the preflight does not read them,
    and the commit's decode loads what it needs;
  - `SetDefaultStance`'s other stance fields (NATIVE_MAYA9_AND_AFTER.md 3.1) are not loaded: that native is not
    implemented yet.
- **How each load is logged:** "harness input: set-up preload: <path> (UNVERIFIED: streamed with the character)",
  then one count line. An object already resident is neither loaded nor logged. A stand-in (slots not built) is
  skipped and logged. A failed load is the step's error. Off only (the class inputs run only in Off).
- **Reference:** unchanged.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; main 0847259 and this change):
  6 ok / 8 not ok, 51.2% on both, and every row unchanged.
  - **14 objects** were preloaded, the body class already being resident: Maya's five stances (`RunStance0`,
    `PatrolStance0`, `SprintStance0`, `InjuredStance0`, `CrouchStance0`), `Death_Siren` and `PlayerEvents_Siren`, each
    with its provider definition. The analyst probe's walk had visited 156.
  - On main, **link 1 still stops at the unbound `Pawn.IsPlayerPawn()`**, before `InitializeBehaviorProviders`. Lane F
    is binding it, and its branch was not pushed, so this reports against main.
  - **A local probe** (a temporary `IsPlayerPawn` stub answering true, never committed, reverted): links 1, 3, 4, 5, 13
    and 14 then pass `InitializeBehaviorProviders`' residency checks. They stop at its **pass-2 `LayerAbsent`**, on
    `PlayerEvents_Siren`'s sequence `Ep10_MordecaiChatter` and its `CustomEnableCondition`. That is #771's stop
    (NATIVE_MAYA9_AND_AFTER.md section 2, row 5). The `RunStance0` `NotRead` is gone.
- **Tests** (`body_class_preload_tests.rs`, an invented package):
  - the exact load list, in order, logged with the count. The default region outside the list, its definition and an
    unrelated reference stay unloaded;
  - a second run loads and logs nothing;
  - a listed default region comes first, once;
  - a stand-in stance is skipped and logged, with its definition not loaded, and the rest goes on.

  The harness test's line list now expects the preload's no-`BodyClass` line.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, and that these are all the objects the game has resident (only those the port's preflight reads were taken).
- **Sensitive files:** none.
