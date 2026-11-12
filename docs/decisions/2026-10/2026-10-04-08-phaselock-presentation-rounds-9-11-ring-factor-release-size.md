## 2026-10-04: Phaselock presentation rounds 9-11: ring factor, release size, soft streaks and ground wash, hand spikes, check-61 fix

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 9" to "Round 11"). All calibrations UNVERIFIED.

- **Bubble too large (cause):** round 6 had replaced the ridge detector's ring factor (0.88) with the bubble texture's
  own ring (0.78), which drew the bubble about 13% too large. Restored; the per-template stand-in radii are 290, 233
  and 210. The release shell was re-measured on the game frames (112 px radius at 4.80 s, 176 px at 5.00 s); round 9's
  smaller figure was a mis-measure.
- **Hand timing:** round 8's whole-effect early start shrank the swirl and disc. Now only the palm orb (0.03 s) and the
  disc's alpha (0.1 s) run ahead; the disc's size curve is read at its unshifted age. The hand effect starts at the
  cast clip's notify; the clip still plays at 0.85.
- **0.65 s spike rays:** they come from the hand template's star-burst emitter; the game frames show none, and why was
  not found. The host scales that emitter to 0.15 (a new per-emitter gain).
- **Look:** soft pale-blue ground wash (light gain 6.5, radius 0.85x, colour 20% toward white), wider and softer
  streaks, release spikes and ribbons at reduced strength, tattoo glow x0.3.
- **Quest check 61 flake** (light 32 vs 4 once): traced to an intermediate build that reported UE5's intensity rather
  than the data value (32 = 4 x a gain of 8). The light's data-unit brightness is now stored when it is set, so the
  check no longer depends on the host gain. The check itself is unchanged.
- **Visual review:** independent critics scored rounds 9, 10 and 11 at 5.5, 5.5 and 6.3. Blind A/B against round 8:
  round 9 and round 10 each won one sheet of three; round 11 won bubble and release and tied hand, so it is committed.
- **Open:** the 0.30 s dark void (about 200 px against the game's 330; two enlargements made it fade, cause not found),
  the 0.80 s whiteout, interior opacity, rim weight, ice-shard flash facets, straight release shards, fist clench, target
  animation (no stock clips on the dummy), slight egg shape.
- **Checks:** quest suite first run 79/0, resume 11/0 on the final build. CTest and `verify_packages` in the final pass.

