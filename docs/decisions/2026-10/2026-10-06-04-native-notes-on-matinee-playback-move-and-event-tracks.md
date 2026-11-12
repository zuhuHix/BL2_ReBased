## 2026-10-06: native notes on Matinee playback, move and event tracks, remote events and Marcus's walk (UNVERIFIED)

AI-assisted (Claude), analyst lane G10, Ghidra; own-words note `docs/verification/NATIVE_KISMET_MATINEE.md`, no listing text.
`SeqAct_Interp`: position advances by PlayRate × dt and the clamped end is applied exactly; Completed/Reversed is raised one
frame after the last update and only within 0.0001 of the end or of 0 (a stop mid-run fires neither); `bRewindOnPlay` resets
before group instances are built. Move tracks evaluate Hermite keys with stored tangents (the Fire door and target tracks
play as smoothstep over 1.5 s and 2.0 s); pose = key × initial × base, with the initial transform captured at every start
from stopped so the actor starts from its placed pose. Event-track keys fire over half-open intervals by direction. Remote
events fire every enabled same-name event across loaded sequences; mission remote events match mission and name. Marcus's
walk: random destination, arrival/leaving events per node, Finished only at the chain's end or on abort. Corrections listed
in the note: `src/mover.hpp`'s first-key-delta reading differs when keys rotate; `src/kismet.cpp` fires Completed a frame
early; `tools/ghidra/class_layout.py` cannot size classes with map fields. Open: how the runtime reaches the move-track pose
code, Marcus's arrival radius and speeds, move-node link order. All UNVERIFIED in game.

