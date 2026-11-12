## 2026-10-06: native notes on save and load of player and mission state (UNVERIFIED)

AI-assisted (Claude), analyst lane G15, Ghidra; own-words note `docs/verification/NATIVE_SAVE_LOAD.md`, no listing text, no real
save opened. `MissionTracker.InitializeWorldMissionState` rebuilds the tracker from the controller's mission list on every load
(reset, apply in order, drop entries whose progress length mismatches, defer unmet dependencies, re-register listeners and re-grant
the mission weapon for Active/RequiredObjectivesComplete, re-evaluate, replay behavior events, observer kind 0). Pre-load fixup
clamps level, backpack (12..39), bank (6..24) and currencies and recomputes unspent skill points as max(0, L-4) minus points spent.
Save triggers: station use, status-menu close, quit, preference/playthrough changes; mission events never save. Load order and the
save's contents are listed; a mission saved Complete with `bNeedsRewards` still set pays again on load. Corrections to the
controller-helper, inventory and skills notes are recorded in the note. Open: whether the lent pistol returns for the standalone
primary player, DLC level-cap increments, the world-readiness test. All UNVERIFIED in game.

