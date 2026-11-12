## 2026-11-12: Mission page acts on the key release; Marcus walks at the observed 150

AI-assisted (Claude Code). Two host stand-ins that session 1 of 2026-11-12 contradicted. Confirmation for both: observed in
the real game, session 1 of 2026-11-12 (the mission screen capture with the read-only watch, level 8, Sanctuary, every SDK
mod off as read in the game; two runs of the same scripted steps, so two samples of one path).

- **Mission page keys.** The host acted on Enter and Escape at key-down. In the game the press returns false and does
  nothing, and the release accepts the offer, turns in, takes the reward (Enter) and closes the page (Escape, on the page
  after the reward), in the release's own frame; on the reward page Escape does nothing at either edge. Now
  `FMissionPageKeys` passes Enter and Escape on at both edges, `UOpenWillowQuest::OnScreenKey` takes the edge and acts
  only on the release, and the press is still swallowed. Other keys are unchanged (they act, or do nothing, at the press).
  The game's handler returns false on the press, so the game did not consume it, and where the press went next was not
  observed; the host swallowing it is the host's choice, `UNVERIFIED`. A release whose press the page did not see does
  nothing: the host's choice, `UNVERIFIED`. Escape's release on the offer and the turn-in closing the page is the same
  edge carried over, `UNVERIFIED` (the session did not press Escape there).
- **The suite** (`-owmissionpagetest`) sends the two edges a step apart where it checks them. New rows:
  `offer_escape_press_does_nothing`, `offer_enter_press_does_nothing`, `turn_in_enter_press_does_nothing`,
  `reward_enter_press_does_nothing`; renamed: `offer_escape_closes` is `mission_escape_release_closes`,
  `offer_enter_accepts_and_closes` is `mission_enter_release_accepts`. `tools/check_mission_page.py` expects 37 rows.
- **Not changed, open:** after the reward the game's page goes back to its list (one eligible entry in that save) and
  stays open until Escape; the host closes it at once. Escape on the offer before an accept, and a left click, a gamepad
  button or Start on the reward page, were not observed with the mods off and are not built.
- **Marcus's walk speed.** The host walked him at the class's GroundSpeed x WalkingPct, 294 x 1.0 in the local data, and
  every node's AISpeedPercentageHere is 1.0, so 294 uu/s. The game walked him at 150 uu/s in both runs (about 90
  moving intervals each, one walk of about 1,230 uu; no slow-down before nodes 12, 26 and 40, and one turn of about 0.8 s at
  low speeds at node 35) while the same fields read 294 and 1.0.
  The host now uses 150 as a constant for this walk and still reads and checks the class fields (logged with the walk's
  start). The host does not model the turn at node 35. Where the 150 comes from is `UNVERIFIED`; a possible source is the
  Patrol stance's SpeedScale 0.51 (294 x 0.51 = 150), which SANCTUARY_AMBIENT_NPCS.md records for a citizen's patrol walk;
  Marcus's own stance was not read. The node percentage still multiplies it (1.0 on every node).

Verified: `python tests/check_mission_page_test.py` (synthetic); the core build, ctest and `verify_packages.py`; the
Unreal module build. Review r1 on PR #246: the suite's last step (the view after its forced failure), which
`TickComponent` runs after the failure, was still named by its old number, so that row never printed; the two steps
named outside their own case are now constants in `OpenWillowQuest.h`. Not yet run in the engine: `tools/test_mission_page.ps1`, `tools/test_quest.ps1` (its walk waits
25 s; the host path is about 1,240 uu planar, about 8.3 s at 150), the slice gate.
