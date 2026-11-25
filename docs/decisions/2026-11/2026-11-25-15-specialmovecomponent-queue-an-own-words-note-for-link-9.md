## 2026-11-25: SpecialMoveComponent.Queue: an own-words note for link 9

AI-assisted (Claude Code)

Lane ANALYST-SPECIALMOVE-QUEUE, analyst only, with Claude Opus 5.5. Numbers 01 to 14 for this date are taken on open
branches, so 15 is used. The maintainer may renumber it. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** a runner probe (main + #663 + #664 + #668) shows link 9's `cast` stopping with an error at the unbound
  native `SpecialMoveComponent.Queue`. NATIVE_SPECIAL_MOVE_PLAY.md does not cover it.
- **What:** `docs/verification/NATIVE_SPECIAL_MOVE_QUEUE.md` and one NATIVE_INDEX row. They come from the script, a
  local read-only Ghidra reading under the machine lock, and the layout tool. Raw output stays under ignored
  `local/analysis/smqueue/`.
- **Findings** (UNVERIFIED in game):
  - with no definition or no owner interface, `Queue` does nothing;
  - otherwise the given definition must pass `IsLocalAuthority`. Then:
    - with no current move, the request goes through `Play`'s whole path, with `Data` passed as None;
    - with a current move, one entry with all six arguments is appended at the end of `SMDQueue`;
  - an owner below authority also forwards to the server's queue event;
  - a stop drains only the queue's first entry, gated, with its `Data`;
  - `IsQueued` matches definitions by identity.
- **Link 9 (inferred):** the caller is most likely the target AI's `Action_FollowPath.SetSpecialMoves`. Case "nothing
  current" then meets `Play`'s existing replication-record `NotRead`.
- **Proposed Off answer:** bind `Queue` beside `Play` in `ow-skills`, with a write-free preflight (the owner and
  network cases refused as `NotRead`) and the two commits above. Reference unchanged; tests listed.
- **Clean room:** a different lane implements this note.
