## 2026-11-06: M opens the Map and L the Missions during play (controls lane)

AI-assisted (Claude). Record: [docs/verification/CONTROLS.md](docs/verification/CONTROLS.md). Host behaviour change, no parsing
change. The keys come from the stock keyboard table (`GD_Input.Devices.InputDevice_Keyboard`, OpenBLCMM dump), not from a press in
the running game: UNVERIFIED in game, as is what the stock menu refuses (`CanShowModalMenu` not read). The keys reuse the HUD's
existing page toggles, so a vending shop still refuses them. A passing `tools/test_controls.ps1` means the host did this.

