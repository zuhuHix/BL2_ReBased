## 2026-10-05: native notes on mission-linked behavior conditions, population spawning and kernel leftovers (UNVERIFIED)

AI-assisted (Claude), analyst lane G2, Ghidra; own-words note `docs/verification/NATIVE_BEHAVIOR_POPULATION.md`, no listing
text. `BehaviorSequenceEnableByMission`: every `MissionReaction*` recomputes the verdict from the tracker's state and
re-applies Enable/Disable to all linked sequence records (events fire only on a real change); objective states
NotStarted/Active/Complete are derived with mission-status gating; `ObjectiveSetRestrictions` apply; the condition
registers as an observer at provider registration and gets an immediate level-load call (before `OnSpawned`).
`SequenceEventEnableByMission` uses the same verdict. Population: `MissionPopulationAspect` is script; the Fire den is
enabled only while its objective is active and spawns on a later master tick under radius/height/capacity/time gates;
the spawn order up to `OnSpawned` is recorded. Kernel: a thread stops at the next behavior boundary when its sequence is
disabled; context-list resolution; latent copies with a 1/60 s minimum wait; `FilterObject` only with a caller filter.
Corrections recorded in the note: the enable rule in `src/slice.hpp`/BEHAVIOR_DATA_DECODE (not applied yet), the
set-changed notification source (NATIVE_MISSION_DISPATCH B3), and `tools/ghidra/class_layout.py` offsets on
`WillowPawn`-derived classes (4 bytes lower than the executable from `ConsumerHandle` on; cause not investigated). All
UNVERIFIED in game.

