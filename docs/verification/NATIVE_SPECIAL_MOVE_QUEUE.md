# SpecialMoveComponent.Queue: start now or wait behind the current move (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SPECIALMOVE-QUEUE (decision 2026-11-25-15), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/smqueue/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.** It extends
[NATIVE_SPECIAL_MOVE_PLAY.md](NATIVE_SPECIAL_MOVE_PLAY.md) (the Play note), whose terms it uses.

**Why.** A runner probe (main + #663 + #664 + #668, reported by the hub on 2026-11-25) shows slice link 9's `cast`
stopping with an error at the unbound native `SpecialMoveComponent.Queue(object,float,float,name,object,object)`. The
Play note covers Play's own queue steps, but not this native's entry.

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted, and no recorded capture holds a component's queue.
- **The script** (`research/script_disasm.py`, every script function of the nine code packages):
  - the declaration: `Queue(NewSMD, optional PlayRateScale, optional Duration, optional CallbackName, optional
    CallbackObject, optional Data)`. It returns nothing. These are the same six arguments as `Play`;
  - its three script callers:
    1. `Behavior_SpecialMove.PlaySpecialMove`. When the behavior's `bLocal` is clear and the component is already
       playing (`IsPlayingAny`) without `bStopCurrentMove`, it queues the behavior's `SpecialMove` with its
       `Duration`, but only when `bQueueIfCannotPlay` is set;
    2. the owners' `ServerSpecialMove_Queue` events (`GearboxPawn`, `WillowInteractiveObject`, `WillowPopulationPoint`,
       `WillowVehicle`). They pass all six arguments to their own component's `Queue`, on the server. No script calls
       these events: they are the network forward of section 2, step 4;
    3. `Action_FollowPath.SetSpecialMoves` (an AI action). For a move node with special moves, it stops the pawn's
       current move (`StopAny`), then queues each of the node's `SpecialMoves` in order. The last one is queued with the
       callback `SpecialMoveCallback` on the action itself. Afterwards, if anything is playing, the action waits for
       that callback;
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock): `Queue` and its body, with `Play`'s body
  beside it for comparison. Also read: `IsQueued`, `Stop`'s body, and the step that starts the next queued move after a
  stop. Field names come from `tools/ghidra/class_layout.py`:
  - `SMDQueue` is an array of `SpecialMoveData`, whose fields are `Data`, `PlayRateScale`, `Duration`,
    `CallbackName`, `CallbackObject` and `SMD`;
  - `CurrentSMData` has the same struct type.

## 2. What Queue does

**Defaults.** Omitted arguments are the same as `Play`'s: `PlayRateScale` 1, `Duration` 0, `CallbackName` None, and
both objects None.

1. **Nothing to queue.** With no definition (`NewSMD` None), or a component whose owner interface `SMI` is unset (its
   object or its interface part), nothing happens: no start, no entry, no forward.
2. **The authority gate:** the given definition's `IsLocalAuthority(SMI)`, the Play note's rule. It is asked of
   `NewSMD` itself, which is not first resolved through `GetSMDToPlay`. When it fails, nothing starts and nothing is
   queued, but step 4 still runs.
3. **Start now, or wait:**
   - **a. No current move** (`CurrentSMData.SMD` is None): the request goes through `Play`'s whole path (the Play note,
     section 1, steps 2 to 6). That path resolves `GetSMDToPlay`, checks both gates again (`IsLocalAuthority` on the
     resolved definition, then `AuthorityCanPlay`), empties the queue and starts the move. Its duration is dropped.
     **One difference: `Data` is passed as None, not the caller's `Data`.** That is read, not inferred, and it may be a
     slip in the game's code; the port should still copy it. The other five arguments pass unchanged;
   - **b. A current move**: an entry is appended at the **end** of `SMDQueue`, with all six fields as given (`Data`
     included) and the definition as given (not resolved). Nothing else is asked at that point: no `AuthorityCanPlay`,
     no duplicate check (the same definition can be queued twice), no length limit, no event. The current move is
     not touched.
