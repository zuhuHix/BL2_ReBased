## 2026-10-06: native notes on behavior context resolution and Marcus's on-use dialog (UNVERIFIED)

AI-assisted (Claude), analyst lane G21, Ghidra; own-words note `docs/verification/NATIVE_BEHAVIOR_CONTEXT.md`, no listing text.
`BehaviorBase.GetBehaviorContext` is a pure resolver over its arguments: the context struct's selector picks Self, Instigator, the other
participant or the struct's own ContextObject; event data or any other value gives None. It reads no kernel state. The behavior thread
runner fills that struct from an Input link (first resolved object, selector 4) before the behavior runs, and clears it afterwards; both
parts are needed for Marcus's on-use dialog. `WillowAIPawn.PlayOnUseDialog` (script) picks redeemable, eligible, in progress or no missions,
in that order, and triggers the matching generic on-use tag; it has no "first time" gate. `WillowPawn.GetBehaviorConsumerHandle` is a field
read; the field starts at -1 (invalid) and `InitializeBehaviorProviders` assigns it once. A Context link that resolves to nothing skips
the behavior for that pass. Corrections to NATIVE_BEHAVIOR_POPULATION, SANCTUARY_RPG_MISSION, NATIVE_USE_INTERACTION, SLICE_AUDIO_CHAIN and
NATIVE_MARCUS_USE_CHAIN are listed in the note. All UNVERIFIED in game.

