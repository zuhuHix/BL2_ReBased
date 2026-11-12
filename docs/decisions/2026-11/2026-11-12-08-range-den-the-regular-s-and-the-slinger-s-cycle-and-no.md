## 2026-11-12: Range den: the regular's and the slinger's cycle, and no manifest without worn pieces

AI-assisted (Claude Code). Record: `docs/verification/COMBAT_RANGE.md`, acceptance table rows 7 and 8, the brain rules' Swing timing
row, "What changed in the host" and the Data section. Host behaviour, the two range tools and their tests; no parsing change. The
follow-up named in `2026-11-12-07-range-den-baby-collision-per-kind-speeds-and-the-adult-swing.md`.

- **The regular bullymong's and the slinger's swing cycle is 1.222 s** (was the 1.25 s stand-in). Session 2 of 2026-11-12 read the
  regular's cycle at medians of 1.2200 to 1.2227 s per recording (96 gaps over three recordings; it uses the adult's melee
  definition) and a slinger's melee at the player at a median of 1.2224 s (28 gaps, one recording). `KindMeleeRows` gives both kinds
  a row with stand distance 0, which means "no live stand distance": the brain keeps its stand-in stop, 60 uu inside the stock
  swing range (260 for the regular). The checker's bands are the medians widened 0.005 s each way, as for the adult, each edge
  rounded outward to the millisecond: regular 1.215 to 1.228, slinger 1.217 to 1.228. A row without a stand distance grades the cycle only. Because the regular now has a row, the
  in-engine `kind_melee` step runs for it on the default range too (about 6 s more), and `melee_interval` holds enemy 0, a regular,
  to 1.222 instead of 1.25.
- **The stand distance stays open.** No stand distance was sampled for either kind, and the regular's median call distance sat 70
  to 85 uu beyond contact (100 + 42 = 142, its closest call), where the adult's sits at contact. Nothing read explains that, so no
  value was brought in. The regular's chase speed (field 500, measured 375) is unchanged as well.
- **No range manifest without the worn pieces.** On one machine `local/phaselock/target/host_target.json` had been rewritten without
  its placements; `tools/prepare_range.py` then wrote every kind with empty `attachments`, the preflight said ok, and in-engine
  suites ran green on enemies without their worn parts. The expectation now comes from the import's own read of each pawn
  (`identity.json` next to the target, which `tools/prepare_phaselock_target.py` checks against its expected set), not from a count
  in the range tools. `tools/prepare_range.py` stops without writing when the placements differ from that read, or when the identity
  has no entry for an imported asset, no `attachments` list in that entry (an identity from before 2026-11-06) or cannot be read,
  and writes `attachments_expected` per kind (0 on engine shapes, 0 for a pawn that wears
  nothing). `tools/check_range.py --preflight` refuses a manifest whose spawned kinds carry a different number of pieces, or that
  has no such field, with exit 2 and the step to rerun (`tools/import_phaselock_target.ps1 -Steps attach`, then
  `tools/prepare_range.py`). Every manifest written before this change is refused once and must be regenerated. Known limit: the
  preflight trusts the manifest's own `attachments_expected` and does not read `identity.json` again, so a hand-edited manifest
  (0 expected, no pieces) passes; the manifest is a generated file.

Verified live: the regular's cycle (96 gaps over three recordings) and the slinger's melee cycle (28 gaps, one recording), at about
120 frames per second. Confirmed in session 2 of 2026-11-12, den capture on Three Horns Divide, Maya level 8; SDK mods loaded (the
three loot mods off in memory for the cash runs; the cheats mod's options off as read); one map, one level, one session.

Tested in the host only: `tests/range_kind_melee_test.py` (the host rows equal the table, the medians inside the bands, 1.25
outside them, a cycle-only row graded on its cycle) and `tests/range_tools_test.py` (lost placements write no manifest, the preflight
refuses empty or short attachments and an old manifest, a kind that wears nothing passes). Nothing has run in the engine yet; the
default range spawns no slinger, so its row is not exercised there.

UNVERIFIED:
- the regular's and the slinger's stand distance, and why the regular's median call stands beyond contact;
- the rule behind every cycle (clip length, rounding to frames, any dependence on the frame rate);
- the slinger's melee damage, which is still its rock's numbers (stand-in);
- the psycho, which keeps 1.25 s and the stand-in stop: nobody has read it swinging at the player.
