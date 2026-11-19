## 2026-11-19: VM host latent actions: review follow-ups

AI-assisted (Claude Code / Claude Opus 5.5).

Lane VM-LATENT-HOST-B, the follow-ups the review of #389 listed for the seam of decision 2026-11-19-07
(`crates/ow-vm/src/latent.rs`). No new dependency and no NotYet guard changed. The public API keeps its names and
signatures. Two behaviours change, listed under M2 and L3.

- **M1, tests** (`latent/host_tests.rs`). New regression tests cover: a poll that finishes its own call (the finish wins
  and the code continues in the same step); a poll that returns `Err` (the step ends with it, the call stays pending,
  the next step polls it again); an object destroyed while waiting, and one destroyed inside its poll; a poll that
  looks kinds up and registers a new one; a second start in one native replacing the first (only the second ticket is
  current and polled); and a ticket refused on another object. The fixture's single poll flag became a poll-mode enum,
  and it gained an invented `MoveTwice` native and a `Double` state.
- **M2.** `host_latent_of` answers None, and `finish_host_latent` answers false, for an object that is destroyed (or
  freed) even before its state frame is forgotten. `states::process_state` never steps a destroyed object, so such a
  call could never complete.
- **L1.** The `HostLatentPoll` doc now says that an `Err` leaves the call pending and it is polled again, that the
  registry is free while a poll runs, and that a poll must not step its own object.
- **L2.** The `HostLatentKind` doc now says that a kind belongs to the VM that registered it.
- **L3.** `register_host_latent` refuses every kind under `ParityMode::Reference` (`VmError::Registration`), as
  `Actor.Sleep` stays unbound there. No host call could start there anyway. The one user on a branch,
  `fix/rust-ai-movement` (#397), registers only in Off, so it is unaffected.
- **L4.** `host_latent_kind` reads the registry without adding the service, and answers None when there is none.

Verified: synthetic tests only (17 latent tests, the earlier ones unchanged except the Reference one, which now also
checks the refusal). The semantics stay the stock UE3 shape and are UNVERIFIED in the running game.
