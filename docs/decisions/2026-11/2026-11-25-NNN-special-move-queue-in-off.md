## 2026-11-25: SpecialMoveComponent.Queue in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPECIALMOVE-QUEUE, slice link 9. The file number is a placeholder
until the landing run numbers it. Coded from docs/verification/NATIVE_SPECIAL_MOVE_QUEUE.md section 5 alone (#680 @
8e954320, decision 2026-11-25-15, still in review when this was written). Clean room: this lane did not read the
native, Ghidra or REA output, or `local/analysis/`. Synthetic fixtures only.

- **What changes:** `ow-skills` `action_skill/special_move.rs` binds
  `GearboxFramework.SpecialMoveComponent.Queue` in `ParityMode::Off` beside `Play` (added to `PATHS`; first binding
  wins, as before). Reference binds nothing, as for the other special-move natives.
- **The preflight, write-free, in the note's order:**
  1. no definition, or the owner interface (`SMI`) has no object: nothing;
  2. an owner interface and no component `Owner`: `NotYet { NotRead }`;
  3. the component `Owner`'s `Role` below `ROLE_Authority`: `NotYet { NotRead }` (the forward to the server is network
     play, not ported);
  4. `IsLocalAuthority` on the definition as given (not resolved through `GetSMDToPlay`), false: nothing;
  5. `CurrentSMData` must be a struct and `SMDQueue` an array of that struct, else a `VmError::Script` naming the
     field.
- **Loaded-only reads (the hub's note on #680's review):** the component, the `Owner`, the definition and the owner
  interface's object must be resident before anything of theirs is read. Resident means not a handle the store has not
  loaded, and not a stand-in with no slots, the same rule as `Actor.Trace`'s residency check. Otherwise a
  `NotYet { NotRead }` naming it, with nothing loaded.
- **Then:**
  - **nothing current:** the port's own `Play` path is called directly, with the same arguments except `Data`, which
    is passed as None as the note reads the game doing. Its answer is dropped, and its refusals stand as they are,
    with its own record. Queue writes nothing before it, so a refusal in `Play`'s preflight carries no
    partial-writes prefix;
  - **a current move:** one `SpecialMoveData` is appended at the end of `SMDQueue`, every field as given (`Data`
    included) and `SMD` the definition as given. That is the only write.
- **Owner and `SMI`:** the forward test reads the component's `Owner`, and the authority gate reads the owner
  interface's object, as the note separates them. The port states no invariant that they are the same actor. A test
  uses an `Owner` at authority with an autonomous-proxy `SMI` so that `Play`'s path starts. Whether the game ever has
  them differ is not read.
- **Not done:** draining the queue when a move stops (the note's section 3), and `IsQueued`. An entry queued here
  waits in Off.
- **Verified (host only):** one test for each case in the note's section 6, on invented packages. Every refusal
  leaves the component's state, #518's three counters and the name table unchanged, with no event:
  - no definition and no owner interface: nothing;
  - no `Owner`: refused;
  - the forward refused with and without a current move, and before the authority gate for an autonomous proxy with
    `bClientHasAuthority`;
  - two appends of the same definition, all six fields as given, with no event;
  - nothing current on the authority: `Play`'s replication-record `NotRead` stands, with no prefix and the queue
    unchanged;
  - the delegation that starts: the started move's `Data` is None, its other fields as given, and the queue emptied;
  - the two shape errors;
  - a definition that is not resident: refused without a load.

  Reference stays unbound (`reference_binds_nothing` walks `PATHS`). Two mutation checks: passing the caller's
  `Data` to `Play`, and moving the forward test after the gate, each fail their test.
- **Not verified:** nothing in the running game; every rule is **UNVERIFIED**. The installed slice gate was not run
  (no install yet). By the note's prediction, link 9's next stop is `Play`'s replication refusal, not a pass; not
  measured.
