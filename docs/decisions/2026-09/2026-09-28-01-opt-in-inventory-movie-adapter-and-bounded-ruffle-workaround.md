## 2026-09-28: Opt-in inventory movie adapter and bounded Ruffle workaround

The inventory prototype now drives the installed StatusMenu movie using
host recipe IDs and snapshots. `prepare_inventory_movie.py` validates a
converted single-frame library and defers its import tags, preserving their
bytes. In the local SharedWillowInventory benchmark this restored the
backpack, small equipment cells and ammo panel; remaining Ruffle warnings
mean this is not general import compatibility. No package parser changes.

The launcher enables it with `-InventoryMovie`. Names still stand in for
weapon thumbnails; no Maya menu preview or full original inventory behavior
is claimed. Skills keyboard focus and spend guards are corrected. Equip
requests now have a level guard in source; moving the active weapon into
another slot selects that destination instead of leaving the mesh stale.

The independent critic rates the combined result 5/10, below the requested
8/10. Browser interaction checks and package checks pass. Windows Application
Control blocked the fresh UE module load and then the final rebuild, so the
latest host level guard is not compiled or runtime-verified. Full evidence,
benchmark command, local reference provenance and limitations are in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`. AI-assisted implementation;
all game-derived outputs remain local and ignored.

