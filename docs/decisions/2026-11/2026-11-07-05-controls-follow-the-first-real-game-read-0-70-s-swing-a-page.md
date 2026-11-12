## 2026-11-07: controls follow the first real-game read: 0.70 s swing, a page does not cancel it, no pages while down (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md). One live session with few samples, stated per claim there. Host behaviour change, no
parsing change. A swing holds the weapons for 0.70 s and its clip is looked for in the Siren's set. A status page no longer cancels a swing, as the one read showed;
a hit due after the page opened lands under it in the host, UNVERIFIED because the game pauses there. M and L refuse while Maya is down. M no longer closes the Map page and
goes to the Map from the Missions page. A passing `tools/test_controls.ps1` means the host did this.

