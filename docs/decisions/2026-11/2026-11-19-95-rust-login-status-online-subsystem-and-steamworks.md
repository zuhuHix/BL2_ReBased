## 2026-11-19: Rust login status: the online subsystem, the Steamworks login status and the natives after it, in Off

AI-assisted (Claude Code). Lane IMPL-LOGIN-STATUS, the fresh implementer of `NATIVE_LOGIN_STATUS.md` (analyst lane
ANALYST-LOGIN-STATUS, decision 2026-11-19-89; read on `research/rust-login-status` at a2d70e4, #474 after its reviews
r1 and r2). Its section 7 is the Off contract. This lane coded from the note alone and opened no analysis output. There
was no real-game session, so **every rule is UNVERIFIED** in the running game. No dependency was added and no bounds
check was loosened.

- **What is script and stays script.** `UIInteraction.GetLoginStatus` is static script (the note's section 2, which
  supersedes #466's wording that called it a native). Nothing is bound for it. The VM runs it, and
  `WillowPlayerReplicationInfo.IsInvalidName` before it, as installed. The local-player gate is the script's own
  (`IsLocalPlayerController`, Off since decision 2026-11-19-15), and nothing bypasses it.
- **`GameEngine.GetOnlineSubsystem()`** (`crates/ow-world/src/world/online.rs`, bound by the world in Off only;
  Reference keeps the VM's stub policy, R3). Four answers, which exclude each other:
  - no engine object (no `EngineObject` VM service, which is every headless world today): `NotYet { LayerAbsent }`
    naming the engine object and its online subsystem. Never None, because None would make `GetLoginStatus` answer 0
    and so assume "not logged in", which open question 1 leaves open;
  - an engine object that is not a `GameEngine`: None, the native's own answer;
  - a `GameEngine` whose `OnlineSubsystem` slot is empty: None, the field as stored. A world that makes a
    `GameEngine` and leaves the slot empty has chosen this;
  - a `GameEngine` with a subsystem in the slot: that object when the world's set-up record (`OnlineSetUp`, a VM
    service no world registers yet) says its interfaces were initialised. Otherwise it is `NotYet { NotRead }` for
    "the online subsystem's player interface initialisation" (open question 2). The script reads `PlayerInterface` as
    a plain field right after this call, which no native sees, so the refusal sits at this last native boundary. Only
    a modelled-empty `PlayerInterface` gives status 0.

  A `GameEngine` with no object `OnlineSubsystem` field, or a slot of another kind, is a data error. It writes
  nothing.
- **`OnlineSubsystemSteamworks.GetLoginStatus(LocalUserNum)`.** Section 4's rule applies to a subsystem whose login
  state the world's set-up has filled, which the same `OnlineSetUp` record lists: the result is `LoggedInStatus`
  as stored when the byte, widened, equals `LoggedInPlayerNum`, and 0 otherwise. Any other subsystem gets
  `NotYet { NotRead }` naming the login state, since its class defaults are not the game's state. No world fills one
  yet; that is a later lane's set-up decision (open question 1).
- **Bound by key.** Both natives are registered by native key, not by function lookup, so the binding holds before
  the declaring package is loaded. A first draft bound through a lookup and skipped the Steamworks native silently.
- **No preflight: the runtime check** (section 7, "When the gated absence may be preflighted"). The gates that decide
  whether the script calls `GetOnlineSubsystem` are not proven stable between `ApplyCharacterClassStartingValues`'
  preflight and the call (open question 5). These are the authority, the replication info and its owner, whether
  that owner is local, and its `Player` and `ControllerId`. So the refusal comes at the call, after the flow's writes.
  It is terminal through `ow-combat`'s existing `after_partial_writes`: "stopped after partial writes: ", kind and
  record kept, writes left in place.
  - The hub's -95 brief asked for a preflight. A first draft of this lane wrote one. The revised note (a2d70e4) rules
    it out until a lane reads those steps for writes to the gate fields, so it was removed. `ow-combat` is unchanged
    apart from one test.
- **The natives after the login status**, each its own named `NotYet { NotRead }` in Off, refusing only when the
  script reaches it. The keys were taken from the installed declarations (an unbound `--run` of each prints it):
  - `WillowPlayerInput.SetupInputDevices()`, reached only with a `WillowPlayerInput`, before the script clears
    `InputContextSelector`;
  - `WillowPlayerController.UpdateInputContext()`, reached with or without a player input, after that clear, which
    stays;
  - `PlayerController.UpdateURL(string,string,bool)` and `Object.SaveConfig()`, reached only by `SetName` with a
    non-empty name, after `ServerChangeName`. They write outside the world.

  `SetName` and `ServerChangeName` stay installed script: with an empty name they return with no effects, and nothing
  refuses. `Object.SaveConfig` is bound for every Off caller, not only this path, since any save writes the player's
  settings. Before this, Off answered all four with the VM's unbound-native error (or a stub where the policy allows
  one).
