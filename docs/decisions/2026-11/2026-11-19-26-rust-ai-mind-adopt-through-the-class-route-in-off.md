## 2026-11-19: Rust AI mind adopt through the class route in Off

AI-assisted (Claude Code). Lane FIX-MIND-ADOPT, implementer. Number 26 was given by the hub. The branch is `origin/main`
5ce1679a (written on 42b3df2f, where the runs below were taken, then rebased cleanly). The source is b6d8's analyst
note, NATIVE_COMBAT_GROUNDWORK.md section 1.11 (decision 2026-11-19-17, PR #402, now on main). This lane did not
analyse it and opened no `local/analysis/`, Ghidra or REA output. No dependency, no CLI flag, no parsing change. There
was no real-game session; every rule is **UNVERIFIED** in the running game. Reference is unchanged: it keeps the C++
`adopt` (R3).

- **The Off adopt goes through the mind's installed class route** (`ow_combat::hit::adopt`). This is the patch lane
  FIX-OFF-STOCK kept (decision 2026-11-19-15). After the mind is made, its `MyWillowPawn` is set and its script
  `InitializeCharacterClass` runs. In the installed data that is:
  1. `ApplyCharacterClassStartingValues(AIClass)`;
  2. then `ApplyCharacterClassDefaults(MyWillowPawn, AIClass)`: the health pool through its `Controller` super call,
     the pawn's fields, the shield pool, the passive skills, `ApplyAttributeStartingValues` and the recalculation.

  The rest of `WillowMind.Possess` is not run. Reference keeps the C++ path, which calls
  `SetHealthResourcePoolDefinition` directly.
- **`Controller.ApplyCharacterClassStartingValues`** (`ow-combat` `hit::pools`, Off only), the `Controller` base the
  note settles (section 1.10 step 1; section 1.11: `WillowMind` has no override).
  - Every pool of the controller's `ResourcePoolManager` is deleted.
  - Then, with a class: one pool per entry of its `ResourcePools` in array order, then `AccuracyPoolDefinition` into
    `AccuracyPool` and `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only when set. The pools are
    made by `CreatePool` with the controller as owner, and a manager that call spawns is written back.
  - With no class, it only deletes.
  - Refused with `NotYet { NotRead }`:
    - a `WillowPlayerController`: its override ends with a script call whose name is not read (section 1.10 step 2);
    - a class whose pools, its `ResourcePools` and the two accuracy pool definitions together, name one resource twice.
      The note does not give `CreatePool`'s reinitialise flag, and after the delete that flag can matter only for a
      second pool of the same resource.
  - A class argument that is not an object reference is a script error. Reading it as "no class" would delete every
    pool.
- **The natives on the route that have no note** answer `NotYet { NotRead }` by name in Off:
  - `WillowPawn.SetDefaultPhysics` and `WillowAIPawn.GetExpLevel` are bound to that refusal (`hit::pools`);
  - `WillowMind.RecalculateAttributeInitializedState` was already `NotYet` (`ow-world` `levelup`).

  b6d8 is reading the three.
- **Records.** NATIVE_INDEX: the "Implemented" column of the `ApplyCharacterClassStartingValues` row and of the mind's
  route row.
- **Fixture** (`ow-combat` `run/fixture.rs`, invented values, no game data):
  - The toy `Controller` declares the native.
  - The toy `WillowMind` gains `MyWillowPawn`, `InitializeCharacterClass` (the two calls; the installed guard flag left
    out) and `ApplyCharacterClassDefaults` cut to its health step, as the player controller's toy already is
    (decision 2026-11-16-82).
  - A new extra package `toy_starting()` holds a toy character class with `ResourcePools` (only the starting-values
    test adds it).
  - The 20 `ow-combat` tests the route broke pass again. That is FIX-OFF-STOCK's 19, plus one shots test added since.
  - The package bytes of `Engine` and `WillowGame` changed. `ow-cli`'s 16 C++-digest cases still pass: Reference
    prints the same lines. The C++ binary was **not** re-run on the new bytes; the digests are those of the old bytes.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests (`ow-combat`):**
    - an Off spawn goes through the toy route: the mind has the pawn, and the health pool is slot 0 at the pool's
      invented start;
    - the starting values delete every pool, then create `Pool_Health` and `Pool_Ammo` in slots 0 and 1;
    - one resource twice is `NotYet` and changes nothing;
    - no class deletes only;
    - a player controller is `NotYet`;
    - the native is unbound in Reference.
  - **Installed, Off** (main 42b3df2f against this branch):

    | Run | Before | After |
    |---|---|---|
    | five spawns (dummy 8, regular 6, adult 7, ranged 6, baby 6) | 10 health rows, the per-level base without the class factor: dummy 155.73, regular, ranged and baby 117.76, adult 135.44 (Reference: 778.658, 235.51, 471.02, 58.88, 812.64; the dummy's live value is 778.658) | 0 rows: every spawn stops at `WillowPawn.SetDefaultPhysics` (`NotYet`), after the starting values and the health pool and before the attribute starting values |
    | W6-COMBAT's 15 steps | 13 rows | 0 rows: the spawns stop as above, so the later steps find no pawn |
    | `maya:8` | `GetPlayerPawnDataManager` | the same (it stops before any AI spawn) |
    | `--slice-gate` (SLICE-RUN-OFF b456f983 merged into main, throwaway worktree), strict | 0 / 5 / 9 | 0 / 5 / 9, byte-identical (link 1 stops earlier) |

    So the Off rows that read a wrong health are gone. In their place is a stop that names the next unread native.
    The factor itself shows only once `SetDefaultPhysics` is read and the route reaches `ApplyAttributeStartingValues`.
  - **Installed, Reference:** the spawns, the 15 steps and `maya:8` are equal before and after, apart from timing
    fields.
  - **Gates:** workspace build clean; workspace tests 1,925 passed / 0 failed / 43 ignored on 42b3df2f and
    1,949 / 0 / 43 after the rebase onto bbb7c9de (`ow-combat` 137 / 0 / 2; main's later commits up to 5ce1679a are
    docs only);
    `crate_edges_test.py` OK; decisions check OK; Fire checker in Reference: 33 of 33 rules.
- **Open.**
  - The analyst notes for `SetDefaultPhysics`, `GetExpLevel` and the mind's `RecalculateAttributeInitializedState`.
    Then the Off spawn health can be compared with Reference and the live dummy.
  - A re-run of the C++ binary on the changed fixture bytes, to refresh the digests' provenance.
- **Review r1 of #412** (fix commit on this branch): the duplicate-resource guard now covers the two accuracy pools,
  before the delete, and a non-object class argument is a script error instead of a silent delete of every pool. The
  synthetic starting-values test gained both cases (`toy_starting()` gained `Class_Accuracy`).
