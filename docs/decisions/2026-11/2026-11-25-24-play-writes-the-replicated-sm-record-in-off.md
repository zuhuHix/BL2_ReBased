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
- that the struct's path is `GearboxFramework.GearboxPawn.ReplicatedSpecialMoveData`. The note names the struct and
  says `GearboxPawn` holds the record; where the struct is declared is this lane's fit;
- the first-native walk's native bit, as `ow-combat`'s read-only walk takes it;
- everything the note marks UNVERIFIED.

**Side effects to know:**
- `PlayLocal` shares `GetSMDToPlay`'s loaded-only lookup, so a definition class whose code is not loaded is now
  `NotRead` there too, where it used to be loaded;
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
