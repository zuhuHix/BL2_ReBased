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
  - field names: `tools/ghidra/class_layout.py` (the struct, and the `Actor` bool word).

## 2. What SetReplicatedSMData does (GearboxPawn and the three subclasses read)

Given a `SpecialMoveData` (the component passes its `CurrentSMData`), it writes on the pawn, and nothing else:
1. `ReplicatedSMData.SMD` = the move's `SMD`;
2. `ReplicatedSMData.Data` = the move's `Data`;
3. `ReplicatedSMData.PlayRateScale` = the move's `PlayRateScale`;
4. `ReplicatedSMData.Duration` = the move's `Duration`;
5. `ReplicatedSMData.DirtyCounter` += 1, as a byte (255 wraps to 0);
6. the pawn's `bNetDirty` (an `Actor` bool, bit 19 of its word) set.

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

**Preflight, write-free, decided where Play's replication refusal stands today** (before `AuthorityCanPlay`, reads
loaded-only as in NATIVE_SPECIAL_MOVE_QUEUE.md section 5). Only on an `Owner` at `ROLE_Authority`:
1. the class rule above;
2. `ReplicatedSMData` declared as a `ReplicatedSpecialMoveData` struct holding the five fields with their kinds
   (`DirtyCounter` a byte), and `bNetDirty` declared as a bool. Otherwise a `VmError::Script` naming the field.

These checks replace the replication `NotRead`. Play's other refusals stay as they are, in their order. That includes
the `NextSpecialMove` one, and the stop's callback shapes.

**Commit, at the game's points:**
- **start:** after `CurrentSMData` and `StartTime` are written and before `ClientStarted`: the record write (section 2),
  then the definition's `ServerStarted(SMI)` script event, run as script. Asking `GetSkelMesh` and dropping its answer
  is optional: as read it changes nothing;
- **stop, on the authority:** after `CurrentSMData.SMD` is cleared, the definition's `ServerFinished(SMI,
  bInterrupted)` event, then the record write with that half-cleared `CurrentSMData`. The port's stop zeroes the whole
  `CurrentSMData` today. For the record to match the game, it must keep the other five fields of the stopped move and
  clear only `SMD`, or write the record from the old values with `SMD` None. The second keeps the rest of the stop as
  it is; it is the implementer's choice, and either way the record must equal the game's;
- a refusal raised after the commit by a server event (for example an unbound native inside `OnServerStart`) is
  terminal and carries "stopped after partial writes", as `Play`'s later refusals already do.

**No "standalone, so skip" rule.** The game writes the record on every authority, standalone included, so the port
writes it too.

**What link 9 would meet next** (a prediction, not a run). On Maya's `Play` of the Phaselock definition, the record is
written and then `ServerStarted` runs. `GearboxAnimDefinition.ServerStarted` reaches
`BehaviorKernel.IntializeBehaviorProviderForConsumer` and `OnServerStart` for a consumer owner. They are the next
candidates, then `ClientStarted`'s natives (the Play note's section 6 list).

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. a start on an authority owner (a toy subclass of `WillowPlayerPawn` with no native code of its own) with nothing
   current. `ReplicatedSMData` = the request's `SMD`, `Data`, `PlayRateScale` and `Duration`; `DirtyCounter` 0 → 1;
   `bNetDirty` set; then `ServerStarted` runs (a spy event on a toy definition), then `ClientStarted`. Order:
   record, `ServerStarted`, `ClientStarted`;
2. a start over a running move: `DirtyCounter` steps twice. The first write is the interrupting stop's, with `SMD` None
   and the old move's `Data`, rate and duration; the second is the new move's;
3. `DirtyCounter` 255 → 0 on one write;
4. a stop on the authority: `ServerFinished(SMI, bInterrupted)` before the record write. The record has `SMD` None and
   the stopped move's other three fields;
5. an owner below `ROLE_Authority` (no network client): no record write and no server event. The rest of `Play` is as
   today;
6. an `SMI` actor whose first native class is not one of the four (a toy native class): the class `NotRead`, before any
   write;
7. `ReplicatedSMData` of another struct, a missing field, `DirtyCounter` not a byte, or `bNetDirty` not a bool:
   `VmError::Script` naming it, before any write;
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
