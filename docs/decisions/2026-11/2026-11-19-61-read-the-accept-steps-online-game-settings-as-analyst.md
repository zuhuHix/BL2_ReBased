## 2026-11-19: Read the accept step's online game settings stop as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-NEXT-STOP, analyst only. The installed Off `ow-package --slice-gate` on main b985dcbf stops each of the
14 links as follows (strict and `--past-errors` gave the same rows; the table stays under ignored
`local/p2/next-stop/`):

- links 1, 3, 4, 5, 13 and 14 at `maya:8`;
- link 8 at the `InventoryAttributeContextResolver`;
- link 9 at `GetSMComponent`;
- links 10 to 12 at `UIInteraction.GetDataStoreClient`;
- links 2, 6 and 7 have no headless driver.

After the stops other lanes own, `GetDataStoreClient` blocks the most links. The new own-words note
`docs/verification/NATIVE_ONLINE_GAME_SETTINGS.md` and two `NATIVE_INDEX.md` rows record, as behaviour only:

- the stop is not mission logic: `AddMission` refreshes the controller's online game settings, and with no online
  settings from the online subsystem the refresh takes them from the `WillowOnlineGameSettings` data store;
- `GetDataStoreClient` answers the engine's UI interaction client (or the class default's), and `FindDataStore`
  searches the player's data stores and then the global ones by tag;
- the store is a global one in the engine configuration, its defaults carry one settings entry of class
  `WillowOnlineGameSettings`, and who fills that entry was not read.

The script path was found with a temporary local stack print, not committed. Raw output stays under ignored
`local/analysis/next-stop/`; the Ghidra runs were read-only and took the machine lock. Nothing is implemented: both
natives stay as they are in Off until a fresh lane codes them from the note. Every rule is UNVERIFIED in the running
game; the note lists the bl-sdk checks and three open questions, one of them (whether the port needs these settings
for the slice) for the hub.
