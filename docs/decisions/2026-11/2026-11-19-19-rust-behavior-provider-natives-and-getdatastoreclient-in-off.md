## 2026-11-19: Rust behavior provider natives and GetDataStoreClient in Off

AI-assisted (Claude Code). Lane FIX-PROVIDERS-AND-DATASTORE, implementer. Number 19 was given by the hub. The branch is
`origin/main` 9e9a2a38 (after #390), not stacked. No dependency, no CLI flag, no parsing change. There was no
real-game session. This lane opened no Ghidra or REA output and no `local/analysis/` file of the note's author; it
works from `docs/verification/NATIVE_BEHAVIOR_PROVIDERS.md` alone. Every rule here is **UNVERIFIED** in the running
game. Reference is unchanged: neither native is bound there, as in the C++ (R3).

- **1. `BehaviorKernel.BeginNondeterministicProviderRegistration`** (`ow-behavior` kernel pack, Off only), from the
  note's sections 2 and 4. The game caps the consumer's replication slot counter at 32. This port has no slot
  numbers and no replication, so nothing reads the counter, and the native returns without changing anything and
  without writing the handle, for every `PID`. Open question 1 (who reads a slot number) is what would make this
  no-op wrong; it stays open.
- **2. `BehaviorKernel.ForceUnregisterBehaviorConsumer`** (same pack, Off only), from sections 3 and 4.
  - **A kernel process:** it leaves the process table with its providers. Their running threads and pending work
    therefore never run again, and later kernel calls on its handle find no consumer.
  - **Its `PID`** is marked released: a new `Owner::Released`, answered like a handle nobody owns. It is kept out of
    the allocator, as section 4 asks while open question 2 (`PID` reuse) is open.
  - **A handle another crate owns:** `NotYet { NotPorted }`, before anything changes. That crate's teardown is not
    ported here; section 4 leaves this case to the implementer and says not to guess one.
  - **The handle:** in every other case, unknown, -1 and released `PID`s included, the caller's handle comes back -1
    (the out write of section 3, step 4).
  - **The kernel-wide sweep** at the end of unregistering (open question 3): what it collects is not read, and this
    port's kernel holds nothing beyond the process for it to collect, so it is not modelled.
  - Open question 4 (the "gets slot numbers" flag) has no reader here.
- **3. `UIInteraction.GetDataStoreClient()`** (`ow-world`, Off only), a stock UE3 static native. In public UE3 it
  returns the engine's one global `DataStoreClient`, made when the engine starts, so the shipped game always has one.
  The callers on the accept path (for example `WillowPlayerController.GetOnlineGameSettingsDatastore` and
  `UIRoot.StaticResolveDataStore`, read with `research/script_disasm.py`) test it against None and then call
  `FindDataStore` on it. No world here models the client or its data stores, and None would read "no client", which
  the game never answers. So it is `NotYet { LayerAbsent }`, naming the UI layer (W7-UI). The stop stays where it is;
  it is now typed and names what is missing instead of being an unbound native.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests:**
    - `ow-behavior` kernel pack: Begin on a registered, a -1 and an unknown `PID` (handle unchanged, the consumer
      still answers). Force on a kernel consumer with a provider: the handle reads -1, the process and provider are
      gone, events fire nothing, sequences read false, the `PID` is released and not handed out again, and a second
      Force, Force on -1 and Force on an unknown `PID` only write -1. Force on a crate's handle is `NotYet`. Both
      natives are unbound in Reference. 19 passed / 0 failed.
    - `ow-world`: `GetDataStoreClient` is `NotYet { LayerAbsent }` in Off and unbound in Reference.
  - **The Phaselock cast:** no installed driver in the tree calls `Skill.Activate`. The cast's stop at Begin (D42,
    D50, SLICE_RUN_PLAN.md) came from an earlier lane's local run, and SLICE-RUN-OFF's skill link is `notyet` (no
    headless driver). The ignored installed lift test (`ow-skills`) prints the same as on main 9e9a2a38 in Off (the
    Reference run passes; it was not compared with main). `OnActionSkillStarted` still stops at the unbound `ActionSkill.OnActionSkillActivated`, which this
    lane does not own (H6's part B).
  - **Installed `--mission-run` Fire** (`stage:8 player:8:2000 script:accept tick:0.5 script:screen script:turnin
    pool`) and **`--slice-run`** (12 steps), Off. Run on `fix/rust-mission-objectives` 90d5b710 merged with
    `fix/rust-off-stock` 811855ba, in a throwaway worktree, before and after this change:
    - the step records are equal;
    - the stop stays at `GetDataStoreClient`, now `NotYet { LayerAbsent }` (exit 3) instead of an unbound native
      (exit 1);
    - the slice's other two errors (`TriggerDetachSprites`, `AIComponent.ActivateEvent`) are unchanged.
  - **`--slice-gate`** on `feature/rust-slice-run-off` c1d332f1, before and after: byte-identical, strict 0 ok /
    5 notyet / 9 error. That base stops earlier than either native.
  - **On main 9e9a2a38 itself**, before and after: `--mission-run` Fire Off and Reference, `--slice-run` Off (12 and
    22 steps), `--combat-run` with W6-COMBAT's 15 steps (Off and Reference) and the five spawns in Off are equal,
    apart from timing fields. Main's Off accept stops at `GetObjectivesProgress`, before either native.
  - **Gates:** workspace build clean; workspace tests 1,880 passed / 0 failed / 42 ignored; `crate_edges_test.py` OK;
    decisions check OK; Fire checker in Reference: self-test passed, 33 of 33 rules.
