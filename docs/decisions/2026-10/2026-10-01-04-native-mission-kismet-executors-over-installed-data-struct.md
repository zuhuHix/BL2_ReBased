## 2026-10-01: native mission/Kismet executors over installed data; struct-embedded arrays by reflection

AI-assisted. Context: the Sanctuary + Maya slice needs mission, behavior and Kismet logic, and
`MissionTracker`, `BehaviorKernel`, `Behavior_AdvanceObjectiveSet/MissionRemoteEvent/ActivateMission/
CompleteMission` and `SequenceOp` activation are native in this build (no script), while their definitions
are readable data. Decision: implement small native executors in `src/kismet.*` and `src/mission.*` that
read the installed definitions through the VM's reflection-typed property reader, report world-acting ops
at a host boundary instead of running them, and record every guess as `UNVERIFIED` (see
`docs/verification/SANCTUARY_RPG_MISSION.md`). Do not recreate recoverable content.

Parsing behaviour change (`src/vm.cpp`): arrays inside structs are decoded using the struct field's own
reflection, and struct declarations are taken from the property declaration rather than looked up by name in
the object's package (which only worked for structs the package happened to import; `SeqOpOutputLink.Links`
had silently decoded as empty). No bounds check was loosened: element decoding still requires exact
consumption of the tagged size and fails the whole property otherwise. `CMakeLists.txt` gained `kismet.cpp`,
`mission.cpp` and a synthetic test; no dependency or license change. Verified: CTest 9/9, nine package
comparisons, Kismet census 0 unresolved links over `Sanctuary_Dynamic`, in-engine door suite 16/16.
Unverified: all native semantics against the original game (no paired capture yet).

