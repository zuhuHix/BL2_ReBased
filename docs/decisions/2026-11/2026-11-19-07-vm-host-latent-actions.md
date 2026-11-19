## 2026-11-19: Host-driven latent actions in the VM

AI-assisted (Claude Code / Claude Opus 5.5).

Lane VM-LATENT-HOST (slice plan step 4, `ow-vm`). Until now the VM's only latent action was `Actor.Sleep`, so the AI's
latent moves (`FollowPath`, `MoveTo`, `MoveToward`, `FinishAnim` and the waits) could not run (SLICE_RUN_PLAN.md,
section 3.3). `crates/ow-vm/src/latent.rs` now has a seam through which the host or `ow-world` starts, polls and
completes a latent call, after the stock UE3 shape of a latent native plus its poll:

- the host registers a kind once (`register_host_latent`: a name and a poll function) and binds the latent native
  itself with `Vm::register_native`; the native does the host's set-up and calls `start_host_latent`, which allocates a
  ticket and sets the frame's latent action to the new `LatentAction::Host`; the statement then yields as after
  `Sleep`;
- each later state-code step polls first, as for `Sleep`: the poll function gets the object, the ticket and the step's
  delta and answers `Pending` (the step ends) or `Done` (the action is cleared and the code continues in the same
  step); a host that learns of the end another way calls `finish_host_latent`, and the next step continues without a
  poll;
- a state change clears the action as it does for `Sleep`; the host sees its ticket stop being current
  (`host_latent_of`, and `finish_host_latent` answering `false`). A state change made inside the poll wins over its
  answer. Nothing stops the host's own movement or animation, as in stock UE3.

Order and determinism: polls run inside `states::process_state`, so they follow the order in which the caller steps its
objects; tickets count up from 1 per VM in start order. The seam is additive: no existing type or rule changed, the
`Sleep` poll is untouched, `LatentAction` was already `#[non_exhaustive]`, and `ow-vm` gains no dependency (no
`ow-host` or `ow-world` edge; `Cargo.toml` unchanged). Under `ParityMode::Reference` state code does not run, so no host
call can start there (R3 unchanged). No NotYet guard changed.

What is verified: synthetic tests only (`crates/ow-vm/src/latent/host_tests.rs`, an invented `Walker` class with two
host latent natives over a fake host world): suspend and resume through a poll, completion from outside, abandonment on
`GotoState`, a state change from inside the poll, refusal outside state code, a duplicate kind refused, nothing under
Reference, and a three-object fixed-step run whose log is checked entry by entry, repeated exactly, and reordered when
the step order is reversed. The existing VM tests pass unchanged.

UNVERIFIED: the semantics are the public stock UE3 pattern, not read from this executable. Which latent functions this
game binds, how Gearbox's latent natives (`FollowPath` and the AI moves) poll, and whether a poll runs in the step that
starts the call (here it does not, as for `Sleep`) are not read or observed in the running game. Binding the actual
movement and animation natives is the host's and W7-AI part 3's work, not this lane's. Gate results are in the PR.
