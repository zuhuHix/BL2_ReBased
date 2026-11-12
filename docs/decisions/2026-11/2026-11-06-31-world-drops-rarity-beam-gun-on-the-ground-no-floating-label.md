## 2026-11-06: world drops: rarity beam, gun on the ground, no floating label (gunlook)

AI-assisted (Claude Opus 5.5; critic Luna). Record: `docs/verification/WEAPON_VISUALS.md` section 20. Host frames only, no real drop frame.

- **Behaviour change (host look):** a dropped item shows a thin vertical beam in its card's rarity colour (white without one), on every
  tier, and a dropped gun lies on its side at full size. The floating text label is hidden. Stick size from the cooked data; which tiers
  show it, the scale and the pose are UNVERIFIED host choices.
- **Capture:** `-owdropshots` adds `_drop` and `_droplate` frames. The first beam of a run that has not fired is late by some ticks
  (seen in frames; the cause, first use of the effect material, is inferred).

