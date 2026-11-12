## 2026-11-07: loot lies at its attachment point's socket, inside the container

AI-assisted (Claude). Real-game frames show loot inside five lootable types; the host dropped it on the floor in front. The packages hold what looks like the rule: each loot attachment names a point, and each mesh has `SkeletalMeshSocket` objects of those names. `tools/prepare_interactives.py` now lists them in the ignored data and the host places each drop there. The local data must be regenerated. Details are in `docs/verification/VENDORS.md` section 7.

- **Verified:** every attachment point of the six supported types has a socket on its mesh, read from the packages. Offline, the offsets land where the meshes are on the bones that can tell. Synthetic tests of the reader, the roll and the checker.
- **`UNVERIFIED`:** that the game places loot by these sockets, inferred from matching names and five types by eye. The host's result has not been seen in the engine at the time of writing. Socket rotation is not used. The strong box was never seen open.