4. **The network forward.** After steps 2 and 3, whatever they did: when the **component's `Owner`** has a `Role`
   below `ROLE_Authority`, the owner interface's server queue event is called with all six arguments as given. By fit
   that event is `ServerSpecialMove_Queue`, whose six arguments match. Two differences from `Play`'s forward (the Play
   note, step 6), both read:
   - there is no net-mode test (`Play` also requires a network client);
   - the `Owner` is read without being checked for None. With an owner interface set and no `Owner`, the game would
     read through a null pointer. That case does not occur in the slice.

   In the single-player slice the owner is the authority, so the forward never runs. A non-authority caller whose
   step 3a also starts the move would forward twice: once through `Play`'s path, if on a client, and once here.

**What script sees.** No return value. Afterwards, `IsQueued(NewSMD)` is true in case 3b: it answers whether any queue
entry's definition is that exact object (not its class, and not resolved), and false for None. In case 3a, `IsPlaying`
reports the started move, if it started.

## 3. When a queued move starts (the Play note's section 4, step 6, now read)

The queue is only drained when the current move stops (the Play note, section 4). After the stop's events and the
callback, and only when no move is current and the queue holds at least one entry:
1. the **first** entry is taken and removed from the queue, whatever happens next;
2. if its definition is set, and passes `IsLocalAuthority` and then `AuthorityCanPlay`, it is started through the
   start step itself, with **all its fields, `Data` included**. It is not resolved through `GetSMDToPlay`, and the
   queue is not emptied first;
3. if a gate fails, that entry is dropped and **the next entry is not tried** at this stop. The rest stay queued until
   the next move stops.

**So `Queue` is first in, first out.** The Play note's `NextSpecialMove` follow-ups go to the front instead. That is why
a chain of more than one waits in reverse order there; that reading stays UNVERIFIED and is not re-read here.

A `Play` that starts empties the queue (the Play note, step 4). A stop asked with "clear the queue" empties it too.
`IsQueued` matches definitions by identity. `RemoveFromQueue` and `ClearQueue` were not read in detail.

## 4. Which caller link 9 reaches (an inference, UNVERIFIED)

