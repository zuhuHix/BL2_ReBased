## 2026-11-07: a held target's death ends Phaselock at once (maya lane)

AI-assisted (Claude). Record: `docs/verification/ACTION_SKILL_HUD.md` "The held target's death". Host behaviour, no parsing change.

- Live on one Adult killed by one large hit during the hold, the game releases at once, the emblem goes and the cooldown drains. The host ran them to the timeline's end. It now ends through `EndPhaselockEarly`.
- A gun kill and a kill during the lift are UNVERIFIED; the host treats them the same.

