## 2026-10-07: gestalt decode cross-checked against live-game dumps from the maintainer's BL2_RTSE mod

AI-assisted (Claude), lane R1; record `docs/verification/RTSE_GUN_DATA_CROSSCHECK.md` (counts and conclusions only, no dumped values). The 16
GestaltDef tables the maintainer's in-game mod dumped (9 item families plus 7 Remaster weapon variants) match our package decode exactly:
646/646 part entries, 636/636 bounds, 544/544 socket mappings, 541/544 mesh sockets (three pistol scope eye sockets differ by about a unit,
cause UNVERIFIED). This confirms our struct-array decoding of these objects against the running game. The Remaster tables exist in
`Mancana_StartupRemaster.upk` with different ranges; our earlier live gun capture matches the original tables, not the Remaster ones. Gaps
and recommendations (muzzle and eject sockets in the host, a duplicate-name bug in `slice_npc_assets.py`, live golden checks over part
combinations) are in the record. Nothing from BL2_RTSE was copied into the repository; reusing its code is a maintainer provenance decision.