*Amended 2026-11-25 (NATIVE_ACTOR_SET_PHYSICS.md section 1).* The census in section 1 missed a fourth script caller:
**`LiftActionSkill.LiftTarget`**. On the phaselocked target it calls `Play` (the lift animation), then `Queue` (the
phase-lock definition's `LoopAnim`), then `SetPhysics(14)`. Since `Play` runs just before, the `Queue` call normally
finds a current move and takes case 3b (append), unless that `Play` started nothing. Link 9's later stop at
`Actor.SetPhysics` fits this caller. It is therefore the most likely one; the `Action_FollowPath` reading below is
**superseded as the likely caller**, though it remains a caller. Both remain inferences: no caller stack was recorded.

The original reading follows. The hub's probe record names the native, not its caller. A static reading of the
cast's world:
- the `Behavior_SpecialMove` objects in the Siren class package that the cast loads (two) do not set
  `bQueueIfCannotPlay`. Their path also asks `IsPlayingAny` first, which nothing binds in `ParityMode::Off`, so it
  would stop there, not at `Queue`;
- no script calls `ServerSpecialMove_Queue`;
- `Action_FollowPath.SetSpecialMoves` reaches `Queue` right after `StopAny`, which the death pack binds as a no-op in
  both modes (`ow-combat`'s `death::PRESENTATION_NO_OPS`).

So the caller is most likely **the target's AI following a path with a special-move node**. That is the probe's
target pawn and its mind, which link 9 spawns (lane IMPL-LINK9-TARGET-MIND). The probe's call stack would settle it.

**What link 9 would meet next** (a prediction, not a run). It depends on whether the target's `CurrentSMData.SMD`
is already None when `Queue` is called. Off's `StopAny` is a no-op, so whatever was current stays current:
- **if `CurrentSMData.SMD` is None** (nothing was ever started on the target, the likely case), the first `Queue` takes
  case 3a, `Play`'s path in the port (`ow-skills` `action_skill::special_move`). For an authority owner that path
  refuses with its existing `NotYet` of kind `NotRead` on "the record the owner keeps of the current move for
  replication". It does so before `ClientStarted`, before `CurrentSMData` or the queue is touched, and before
  `AuthorityCanPlay`, so also before `GetSMNode`'s missing-tree `LayerAbsent`. **So link 9's predicted next stop after
  `Queue` is `Play`'s replication refusal.** Queue writes nothing before delegating, so no "stopped after partial
  writes" prefix comes from Queue (one the runner adds for its own earlier writes is the runner's);
- **if a move is already current**, every entry takes case 3b, and `SetSpecialMoves` then asks `IsPlayingAny`, which is
  unbound in Off.

## 5. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `GearboxFramework.SpecialMoveComponent.Queue` in `ParityMode::Off` beside
`Play` (`ow-skills`, `action_skill::special_move`, added to its `PATHS`), with this note as record
(`docs/verification/NATIVE_SPECIAL_MOVE_QUEUE.md`). Reference is unchanged: the C++ reference binds no `Queue`, so it
stays with the stub policy.

**Loaded-only reads.** A load is a write, so Queue's preflight reads only objects the store already holds, by name
lookup on resident objects (as `bounding_cylinder`'s refusal N does). It never goes through the loading helpers
(`action_skill::get` / `need`, which load their object first). The objects it touches are the component, the `SMI`
actor, the component's `Owner` and `NewSMD`. Any of them not resident (a lazy handle, or a stand-in with no slots) is
a `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: an object the store has not loaded (a load is a write)",
raised before anything else reads it. The existing `IsLocalAuthority` helper and `Play`'s path read the same objects
through loading helpers. So Queue checks all four for residency **first**, and only then calls them; on a resident
object those helpers load nothing. `GetSMDToPlay`, which `Play`'s path calls, answers the definition itself in the
port, so it touches no further object.

**The interface value.** In the VM, `SMI` is one optional object reference (`Value::Interface`). There is no separate
native part to be unset, so "`SMI` unset" means that reference is None.

**One owner, two reads.** The game's forward test reads the **component's `Owner`**. Its `IsLocalAuthority` reads the
**actor `SMI` holds**. Nothing in the port guarantees that the two are the same actor: they agree in every fixture so
far, but no rule ties them. So the preflight checks it.

**The preflight, write-free, in the game's order.** Every refusal is decided before any write:
1. `NewSMD` None, or `SMI` None: return, and nothing changes;
2. residency, as above: the `NotYet` "an object the store has not loaded";
3. `Owner` None, with `SMI` set: a `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: a component with an owner
   interface and no Owner (the game reads its Role unchecked)". The port must not guess what a null read does;
4. `Owner` not the actor `SMI` holds: a `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: a component whose
   Owner is not its owner interface's actor (the forward reads Owner, the gate reads SMI)". The game would test two
   different actors; which one wins in practice is not read;
5. the `Owner`'s `Role` below `ROLE_Authority` (by the `Role` enum's order, as `IsLocalAuthority` already does): a
   `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: the forward to the server (ServerSpecialMove_Queue) on an
   owner below authority". Network play is not ported. The game does the authority gate and the commit first, so this
   differs only on a path that is refused anyway;
6. `IsLocalAuthority(NewSMD)` by the existing port, on resident objects only (step 2). False: return, and nothing
   changes. After steps 4 and 5, only an `SMI` actor at `ROLE_Authority` gets here, and it always passes, so this step
   cannot answer false in the port today. It stays so that a later network port keeps the game's order;
7. `CurrentSMData` must be a `SpecialMoveData` struct and `SMDQueue` an array of it, else a `VmError::Script` naming
   the field. Then read `CurrentSMData.SMD`.

**Commit:**
- **3a, nothing current:** call the port's own `Play` path with the same arguments but `Data` None, and drop its
  answer. Whatever that path refuses or answers stands as it is, with its own record. Queue writes nothing before
  delegating, so a refusal inside `Play`'s preflight carries no "stopped after partial writes" prefix. One raised after
  `Play`'s commit keeps the prefix `Play` gives it;
- **3b, a current move:** append one `SpecialMoveData` to `SMDQueue`: `Data`, `PlayRateScale`, `Duration`,
  `CallbackName`, `CallbackObject` and `SMD` = `NewSMD`, all as given. That is the only write, and nothing else
  changes. If the VM cannot write a struct element into that array, a `NotYet` of kind `LayerAbsent` says so before
  any write.

**Not part of this native, and left as it is:** draining the queue when a move stops (section 3). The port's stop
currently only empties the queue, which matches the "clear the queue" stops `Play` uses. A stop that keeps the queue
and starts its head is a separate implementation from section 3, with its own tests. Until that exists, an entry
queued by 3b waits forever in Off. That is safe, because nothing in the slice reads it back except `IsQueued`, which is
not bound either. `IsQueued(SMD)` (section 2) is a cheap read-only companion, if the implementer wants it in the same
lane.

## 6. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. `NewSMD` None, and `SMI` None (one test each; the VM's interface value is a single optional reference, so there is
   no separate native part to unset): nothing changes, no refusal, no event;
2. an owner with `SMI` set and no `Owner`: the `Owner` `NotRead`, nothing written;
2a. unloaded input: the component's `Owner`, the `SMI` actor, or `NewSMD` as a lazy handle (one test each): the
    "an object the store has not loaded" `NotYet`. #518's three write counters and the name table are unchanged,
    nothing is loaded, and the text carries no "stopped after partial writes" prefix;
2b. `Owner` and the `SMI` actor two different resident actors, both at `ROLE_Authority`: the mismatch `NotRead`,
    nothing written;
3. an owner below `ROLE_Authority`: the forward `NotRead`, nothing written, even when a move is current;
4. an owner at `ROLE_AutonomousProxy` with a definition that sets `bClientHasAuthority`: test 3's forward `NotRead`
   comes first, and nothing is queued. This pins the order of preflight steps 3 and 4;
5. a current move, authority: one entry appended with all six fields as given, `Data` included. A second `Queue` of the
   same definition appends a second entry after the first. `CurrentSMData`, `StartTime` and the existing entries are
   unchanged, and no definition event runs;
6. no current move, authority, split in two because Off's `Play` refuses every authority start before
   `ClientStarted`, `CurrentSMData` or the queue clear, so no successful immediate start can be observed today:
   - **6a, integration:** `Queue` on an authority owner with `CurrentSMData.SMD` None and one entry already in
     `SMDQueue` answers `Play`'s replication-record `NotRead` unchanged (its text and record), with no "stopped after
     partial writes" prefix. `SMDQueue` still holds that entry, `CurrentSMData` is unchanged, and the three counters
     and the name table are unchanged;
   - **6b, delegation:** a focused test of the seam Queue uses to delegate (for example an inner function taking the
     request) captures the arguments `Play`'s path receives: the five as given, and `Data` None even when the caller
     passed a `Data` object;
7. `CurrentSMData` not a struct, or `SMDQueue` of another element type: `VmError::Script` naming it, nothing written;
8. Reference: unchanged (unbound).

## Open questions

1. Not read: the record the owner keeps of the current move for replication (the owner interface's native hook that
   `Play`'s start and the stop call on the authority). It blocks `Play` in Off for every authority owner, and with it
   Queue's case 3a;
2. Not read: `StopAny` (bound as a no-op in Off) and whether it empties the queue;
3. which caller link 9 reaches (section 4), and whether the target's mind runs `Action_FollowPath` in the probe's world;
4. UNVERIFIED in game: everything above, including the dropped `Data` of case 3a.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. While a pawn plays a long move, call `Queue` twice with the same definition and different `Data`. Read `SMDQueue`:
   expect two entries in call order with their own `Data`, and the current move untouched.
2. With nothing playing, call `Queue` with a `Data` object, then read `CurrentSMData.Data`: expect None.
3. Let the first move end, and read the queue and the current move: expect the first entry playing with its `Data`, and
   the second still queued.
