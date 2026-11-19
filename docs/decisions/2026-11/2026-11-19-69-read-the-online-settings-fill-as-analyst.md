## 2026-11-19: Read who fills the online game settings entry, and the refresh's natives, as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-SETTINGS-FILL, analyst only. With IMPL-ONLINE-GAME-SETTINGS (decision 2026-11-19-65), slice links 10 to 12
stop at the online game settings entry, open question 2 of NATIVE_ONLINE_GAME_SETTINGS.md. The new own-words note
`docs/verification/NATIVE_SETTINGS_FILL.md` and five `NATIVE_INDEX.md` rows record, as behaviour only:

- The store's own initialisation fills each entry: a new object of `GameSettingsClass` as `GameSettings`, then a
  settings provider (the store's `SettingsProviderClass`, else `UIDataProvider_Settings`) bound to it. It needs no
  online subsystem, no player and no configuration beyond the class defaults. When it runs was not read (public UE3:
  at engine start).
- `WillowOnlineGameSettings`' defaults already list the eight integer properties the refresh writes.
- `Settings.SetIntProperty` sets an existing integer property by id and otherwise does nothing.
  `GetCurrPlaythrough` reads the game replication info's `CurrentPlaythrough`. `GetDLCManager` reads the engine's
  `DLCManager`. The two DLC manager queries were not read past their entry. The two mission-number natives point to
  their existing notes.
- What Off does for each, and what stays a named `NotYet`. A recommendation for the hub: the port's engine has no DLC
  manager, so `GetDLCManager` answers None and the refresh skips the DLC writes.

Raw output stays under ignored `local/analysis/settings-fill/`; the Ghidra runs were read-only and took the machine
lock. Nothing is implemented. Every rule is UNVERIFIED in the running game; the note lists the bl-sdk checks and two
open questions.
