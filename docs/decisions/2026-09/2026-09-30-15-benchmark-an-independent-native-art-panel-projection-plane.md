## 2026-09-30: Benchmark an independent native-art panel projection plane

AI-assisted developer-only benchmark using existing Ruffle
0.7.0-nightly.2026.9.26 and local StatusMenu payloads; no external backend,
new dependency, game-derived code or production rendering change. A temporary
second player exposes native equipment-panel art inside a CSS plane. Synthetic
rotateY(20deg), perspective 1200 px; HTML hit target shares that plane.
Native cell bounds 429.55..597.05 x / 119.8..189.3 y, transformed target
169.52 x 82.71 px. Center elementFromPoint hits the target. Snapshot confirms
the panel art and green target transform together. Benchmark completes in
2990.5 ms; 60 browser RAF intervals mean 5.97 ms/max 8 ms. This is browser
cadence, not UE render throughput or a full interaction/performance gate.
Cleanup verified: one remaining main player, main inventory ready.

The first awaited preview call timed out at 15 s. Its temporary DOM was then
observed removed; a subsequently instrumented probe reported initialization
failure. Corrected readiness to wait for all movie frames, as the main adapter
already does, and used _level1-relative bounds. Corrected probe completes with
explicit state/results and removes its temporary player in finally. Two
successful runs (3018.1 and 2990.5 ms), zero output assets/duplicates. No memory,
drag, all-cell alignment, multi-panel overlap or tween-synchronization claim.

This establishes a bounded possible rendering path, not original projection
values or 1:1 stock layout. The production affine adapter remains until real
transforms and broader interaction/performance checks support replacing it.
CTest 6/6 (8.69 s); all nine package checks match; JS syntax/diff pass. No new
engine run or independent critic because runtime unchanged. Native capture
connection is still unavailable. Full menu parity remains open.

