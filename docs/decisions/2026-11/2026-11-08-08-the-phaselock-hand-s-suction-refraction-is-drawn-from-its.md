## 2026-11-08: The Phaselock hand's suction refraction is drawn, from its compiled shader (showcase follow-up)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "The suction refraction".

- **Change:** `Mat_SirenSuction` is no longer an allowed skip of the effect import. It is built as an exact material: the scene colour read at the
  screen UV shifted by its distortion map, with the opacity terms read from the shader (camera-distance fade, mask x particle alpha, soft depth fade,
  screen-edge mask).
- **Why:** the +0.80 s burst is the largest remaining gap on the showcase cards, and this was the one layer of it the host never drew.
- **Not verified:** not imported or captured yet; whether it closes the gap is for the capture and a critic. No parsing behaviour changed.


