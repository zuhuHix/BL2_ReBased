## 2026-11-13: Slice gate: bounded waits for a clear view line and Maya's own recovery, and link 8 fires the looted gun

AI-assisted (Claude Code). Lane U of the hub run of 2026-11-13. The slice gate's unforced variant failed three of its first eight
runs (2026-11-11, main 969307f). Lane U read those logs and the host code again: all three were the driver's timing, not a host
rule ([SLICE_GATE.md](../../verification/SLICE_GATE.md), "First runs"). Two `pick_up_aimed_not_nearest` FAILs came from a living
bullymong standing on the line from Maya's eye to the aimed drop at the use-key press (the card's sight test logged it at 62 and
10 uu). One `phaselock_enemy` FAIL came from Maya going down 0.42 s after the buy, so the injured state's cast gate refused until
the driver's 12 s ran out. No host rule was changed. The aimed pickup's reach and cone stay invented stand-ins (`UNVERIFIED`).

- **View-line wait (a fixture, all variants).** Before the use-key press on links 6 and 7 the driver waits until the card's own
  sight test (`OpenWillowPickupCard::InSight`) has seen the aimed drop on every tick for 0.2 s, at most 10 s past the moment the
  press was ready. The 0.2 s covers the card's 10 Hz search. Each link logs `view_wait=<s>s/10s view_line=clear|blocked` and names
  the fixture `waited for a clear view line to the aimed drop` when it waited. A line still blocked at 10 s is a FAIL. The checker
  fails a PASS without the token, with a wait past its bound, or with the line not clear. The wait tests the sight line only; the
  ids still grade the reach and the cone, so a wrong pick with a clear line is still a FAIL.
- **Placement in the unforced variant.** At link 6 Maya stands on the drop's side that faces the nearest living enemy within
  2,500 uu, so an enemy walking up to her comes up behind her. Link 7's two test drops are made at the saved station's start exit,
  far from the range (in lane U's fourth run, with them in the range, the enemies downed her twice during the link). The
  Phaselock cast goes at the nearest living enemy (in the fifth run an enemy meleeing her blocked the line to the one the driver
  aimed at, and the cast lifted the blocker).
- **Down inside a link (unforced, a fixture).** Links 6 and 7 before the press, link 8's burst, and `phaselock_enemy` until the
  cast goes out wait for Maya's own second wind or respawn, at most 50 s each (the existing rule for a link group's start), and
  never revive her. Each detail carries `down_wait=<s>s/50s`; the checker fails an unforced PASS without it or past its bound.
- **Respawn link, unforced.** The start-exit half of `bleed_out_respawn` wanted the run's only respawn, which an unforced run with
  an earlier bleed-out cannot meet (lane U's first run reached link 14 at respawn 5). The driver records the run's first respawn,
  and in this variant the half is judged on it. The older variants keep the only-respawn rule.
- **Link 8 fires the looted gun (all variants).** After the draw, five shots (fewer when a full magazine holds fewer) through
  `AOpenWillowWalker::TestFireInput`, the fire key's own handlers, at the range dummy farthest from the living enemies, with Maya on
  the enemies' side of it so the shots and any splash go away from them. A gun with burst count 0 is held; any other is clicked
  once per tick. No walker change was needed. The detail logs the shots, kind, projectiles per pellet, the block's rate, burst
  count, gap and spin, the skills hook's factor, each interval, the longest frame, the nearest enemy's distance and how many
  enemies were hurt. The checker fails a PASS unless every shot fired, the kind and projectiles match the block, and each interval
  lies within the block's interval (or the burst gap after each burst), x the spin start, plus at most one frame, with 0.002 s for
  the log's rounding and the walker's float clock. The enemies-hurt count is reported, not graded.
- **Unchanged:** the link count and order, the suite map's `#pass` lines, the older variants' fixtures apart from the two new
  ones, and every host rule.
- **Re-aim on link 6.** While the view-line wait runs on the real drop, Maya is placed on another side of it every 2 s (120
  degrees round) and must be on the ground before the press; in the eleventh run an enemy stood on her eye for the whole 10 s.
- **Verified (host only, zuhu PC, 2026-11-13):** 14 gate runs. Unforced: 3 of 6 green on 3604058 and the build before it, 4 of
  5 on 6e14cd1 (the first four in a row), 1 of 2 on a6c37ab; plain on a6c37ab: 14 / 0 / 0. Every FAIL was read in its log: the
  driver problems above, each answered in the next head, and one that is open: in run 12 the kill roll dropped a gun above
  Maya's level and `EquipItem` refused it under the level rule, which was not touched. The burst passed the checker in all 12
  runs that reached it (1.12 to 16.67 shots a second, no enemy hurt). Five unforced runs in a row on the final head were not
  reached; the run budget ran out. Checker tests 96 of 96; ctest 174 of 174.
- **A gun above Maya's level, unforced link 8 (review round, same day).** Run 12's level-9 gun came from a level-8 psycho whose
  loot rolled at item stage 9: the item stage carries the data's offset, and the range's sampled band for that kind and level
  is 6 to 9 (NATIVE_LOOT.md, the item game stage; COMBAT_RANGE.md, "Item stage"). The roll did what the records say the game
  does (`UNVERIFIED` live for the psycho), so the loot code was left alone. When the host's equip refuses a gun above Maya's
  level, the unforced equip link now keeps it and loots on with the loot link's farm, within that link's own bound (100 kills,
  720 s, counted on), picks the next gun up as link 6 does and tries again (fixture `kept looting for a gun Maya can equip`).
  Past the bound it FAILs with "no equippable gun within the loot bound". The detail carries `equip_refused=<n>` and, after a
  refusal, `loot_kills=` and `loot_time=`; the checker fails an unforced PASS without them or past the bound. The level rule is
  untouched, and the other variants grade as before. Not seen in the engine: in the four runs after it no gun above Maya's
  level dropped, so the re-loot rests on its synthetic checker cases (`UNVERIFIED` in the host).
- **Review round 1 (same day).** The pickups FAIL when Maya is still down at the 50 s bound, and the checker treats reaching a
  bound as past it. The burst logs each shot's own frame and the block's spin-up time, and each interval is bounded by that
  frame and by the spin-up the block allows at that shot. The gate ticks after its walker (a tick prerequisite), so the shot is
  seen in the frame it was fired. What the check still cannot resolve is about one frame per interval (roughly 10 % at 16.67
  shots a second and 60 frames a second). Link 8 FAILs when the burst hurt an enemy a later link needs, the stand-in included.
  The view-line wait and the re-aim run only for an enemy on the line (`view_blocker=`); any other blocker fails the pickup at
  once with its name. Runs on that head: two unforced and one plain, each 14 / 0 / 0; checker tests 107 of 107. The `-Shield`
  and `-DownAfterFight` variants were not run by the lane.
- **Target set aside (the hub, 2026-11-13).** Five unforced runs in a row are not chased now: another lane's leaps and thrown
  rocks will change every fight. The unforced variant is not reliable yet.
- **UNVERIFIED:** the block, and so the gun's rate in play, against the real game; whether the game's pickup trace is blocked by a
  pawn; the pickup's reach and cone. The burst's grading compares the host with its own data.
