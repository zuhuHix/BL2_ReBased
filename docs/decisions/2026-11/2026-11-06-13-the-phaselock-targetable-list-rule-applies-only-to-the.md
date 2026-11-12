## 2026-11-06: the Phaselock targetable-list rule applies only to the mission's dummy (range lane)

AI-assisted (Claude). The slice gate's `phaselock_enemy` link failed: `PreferredPhaselockTarget` skipped every stock pawn the quest had not registered whenever the quest was on
(written for the mission's dummy, 2026-10-02), so with `-owquest` and `ow.Range` no range pawn could be targeted. The rule now covers only the mission's dummy, the quest's one stock
pawn by reading the code, which keeps it. Other targets count as registered, as they already did with the quest off (host stand-in, UNVERIFIED in game). Host behaviour only;
no parsing change. Record: docs/verification/COMBAT_RANGE.md "Phaselock on range pawns".


