## 2026-11-12: Maya starts at the saved station's first exit; the first respawn takes the next

AI-assisted (Claude Code). With `-owquest` the host used to start Maya at the level's player start (`WillowCoopPlayerStart_0`
raised 100 uu, yaw 45), 11,930 uu and 45 deg away from where the game puts a loaded player. Observed in the real game, session 1
of 2026-11-12 (level 8, Sanctuary, every SDK mod off as read in the game; two loads of one save, so two samples of one case): Maya
stood on FastTravelStation_0's first exit point (1.15 uu above it) with that exit's yaw, the saved station was that station, and
the first respawn afterwards used the next exit point of the same station.

- `UOpenWillowQuest::StartPoint` picks the start from the stations the host already loads for the respawn
  (`FOpenWillowSliceData::Stations`): the first exit point, position and yaw. The quest places her there on its first tick with a
  controller. `-owspawnx=` (the inspection point) still wins.
- HOST STAND-IN: the host save carries no station, so the level's one fast-travel station (class `FastTravelStation`) stands in for
  the saved one; it is the station the observed save named. With none, or more than one, the level's player start is kept.
- The start counts as one use of that station's exit counter (`StationCounters`), so the first respawn there takes the next exit.
  `UNVERIFIED` that the game's load advances the same counter: observed only that the first respawn after a load did not reuse
  the load's exit (session 1, and session 2's four respawns on another map agree).
- `UNVERIFIED`: which rule placed her at the load (the saved station was also the only active and the nearest one, and the load
  chain was not hooked).
- Checks whose meaning changed: the quest suite's `respawn_at_decoded_station_exit_point` now expects the exit after the start's
  (exit 1 of FastTravelStation_0) instead of exit 0, and the new `spawn_at_saved_station_exit` check covers both halves. On the
  slice gate, `spawn_maya` also FAILs unless she stands within 1 uu planar and 1.2 uu vertical of the start exit and faces its
  yaw within 1 deg, and `bleed_out_respawn` also FAILs if the first respawn reused the start's exit. Both are stricter, not
  looser. `tools/check_slice_gate.py` does not read these details and is unchanged.
- Not changed: the pools at spawn (`spawn_pools_full` stays open), the full load chain, the selection rule between several
  stations, the respawn's timing (`OpenWillowRespawnTimeline.h`).
