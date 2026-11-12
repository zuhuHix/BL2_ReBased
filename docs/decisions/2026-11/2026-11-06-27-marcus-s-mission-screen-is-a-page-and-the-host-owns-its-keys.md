## 2026-11-06: Marcus's mission screen is a page, and the host owns its keys (mission UI lane)

AI-assisted (Claude). Record: `docs/verification/MISSION_SCREEN.md`. With a page URL, E at Marcus now opens BL2's own movie under Ruffle (`FOpenWillowMissionPage`) instead of confirming by itself: Enter confirms on the offer, the turn-in and the reward
page, Escape leaves the first two, and E does nothing there. The accept and the reward page's order were observed in the live trace; the key rules are read from the installed script and stay UNVERIFIED until the live read. The host takes the keys itself so a page cannot
trap the player, and a missing movie gives a plain panel. Without a page URL or a ready page the old path runs, so `tools/test_quest.ps1` and the slice gate keep their flow. Host behaviour only; no parsing change.

