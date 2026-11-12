## 2026-10-05: script swap 4: Marcus's dialog through the stock trigger, poll and line-end rules (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_DIALOG.md and NATIVE_BEHAVIOR_POPULATION.md section 3 only. Replaces
the stand-in where `Behavior_TriggerDialogEvent` selected Out and Finished at once.

- Behavior kernel (`src/behavior.*`): latent behaviors (`run().wait`, 1/60 s minimum, per-thread state); `tick()` steps to
  each due wake, so a long frame passes every wake and poll; a waiting thread whose sequence was disabled ends.
- New `src/dialog.*`: Out on the first run, the dialog triggered one kernel wake later, Finished on the first 0.1 s poll
  where the talk act is no longer live (`bForcePlayImmediate`: Finished then Out at once); event selection (last enabled
  entry for the tag, inline act or link-table act), talker (registered pawn by exact name tag, else echo caller), priority
  arbitration (index in `Priorities`, tracked-mission floor), line end from a host line-player interface plus
  `OutputDelay`. With no line player (no audio device) no line starts and Finished follows on the triggering wake, as the
  note describes. Nodes not on the Fire route throw "dialog: not implemented".
- Host: each chosen line is a host event (tag, group, talker, talk act, AkEvent, outcome); the host resolves the AkEvent in
  its audio manifest (new check `dialog_lines_name_the_manifest_ak_events`) and registers Marcus as the pawn talker (his
  name tag is a host choice: the manifest's most common talker tag). Nothing is played.
- Real data: the 16 non-dialog host events are identical and in order; Marcus's walk now starts about three frames after
  the accept (kickoff on the next tick, dialog one wake later) instead of inside the accept call. With a test line player
  the first objective set waits for the line's end, as the note predicts. Two lines (03a, 03b) now report "no talker":
  their acts name the instigator, and what context a mission behavior has is not in the note (open).
- Quest suite: step 4 records `use_key_accepts_mission` at once and waits up to 2 s for the walk before the two unchanged
  walk checks (timing follows from the note). Checks: CTest 11/11 (new dialog scenario on invented data), packages 9/9,
  UE build Succeeded, quest PASS 82/82 and resume PASS 11/11, door PASS 16/16. Sensitive file: `CMakeLists.txt`
  (`src/dialog.cpp` added).

