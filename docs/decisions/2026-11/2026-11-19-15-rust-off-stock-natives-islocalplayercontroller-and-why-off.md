## 2026-11-19: Rust Off stock natives: IsLocalPlayerController, and why Off spawn health lacks the class factor

AI-assisted (Claude Code). Lane FIX-OFF-STOCK. Number 15 was given by the hub. No dependency, no CLI flag, no parsing
change. There was no real-game session, and no Ghidra or REA session. Every rule here is **UNVERIFIED** in the running
game.

### 1. `PlayerController.IsLocalPlayerController()`, implemented (Off only)

A stock UE3 native. Its public UE3 meaning: true when the controller's `Player` is a local player, not a network
connection. FIX-MISSION-OBJECTIVES (decision 2026-11-19-10) made it the next Off stop at the mission accept.
`ow-world` (`world.rs`) binds it outside Reference, beside `WorldInfo.IsPlayInEditor`. The world models local players
by its local-player list (`Actors::add_local_player`, decision 2026-11-16-81), not by `Player` objects:

- a listed controller that is not destroyed: true;
- an unlisted controller whose `Player` is None: false;
- an unlisted controller whose `Player` is set: `NotYet { NotPorted }`, because the world cannot tell a local player
  from a connection without the `Player` object;
- no world: `NotYet { LayerAbsent }`.

Whether this game's own controller classes change the rule is UNVERIFIED. Two answers are this port's modelling
choices, not UE3's rule (which decides by `Player` alone), and are UNVERIFIED too: a listed controller that is
destroyed answers false, and a listed controller whose `Player` is None answers true. Reference keeps the C++ zero-result stub
(R3); `--slice-run`'s allow list keeps it, and only its reason text changed.

### 2. The Off spawn health factor: diagnosed, not fixed (an unread native)

- **Reproduced** on `origin/fix/rust-fire-to-hit` (40200264, the branch whose GRI handler lets Off spawns reach
  health). On main, Off spawns stop earlier, at the `GameReplicationInfo` query.
  - Spawns: dummy 8, regular 6, adult 7, ranged 6, baby 6.
  - Off health is the per-level base and nothing more: 155.73, 117.76, 135.44, 117.76, 117.76. The 117.755 and 135.439
    are decision 2026-10-08-08's per-level bases at levels 6 and 7.
  - Reference is that base times the kind's factor: 5, 2, 6, 4 and 0.5, which gives 778.658 for the dummy, its live
    value.
- **The skipped step.** The factor is the mind's noun `Attribute_HealthMultiplier` (NATIVE_NOUN_ATTRIBUTES.md
  section 1).
  - Reference reads it straight from the AI class's `AttributeStartingValues`; that is the C++ `nounValue` stand-in.
  - Off reads the mind's noun store. Nothing has written the class value there, so it reads `DefaultValue`, 1.0. The
    note's section 8 predicted exactly this.
  - In the game the value arrives through script, read here with `research/script_disasm.py`:
    1. `WillowMind.Possess` calls `InitializeCharacterClass`.
    2. That calls `ApplyCharacterClassStartingValues(AIClass)`, then `ApplyCharacterClassDefaults(MyWillowPawn, AIClass)`.
    3. Inside the second, the `Controller` super call makes the health pool. Then `WillowPawn.ApplyAttributeStartingValues`
       runs (`SetBaseValue` per entry, which sets the mind's nouns), then `RecalculateAttributeInitializedState`.
  - The port's `ow_combat::hit::adopt` (the C++ `adopt`) skips that route in both modes: it calls
    `SetHealthResourcePoolDefinition` directly.
- **Why it is not fixed here.** Tried locally (patch under ignored `local/p2/fix-off-stock/`), the Off adopt calls the
  mind's installed `InitializeCharacterClass`.
  - On the installed data it stops at once at `Controller.ApplyCharacterClassStartingValues(object)`. That is the
    `WillowMind` override, which NATIVE_COMBAT_GROUNDWORK.md (sections 1.10 and "Not read yet") lists as not read.
  - The same change breaks 19 synthetic Off tests in `ow-combat`, whose toy minds have no such script. (Both results
    were seen locally on pc-hummer with that reverted patch; they are not reproducible from the tree.)
  - So it was reverted, and Off spawn health stays as it is. Those rows are counts, not parity, as the FIX-FIRE-TO-HIT
    branch (`fix/rust-fire-to-hit`, decision 2026-11-19-06) already says.
- **Question for an analyst** (no Ghidra here): what does the `WillowMind` override of
  `ApplyCharacterClassStartingValues` do for an AI mind? Which pools does it make, in what order, and does it call the
  `Controller` base? After that, a fresh implementer can route the Off adopt through `InitializeCharacterClass` and
  update the 19 fixtures. The script's other reaches on that route are `GetSkillManager` (only with `PassiveSkills`) and
  `Actor.Spawn` of an interaction proxy (only with an `InteractionDefinition`).

### Verified (automated, local, pc-hummer)

- **Synthetic tests (`ow-world`):** the listed, unlisted, set-`Player` and destroyed cases, the no-world `NotYet`, and
  unbound under Reference. 93 passed / 0 failed / 4 ignored.
- **Installed runs on main 86c28d00, before and after:** `--mission-run` Fire Off and Reference, `--slice-run` Off
  (12 steps and all 22), `--combat-run` with W6-COMBAT's 15 steps in Off and Reference, and the five spawns in Off.
  Every output is byte-identical or differs only in timing fields. The native is not reached on main.
- **On `origin/fix/rust-mission-objectives` (90d5b710) with this lane's `ow-world` change applied** (a throwaway
  worktree):
  - Off `--mission-run` error: `IsLocalPlayerController` before, `UIInteraction.GetDataStoreClient()` after;
  - Off `--slice-run`, 12 steps: the same swap, with the step records equal;
  - Reference `--mission-run` and the full Off `--slice-run` stdout (the Pawn IAIInterface stop): unchanged.
