## 2026-11-03: range enemy AI: notice, approach and melee from the live records (range lane, PR2)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md) "Enemy AI (PR2)".
- **New host behaviour, no parsing change.** Range enemies share one threat record (the range is one den). They notice
  Maya by sight (previous update's line of sight, front half-plane, within the den's ThreatRadius) or by proximity (768 minus
  the collision radius, within the PatrolRadius), and are provoked by her damage. ForgetTime is +10 s / +25 s, with the fall-back
  at the den's 1 s update. They run at her and swing: damage 0.41 s after the swing start if she is still in the fitted melee
  volume, one swing per 1.25 s. The rules come from AI_AWARENESS_LIVE and AI_MELEE_TARGETING_LIVE; the host's reproduction
  is UNVERIFIED in game.
- **Melee damage from the VM:** `tools/prepare_range.py` now also runs `maya:L spawn:<kind>:L swing:<n>:melee` per kind and
  level; its arrival and ratio rows reproduce every live melee row of COMBAT_MELEE (adult L5/6/7, regular L6, baby L6). The
  level scale beyond two levels above Maya is a stand-in line to 3.0.
- **Delivery to Maya** is one stub (`RangeDamagePlayer`, UE ApplyDamage) until the maya lane's
  `OpenWillowApplyDamageToPlayer` (PR #72) merges.
- **Not modelled:** momentum on Maya, leaps, the slinger's throw, the stock AI tree.

