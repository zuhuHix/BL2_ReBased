## 2026-11-03: a gun range outside Sanctuary with static dummies and moving enemies (range lane, PR1)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec, reviewed by the lead). Record:
[docs/verification/COMBAT_RANGE.md](docs/verification/COMBAT_RANGE.md).
- **New host feature, no parsing change.** `ow.Range` builds a test area and teleports Maya there: three Fire-mission stock
  dummies (engine shapes without the slice data) and moving enemies (`AOpenWillowEnemy`, a subclass of the combat target so
  every hit path reaches it) with a patrol. Toggles `ow.RangeMode normal|invulnerable|passive|both`, `ow.RangeLevel`.
- **Placement is a run-time scout** (rings 6,000 to 14,000 uu around the plaza; flat, open, near the plaza's height), because
  this lane cannot look at the map; a spot found good is pinned in `local/range/range.json`. Importing another map (Three Horns
  Divide) for the range stays a maintainer decision.
- **Stock numbers:** enemy max health per level from the VM's spawn of the stock pawn (`tools/prepare_range.py`, ignored
  manifest), critical x2.0 (class default). Host stand-ins: the head sphere as the critical region, shield-first order on enemies,
  ragdoll after 0.84 s, 8 s respawn, the health bar and the orange critical number. All UNVERIFIED in game.
- **AOpenWillowCombatTarget:** additions only, off by default (invulnerable flag, respawn of a stock pawn with ragdoll, damage and
  death hooks); the Fire dummy, the firing test and the Phaselock paths keep their old behaviour.
- **Open:** the VM's adult L5 maximum (614.08) against the recorded fight's first health (360.876); bandit and psycho not
  spawnable in the VM yet; AI (PR2) and loot (PR3) to follow.

