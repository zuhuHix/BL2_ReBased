# The owner's replicated special-move record: SetReplicatedSMData (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SETREPLICATEDSMDATA (decision 2026-11-25-19), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/repsm/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED in the running game.** It completes
[NATIVE_SPECIAL_MOVE_PLAY.md](NATIVE_SPECIAL_MOVE_PLAY.md) (the Play note) at its step 4.4 and section 4 step 3, and is
used by [NATIVE_SPECIAL_MOVE_QUEUE.md](NATIVE_SPECIAL_MOVE_QUEUE.md).

**Why.** Off's `Play` refuses every start on an authority owner with "the record the owner keeps of the current move for
replication (section 1 step 4.4 does not name it)" (decision 2026-11-19-52). Its stop has the same gap on the
authority. With the Queue port, link 9 is predicted to stop there: Maya is `ROLE_Authority` in a standalone world.

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, the nine code packages):
  - `SpecialMoveInterface` declares `SetReplicatedSMData` as a native. `GearboxPawn` registers a native body for it,
    and so do `WillowInteractiveObject` and others. **No script function calls it**;
  - `GearboxPawn` holds the record `ReplicatedSMData`, a `ReplicatedSpecialMoveData` struct with the fields `Data`,
    `PlayRateScale`, `Duration`, `DirtyCounter` (a byte) and `SMD`;
  - `GearboxPawn.ReplicatedEvent` hands a changed `ReplicatedSMData` to its component's `PlayReplicated`. That runs only
    where a replicated value arrives, on a network client, never on a standalone authority;
  - the server events on the Phaselock chain:
    - `SpecialMoveDefinition`'s `ServerStarted` and `ServerFinished` are empty;
    - `GearboxAnimDefinition.ServerStarted`: for a behavior-consumer owner, it initialises the definition's
      `BehaviorProviderDefinition` for it and fires `OnServerStart`;
    - `GearboxAnimDefinition.ServerFinished`: for such an owner, it fires `OnServerStop`;
    - `WillowAnimDefinition`'s pair puts AI holds on, then releases them (`Hold` / `HoldMovement` / `HoldHeadLook`, then
      `Release` / `ReleaseMovement` / `ReleaseHeadLook`). This happens only with `bStopAI`, `bStopMovement` or
      `bStopHeadLook` set, and only on an owner that is an AI. The Phaselock definitions set none of the three.

    No server event tests the net mode;
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `GearboxPawn`'s exec stub for `SetReplicatedSMData` calls one slot of the pawn's `SpecialMoveInterface` table. The
    same slot is the one the component's start and stop steps call on the authority. That ties the unnamed hook in
    the Play note to this native, by the code;
  - that slot's body is the same for `GearboxPawn`, `WillowPawn`, `WillowPlayerPawn` (Maya) and `WillowAIPawn` (link 9's
    target). The tables were found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - the component's start step and stop step, re-read for the order around the call;
  - field names: `tools/ghidra/class_layout.py` (the struct, and the `Actor` bool `bNetDirty`).

## 2. What SetReplicatedSMData does (GearboxPawn and the three subclasses read)

Given a `SpecialMoveData` (the component passes its `CurrentSMData`), it changes the pawn's record and one flag, and
nothing else. The record's `SMD`, `Data`, `PlayRateScale` and `Duration` take the move's values of the same names. The
record's byte counter `DirtyCounter` goes up by one and wraps from 255 to 0. The pawn's `bNetDirty` bool becomes true.

The callback name and object are not copied. There is no net-mode test, no role test (the callers test the role), no
event, and no replication call: setting `bNetDirty` is all. On a standalone authority the record is therefore written
and never read back (`ReplicatedEvent` does not run there).

**Other implementers** (`WillowInteractiveObject` and the other classes that register their own body) were not read.

## 3. Where the component calls it

