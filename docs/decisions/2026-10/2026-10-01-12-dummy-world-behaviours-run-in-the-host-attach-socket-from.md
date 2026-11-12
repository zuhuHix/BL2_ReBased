## 2026-10-01: dummy world behaviours run in the host; attach socket from data; dependency fixture kept

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `Behavior_Transform` and `Behavior_RegisterTargetable` are readable script. The first sets
  `WillowAIPawn.TransformType`, whose readable consumer is `GetTargetName` (the balance's per-playthrough transformed
  display name; the lookup itself is native). The second registers the pawn in the native global `TargetableList`.
  The host now runs both for the target dummy: a transform type with its target name, and a targetable flag that no
  host targeting reads yet. IntMath and ChangeInstanceDataSwitch stay logged.
- The `AttachToActor` "bone" `Target` is the holder's `SocketComponent` of that name, not a skeletal bone, so no
  carrier mesh import is needed. `tools/prepare_slice_world.py` emits its pose; the same composition convention
  reproduces the holder's own Base attachment to 0.0 uu (new oracle, 21 of 21 pass). The host puts the dummy's origin
  on the socket at attach. By eye the dummy now stands on the lane floor instead of being sunk to its knees.
- UNVERIFIED: native `SeqAct_AttachToActor.Activated` and `bUseConstructAttachment`; the transformed-name lookup rule
  and the playthrough (1 assumed); which native code reads the targetable list.
- The `GD_Episode03.M_Ep3_CatchARide` dependency cannot be satisfied from installed data: the mission lists it as a
  plain dependency, Sanctuary is reachable while that mission is still active, and the only bulk-completion path
  (the mission fast-forward) is native with a trigger set at run time. It stays a fixture standing in for save state,
  now labelled as such, logged at start and filled from the mission's declared `Dependencies` instead of a
  compiled-in name.
- Checks for the three entries above (one build, this machine): quest suite 73/73 first run and 10/10 resume
  (`run-first-20261001-223529`, `run-resume-20261001-223643`), door suite 16/16, CTest 10/10, packages 9/9. One
  earlier run failed 67/68 (`run-first-20261001-222312`): the new ceiling check found no low surface in the lane.
  Inventory suite on this machine: 45 PASS, 0 FAIL, 2 NOT_RUN (the locally seeded backpack has fewer than nine
  rows), 2 KNOWN_DIVERGENCE.

