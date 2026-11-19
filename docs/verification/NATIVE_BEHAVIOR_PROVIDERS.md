# Native behavior providers: `BeginNondeterministicProviderRegistration` and `ForceUnregisterBehaviorConsumer` (2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst part of lane FIX-PHASELOCK-CAST part A, decision 2026-11-19-02.
Read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, the saved project), written in our own
words. No decompiler output, pseudo-code or listing structure is reproduced here, and the raw output stays under
ignored `local/analysis/`. **Every rule is UNVERIFIED in the running game.** Nothing here was checked live. Function
flags come from the package UFunction list (`tools/ghidra/script_natives.py`). Related notes:
[NATIVE_BEHAVIOR_CONTEXT.md](NATIVE_BEHAVIOR_CONTEXT.md) (the `BehaviorConsumerHandle` struct, one `int PID`, invalid
value -1), [NATIVE_ACTION_SKILL.md](NATIVE_ACTION_SKILL.md) (`Skill.Activate` step 2 and `Skill.Deactivate`, the
callers on the Phaselock path), decision 2026-11-16-50 (the kernel pack, which lists both natives as unbound).

## Summary

Both natives take the consumer handle by reference and look up the kernel's consumer record for its `PID`.

- **`BeginNondeterministicProviderRegistration`** changes no script-visible value. It only tells the kernel that
  providers registered on this consumer from now on get no replication slot number (section 2). In a single-player
  run that is the whole effect, as far as this note can see. **Off rule: a no-op that returns, once the handle has
  been checked** (section 4).
- **`ForceUnregisterBehaviorConsumer`** tears the consumer down at once: it marks the record unregistered, stops every
  running thread of the consumer, drops its queued deferred-provider entries, and then **sets the caller's handle
  `PID` to -1**. The -1 write happens whether or not the handle named a live consumer.

