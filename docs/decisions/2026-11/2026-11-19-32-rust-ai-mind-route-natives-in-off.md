## 2026-11-19: Rust AI mind route natives in Off

AI-assisted (Claude Code). Lane FIX-MIND-ROUTE, implementer. Number 32 was given by the hub. The branch is `origin/main`
388a55ee, after #412 (FIX-MIND-ADOPT, decision 2026-11-19-26, with its review r1) landed; its route reaches these
natives. The source is b6d8's NATIVE_MIND_ROUTE.md (decision 2026-11-19-25), first read from its research branch at
4bf83c43 and checked against the version on main after #410's review fixes. Those fixes change no rule implemented
here: both open branches of section 1 now say to return `NotYet` rather than pick one, which this lane already does,
and section 2 says only that the answer is the pawn's own `IBalancedActor` one, its `ExpLevel`. This lane did not
analyse it and opened no `local/analysis/`, Ghidra or REA output. No dependency, no CLI flag, no parsing
change. There was no real-game session; every rule is **UNVERIFIED** in the running game. Each rule below is Off only.
Reference has no C++ stand-in for any of them and is unchanged (R3).

- **`WillowPawn.SetDefaultPhysics()`** (`ow-combat` `hit::pools`, section 1).
  - The gates are implemented: nothing changes when the pawn is gone (`bDeleteMe` or `bIsDead`), when it has no
    `SMComponent`, or when the component's `CurrentSMData.SMD` is a `WillowGame.SpecialMove_Spawned`. With a special
    move set, a failed lookup of that class is an error, not a passed gate.
  - The `ObjectArchetype`-is-a-`Pawn` gate always passes for a pawn here, because its archetype is its template or its
    class default object, both pawns. That is UNVERIFIED; the port does not model `ObjectArchetype`.
  - Past the gates the native sets the default physics and `bCollideWorld`, then decides on the collision flags by two
    pawn queries the note does not name. So it is `NotYet { NotRead }`, decided before anything changes. The default
    physics rule for an AI pawn is therefore not coded yet: nothing past the gates can run until the queries are read.
- **`GetExpLevel()`** (`WillowPawn` and `WillowAIPawn` declarations, one rule; section 2): on a `WillowAIPawn` it is
  the pawn's own `ExpLevel` as stored. On any other pawn it is `NotYet { NotRead }`: that pawn's `IBalancedActor`
  answer is not read.
- **`WillowMind.RecalculateAttributeInitializedState()`** (`ow-world` `levelup`, section 3). This replaces the blanket
  `NotYet` for a mind. The refusals of the mind's own part and of the controller part's class-level step are decided
  before anything changes; the skill refresh's own refusal comes after `GetSkillManager`, which may already have made
  the manager.
  - With both `AIClass` and `MyWillowPawn`, the mind's own part (`CommerceMarkup` and ten status-effect resistances
    from the class's init data) needs an evaluation context the note does not name: `NotYet { NotRead }`.
  - Otherwise, the skill refresh runs when the world's game info is a `WillowGameInfo`. The manager is fetched through
    the script native `GetSkillManager`. If `bUpdatingSkills` is clear, an `ActiveSkills` entry of this mind would get
    a call that is not read: `NotYet`. With no such entry, the flag set and cleared leaves nothing changed.
  - Then the shared controller part runs as before: its class-level step stays `NotYet`, and the pools are rebased.
  - The note's 13 writes against NATIVE_LEVEL_UP_ATTRIBUTES.md's 12 is not reconciled. It sits in that class-level
    step, which refuses either way.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests:**
    - `ow-combat`: `SetDefaultPhysics` changes nothing with no component and for a dead pawn, and is `NotYet` past the
      gates; `GetExpLevel` is the AI pawn's 4 and `NotYet` on a player pawn. A new extra package `toy_route()` holds
      the toy pawn and component.
    - `ow-world`: a mind with no class and no pawn rebases its pools; with both it is `NotYet` before the rebase; in
      Reference it is `NotYet`. The toy mind now declares `AIClass` and `MyWillowPawn`.
  - **Installed**, against `fix/rust-mind-adopt` (the base) and main 42b3df2f:

    | Run | main | adopt (base) | this branch |
    |---|---|---|---|
    | five spawns, Off: health rows | 10, without the class factor (dummy 155.73) | 0, at `SetDefaultPhysics` (not read) | 0, at `SetDefaultPhysics` past its gates (the collision queries) |
    | five spawns, Reference | dummy 778.658 (live 778.658), regular 235.51, adult 812.64, ranged 471.02, baby 58.88 | equal | equal |
    | W6-COMBAT's 15 steps, Off / Reference rows | 13 / 29 | 0 / 29 | 0 / 29 |
    | `maya:8`, Off | `GetPlayerPawnDataManager` | the same | the same |
    | `--slice-gate`, strict | | 0 / 5 / 9 | 0 / 5 / 9, byte-identical |

    The installed pawns' copies have an `SMComponent` with no current special move (`SMD` None, seen with a temporary
    trace, not kept), so they pass the gates. A fresh spawn in the game may be in its spawn move; the port does not
    model one yet (UNVERIFIED). The Off spawn stop
    stays in `SetDefaultPhysics`, one step further in. Past it, the mind's recalculation would stop on its own part
    before the pool rebase that shows the class factor.
- **Open, for an analyst:** the two collision-gate queries and the navigation handle's query of section 1; the
  evaluation context of section 3's mind part; the skill call of section 3 step 1.
- **Gates** (on aaa93bb3, before the move onto 569dabb5; rerun after it below): workspace build clean; workspace
  tests 1,966 passed / 0 failed / 43 ignored; `ow-combat`
  138 / 0 / 2 and `ow-world` 98 / 0 / 5 before the rebase; `crate_edges_test.py` OK; decisions check OK (2 added: this
  one and the base's); Fire checker in Reference: 33 of 33 rules. After the rebase the Off spawns still stop at
  `SetDefaultPhysics` past its gates, the Reference dummy reads 778.658, and `--slice-gate` grades 0 / 5 / 9.
- **On main 388a55ee** (after the rebase): workspace tests 1,976 passed / 0 failed / 45 ignored;
  `crate_edges_test.py` OK; Fire checker in Reference 33 of 33; the Off spawns stop at `SetDefaultPhysics` past its
  gates with 0 health rows; the Reference dummy reads 778.658; `--slice-gate` grades 0 / 5 / 9.
- **Review r1 of #418** (fix commits on this branch).
  - The spawn-move gate looked the class up as `GearboxFramework.SpecialMove_Spawned`. That class is `WillowGame`'s,
    and the failed lookup passed the gate silently, so it never fired. It now names `WillowGame.SpecialMove_Spawned`,
    and a failed lookup is an error. New synthetic test: a pawn in a toy `WillowGame.SpecialMove_Spawned` keeps its
    physics, and one in another move is `NotYet`. The fixture's `willow_game()` gained that one toy class (its bytes
    changed; `ow-cli`'s C++-digest cases still pass, and the C++ was not re-run on the new bytes), and `toy_route()`
    gained `ToyOtherMove`.
  - The mind override now checks the controller part's class-level refusal before the skill refresh, and the docs say
    which refusals come before any change.
  - Installed Off spawns after the fix: the five still stop at `SetDefaultPhysics` past its gates, 0 health rows.
