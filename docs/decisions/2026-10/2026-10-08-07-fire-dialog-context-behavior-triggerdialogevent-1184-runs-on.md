## 2026-10-08: Fire dialog context, `Behavior_TriggerDialogEvent_1184` runs on the dummy (implementer lanes I8/I8b)
AI-assisted (Claude). From the live record of lane L4 (the behavior's context is the target dummy pawn, from the Kismet object variable `TargetDummy`; with
the Dialog Skipper on the chain is never applied). VM: (1) `BehaviorProvider::bindKismetVariable`: a `BVAR_NamedKismetVariable` link resolves to the object
bound under that name (the slice binds `TargetDummy` to the stock dummy pawn after its spawn behaviors; before that it is empty); (2) `DialogSystem::behavior`
reads its context list at the first run: an empty list drops the application, else the context's registered talker is the event's instigator; the dummy is
registered as a talker under its body class's dialog name; (3) an instigator-talker act with no instigator actor takes a TalkData entry (line 03b, spoken by
Marcus with the mission tracker as context). Consequence: the default `--slice-run` (no line player) applies 03a to an empty variable and drops it, 04 never
starts (the live skipper-on result); with `talker:<Marcus> lines:<s>` the unmodded order plays (03b, dummy bound, 03a spoken by the dummy, 04). The UE host
still plays 03a and 04 (the dummy is spawned before the chain). `--mission-run` gains `kismet:<variable>|<name tag>`; `verify_fire_event_order.py` gains a lines
run: 32 rules (HEAD's 24 hold unchanged). Verified (automated): ctest 11/11, verify_packages 9/9 (3,339/3,339), event-order self-test + 32/32, test_quest
107/107 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0 (rerun by the orchestrator on HEAD e0094fc: same numbers). UNVERIFIED: see above. Not changed: ChangeRemoteBehaviorSequenceState_171, line lengths.

