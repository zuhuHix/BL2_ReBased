## 2026-10-06: script swap 6c, Marcus's mission lists run as script (UNVERIFIED natives)

AI-assisted (Claude), implementer lane I1. The accept / turn-in screen's list logic runs as the game's script on a VM Marcus built from his
stock archetype (`GD_Marcus.Character.Pawn_Marcus`, his own directive table): `GetEligibleMissions`, `GetInProgressMissions` and
`GetRedeemableMissions` run unchanged. The tracker's availability queries `CanStartMission`, `CanEndMission` and `GetCompletedBranch` are bound
to MissionSystem predicates that `accept()` shares, UNVERIFIED (read from native code, not confirmed in game). `AcceptMission` and
`ServerCompleteMission` now get Marcus as the director. The host's use key confirms the one offered entry (redeemable first), a stand-in for
the button press; the screen itself is not hosted. Not modelled: Failed, repeatable and blocked missions, RequiredObjectivesComplete; the
tracker holds one mission record, so Marcus's other six directive entries are never offered. Checks: ctest 11/11, verify_packages 9/9, UE
build succeeded, quest suite 88/88 + resume 11/11 (was 86 + 11), door suite 16/16. No new stubs reached. Verified: automated suites only.

