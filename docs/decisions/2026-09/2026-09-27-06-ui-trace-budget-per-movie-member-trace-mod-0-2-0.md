## 2026-09-27: UI trace budget per movie member (trace mod 0.2.0)

The two existing traces lost the skill tree's `SetBranchProgression` values
and its spend flourish. `GFxObject:Invoke` had one 200-record budget for every
movie method, and 99 SetInfo calls plus hover calls spent it. The trace mod
now keeps a separate budget for each member a bridge call names (its
`Member`, `Method` or `Path` argument), and a return follows its call's
decision. Not run yet: it needs the game with the mod SDK. The installed copy
in the game's `sdk_mods` is still 0.1.0 until the maintainer replaces it. The
next recording should cover the Skills tab's Q overview, which no trace has
used, plus spends in every branch, a respec, and a class-mod equip.

