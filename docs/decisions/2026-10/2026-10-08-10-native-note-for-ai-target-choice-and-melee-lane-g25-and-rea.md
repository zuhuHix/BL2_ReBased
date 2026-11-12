## 2026-10-08: native note for AI target choice and melee (lane G25), and REA 6.0.0 cannot open Borderlands2.exe

NATIVE_AI_MELEE.md (lane G25): first read of how an AI picks a target and lands a melee hit, for plan step S-h. Script-first: the bullymong chain is Action_AnimAttack (script) ->
ActionSequence.TriggerBehavior("Attack") -> animation -> Behavior_MeleeAttack (script) -> MeleeDefinition.MeleeAttack (native). Target choice and action selection are native AIComponent code driven by
AIDefinition.NodeList and GD_AI_Targeting.Targeting_ShortRange data. Read: AIComponent cadence (target update then state update, one UpdateRate gate), FindTargets (XY radius), AddTarget, stale removal,
CalcTargetInfo, PrioritizeTarget (weighted mean of iterator OnePriority), ChooseBestTarget, SetTarget (Target_Lost / Target_Acquired), ValidCurrentTarget (CanTargetIf to acquire, StopTargetingIf to drop),
the targeting iterators' formulas, tree selection (action / random / priority / sequence, TimedStart armed on first target), TriggerBehavior, MeleeAttack radius volume (vertical band centre +- DamageHeight,
half-angle DamageConeAngle / 2, 3D radius, line-of-sight trace), hit application (hit-once, allegiance flags, damage and knockback, behavior lists). Everything UNVERIFIED; the note lists the pyunrealsdk
reads that confirm each rule. Not read: WillowAIComponent overrides, WillowMind awareness natives, IMeleeAttacker implementations, the over-time repeat. Tooling: REA 6.0.0 could not open Borderlands2.exe
(fixed 330 s startup deadline); the existing Ghidra project was driven with tools/ghidra/run.ps1 instead. No tracked source changed.

