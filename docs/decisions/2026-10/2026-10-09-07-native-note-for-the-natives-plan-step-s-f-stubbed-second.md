## 2026-10-09: native note for the natives plan step S-f stubbed: second wind, injured queries, timers, special moves, protection timer, respawn (lane G27)

NATIVE_DOWN_TIMERS.md (lane G27): analyst read of the natives plan step S-f stubbed when Maya goes down, bleeds out, takes a second wind or respawns. Read: `SetSecondWindReason` / `ClearSecondWindReason` (a guarded
byte store: only for an injured pawn, first writer wins; no health change, so the kill-frame 25 percent health is script or data, probably the `KilledEnemyBehaviors` run in the same call), `PassSecondWindReasonToLeviathan`
(statistics hand-off), `Pawn.IsInjured` / `IsDead` / `WillowPawn.IsInjuredDead` (flags; a downed Maya is injured, not dead), the down state's timers (all one-shot; `CheckShouldBeAutoRevived` is armed once per injury, not every
2 s), `SpecialMoveComponent.Play` (returned length is the animation length divided by the effective rate; the named callback runs once at the move's end, with `bInterrupted` true when replaced, at once when the length is 0),
`GetAnimLength`, the protection timer (`EnableProtectionTimer` sets a flag, a 2 s one-shot and a random health floor between 1 and 10 percent of the maximum capped by current health; `ProtectionTimerElapsed` clears the flag),
`CheckLowHealthState` (per-frame edge trigger of the player class's low-health behavior lists), `ResetAI` (low confidence), `Pawn.SetAnchor`, `BehaviorBase.RunBehaviors` (loop over `ApplyBehavior`), and, as script facts,
`TeleporterDestination.GetNextExitPoint`, `TravelStation.CanResurrectHere` and the scheduler of `ShowRespawnDialog` (script, two 2.0 s one-shots). Corrections to NATIVE_LEVEL_UP_ATTRIBUTES.md section 9 (the guard is `IsInjured`),
NATIVE_DAMAGE_DEATH.md 10.2 (auto-revive check once) and stage 15 (random floor). Every rule lists the live observation that would confirm it. REA not used (cannot open Borderlands2.exe); Ghidra via
`tools/ghidra/run.ps1` plus G26's three local scan scripts. No tracked source changed.

