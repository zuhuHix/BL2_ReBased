## 2026-11-12: Save values leave the records: shield, Badass Rank, gun and items

AI-assisted (Claude Code). The second pass of the clean-up that
`docs/decisions/2026-11/2026-11-12-08-save-values-leave-the-checkers-fixtures-and-fee-records.md` began. Rule 1 says nothing
copied from a player's save is tracked. The records still held the rest of the maintainer's own save state: the numbers of
her own shield (capacity, recharge delay and rate, and the fractions of them that give the capacity back), the values that
carry the profile's Badass Rank (the health maximum with the bonus, exact or rounded, the bonus factor, the damage factor of
the spawned guns that gives it back with `1.13^8`, the live gun damages built on either), her controller's grenade damage
modifier and the splash amounts and target health trail that give it back, her own gun's damage and its level-up value, a
skill-modified accuracy impulse with the kicks, floor and fall rate it sets, and her own items named (the shotgun and its
clip size, the slot-1 gun, the grenade mod, a pistol from the save, and the makers and elements of six of her weapon cards
in a UI trace). Each statement now says what it needs about behaviour: the rule, the base value that the game data gives
for any save (the base health `80 x 1.13^L`, the target's health, the card damages at a grade, the damage chain's hit amounts
on her), a ratio or fraction that does not give the save value back (0.25 and 0.9 of the maximum, 0.5 of the capacity per
second, a +1.0 Scale on the existing sum, 1.5 for the flesh modifier), the sample count, and that the amount was read live
and is in the maintainer's private record, the only place it survives. The weapon type stays where a table needs it. Every
`UNVERIFIED` stays. History is not rewritten.

Edits: docs/decisions/2026-09/2026-09-27-02-host-owned-skill-points-for-maya-level-xp-spending.md
Edits: docs/decisions/2026-10/2026-10-01-06-weapon-balances-card-stats-and-loot-pools-decoded-for-the.md
Edits: docs/decisions/2026-10/2026-10-07-12-real-game-lane-l3-a-real-fight-recorded-a-level-up-resets.md
Edits: docs/decisions/2026-10/2026-10-07-13-script-swap-9-the-natural-level-up-s-timed-weapon-damage.md
Edits: docs/decisions/2026-10/2026-10-08-09-the-stock-hit-chain-on-the-vm-for-a-player-hit-on-an-ai-pawn.md
Edits: docs/decisions/2026-10/2026-10-08-11-real-game-lane-l7-projectiles-and-firing-patterns-observed.md
Edits: docs/decisions/2026-10/2026-10-08-12-the-stock-death-chain-for-a-player-kill-of-an-ai-pawn-on-the.md
Edits: docs/decisions/2026-10/2026-10-08-14-host-gun-fire-follows-the-live-firing-mode-lane-hf.md
Edits: docs/decisions/2026-10/2026-10-09-01-real-game-lane-l8-bullymong-targeting-and-melee-observed-in.md
Edits: docs/decisions/2026-10/2026-10-09-04-maya-s-down-state-second-wind-and-respawn-on-the-vm-plan.md
Edits: docs/decisions/2026-10/2026-10-09-06-real-game-lane-l9-bullymong-awareness-attack-route-melee.md
Edits: docs/decisions/2026-11/2026-11-03-16-accuracy-pool-view-kick-and-spread-from-live-reads-slice.md
Edits: docs/decisions/2026-11/2026-11-06-19-maya-throws-the-stock-grenade-mod-on-g-slice-projectiles.md
Edits: docs/decisions/2026-11/2026-11-06-20-the-frag-s-contact-fuse-and-falloff-come-from-its-behaviour.md

In each of these only the save value changed, and nothing else: the level-45 trace's experience total (now "between the
level-45 and level-46 thresholds"; the next-level value is the curve's and stays), the makers and an element of two of her weapon cards,
her own shield's delay, rate and boost (now "its own delay and rate", "0.5 x the capacity per second"), the level-up damage
pair and scale sums (now "+1.0"), the grenade modifier, the pistol and splash amounts and the trail they give (now "1.5 times
what arrives", "matches its live row to a relative 3e-7", "what leaves 1.0"), the pellet damage and the health trail it
gives (now "three equal steps, matching the live trail to within 0.01"), the spawned guns' damage factor (now "a constant
fraction", "L7's damage factor"), the killing pellet's health result, the Badass Rank scale and the prediction built on it,
the respawn fees with the cash they were charged on (the 0.07 half-up rule and the three live respawns stay), the shotgun's
kind and the grenade mod's name.

- Records: `COMBAT_GOLDEN_TRACE.md`, `COMBAT_DOWN_RESPAWN.md`, `NATIVE_LEVEL_UP_ATTRIBUTES.md`, `NATIVE_DOWN_TIMERS.md` (an
  example maximum of 500 in place of the save's), `NATIVE_PROGRESSION.md`, `AI_MELEE_TARGETING_LIVE.md`,
  `PROJECTILE_FIRING_PATTERNS_LIVE.md`, `SANCTUARY_RPG_MISSION.md`, `COMBAT_HIT_CHAIN.md`, `COMBAT_DEATH.md`,
  `AI_AWARENESS_LIVE.md`, `WEAPON_PROJECTILES.md`, `WEAPON_BALANCE_DECODE.md` (its six cards are now named by type, "SMG 1"
  and "SMG 2", and "elemental"), `NATIVE_WEAPON_RULES.md`, `SLICE_AUDIO_CHAIN.md`, `SLICE_AUDIO_LIVE.md`,
  `REALGAME_GROUND_TRUTH.md` (also the Fire mission's experience total, the first pass's item, now "+395").
- `COMBAT_DEATH.md`: the killing-pellet step takes an invented 40. Verified on 2026-11-12: the `--combat-run` with 40 and with
  the live pellet differ only in the amounts and the run times (15 of 434 output values), so the death order and post-state
  the record gives hold for the invented step.
- Fixtures: `tests/realgame_snapshot_test.py` (the stub's shield capacity and her own shield's level, invented 150 and 4 as
  in the first pass) and `tests/phaselock_integrity_probe_test.py` (the current shield strength, invented 75, half the
  invented 150). Both say so in a comment, and both tests pass unchanged in what they check.
- Three references to the session of 2026-11-12 now say Quick Startup stayed on (`SANCTUARY_RPG_MISSION.md` twice,
  `COMBAT_DOWN_RESPAWN.md`), and two of them give the full "every SDK mod of the session's list off as read in the game".
- Claims whose only evidence is now in the private record say so where they stand: the +1.0 on the existing scale sum, the
  constant effective-to-base ratio, the hit chain's match on the Fire dummy, and the read kicks of the accuracy table.

Not changed here: the host's default shield values in `OpenWillowMayaState.h` and the records that state them or the old
stand-in capacity (`COMBAT_DOWN_RESPAWN.md`, the recharge bullet and the capacity correction; the 2026-11-03-11 and
2026-11-07-19 decisions), which `2026-11-12-09-the-host-s-default-shield-recharge-becomes-an-invented-stand.md` rewrote
without the save's values. Left for one follow-up change: the save's values in comments under `host/` and `tools/`
(comments only, checked by the hub's in-engine run). Nothing else is deliberately left for a further pass.