**The start** (the Play note, section 1 step 4, in the game's order, read again here):
1. a current move is stopped first, interrupted, with the queue cleared (section 4 below; on the authority that stop
   writes the record too);
2. **with no `Owner`, the start ends here and answers 0.** The stop in 1 has already happened;
3. the request becomes `CurrentSMData` (all six fields), and `StartTime` = the world time;
4. the definition's `NextSpecialMove` chain is queued (the Play note);
5. the owner interface's `GetSkelMesh` is asked. Its answer is not used (as read);
6. **only when the component's `Owner` has `Role` = `ROLE_Authority`:** `SetReplicatedSMData(CurrentSMData)`, then the
   definition's `ServerStarted(SMI)` event;
7. `ClientStarted(SMI, CurrentSMData)`, whose answer is the duration. 0 or less stops the move again, not interrupted,
   with the queue cleared.

**The stop** (the Play note, section 4), when a move is current, the owner interface is set and the component has an
`Owner`:
1. the queue is emptied when asked;
2. the current move is cleared by setting **only its `SMD`** to None. Its other fields keep the stopped move's values;
3. **only when the component's `Owner` has `Role` = `ROLE_Authority`:** the definition's `ServerFinished(SMI,
   bInterrupted)` event, then `SetReplicatedSMData(CurrentSMData)`. So the record gets `SMD` None, the stopped move's
   `Data`, `PlayRateScale` and `Duration`, and another `DirtyCounter` step;
4. `ClientFinished`, the callback, then the queue's first entry (NATIVE_SPECIAL_MOVE_QUEUE.md section 3).

**So a start over a running move steps `DirtyCounter` twice:** once for the interrupting stop, once for the start.

**The role used** is the component's `Owner`, not the actor `SMI` holds. These are the same actor for a pawn's own
component; NATIVE_SPECIAL_MOVE_QUEUE.md section 5 already refuses a mismatch on its path.

## 4. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Record: this note, `docs/verification/NATIVE_SET_REPLICATED_SMDATA.md`. Reference
is unchanged.

**A record write, not a script native.** No script calls `SetReplicatedSMData`. Bind nothing for it. Give
`ow-skills`' `action_skill::special_move` an internal step, "write the owner's replicated record", used by `Play`'s
start and by the stop. Binding the script native too would be harmless, but nothing reaches it.

**Which owners.** Walk up from the `SMI` actor's class to its first native class (the fitted `CLASS_NATIVE` bit, as
`ow-combat`'s `pawn_interface` read-only walk does). The record is read for `GearboxFramework.GearboxPawn`,
`WillowGame.WillowPawn`, `WillowGame.WillowPlayerPawn` and `WillowGame.WillowAIPawn`. Any other first native class is
a `NotYet` of kind `NotRead`, "SpecialMoveComponent: the replicated record of <class>".

**Two actors, two roles.** The **component's `Owner`** is the role source: the game tests its `Role` for the record
hook. The **actor `SMI` holds** receives the record. For the hook the comparison is **exactly `ROLE_Authority`**. The
local-authority gate (`IsLocalAuthority`) keeps its "at least" comparison, as now.

**Preflight on a direct `Play`, write-free.** Loaded-only, in this order:
1. **first read:** the component, its `Owner`, the `SMI` actor and the request's definition must be resident (by name
   lookup on resident objects, as NATIVE_SPECIAL_MOVE_QUEUE.md section 5 does). A non-resident one is a `NotYet` of
   kind `NotRead`, "SpecialMoveComponent.Play: an object the store has not loaded (a load is a write)". This comes
   before `Play` resolves the definition (`GetSMDToPlay`) or reads any role through its loading helpers, so those
   helpers load nothing afterwards;
2. `Owner` None with `SMI` set: a `NotYet` of kind `NotRead`, "SpecialMoveComponent.Play: a component with an owner
   interface and no Owner". `Owner` not the `SMI` actor: the same kind, "SpecialMoveComponent.Play: a component whose
   Owner is not its owner interface's actor (the role is read from Owner, the record goes to SMI)";
3. Play's existing refusals, in their order: the network-client forward, `IsLocalAuthority`, `NextSpecialMove`, and the
   stop's callback shapes;
4. **only when the `Owner`'s `Role` is exactly `ROLE_Authority`**, replacing today's replication `NotRead`:
   - the class rule above;
   - `ReplicatedSMData`'s declared type is exactly `ReplicatedSpecialMoveData` (the struct the store has resident, by
     identity). Its fields are declared with these kinds: `SMD` an object of `SpecialMoveDefinition`, `Data` an
     object, `PlayRateScale` and `Duration` floats, `DirtyCounter` a byte. `bNetDirty` is declared as a bool.
     Otherwise a `VmError::Script` naming the field;
   - the server event the commit will call (`ServerStarted`, and on a stop `ServerFinished`) resolves on the
     definition's class. A lookup that fails is a `VmError::Script`, decided here and not after the commit.

Every refusal in this list is a **preflight** refusal: nothing is written, loaded or counted.

**Commit, at the game's points:**
- **start:** after `CurrentSMData` and `StartTime` are written and before `ClientStarted`: the record write (section 2),
  then the definition's `ServerStarted(SMI)` script event, run as script. Asking `GetSkelMesh` and dropping its answer
  is optional: as read it changes nothing;
