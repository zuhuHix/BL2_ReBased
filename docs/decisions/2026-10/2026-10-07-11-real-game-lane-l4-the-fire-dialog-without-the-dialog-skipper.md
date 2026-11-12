## 2026-10-07: real-game lane L4, the Fire dialog without the dialog skipper

AI-assisted (Claude), real-game lane L4 (work queue item 3), with the same method as L1 and L2: one game process under the run lock, saves and the
SDK settings folder backed up and hashed first, saves blocked from the first frame, memory-only. Launch 1 had the installed Dialog Skipper mod
disabled in memory (`Mod.disable(dont_update_setting=True)`, so no settings write); launch 2 had it on, which was L2's condition. The new probe is
`tools/real_game/scripts/dialog_probe.py`.

Observed in launch 1: the whole Fire dialog plays. Line 03b (6.59 s) is followed by `_1184`, which plays 03a with the target dummy pawn as
speaker (2.01 s), and then by `_1183`, which plays 04 (5.01 s). Marcus's on-use `DET_NPC_OnUse_MissionsAvailable` (3.10 s) is real too. So the
4 "extra" VM dialog items of the L2 comparison come down to 1, `AttemptStatusEffect_9` (the hit model).

The cause of L2's gap: the skipper stops each line of a non-hostile pawn at its first instant. `_1186` then selects Finished in the frame in
which the dummy spawns, before the dummy is written into the Kismet variable `TargetDummy` that is `_1184`'s context. `_1184` is therefore not
applied. Also observed: a dialog behavior's resumed runs are applications of a transient copy, which L2's name filter could not see.

UNVERIFIED: that the variable is empty at that instant (the row order fits, but it was not read in that frame), and that an empty context list
drops the application.

Not changed yet, for a VM lane: bind `TargetDummy` to the dummy at spawn, run `_1184` with the dummy as context and talker, and give an empty
context no application. Live line lengths are recorded for a host line player.

A Python re-trigger of one line with `None` object arguments crashed the game once, after the data had been dumped; nothing was written.

Save evidence: 33 save and config files are byte-identical after both launches, and the 19 SDK settings files are unchanged. Checks: ctest
11/11, verify_packages 9/9 (docs and a game-side probe only).

