## 2026-10-05: native notes on stock Engine/Core natives: timers, spawn, iterators, states (UNVERIFIED)

AI-assisted (Claude), analyst lane G3, Ghidra; own-words note `docs/verification/NATIVE_ENGINE_CORE.md`, no listing text.
Actor timers (replacement, clear-by-zero-rate removed on the next update, strict greater-than firing, loops firing
`floor(count/rate)` times per pass, tick order Tick → state code → timers → LifeSpan → physics), `Spawn` (the tag is not
used; event order GainedChild → PreBeginPlay → PostBeginPlay → SetInitialState) and `Destroy`, the actor iterators (lazy
walks, start indices, filters), `GetALocalPlayerController` / `GetWorldInfo`, and the state machine (EndState/BeginState
order, default label `Begin`, push/pop events, probe-mask gating by `Enable`/`Disable`, latent `Sleep`). Much of the
engine-side machinery has no registered name and was identified by call structure; the note marks those identifications.
Differences from our code are listed in the note (`src/mover.cpp` timer validation, `src/natives_core.cpp` state model)
and are not changed yet. All UNVERIFIED in game.

