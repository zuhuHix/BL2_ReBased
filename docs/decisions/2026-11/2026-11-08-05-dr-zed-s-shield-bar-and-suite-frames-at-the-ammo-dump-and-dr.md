## 2026-11-08: Dr. Zed's shield bar, and suite frames at the Ammo Dump and Dr. Zed (vendor movie PR C)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/VENDORS.md` section 10. Stacked on PR B.

- **Change:** the vendor state gains `shield: {current, max}` from the walker. The shop movie shows it in the shield bar at the health machine, and hides the bar without a shield. `-owvendortest` also captures `OWVendor_ammo.png` and `OWVendor_zed.png` with the existing Ammo Dump and Dr. Zed reports; no step is added or renumbered. `tools/test_vendors.ps1` fails a run whose screenshots lack any of the three frames.
- **UNVERIFIED:** how the real bar rounds and shows a stand-in shield; the frames are host frames of a test state.
- **Checked:** synthetic tests, including the movie's draw path with a recording stub player, and the UE module build. The in-engine suite has not run it.

