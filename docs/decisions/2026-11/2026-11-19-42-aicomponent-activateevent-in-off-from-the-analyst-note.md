## 2026-11-19: AIComponent.ActivateEvent in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane FIX-ACTIVATE-EVENT, implementer. The native is coded from
`docs/verification/NATIVE_SLICE_OFF_STOPS.md` alone. That note's analyst was lane FIX-SLICE-OFF-1 (decision 2026-11-16-81),
not this thread, and this lane opened no analysis output, Ghidra or REA. Field kinds were read from the packages only. There
was no real-game session: every rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was
loosened and no existing `NotYet` guard was relaxed.

**What changed.**

- `crates/ow-ai/src/awareness/actions.rs`: `AIComponent.ActivateEvent(EventName)`, bound in `ParityMode::Off` only
  (`register_activate_event`, which `actions::Pack` also calls). The note's rule, in order:
  - With the component's `AI` or `AIDef` unset, nothing happens and the answer is false.
  - With a running node (the component's own `CurrentState` pointer set), the result is `NotYet { NotRead }` and
    nothing is delivered. The running action's first delivery goes through a routine the note did not open (its "Not
    read" item). A `CurrentState` without the pointer field is bad data, not "not running".
  - Otherwise every `AIDef.NodeList` node whose `Action` is set gets the script event `ActivateEvent(EventName)`, in
    list order, and the answer is false.
  - **Who receives it (review r1 of #429).** The note leaves open whether the game's list holds per-AI copies or the
    archetypes. Here that decides the receiver: the Rust `AIDef` is the shared package definition, while ow-ai part 2's
    brain keeps its own per-AI copies of the actions, and handlers write to the receiver. So a mind with a built brain
    (running or not) is `NotYet { NotRead }`, and so is a set `Action` that no brain owns as a per-AI copy (a
    definition's archetype or template). Both name the note's per-AI copies item. In practice no definition action is
    delivered to until that item is settled.
  - Every node is read and checked before any delivery, so an error or a `NotYet` leaves nothing delivered.
  - A component or node class without the fields read, or a node `Action` of the wrong kind, is bad data.
- **Reference is unchanged.** The key stays unbound there, so the slice's C++ stub (`REFERENCE_STUBS`) serves it, as
  before.
- `crates/ow-cli/src/cmd/slice.rs` and `crates/ow-cli/src/cmd/mission.rs`: one call each to `register_activate_event`.
  Their worlds do not register ow-ai's packs, and registering the whole pack would also bind the hold natives under
  Reference.

**Verified (automated, invented classes).** `ow-ai`: 5 tests after review r1; `ow-cli`: the stub-list test.

- In Off, the definition's own actions (owned by no brain) are `NotYet` naming the per-AI copies item, with nothing
  delivered, also when only some are owned. Actions marked as owned per-AI copies (an invented stand-in) get the event
  in list order (the root, which has no action, is skipped) and the answer is false. Under Reference nothing is bound.
- In Off, an unset `AI` or `AIDef` (each, and both) does nothing and answers false.
- In Off, a running node answers `NotYet { NotRead }` naming the first delivery; a built brain, running a node or not,
  answers the per-AI copies `NotYet`. Nothing is delivered in any case.
- In Off, a node `Action` of the wrong kind, and a `CurrentState` without its pointer field, are errors with nothing
  delivered.
- `ow-cli`: under Reference the slice's stub list still lets `AIComponent.ActivateEvent(name)` through; Off refuses it.
- Gate numbers are in `docs/verification/GATES.md`.

**Installed (Off; before is main cb01e96f, after is this branch; outputs under ignored
`local/p2/fix-activate-event/`).**

- `--slice-gate` strict: before 0 ok / 11 `NotYet` / 3 errors; after 0 / 14 / 0. **Link 10's stop moves** from the error
  at `use` ("unbound native AIComponent.ActivateEvent(name)") to a `NotYet` at `accept`:
  `UIInteraction.GetDataStoreClient` (decision 2026-11-19-19's open item). Links 11 and 12 follow it.
- `--slice-run`, the Fire event-order tool's steps: the partial report is byte-identical before and after. The run still
  ends at the Pawn `IAIInterface` refusal after the dummy's spawn. The `ActivateEvent` refusal was an error line, which
  this partial report does not print.
- `--mission-run` Fire (`Startup.upk`; `stage:8 player:8:21000 script:use script:accept kickoff tick:1`): identical
  before and after (status 1, exit 3). It stops earlier, at `script:use` on `GetDataStoreClient`, and does not reach
  `ActivateEvent`.

- **After review r1** (the release binary of the fix commit, output under ignored `local/p2/fix-activate-event/r1/`):
  `--slice-gate` strict still 0 / 14 / 0, and rows 10 to 12 are unchanged: `NotYet` at `accept` on
  `UIInteraction.GetDataStoreClient`. No row stops at `ActivateEvent`. That `use` step raises no `NotYet` and no error
  suggests the installed call there answers false for an unset `AI` or `AIDef`; that is inferred from the rows, not
  traced.

**Open.** The running action's first delivery (needs the analyst); the per-AI copies question (a bl-sdk read, as the
note says), which now holds every delivery back; `GetDataStoreClient`, now link 10's stop.
