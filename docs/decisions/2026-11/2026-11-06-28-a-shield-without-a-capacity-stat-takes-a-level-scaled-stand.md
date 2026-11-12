## 2026-11-06: a shield without a capacity stat takes a level-scaled stand-in when equipped (maya lane)

AI-assisted (Claude). Record: `docs/verification/COMBAT_DOWN_RESPAWN.md` "Stand-in shield capacity". Host behaviour, no parsing change.

- Vendor and loot shields have no stats, so they gave Maya no protection. The walker now gives such a shield a capacity from a rule whose numbers are in ignored local data; a card's own capacity wins, and without the file nothing changes but a warning.
- The rule is UNVERIFIED, a stand-in until a live read of capacity against level. No vendor code changed.

