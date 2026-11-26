## 2026-11-26: Link 6's take: UpdateAmmoCounts to the end of key

AI-assisted (Claude Code)

Lane ANALYST-UPDATEAMMOCOUNTS, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** with #778, link 6 stops at `UpdateAmmoCounts`' `NotRead` inside `PickupPickupable`. The hub asked for its
  body and the rest of the take, through the end of `key`.
- **What:**
  - the new note `docs/verification/NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md`;
  - eight NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (the execs and class-table slots of eight natives);
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off strict `--slice-gate` on #778's head 2f6f7da with a call trace, each stop stubbed
    in turn. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/ammo/`.
- **Findings** (UNVERIFIED in game):
  - `UpdateAmmoCounts` walks the eight ammo kinds, creating missing pools, and announces a rise only when not silent.
    The second call is silent exactly when the take added an item;
  - read with Off answers: `GetSaveRotation`, `ComponentList`, `MakeNoise` (a no-op in this build), `IsUsingVehicle`,
    `IsDLCRequirementMet`, `SetOwner` (two steps beyond the port's `set_owner`) and `GetResourcePoolContext`;
  - two gaps:
    - the pickup world's controller has no `WorldInfo`, so `PickupQuery` reads through None and the take fails into
      `FailedPickup`;
    - `EvaluateExpressions` is not bound as a native.
- **Next** (from the probe): `Actor.SetBase` in `WillowInventory.GivenTo`, an existing `NotRead`.
- **Needs the hub:**
  - the controller and pawn `WorldInfo` harness input;
  - `GetSaveRotation`'s at-rest answer.
- **Clean room:** a different lane implements this note.
