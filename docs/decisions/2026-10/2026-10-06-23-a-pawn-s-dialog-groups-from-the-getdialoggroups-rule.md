## 2026-10-06: a pawn's dialog groups from the GetDialogGroups rule (replaces the swap 6e stand-in, UNVERIFIED)

AI-assisted (Claude), implementer lane I3, from `docs/verification/NATIVE_DIALOG_GROUPS.md`. `DialogSystem::pawnDialog` builds a pawn's group
list as the note describes: body class `DialogGroups`, then only for `bNPCDialog` bodies the name tag's DLC expansion groups and the globals'
`NPCDialogGroups`, then always the globals' `DefaultTemplateGroup`; no de-duplication. The group search follows `GetMatchingEvent`: first listed
group with an enabled entry wins, a non-matching group's parent goes to the end once by identity, template groups only when allowed (not on a
re-trigger with reused event data). The old 32-entry search cap is removed (it would cut a 127-group search). A dialog reference that no installed
package resolves is looked up by the same path among the dialog package's exports, otherwise it still fails. Marcus: 127 groups, on-use lines
unchanged. Corrections applied to NATIVE_DIALOG.md. Not modelled: `SetDialogNameTag`, the manager's `bEnabled` gate and registered list, the
echo-actor and interactive-object variants. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 96/96 + resume 11/11,
door suite 16/16, no new stubs. Verified: automated suites only.

