## 2026-11-26: The targetable resolver is the champion test

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-TARGETABLE-RESOLVER, the analyst half of the clean room (another
lane implements it), on main 5dd2906. Raw output stays under ignored `local/analysis/targetable/`. Docs only: no code
changed. Every rule is UNVERIFIED in the running game.

- **What was decided** (the note, docs/verification/NATIVE_TARGETABLE_RESOLVER.md):
  - **`TargetableAttributeValueResolver`'s get and get-base** cast the context to the native `ITargetable`. They answer
    1.0 only when the cast succeeds, `ValueToResolve` is `TARGETABLE_IsChampion` (the enum's only real value) and the
    targetable's `IsChampion` is true; else 0.0. They write and load nothing;
  - **`IsChampion`:**
    - false on `Pawn`, `WillowPawn` and `WillowPlayerPawn`;
    - on `WillowAIPawn`, its `BalanceDefinitionState.BalanceDefinition`'s `DoesSpawnChampion`, which on an
      `AIPawnBalanceDefinition` is its `Champion` bool;
    - vehicles, projectiles, interactive objects and other balance classes were not read;
  - **the Off answer** (section 5): a load-free, write-free preflight by the class chain and the first native classes,
    with typed `NotRead`s for what was not read, and 0.0 for a class with no native `ITargetable`;
  - **links 3 and 4** (a local probe, reverted):
    - the context the spawn passes is the enemy's `WillowMind`, which has no native `ITargetable`, so 0.0. Both
      Primal Beast balances have `Champion` false anyway;
    - the spawn's next stop is `WillowPawn.SetDefaultPhysics` past its gates (NATIVE_MIND_ROUTE.md section 1's open
      queries), the next read.
- **For the hub:** the port's spawn never fills `BalanceDefinitionState`, which the game's spawn does.
- **How it was checked:**
  - the field places land on declared properties with no shift;
  - the four pawn constructors' object sizes match their registrations;
  - the base balance virtual names itself in its error text;
  - the champion flag is the only bit at its place.
- **Index:** two NATIVE_INDEX rows, and NATIVE_ATTRIBUTES.md's "Not read yet" points here.
- **Verified:** nothing in the running game; the probe's rows are the port's. **Not verified:** everything in the game.
- **Sensitive files:** none.
