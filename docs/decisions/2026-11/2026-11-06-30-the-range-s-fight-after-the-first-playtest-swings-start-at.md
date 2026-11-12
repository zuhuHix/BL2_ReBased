## 2026-11-06: the range's fight after the first playtest: swings start at the stock range, the den starts unalerted, corpses limited (range lane)

AI-assisted (Claude). Record: docs/verification/COMBAT_RANGE.md "The first playtest's fight". Host and tool change, no parsing change. Automated checks only so far; not yet run in the
host, and no rule here is confirmed in the running game.

- A swing now starts inside the kind's stock attack range (`melee_start_range`, read by `tools/prepare_range.py` from the AI definition's melee actions); the fitted 640 volume only decides
  whether it lands. How that range is measured is UNVERIFIED. The host builds no range on a manifest without it, or with no manifest at all (the old engine-shape fallback for that case is gone), whichever tool started it, and any failed build leaves a reason on screen and for the slice gate.
- Critical hits from behind: the check now reaches the head from any side, and the shot's line must still cross the head sphere. A host rule, UNVERIFIED.
- Worn pieces are exported from a package that really holds them; a piece naming no material keeps what its import makes (the psycho's axe, UNVERIFIED look).
- Range enemies spawn facing away from the spot, so arriving does not trip the sight rule at once. A host layout, not a game rule. This addresses the first-update lockstep; it does not stagger the fight: once the den is alerted, members can still converge and land together. Damage and run speed unchanged; no stagger number invented.
- Ragdoll: depenetration limited, rise and sideways speed capped, each corpse measured and logged. Host stand-ins; the cause of the flung corpses is still UNVERIFIED.
- One way in: `tools/play_range.ps1` and `-owrangestart`, Sanctuary only. The name is not `-owrange`, which already passes the manifest path.
- `tools/real_game/cash_capture.ps1` gains a `den_read` stage with its own result, after the cash read. It has not run.

