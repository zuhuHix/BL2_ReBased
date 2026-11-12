## 2026-09-10: UE5 host probe scaffolded; engine decision deferred to the gate

`host/ue5/OpenWillow` is a minimal UE5 C++ project plus an editor Python
import script, following the plan's UE5 recommendation. It has not been built
or run because UE5 is not installed on the development machine. The Phase 0
gate (mesh and texture rendered in the host engine, screenshot captured) and
the UE5-versus-Godot decision therefore remain open. The scaffold is committed
so the gate can be attempted as a self-contained next task; it must not be
read as evidence that the engine choice is made.
