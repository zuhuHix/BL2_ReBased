## 2026-11-12: The respawn wait split on the station path

AI-assisted (Claude Code). Until now a respawn put Maya at the station at once and one 11.8 s timer charged the fee. The
real game splits those 11.8 s into parts. Observed in the real game on 2026-11-12: six respawns on the station path on
two maps (two in Sanctuary with every SDK mod off, four in an enemy area with the mods loaded), level 8. All six agree to
within a frame or two.

- `FOpenWillowRespawnTimeline` (`OpenWillowRespawnTimeline.h`, plain C++) names the moments, counted from the respawn
  start (the game's `ResurrectPlayer`): the hold ends at 3.0 s with the arrival at the exit point. From the arrival come
  the effect start (+1.5), gear back (+4.5), the effect end (+6.5), the cost display (+6.8) and the fee (2.0 s after the
  cost display, 11.8 s after the start in all). The walker owns one and applies the moments as they come. The fee reads
  its due time from it, and `FOpenWillowRespawnFee::DelaySeconds` is the timeline's total.
- What the walker does with them: at the start, full health, the exit picked, pages closed (as before), and then hidden,
  with the pawn's input off (`DisableInput`) and hits ignored. At the arrival she is moved there, and the quest's respawn
  count and the "died and respawned" log line move to this moment. At the effect start she is unhidden. Before the gear
  is back, fire, reload and melee do nothing. Input comes back at the cost display. The effect end and the cost display
  draw nothing: the host has no death camera, no re-materialise effect and no cost-display widget.
- The cost display is at 6.8 s, not 6.79: the six samples lie between 6.795 and 6.808, and 6.8 keeps the total at the
  observed 11.8 (11.800 to 11.814).
- `UNVERIFIED`: the frame at which input comes back (the game's is only bracketed, 6.4 to 7.4 s after the arrival; the
  host takes the cost display, which lies inside that bracket). Also that nothing reaches her during the hold (inferred
  from the game's holding cell, not tried), and that hits land from the arrival on while she is still hidden. The path
  without a station was not observed, and nothing is said here about the visuals.
- A known host difference, `UNVERIFIED` for what it does to a fight: in the game she spends the hold in a holding cell
  far from the map. The host leaves her where she went down, as an invisible capsule that still collides, so enemies
  keep aiming and shooting at that spot and their projectiles stop on her without damage. A teleport by any other
  system during the hold is overwritten at the arrival.
- Every held key is let go at the respawn start (fire, aim, sprint, crouch, jump), because a key released during the
  input lock never reaches its release handler.
- The fee rule is unchanged, and so is a second respawn before the charge (it starts the timeline again and replaces the
  fee). The charge still falls due while she is down again. One consequence of the hold: she can only be down again
  before the charge after the arrival.
- The slice gate's `bleed_out_respawn` link now waits past the arrival. It also FAILs unless the hold was seen with
  Maya hidden and without input in every frame, and the arrival came 3.0 s after the start, late by at most the longest
  frame of the hold. Its "respawned N s after going down" now comes from the walker's own down and respawn times, so it
  is the same 20 s as before. The gate's waits for her own respawn last until input is back, so no later link starts
  while she is held. The link is judged once input is back, and it also FAILs unless she is then visible with her
  input on again: this is the only check on the actor's restore. The HUD down-shot run, the weapon-firing suite's burst
  respawn case and the controls suite's second down wait for the same moment; the firing case keeps counting shots
  until then, so a burst that still owed shots would fire them once the gear is back and fail its `extra_shots=0`. The
  quest suite's wait for the respawn grows by the hold (`HoldSeconds`).
  `tools/check_slice_gate.py` reads only the link names and the `how=` token, so it is unchanged.

Checked by this lane: the UE module build and the light suite. Written but not run by this lane (it needs the editor):
the new automation test `OpenWillow.MayaState.RespawnWaitSplit` (invented start times and purse: each part within one
frame, the hold hidden and unable to act, the fee from the timeline), and the in-engine suites that show the hold in
the running host. Both are for the integration run.
