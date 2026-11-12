## 2026-11-03: Phaselock round 22: the Adult bullymong, the palm orb's facets, the LDR cap (maya lane, item 3)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 22". Presentation change; no live reads.

- **Target archetype (tool data change).** `tools/prepare_phaselock_target.py` imports `GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult`
  (the "Adult Bullymong" of the real frames: brown `Mati_BullymongAdult`, 150 x 90 cylinder, no scale override), not the white
  `Pawn_PrimalBeast`. Checked with the tool's light steps (identity, extract, editor job: 4 UModel jobs, 0 failed).
- **Palm orb.** The star-burst layer's material alpha is stock again and its emitter gain is 0.5 (was about 3 % of stock).
  The hand-written effect stand-ins cap colour at 1 per channel, as the importer's own LDR rule does (was 4.0).
- **Capture switch** `-owtonecurve=<0..1>` for a tone-mapping A/B. The target's facing (0 or 180) is to be settled by an A/B capture.
- All UNVERIFIED until the hub's re-import, capture and critic round.

