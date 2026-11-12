## 2026-10-06: script swap 6b, the use key runs Marcus's stock OnUsed chain on the VM (UNVERIFIED natives)

AI-assisted (Claude), implementer lane I1, from `docs/verification/NATIVE_MARCUS_USE_CHAIN.md`. Pressing use on Marcus raises `OnUsed`
(link filter 2, payload instigator plus used component) on his AI-definition provider through the existing BehaviorProvider; the payload
fills `Brain`'s `PlayerWhoUsedMe` by connection index. The chain's behaviors (nine `Behavior_IsSequenceEnabled` checks, remote events, the
context dialog, `HasMissions`, `ShowMissionInterface`) run their own script. Bound natives, all UNVERIFIED: `IsBehaviorSequenceEnabled`,
`ResolveBehaviorProviderDefinitionReference`, `ActivateBehaviorEventFromScript`, `ActivateBehaviorOutputLink`, `Object.QueryInterface` (still
on the structural implements stand-in) and `WillowPawn.GetBehaviorConsumerHandle` (an accessor of the pawn's own field; no note describes it).
A small VM hook, `Runtime::overrideScript`, lets a host binding replace one presentation-only script RPC (`ClientGFxPlayMovie`); the host's
confirm of the offered entry (swap 6c) now runs from that event instead of directly from the key. On real data all nine checks answer "not
enabled" and the chain reaches the mission interface. Open: `BehaviorBase.GetBehaviorContext` is a stub, so the on-use dialog stays silent;
the spawn-time behaviors of his provider are not run. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 90/90 +
resume 11/11 (was 88 + 11), door suite 16/16. Verified: automated suites only.

