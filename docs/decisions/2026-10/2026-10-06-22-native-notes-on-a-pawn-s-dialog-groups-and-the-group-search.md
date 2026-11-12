## 2026-10-06: native notes on a pawn's dialog groups and the group search (UNVERIFIED)

AI-assisted (Claude), analyst lane G22, Ghidra; own-words note `docs/verification/NATIVE_DIALOG_GROUPS.md`, no listing text. The pawn's
`GetDialogGroups` lists its body class's `DialogGroups`, then (only when the body class has `bNPCDialog`) its name tag's DLC expansion groups and
the dialog globals' `NPCDialogGroups`, then always the globals' `DefaultTemplateGroup`; no de-duplication. Interactive objects and echo actors
have their own simpler lists. `GetMatchingEvent` takes the first listed group with an enabled entry and appends a non-matching group's parent to
the end of its search, unique; template groups only when the caller allows them. For Marcus this gives 127 groups (his own, the 125 NPC groups,
the template group), which matches swap 6e's stand-in on the on-use route; the note lists the differences (the bNPCDialog condition, the
template group) and corrections to NATIVE_DIALOG. A second on-use press while his line is live is blocked by equal priority. All UNVERIFIED in game.

