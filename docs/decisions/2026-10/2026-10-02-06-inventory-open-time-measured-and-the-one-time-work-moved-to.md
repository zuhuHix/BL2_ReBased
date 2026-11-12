## 2026-10-02: inventory open time measured and the one-time work moved to level start

AI-assisted. Host change, no parsing change. Measured on one PC (1280x720, `-d3d11`), key press to the page showing
the open inventory: first open with the page loaded 443 ms (358-488, 4 launches) -> 234 ms (213-252, 3 launches);
repeat opens about 60 ms (unchanged); first preview of each starting weapon about 32 ms with spikes up to 1036 ms ->
about 1 ms. The cost was the inventory VM build (about 120 ms) and Maya's menu meshes (45-115 ms) in the open frame,
plus each weapon mesh loading on first preview; these now preload at level start (about 140-270 ms added there;
`-owinvnopreload` turns it off for comparisons; `-owinvopenbench` and `OWINVTIME` log lines measure it). Not fixed:
pressing open in the first ~5 s of play still takes about 5.5 s, which is Ruffle booting the converted movie and its
shared libraries (two fetched more than once); a 60 Hz page frame rate and HTTP caching made no measurable difference
and were reverted. Also: page hover selects a cell only when the pointer actually moves (UE resent an unchanged position
about 500 times per open, which re-selected an equipped cell and broke the suite), and the quest save is written only
when its text changes. No real-game open time exists yet, so nothing here is compared with the game. Details:
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (2026-10-02 section).