- **stop, on the authority:** clear **only** `CurrentSMData.SMD`, keeping its other five fields, so that
  `ServerFinished` observes what the game's event observes. Then run the definition's `ServerFinished(SMI,
  bInterrupted)` event. **After** the event, copy the record from `CurrentSMData` as it then stands (section 2). Do not
  zero `CurrentSMData` and rebuild the record from saved values. The port's stop zeroes the whole struct today, and
  that must change for this path;
- a refusal raised after the commit by a server event (for example an unbound native inside `OnServerStart`) is
  terminal and carries "stopped after partial writes", as `Play`'s later refusals already do.

**No "standalone, so skip" rule.** The game writes the record on every authority, standalone included, so the port
writes it too.

**What link 9 would meet** (a conditional prediction, not a run). The runner's latest probe (main + #682) stops at
`Actor.SetPhysics` before any of this is reached. Once that is answered:
- this record write is reached only by a `Play` that passes the earlier gates. `AuthorityCanPlay` can refuse first,
  for example through `GetSMNode`'s missing-tree `LayerAbsent`;
- for a `Queue` call, only case "nothing current" reaches `Play` (NATIVE_SPECIAL_MOVE_QUEUE.md). A current move takes
  the append path, which writes no record. The caller that note inferred, `Action_FollowPath.SetSpecialMoves`, is the
  analyst's inference;
- if a start does reach it, the record is written and `ServerStarted` runs. `GearboxAnimDefinition.ServerStarted`
  reaches `BehaviorKernel.IntializeBehaviorProviderForConsumer` and `OnServerStart` for a consumer owner, and those are
  the next candidates.

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each **preflight** refusal leaves #518's three counters and the name table
unchanged:
1. a start on an authority owner (a toy subclass of `WillowPlayerPawn` with no native code of its own) with nothing
   current. `ReplicatedSMData` = the request's `SMD`, `Data`, `PlayRateScale` and `Duration`; `DirtyCounter` 0 → 1;
   `bNetDirty` set; then `ServerStarted` runs (a spy event on a toy definition), then `ClientStarted`. Order:
   record, `ServerStarted`, `ClientStarted`;
2. a start over a running move: `DirtyCounter` steps twice. The first write is the interrupting stop's, with `SMD` None
   and the old move's `Data`, rate and duration; the second is the new move's;
3. `DirtyCounter` 255 → 0 on one write;
4. a stop on the authority, with a spy `ServerFinished` on a toy definition. **During** the event, `CurrentSMData`
   has `SMD` None and the stopped move's other five fields, and the record still holds its old values. **After** the
   event, the record has `SMD` None and the stopped move's `Data`, rate and duration, with the counter stepped;
5. a direct `Play` (not `PlayLocal`, which calls `ServerStarted` separately) in a non-client world. The owner is at
   `ROLE_AutonomousProxy` and the definition sets `bClientHasAuthority`, so `IsLocalAuthority` passes: the move starts,
   with no record write and no server event;
5a. preflight order: a lazy `Owner`, `SMI` actor or definition gives the residency `NotRead` before anything is
    resolved. An `Owner` that differs from the `SMI` actor gives the mismatch `NotRead`. Neither loads or writes
    anything;
5b. a post-commit failure: a toy `ServerStarted` that reaches an unbound native. The refusal carries "stopped after
    partial writes", and the record and `CurrentSMData` keep what the commit wrote. This case has its own assertion,
    not the "unchanged" one;
6. an `SMI` actor whose first native class is not one of the four (a toy native class): the class `NotRead`, before any
   write;
7. `ReplicatedSMData` of another struct, a missing field, `SMD` or `Data` not an object, `PlayRateScale` or
   `Duration` not a float, `DirtyCounter` not a byte, `bNetDirty` not a bool, or a definition class with no
   `ServerStarted`: `VmError::Script` naming it, before any write;
8. the callback name and object are not copied into the record;
9. Reference: unchanged.

## Open questions

1. Not read: the other owner classes' bodies (`WillowInteractiveObject` and the rest);
2. Not read: `PlayReplicated`, the client side, which never runs standalone;
3. UNVERIFIED in game: everything above. In particular: that the record keeps the stopped move's `Data`, rate and
   duration with `SMD` None; that a start over a running move steps the counter twice; and that `GetSkelMesh`'s answer
   is unused.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. In a single-player game, read Maya's `ReplicatedSMData` before and after a Phaselock cast: expect `SMD` the cast's
   definition and `DirtyCounter` one higher. After the animation ends, expect `SMD` None, the same `Data`, rate and
   duration, and the counter one higher again.
2. Cast while another special move plays: expect the counter two higher after the cast starts.
