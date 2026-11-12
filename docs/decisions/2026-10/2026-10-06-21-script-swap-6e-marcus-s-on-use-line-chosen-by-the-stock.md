## 2026-10-06: script swap 6e, Marcus's on-use line chosen by the stock dialog data (UNVERIFIED)

AI-assisted (Claude), implementer lane I1, from `NATIVE_DIALOG.md` and `NATIVE_BEHAVIOR_CONTEXT.md`. The existing DialogSystem gains the
component `TriggerEvent` route: group search with the ParentGroup rule, the speaker as instigator, a Talk act's no-match output into a Trigger
act, and the Trigger act re-firing the event on the instigator's own groups. On real data the generic `VO_NPC_OnUse_*` tag has no Marcus entry,
so the no-match path fires Marcus's own `DET_NPC_OnUse_*` tag: NotStarted plays `Ak_Play_VOCT_Marcus_Quest_New`, Active `..._Quest_During`,
after completion `..._Quest_No_New`. ReadyToTurnIn is silent in the data (an entry for Marcus without an AkEvent); no line was invented.
"Plays" means chosen and found in the audio manifest; the host has no audio device. Stand-in: the pawn's dialog groups are its body class's
`DialogGroups` plus the dialog globals' `NPCDialogGroups`; `WillowPawn.GetDialogGroups` has no note yet (UNVERIFIED). Checks: ctest 11/11,
verify_packages 9/9, UE build succeeded, quest suite 96/96 + resume 11/11 (was 92 + 11), door suite 16/16 (rerun by the orchestrator), no new
stubs. Verified: automated suites only.