| Native | Script shape | Slice use | Confidence | Status |
|---|---|---|---|---|
| `BehaviorKernel.BeginNondeterministicProviderRegistration` | static final native, one by-reference `BehaviorConsumerHandle` (the function has the out-parameters flag), no return value written | `Skill.Activate` step 2, right after `RegisterBehaviorConsumer` (Phaselock's cast) | high (what it writes), medium (what the slot number is for) | UNVERIFIED |
| `BehaviorKernel.ForceUnregisterBehaviorConsumer` | same shape | `Skill.Deactivate`, last step (Phaselock's end) | high (handle write, state mark, thread stop), medium (the queue and the kernel-wide sweep) | UNVERIFIED |

The parameter's script name was not read. Both are bound by name in `UBehaviorKernel`'s native table, next to
`RegisterBehaviorConsumer` and `IntializeBehaviorProviderForConsumer`.

## 1. The consumer record (what both natives touch)

`RegisterBehaviorConsumer` (the one decision 2026-11-16-50 already models) fills one small record per consumer, found by
`PID`. The fields this note needs, by meaning:

- the consumer object and the record's own `PID`;
- a **state byte**: 0 = no consumer (a free or never-used slot), 2 = registered, 1 = unregistered. Registration
  writes 2;
- a **slot counter byte**, which registration sets to 0;
- a **"gets slot numbers" flag**, which registration sets from a question it asks the consumer object (a virtual call on
  the consumer's interface; which question it is was not read);
- a **deferred-provider count byte** (how many entries of this consumer wait in a kernel-wide queue of deferred
  provider registrations).

**Lookup.** A handle whose `PID` is negative, at or past the kernel's process count, or names an unused slot, finds an
empty record whose state byte is 0. Both natives then do nothing to the kernel (but see section 3, step 4 for the handle).

**"The kernel exists."** Both natives, and the kernel calls they make, first check that the engine has a behavior
kernel. If it has none, the native does nothing at all, including the -1 write in section 3, step 4. In our port the kernel pack is
always there once it is registered, so this check is always true.

## 2. BeginNondeterministicProviderRegistration

1. Read the handle (by reference) and finish the call's parameters.
2. If the kernel exists and the handle's record has a state other than 0, **set the record's slot counter to 32**.
3. Nothing else. The handle is not written and no value is returned.

**What the counter is for.** When the kernel initializes a provider on a consumer (inside
`IntializeBehaviorProviderForConsumer`, the "load the provider" path), it gives each of the provider's sequences a
**slot number**:

- if the consumer's "gets slot numbers" flag is clear, the number is 32;
- otherwise it is the consumer's current slot counter, and the counter goes up by one while it is below 32.

So a consumer numbers the sequences of its first providers 0, 1, 2 and so on, up to a cap of 32. 32 means "no slot".
After `BeginNondeterministicProviderRegistration`, every later sequence on that consumer gets 32. The slot number is
stored with each new process record of the sequence. The name of the native and the `ProcessReplicated*` natives next
to it suggest the number pairs processes between server and client, for providers created in the same order on both.
"Nondeterministic" providers (registered later, maybe in a different order) then get no pairing. **That the number is
used only for replication is an inference.** Its readers were not traced beyond that write (open question 1).

## 3. ForceUnregisterBehaviorConsumer

1. Read the handle (by reference) and finish the call's parameters.
2. If the kernel does not exist, stop: nothing happens, and the handle is not written.
3. Look up the handle's record. If its state is not 0, **unregister** it. That step does nothing if the state is
   already 1; otherwise:
   1. state := 1 (unregistered);
   2. the record's `PID` goes onto a kernel-wide list of consumers to release. When that list is drained, and
      whether the `PID` can be handed out again before then, was not read (open question 2);
   3. if the kernel's "current consumer" `PID` is this one, it is reset to -1;
   4. **every running thread whose consumer is this `PID` is stopped**: its per-thread state slot is released, and
      the thread is marked finished (thread state 2), so it does not run again;
   5. if the record's deferred-provider count is not 0, every entry of this `PID` is removed from the deferred queue,
      and the count is set to 0 (in every case of this branch);
   6. a kernel-wide sweep runs over the kernel's provider and process lists (what it collects was not read in
      detail: open question 3).
4. **Set the caller's handle `PID` to -1**, whatever step 3 did. A handle that already named no consumer also comes
   back -1.

Because the parameter is by reference, the -1 lands in the script variable the caller passed (for `Skill.Deactivate`,
the skill's own consumer handle). A second call with that handle then finds no consumer and only writes -1 again.

## 4. For the implementer: the Off rules (`ow-behavior` kernel pack)

Implement from this section and section 1. The kernel pack's terms (process, `PID`, owner, thread) are decision
2026-11-16-50's.

- **`BeginNondeterministicProviderRegistration`, Off.** Return with no value. If the handle's `PID` is a process the
  kernel registered, it may record "providers from now on get no slot" on that process. Our kernel has no slot numbers
  and no replication, so leaving out the record is allowed as long as nothing reads it. A `PID` that is -1, unknown or
  owned by another crate: return, no error (the native does nothing for an empty record). Do not write the handle.
- **`ForceUnregisterBehaviorConsumer`, Off.** For a `PID` the kernel registered: stop the process's running threads
  (they do not run again), drop its pending events or deferred work for that `PID`, and mark it unregistered so later
  kernel calls on it find no consumer. Then **write -1 into the caller's handle variable** (the out parameter), in
  every case, including an unknown or -1 `PID`. A `PID` another crate owns (an `Owner::Crate`): routing it to that
  owner, or answering `NotYet`, is the implementer's call. Do not guess a teardown for a consumer the kernel does not
  hold.
- **`PID` reuse after unregistering is not read** (open question 2). Until it is, the implementer should keep a
  released `PID` out of the allocator, or answer `NotYet` on a reuse that would change a result, and not invent a rule.
- Both rules are the same in Reference. The C++ binds neither, so Reference may keep the stub (decision 2026-11-16-50).

Synthetic tests to write: Begin on a registered `PID`, on -1 and on an unknown `PID` (all return, handle unchanged);
Force on a registered `PID` with a running thread (the thread does not run again, the handle variable reads -1, a
second Force only writes -1); Force on -1 and on an unknown `PID` (handle -1, nothing else changes).

## Open questions (analyst)

1. Who reads a sequence's slot number (32 = none), and whether anything but the replication path does. If something
   single-player reads it, Begin's Off no-op is wrong.
2. When the list of consumers to release is drained, and whether a released `PID` can be given out again (it links to
   D50's open item "the allocator is not read").
3. What the kernel-wide sweep at the end of unregistering collects.
4. Which question registration asks the consumer to set the "gets slot numbers" flag.

## How this was read

`tools/ghidra/run.ps1 -ReadOnly -Query` on the two natives, `RegisterBehaviorConsumer` and
`IntializeBehaviorProviderForConsumer`, with callees to depth 2, then a follow-up query on the helpers they call. About
40 s of Ghidra in total. One headless run at a time, and no other Ghidra or REA session was running on this machine.
REA was not used: its warm-up on this executable is about 20 minutes, and the saved project already answered these
questions.
