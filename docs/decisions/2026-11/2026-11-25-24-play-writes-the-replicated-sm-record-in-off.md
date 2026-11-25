## 2026-11-25: Play writes the owner's replicated special-move record in Off (SetReplicatedSMData)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SET-REPLICATED-SMDATA, on main 03896e5. Coded from
docs/verification/NATIVE_SET_REPLICATED_SMDATA.md alone, at its review r1 head fa346d2 (#684, decision 2026-11-25-19).
Clean room: the analyst lane read the native; this lane did not open the executable, Ghidra, REA or the analysis
output. Synthetic fixtures only. Off only; Reference binds nothing, as before.

**What changes** (`ow-skills` `action_skill/special_move.rs`):
- **No binding.** No script calls `SetReplicatedSMData`, so it is an internal step, "write the owner's replicated
  record", used by `Play`'s start and by its stop. On the `SMI` actor, `ReplicatedSMData`'s `SMD`, `Data`,
  `PlayRateScale` and `Duration` take the move's values, `DirtyCounter` goes up by one as a byte (255 wraps to 0), and
  `bNetDirty` is set. The callback is not copied.
- **`Play`'s first reads are loaded-only.** These come before `GetSMDToPlay` or any role read:
  - the component must be resident; then, with a `NewSMD` and an `SMI`, the component's `Owner`, the `SMI` actor and
    the definition. A lazy handle, a failed one or a stand-in with fewer slots is `NotYet { NotRead }`, "an object the
    store has not loaded";
  - `Owner` None, or an `Owner` that is not the `SMI` actor, is its own `NotRead`;
  - `Play`'s method checks (`GetSMDToPlay`, `ClientStarted`, `ClientFinished`, the callbacks) look in loaded code only
    (`Vm::find_loaded_method`). Code not loaded is `NotRead`, and nothing is loaded.
- **The record's preflight** replaces the old refusal of every authority `Play` ("the record the owner keeps of the
  current move for replication"). It runs after `Play`'s own refusals, only when the `Owner`'s `Role` is exactly
  `ROLE_Authority` (the local-authority gate keeps its at-least comparison):
  - the `SMI` actor's first native class is one of `GearboxPawn`, `WillowPawn`, `WillowPlayerPawn` or `WillowAIPawn`,
    else `NotRead`, "SpecialMoveComponent: the replicated record of <class>";
  - `ReplicatedSMData` is exactly `ReplicatedSpecialMoveData`, with `SMD` an object of `SpecialMoveDefinition`, `Data`
    an object, the rate and duration floats and `DirtyCounter` a byte, and `bNetDirty` is a bool;
  - the definitions declare the server events the commit will call: `ServerStarted`, and `ServerFinished` for the
    old move and for the new one, whose stop a zero duration may run;
  - any of these failing is a `VmError::Script` naming it, with nothing written.
- **The commit.**
  - **Start:** after `CurrentSMData` and `StartTime`, the record write, then the definition's `ServerStarted(SMI)`,
    then `ClientStarted`.
  - **Stop:** clears only `CurrentSMData.SMD`. Before, it zeroed the whole struct; now the other fields keep the
    stopped move's values, on every role, as the note's stop does. On the authority it then runs `ServerFinished(SMI,
    bInterrupted)` and writes the record from `CurrentSMData` as it reads after the event.
  - A start over a running move therefore steps the counter twice. A refusal after the commit stays terminal, with
    "stopped after partial writes".

**Fits, UNVERIFIED:**
- (corrected in review r1, item 6) the struct's path is no longer a fit, see review r1 below;
- the first-native walk's native bit, as `ow-combat`'s read-only walk takes it;
- everything the note marks UNVERIFIED.

**Side effects to know:**
- `PlayLocal` keeps main's loading lookup of `GetSMDToPlay` (review r1, item 3, below), so it is unchanged;
- the stop's half-clear changes what `CurrentSMData` holds after any stop, not only on the authority. Every reader in
  the port tests only its `SMD`.

**Effect on the slice** (predicted, not run): link 9 stops before this today, at the unbound `Actor.SetPhysics(byte)`
after `Queue` (the runner). This change unblocks `Play`'s authority path for later. Once a start reaches it, the record
is written and `ServerStarted` runs. `GearboxAnimDefinition.ServerStarted` then reaches the behavior provider's
initialisation and `OnServerStart`, the next candidates.

**Tests** (`special_move_tests.rs`, synthetic; each preflight refusal leaves #518's three counters, the name table,
the component, the record and the event list unchanged). The fixture gains `GearboxPawn` with the record,
`WillowPawn`, `WillowPlayerPawn`, `WillowAIPawn`, a script-only `ToyMayaPawn`, a native `ToyNativePawn`,
`ServerFinished`, the component's `Owner`, `bNetDirty`, lazy handles and a definition class left without code. The
note's tests:
1. the start on an exact-authority owner, in the order record, `ServerStarted`, `ClientStarted`;
2. a start over a running move: two counter steps, the stop's write holding `SMD` None and the old move's rate;
3. the counter wraps 255 to 0;
4. a stop: during `ServerFinished` the move has `SMD` None and keeps its rate while the record is still old; after
   it, the record is the stopped move's;
5. an autonomous owner, and one at `ROLE_MAX`: no record write and no server event;
5a. the first reads: no `Owner`, another `Owner`, a lazy definition, a lazy `Owner` and a lazy `SMI` actor (each still
    unloaded after), and a cold definition class (its code still not loaded after, checked with `class_code_loaded`);
5b. a `ServerStarted` failure after the commit: terminal, and the record and the move keep what the commit wrote;
6. a native owner class not read: `NotRead`;
7. seven declaration variants (another struct, a missing field, `SMD`, rate, counter or `bNetDirty` of another kind,
   no `ServerStarted`): each a script error naming it;
8. the record holds only its five fields; the callback is not copied;
9. Reference: the existing binding test.

The existing authority refusals now expect the class `NotRead` for the fixture's plain `Engine.Pawn` owner.

**Not verified:** anything in the running game.

**Review r1** (Opus high, on #689):
1. **The definition `GetSMDToPlay` answers must be resident.** A script override can answer another definition, and
   reading it through the loading helpers would load it before a later refusal. `Play` now checks it right after
   `GetSMDToPlay`. Test: a toy `ToyRedirectMove` whose `GetSMDToPlay` answers a lazy definition gives the residency
   `NotRead`, with the handle still unloaded and nothing written. The test fails without the check.
2. **The loaded-only lookups refuse cold class code on every role**, including the autonomous path that used to work
   by loading the code. The strictness is kept: a refusal stays load-free, as in #570 and decision 2026-11-19-185.
   **The prediction changes:** once link 9 reaches `Play`, its next stop may be "<function> on <class>: the class's
   code is not loaded" on the definition's class. A set-up preload like decision 2026-11-19-185's (a labelled harness
   input) may then be needed.
3. **`PlayLocal` gets main's lookup back.** It has no write-free preflight, and its `ServerStarted` and
   `ClientStarted` go through the loading lookup anyway. A loaded-only `GetSMDToPlay` would only add a new `NotRead`.
   Test: `PlayLocal` on a cold definition class loads that code and plays, as on main; `Play` refuses the same
   definition write-free.
4. **The hand-declared fallback.** For a class with no code key, `loaded_method` looks up the declared function by
   path with `Vm::find_function_if_any`. Such a class has no package export, so that lookup cannot load. Errors are
   now returned instead of hidden behind `if let Ok`. The lookup also fails as a script error if the store's load
   count or the name table moved during it. `Vm::class_code_loads` (#685) can join that check once it is on main.
5. **Test 8 can fail now.** The `CallbackFields` variant gives the record's struct `CallbackName` and
   `CallbackObject` fields. A start with a callback leaves both at their defaults while the move itself carries the
   callback.
6. **The struct's path, read** (the runner's names-only probe of the installed packages, 2026-11-25). The struct is
   `GearboxFramework.SpecialMoveInterface.ReplicatedSpecialMoveData`: its outer is the `SpecialMoveInterface`
   interface class, and it is the only struct of that name. Its members, in order: `Data` (an object, `Core.Object`),
   `PlayRateScale` (a float), `Duration` (a float), `DirtyCounter` (a byte with no enum) and `SMD` (an object,
   `GearboxFramework.SpecialMoveDefinition`). The identity check uses that path; the earlier guess,
   `GearboxPawn.ReplicatedSpecialMoveData`, was wrong. The fixture now declares the struct in `SpecialMoveInterface`,
   and the `OtherStruct` variant puts a lookalike in `GearboxPawn`.
   - **Three classes declare `ReplicatedSMData` of that type:** `GearboxFramework.GearboxPawn`,
     `WillowGame.WillowInteractiveObject` and `WillowGame.WillowVehicle`. The note's owner rule reads only the four
     pawn classes, so an interactive object or a vehicle as the `SMI` actor gets the class `NotRead` ("the replicated
     record of <class>"). The note lists `WillowInteractiveObject`'s own body as not read (its open question 1) and
     says nothing of `WillowVehicle`.
