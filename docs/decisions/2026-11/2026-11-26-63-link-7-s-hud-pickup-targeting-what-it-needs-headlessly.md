## 2026-11-26: Link 7's HUD pickup targeting: what it needs headlessly

AI-assisted (Claude Code)

Lane ANALYST-LINK7-HUD-TARGETING, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** link 7 stops at `look` with the pickup targeting's no-HUD `LayerAbsent`; the hub asked whether the HUD's
  answer can be decided headlessly or needs the Flash/HUD layer.
- **What:**
  - the new note `docs/verification/NATIVE_LINK7_HUD_TARGETING.md`;
  - two NATIVE_INDEX rows and one edited row;
  - a pointer in NATIVE_PICKUP_TARGETING.md (open question 3);
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py` and a local scan of all script functions;
  - a local read-only Ghidra reading under the machine lock (`GetPlayerViewPoint`'s bodies, the shrink step,
    `IsSplitscreenPlayer`);
  - the layout tool;
  - `ow-package --def-dump`;
  - local probes: installed Off strict `--slice-gate` runs on main 21a668a with the inputs stood in, reverted and
    never committed.

  Raw output stays under ignored `local/analysis/hud/`.
- **Findings** (UNVERIFIED in game):
  - the pass is a pure query on actor state and the HUD definition; no Flash is read;
  - its inputs are what a headless run lacks:
    - the HUD's data (script that also reaches movie natives; `HUDDef` set beside the movie's creation);
    - the pickups' registration (`PostBeginPlay` skipped while the pickup world has not begun play);
    - the view point (the `WillowPlayerCamera` layer);
  - the shrink step is read: not a split-screen player; the limit `MaxNumPickups` (by fit);
  - with every input stood in, the `look` passes; rerun on main eef4f83 (SetBase bound), link 7 passes all four
    steps.
- **Hub ruling (review of #806):** all four yes, each Off only, logged as a harness input, UNVERIFIED, with Reference
  unchanged:
  - a harness HUD that runs its data steps only, with no movie natives;
  - begun play for the pickup world (the `AddPickup` registration as the fallback). Link 6 must keep passing: an
    installed Off strict `--slice-gate` with the setting on, link 6 ok 3/3, each spawn's begin-play events once;
  - a view stand-in at the pawn's eye point, `Location` plus `BaseEyeHeight`, with the controller's `Rotation`;
  - the line check under `NoLevel` hits nothing, as the base-search stand-in does.

  Written into the note (sections 3, 4, 5, 8, the tests and the open questions).
- **Clean room:** a different lane implements this note.
