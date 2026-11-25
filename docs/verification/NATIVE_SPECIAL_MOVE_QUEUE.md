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

The hub's probe record names the native, not its caller. A static reading of the cast's world:
- the `Behavior_SpecialMove` objects in the Siren class package that the cast loads (two) do not set
  `bQueueIfCannotPlay`. Their path also asks `IsPlayingAny` first, which nothing binds in `ParityMode::Off`, so it
  would stop there, not at `Queue`;
- no script calls `ServerSpecialMove_Queue`;
- `Action_FollowPath.SetSpecialMoves` reaches `Queue` right after `StopAny`, which the death pack binds as a no-op in
  both modes (`ow-combat`'s `death::PRESENTATION_NO_OPS`).

So the caller is most likely **the target's AI following a path with a special-move node**. That is the probe's
target pawn and its mind, which link 9 spawns (lane IMPL-LINK9-TARGET-MIND). The probe's call stack would settle it.

**What link 9 would meet next** (a prediction, not a run). Since `StopAny` stopped nothing, the target's component has
no current move, so the first `Queue` takes case 3a: `Play`'s path in the port (`ow-skills`
`action_skill::special_move`). For an authority owner that path refuses before any write, with its existing `NotYet` of
kind `NotRead` on "the record the owner keeps of the current move for replication". That refusal comes before
`AuthorityCanPlay`, and so before `GetSMNode`'s missing-tree `LayerAbsent`. With no write before it, no "stopped after
partial writes" prefix is expected. If instead the target already had a current move, every entry takes case 3b.
`SetSpecialMoves` then asks `IsPlayingAny`, which is unbound in Off.

## 5. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `GearboxFramework.SpecialMoveComponent.Queue` in `ParityMode::Off` beside
`Play` (`ow-skills`, `action_skill::special_move`, added to its `PATHS`), with this note as record
(`docs/verification/NATIVE_SPECIAL_MOVE_QUEUE.md`). Reference is unchanged: the C++ reference binds no `Queue`, so it
stays with the stub policy.

**The preflight, write-free, in the game's order.** Every refusal is decided before any write:
1. `NewSMD` None, or `SMI` unset: return, and nothing changes;
2. `Owner` None, with `SMI` set: a `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: a component with an owner
   interface and no Owner (the game reads its Role unchecked)". The port must not guess what a null read does;
3. the `Owner`'s `Role` below `ROLE_Authority` (by the `Role` enum's order, as `IsLocalAuthority` already does): a
   `NotYet` of kind `NotRead`, "SpecialMoveComponent.Queue: the forward to the server (ServerSpecialMove_Queue) on an
   owner below authority". Network play is not ported. The game does steps 2 and 3 first, so this differs only on a
   path that is refused anyway;
4. `IsLocalAuthority(NewSMD)` by the existing port. False: return, and nothing changes. After step 3 only owners at
   `ROLE_Authority` get here, and they always pass it, so this step cannot answer false in the port today. It stays,
   so that a later network port keeps the game's order;
5. `CurrentSMData` must be a `SpecialMoveData` struct and `SMDQueue` an array of it, else a `VmError::Script` naming
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
1. `NewSMD` None, and `SMI` unset (object None, and interface part None): nothing changes, no refusal, no event;
2. an owner with `SMI` set and no `Owner`: the `Owner` `NotRead`, nothing written;
3. an owner below `ROLE_Authority`: the forward `NotRead`, nothing written, even when a move is current;
4. an owner at `ROLE_AutonomousProxy` with a definition that sets `bClientHasAuthority`: test 3's forward `NotRead`
   comes first, and nothing is queued. This pins the order of preflight steps 3 and 4;
5. a current move, authority: one entry appended with all six fields as given, `Data` included. A second `Queue` of the
   same definition appends a second entry after the first. `CurrentSMData`, `StartTime` and the existing entries are
   unchanged, and no definition event runs;
6. no current move, authority: the port's `Play` path runs with `Data` None (spy on what `ClientStarted` receives, or
   on what `CurrentSMData.Data` holds after a start). The queue is emptied by that path, and `Play`'s own refusals pass
   through unchanged (for the slice: the replication-record `NotRead`, with no "stopped after partial writes" prefix);
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