- **The hub's heads-up for #474's final round.** It expects the preflight to refuse only on gates that cannot change
  before the call, with the rest as terminal runtime checks. This lane codes the conservative form: no preflight at
  all, every stop a runtime check. Where the current note is silent, the answer is a named `NotYet`.
- **Not reached yet on the slice.** Link 1 still stops at `InitPlayerSkillTree`'s `NotYet { NotPorted }` before any
  of this, until decision 2026-11-19-88 binds it. The path is proved by tests:
  - `world/online_tests.rs`, 7 tests:
    - the answers of `GetOnlineSubsystem`, with and without the set-up record;
    - the four natives after the login status, and the clear before `UpdateInputContext`;
    - the typed stop from script;
    - Steamworks `NotRead` without the set-up record;
    - the section 4 rule with the record;
    - the Off and Reference binding by key;
  - `run/tests.rs`, 1 test: a bound stand-in `InitPlayerSkillTree` asks the online answer on a world with no engine.
    The refusal comes after the pools are made, marked "stopped after partial writes", with `LayerAbsent` and the
    record kept, and the pools stay.
- **Not changed:** the note and its index rows, which sit on #474's branch and are left to their lane, and d2b6's
  skill-tree files.

## Edits: review r1 of #479

AI-assisted (Claude Code). Sol's r1 review (high) asked for five changes. All are in one fix commit:

1. **`SetupInputDevices` marks its own refusal.** The controller's script `SetupInputDevices` is overridden in Off
   (`override_script`), coded from section 6.3's steps:
   - with a `WillowPlayerInput`, its native first; that refusal passes unchanged, since this function has written
     nothing yet;
   - then the clear of `InputContextSelector`;
   - then `UpdateInputContext`, whose refusal is now terminal at this boundary: "stopped after partial writes: ",
     kind and record kept, the clear left in place. The test asserts the exact prefix.
2. **No class lookup for a non-game engine.** `GetOnlineSubsystem` walks the engine object's own class chain in the
   layout table instead of `find_class`. So it loads nothing to answer None, and a class id the table did not make is
   a data error. The test gives an engine object of a class from another package while `Engine.GameEngine` is not
   loaded; it stays unloaded.
3. **A non-Steamworks `PlayerInterface`.** For an initialised subsystem, `GetOnlineSubsystem` checks the receiver
   held in `PlayerInterface`: a class not below `OnlineSubsystemSteamworks` gets `NotYet { NotRead }` naming that
   class, before any dispatch. An empty interface or a Steamworks one gives the subsystem.
   - **Why the check sits here and not at the dispatch:** the VM has no per-dispatch hook. Every `PlayerInterface`
     dispatch on the login path asks this native for the subsystem first (sections 2, 5 and 6.2), and nothing on the
     path writes that field in between.
4. **`ServerChangeName` guards the resolved `ChangeName`.** The script is overridden in Off, coded from section 6.2:
   - an empty name returns with no effects;
   - no `WorldInfo` or no game info skips the call (the accessed-None answer);
   - the base `GameInfo.ChangeName` runs as installed with `(Self, S, true)`;
   - any other resolved declaration (an override, not read) is `NotYet { NotRead }` naming the game info's class,
     before its body runs.
5. **GATES.md** keeps both records after the rebase onto main.

Of `SetupInputDevices`, `ServerChangeName` and `GameInfo.ChangeName`, only that each is
script was checked in the installed packages (`--disasm` exit codes); no body was read, and the steps come from
the note.
