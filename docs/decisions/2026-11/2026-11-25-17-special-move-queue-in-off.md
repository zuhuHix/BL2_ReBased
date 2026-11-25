## 2026-11-25: SpecialMoveComponent.Queue in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPECIALMOVE-QUEUE, slice link 9.
Coded from docs/verification/NATIVE_SPECIAL_MOVE_QUEUE.md section 5 alone, at its review-r1 text (#680 @ 7dcb5cd,
decision 2026-11-25-15). The first version followed 8e954320; review r1 of #682 brought it to 7dcb5cd. Clean room:
this lane did not read the native, Ghidra or REA output, or `local/analysis/`. Synthetic fixtures only.

- **What changes:** `ow-skills` `action_skill/special_move.rs` binds
  `GearboxFramework.SpecialMoveComponent.Queue` in `ParityMode::Off` beside `Play` (added to `PATHS`; first binding
  wins, as before). Reference binds nothing, as for the other special-move natives.
- **The preflight, write-free, in the note's order.** Each refusal is `NotYet { NotRead }` with the note's text:
  1. no definition, or `SMI` None: nothing. The component is checked resident before its `SMI` is read;
  2. residency: the component's `Owner` (when set), the definition and the `SMI` actor must be resident, else "an
     object the store has not loaded". Resident means not a lazy handle and not a stand-in with no slots, as
     `Actor.Trace`'s check. The loading helpers (`need`, the `IsLocalAuthority` port, `Play`'s path) only ever read
     objects that passed this, so they load nothing;
  3. no component `Owner`: refused;
  4. an `Owner` that is not the `SMI` actor: refused (the forward reads one, the gate reads the other);
  5. the `Owner`'s `Role` below `ROLE_Authority`: refused (the forward to the server is network play, not ported);
  6. `IsLocalAuthority` on the definition as given (not resolved through `GetSMDToPlay`), false: nothing;
  7. `CurrentSMData` must be a struct and `SMDQueue` an array of that struct, else a `VmError::Script` naming the
     field.
- **Then:**
  - **nothing current:** the port's own `Play` path is called directly. Its arguments come from one seam,
    `play_args`: the first five as given (an omitted one stays omitted), and `Data` as None, as the note reads the
    game. Its answer is dropped, and its refusals stand as they are, with its own record. Queue writes nothing before
    it, so a refusal in `Play`'s preflight carries no partial-writes prefix;
  - **a current move:** one `SpecialMoveData` is appended at the end of `SMDQueue`, every field as given (`Data`
    included) and `SMD` the definition as given. That is the only write.
- **Cold class code (review r2 of #682):** on the delegated path, `Play`'s first step dispatches `GetSMDToPlay` on
  the definition by a lookup that loads the class's code when it is cold (it registers functions and interns names).
  So Queue first asks `Vm::find_loaded_method` and refuses `NotYet { NotRead }` when the definition's class code is not
  loaded. With the code loaded, `Play`'s own lookup loads nothing, so `Play`'s shared logic is unchanged. A test uses a
  resident definition of a class nothing warmed: refused, its class code still cold. #518's counters alone would not
  catch the load (without the check they stay unchanged while the class code loads); the test checks the class code.
- **Owner and `SMI`:** the port now requires them to be the same actor (step 4). The first version did not, and
  review r1 of #682 found the hole: an `Owner` at authority with an autonomous-proxy `SMI` actor passed the forward
  test, and `Play`, reading `SMI`, then committed a start. Which actor the game would follow when they differ is not
  read.
- **Not done:** draining the queue when a move stops (the note's section 3), and `IsQueued`. An entry queued here
  waits in Off.
- **Verified (host only):** the note's section 6 list at 7dcb5cd, on invented packages. Every refusal leaves the
  component's state, #518's three counters and the name table unchanged, with no event and no partial-writes prefix:
  1. no definition, and `SMI` None: nothing;
  2. no `Owner`: refused;
  2a. a lazy `Owner`, a lazy `SMI` actor and a lazy definition (store handles that stay unloaded), plus a stand-in
     definition with no slots: each refused for residency, nothing loaded. A lazy definition is the residency
     refusal also with a resident owner below authority and with no `Owner` (residency first);
  2b. an `Owner` and an `SMI` actor that are two resident authority pawns: the mismatch;
  3. and 4. the forward refused with and without a current move, and before the gate for an autonomous proxy with
     `bClientHasAuthority`;
  5. two appends of the same definition, all six fields as given, with no event;
  6a. nothing current on the authority, with one entry already queued: `Play`'s replication-record `NotRead` stands
     unchanged (its text and record), with no prefix, and the queue and current move are unchanged;
  6b. the seam: the five arguments as given, or omitted, and `Data` None even when the caller passed one;
  6c. a resident definition with cold class code: refused before `Play`'s path, the class code still unloaded;
  7. the two shape errors;
  8. Reference unbound (`reference_binds_nothing` walks `PATHS`).

  Mutation checks: dropping the `Owner` or the `SMI` actor from the residency check fails 2a; dropping the mismatch
  fails 2b; handing the caller's `Data` to `Play` fails 6b; moving the forward test after the gate fails the test of 3 and 4
  through its simulated-proxy rows (where the gate answers no). Test 4's autonomous-proxy row alone cannot tell the
  two orders apart, since the gate passes there and the forward refuses either way.
- **Not verified:** nothing in the running game; every rule is **UNVERIFIED**. The installed slice gate was not run
  (no install yet). By the note's prediction, link 9's next stop is `Play`'s replication refusal, not a pass; not
  measured.
