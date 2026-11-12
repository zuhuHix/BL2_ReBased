## 2026-11-12: Den down window, health rule and strongest kind

AI-assisted (Claude Code). `cash_capture -DenDown` places Maya at a den with the run's immunity hook removed and waits for the pack
to down her. Session 2 of 2026-11-12 showed the old re-placement rule working against that: her shield refilled between
engagements and absorbed the first hits, so a rule on health plus shield saw "hits" that never reached her body, and a
re-placement at the nearest enemy could put her at a lone baby or slinger. In the first S2.1 run one re-placement ran and no down
followed within 150 s.

- `-DownSeconds` defaults to 240 s (was 150) and `-ReplaceAfterSeconds` to 20 s (was 10).
- A re-placement waits on her health only: `cp_down_state` returns the health, and a fall of it since the last poll is a hit. No
  re-placement until a health value has been read since the last one.
- `dp_replace` takes the strongest kind in reach first, from the pawn's `AIClass` field (`CharClass_PrimalBeastAdult`, then
  `CharClass_PrimalBeast`, `CharClass_PrimalBeast_Ranged`, any other kind, and `CharClass_PrimalBeastBaby` last); within that kind,
  the first placement's enemy, then the nearest. The first placement's enemy is placed at bearing 180, any other on her side.

Unchanged: at most four re-placements, none after a down, and every bound and driver-lost rule of #245. What is called on what:
`GetHealth` and `GetShieldStrength` on the player's own pawn; field reads only on each scanned pawn; `cb_place` sets the player's own
pawn's `Location`, calls `SetPhysics` on it and turns the player's own controller (`ClientSetRotation`).

Verified: the synthetic tests (stand-ins, no game), each new one failing on the old code. `UNVERIFIED`: whether the new rule brings a
down in the game, and whether the ranking by kind matches the den's real composition on another day. The worst-case time of an item
with `-DenDown` grows by about 90 s; the private runbooks carry the times.
