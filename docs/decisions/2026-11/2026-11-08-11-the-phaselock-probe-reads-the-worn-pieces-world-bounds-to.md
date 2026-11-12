## 2026-11-08: The Phaselock probe reads the worn pieces' world bounds, to check their placement (showcase follow-up)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "The worn pieces' placement: a world-space check".

- **Change:** the live probe adds each attached piece's world `Bounds`; `tools/phaselock_attachment_check.py` turns the bounds' origin into an offset on
  the piece's bone and compares it with `host_target.json`: the pivot position at one held frame of the kept hit attempt, within 8 units (a host
  choice). Not orientation or scale. No host or placement change.
- **Why:** the showcase reads the claws as separated, but `LocalToWorld` read null, so the placement was never checked. The frames point at colour first.
- **Not verified:** the probe change has run on stubs only; it needs one real-game hit run.

