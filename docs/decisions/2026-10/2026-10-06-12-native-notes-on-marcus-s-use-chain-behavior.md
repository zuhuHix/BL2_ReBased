## 2026-10-06: native notes on Marcus's use chain (`Behavior_IsSequenceEnabled` cascade) (UNVERIFIED)

AI-assisted (Claude), analyst lane G18, Ghidra; own-words note `docs/verification/NATIVE_MARCUS_USE_CHAIN.md`, no listing text.
Correction: `Behavior_IsSequenceEnabled`, `Behavior_HasMissions`, `Behavior_PlayAIMissionContextDialog`, `Behavior_ShowMissionInterface`
and `Behavior_RemoteCustomEvent` are script, not native; the VM runs them. The natives behind them: `BehaviorKernel.IsBehaviorSequenceEnabled`
(true only when the consumer has that provider registered, the provider has a sequence of that name and its enabled bit is set; a None
provider or an unknown name is false), `BehaviorHelpers.ResolveBehaviorProviderDefinitionReference` (non-empty path wins, else the reference,
else the behavior's Outer) and extra detail on `ActivateBehaviorEventFromScript` (None provider fires nothing, omitted filter means all
links). Check outputs: 0 enabled, 1 not enabled. Marcus's nine checks are a cascade ending in the context dialog, `Behavior_HasMissions` and
the mission interface; three of the names have no sequence in his provider. `OnUsed` has one output link (id 2) and writes the instigator
into `Brain`'s `PlayerWhoUsedMe`. This unblocks script swap 6b. Corrections to SANCTUARY_RPG_MISSION, NATIVE_USE_INTERACTION and
NATIVE_MISSION_DISPATCH are listed in the note. All UNVERIFIED in game.

