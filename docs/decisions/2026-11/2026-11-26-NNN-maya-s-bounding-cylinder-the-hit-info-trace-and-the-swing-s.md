## 2026-11-26: Maya's bounding cylinder, the hit-info trace and the swing's tail

AI-assisted (Claude Code)

Lane ANALYST-GETBOUNDINGCYLINDER-AND-AFTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and
no bounds check changed.

- **Why:** #796 puts links 4 and 13's next stop at `Actor.GetBoundingCylinder`'s world transform `NotRead`.
- **What:**
  - the new note `docs/verification/NATIVE_BOUNDING_CYLINDER_AND_AFTER.md`;
  - five NATIVE_INDEX rows and three edited rows;
  - pointers in NATIVE_GO_DOWN_PATH.md (3.2 and section 5) and NATIVE_COMPONENT_BOUNDS.md;
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (execs and class-table slots);
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off `--combat-run` on main 54c9cf05 with a call trace, each stop stubbed in turn. The
    temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/bcyl/`.
- **Findings** (UNVERIFIED in game):
  - Maya's stop is her skeletal mesh component's non-zero `Translation`. With zero rotations and unit scale the world
    origin is `Location` + `Translation` under any composition order;
  - `TraceComponent` traces one attached component with its own line check: a `LayerAbsent` in the port;
  - read with Off answers: `TryFullBodyGib` (its gates), `GetParent` (resolved), the shield getters, `ShakeView`,
    `DoForceFeedbackWaveform` (its gates), `GetCombatMusicManager` (the field).
- **Next** (from the probe): `GetCombatMusicManager`'s lazy creation, `NotRead` with the field None.
- **Needs the hub:** whether `TraceComponent` may answer a miss as a stand-in.
- **Clean room:** a different lane implements this note.
