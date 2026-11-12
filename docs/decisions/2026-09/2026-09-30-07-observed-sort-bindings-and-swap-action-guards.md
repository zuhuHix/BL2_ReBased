## 2026-09-30: Observed sort bindings and swap action guards

AI-assisted UI correction based on existing local UI Trace SDK observations;
no new extraction backend. In `uitrace_20260926_221202.jsonl`, seq 16508-16512
resolves GBA_SortInvForward/GBA_SortInvBackward to Page Up/Page Down and marks
Drop/Sort disabled during transfer. The page now uses those directional sort
keys, preserves the selected stable instance, shows the contextual Sort hint,
and blocks Drop/Sort while swapping. Slate test routing now includes both page
keys. The sort-mode list itself remains a host approximation: the traces do
not exercise the full stock cycle, so binding agreement is not full sort parity.

Repeated browser renders revealed an additional nonzero-scrollRect-origin
regression: native cells drifted from their HTML hit targets after sort/filter/
transfer renders. Backpack rows now use panel-local coordinates under a fixed
zero-origin mask. Twelve repeated renders leave the first native hit clip at
803.75/194.95 and HTML target at 803.75/194.9375 in 1280x720; no accumulating
offset. Engine comparison/sort checks now assert native/HTML alignment.
The first action run used reversed sort directions; the original alias return
was then inspected, directions corrected, and checks rerun. Final validation
is recorded in the inventory verification record. Full focus navigation, exact
stock sort cycle, Inspect/perspective/shader parity remain incomplete.

Final: UE build 8.06 s; in-engine 40/40 PASS (`run-20260930-020452.log`),
comparison capture reviewed; CTest 6/6 (22.16 s), all nine package checks match,
JS syntax/diff checks pass. Game-derived output remains ignored; 1:1 active.

