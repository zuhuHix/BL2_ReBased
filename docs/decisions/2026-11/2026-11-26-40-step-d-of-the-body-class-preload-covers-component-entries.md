## 2026-11-26: Step (d) of the body-class preload covers component entries

AI-assisted (Claude Code, Claude Opus 5.5). Queue 0al, review of #779. A wording fix: no behaviour changes.

Edits: docs/decisions/2026-11/2026-11-26-37-the-body-class-s-provider-definitions-are-preloaded.md

That entry said step (d), the pawn's instance-data hit regions, "adds nothing". It now says it adds nothing **for
component entries**: the body composition refuses a component with a hit region, and it is the port's only writer of
`InstanceDataState.Data`, so no other kind of entry is there.

- **Why not preload (d) for every kind:** NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md step (d) reads
  `ComponentData.HitRegionDefinition` on each entry. For an entry of another union kind that field overlays the other
  kind's data, and what it holds there is not read. Preloading through it would load on an unproven read, so the
  preload stays as it is, and the wording names the case it covers.
- **Also in this change** (no decision needed, tests only): two `GetPawnInventoryManager` tests (review of #778), a
  `WillowGame.WillowPawn` pawn's manager and a manager the store has not built (`NotRead`).
- **Verified:** the tests on invented data. **Not verified:** anything in the running game.
- **Sensitive files:** none.
