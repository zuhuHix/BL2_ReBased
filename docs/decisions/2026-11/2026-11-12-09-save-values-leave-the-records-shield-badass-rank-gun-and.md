## 2026-11-12: Save values leave the records: shield, Badass Rank, gun and items

AI-assisted (Claude Code). The second pass of the clean-up that
`docs/decisions/2026-11/2026-11-12-08-save-values-leave-the-checkers-fixtures-and-fee-records.md` began. Rule 1 says nothing
copied from a player's save is tracked. The records still held the rest of the maintainer's own save state: the numbers of
her own shield (capacity, recharge delay and rate, and the fractions of them that give the capacity back), the values that
carry the profile's Badass Rank (the health maximum with the bonus, the bonus factor, a gun's damage with it, her
controller's grenade damage modifier), her own gun's damage and its level-up value, a skill-modified accuracy impulse, and
her own items named (the shotgun, the slot-1 gun, the grenade mod). Each statement now
says what it needs about behaviour: the rule, the base value that the game data gives for any save (the base health
`80 x 1.13^L`, the target's health, the damage chain's hit amounts), a ratio or fraction (0.25 and 0.9 of the maximum, 0.5
of the capacity per second, a +1.0 Scale on the existing sum), the sample count, and that the amount was read live and is in
the maintainer's private record, the only place it survives. Every `UNVERIFIED` stays. History is not rewritten.

Edits: docs/decisions/2026-09/2026-09-27-02-host-owned-skill-points-for-maya-level-xp-spending.md
Edits: docs/decisions/2026-10/2026-10-07-12-real-game-lane-l3-a-real-fight-recorded-a-level-up-resets.md
Edits: docs/decisions/2026-10/2026-10-07-13-script-swap-9-the-natural-level-up-s-timed-weapon-damage.md
Edits: docs/decisions/2026-10/2026-10-08-09-the-stock-hit-chain-on-the-vm-for-a-player-hit-on-an-ai-pawn.md
Edits: docs/decisions/2026-10/2026-10-08-12-the-stock-death-chain-for-a-player-kill-of-an-ai-pawn-on-the.md
Edits: docs/decisions/2026-10/2026-10-09-01-real-game-lane-l8-bullymong-targeting-and-melee-observed-in.md
Edits: docs/decisions/2026-10/2026-10-09-04-maya-s-down-state-second-wind-and-respawn-on-the-vm-plan.md
Edits: docs/decisions/2026-10/2026-10-09-06-real-game-lane-l9-bullymong-awareness-attack-route-melee.md
Edits: docs/decisions/2026-11/2026-11-03-16-accuracy-pool-view-kick-and-spread-from-live-reads-slice.md
Edits: docs/decisions/2026-11/2026-11-06-19-maya-throws-the-stock-grenade-mod-on-g-slice-projectiles.md
Edits: docs/decisions/2026-11/2026-11-06-20-the-frag-s-contact-fuse-and-falloff-come-from-its-behaviour.md
Edits: docs/decisions/2026-11/2026-11-07-19-the-stand-in-shield-rule-takes-the-maintainer-s-numbers-maya.md

In each of these only the save value changed, and nothing else: the level-45 trace's experience total (now "between the
level-45 and level-46 thresholds"; the next-level value is the curve's and stays), her own shield's delay, rate and boost
(now "its own delay and rate", "0.5 x the capacity per second"), the level-up damage pair and scale sums (now "+1.0 on the
existing sum"), the grenade modifier, the pellet damage and the health trail it gives (now "three equal steps, the live trail
to 0.01"), the killing pellet's health result, the Badass Rank scale and the prediction built on it, the respawn fees with
the cash they were charged on (the 0.07 half-up rule and the three live respawns stay), the shotgun's kind, the grenade mod's
name and the old stand-in shield value.

- Records: `COMBAT_GOLDEN_TRACE.md`, `COMBAT_DOWN_RESPAWN.md`, `NATIVE_LEVEL_UP_ATTRIBUTES.md`, `NATIVE_DOWN_TIMERS.md` (an
  example maximum of 500 in place of the save's), `NATIVE_PROGRESSION.md`, `AI_MELEE_TARGETING_LIVE.md`,
  `SANCTUARY_RPG_MISSION.md`, `COMBAT_HIT_CHAIN.md`, `COMBAT_DEATH.md`, `AI_AWARENESS_LIVE.md`, `WEAPON_PROJECTILES.md`,
  `SLICE_AUDIO_CHAIN.md`, `SLICE_AUDIO_LIVE.md`, `REALGAME_GROUND_TRUTH.md`. In the last, the
  Fire mission's experience total (the first pass's item) is left for a later change, because an open change adds to that
  file.
- `COMBAT_DEATH.md`: the killing-pellet step takes an invented 40. Verified on 2026-11-12: the `--combat-run` with 40 and with
  the live pellet differ only in the amounts and the run times (15 of 434 output values), so the death order and post-state
  the record gives hold for the invented step.
- Fixtures: `tests/realgame_snapshot_test.py` (the stub's shield capacity and her own shield's level, invented 150 and 4 as
  in the first pass) and `tests/phaselock_integrity_probe_test.py` (the current shield strength, invented 75, half the
  invented 150). Both say so in a comment, and both tests pass unchanged in what they check.
- Two references to the session of 2026-11-12 now give its full condition: every SDK mod of the session's list off as read
  in the game, with Quick Startup on (`SANCTUARY_RPG_MISSION.md`, `COMBAT_DOWN_RESPAWN.md`).

Not changed here: the host's default shield values in `OpenWillowMayaState.h` and the sentences that state them
(`COMBAT_DOWN_RESPAWN.md`, the recharge bullet's stand-in sentence; the 2026-11-03-11 decision), which another change
replaces; comments under `host/`; the checker `tools/verify_combat_pawn.py` (a comment); and values that give a save value
back only through a ratio or a damage chain (the 0.7589 damage factor, the live damages that factor gives, the grenade splash
amounts), and the kinds of six of the player's own weapon cards in `WEAPON_BALANCE_DECODE.md` and
`NATIVE_WEAPON_RULES.md` (that record's results are set out by those kinds; its per-card values were already private). Those
need the maintainer's call.
