## 2026-11-19: The online game settings entry filled in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SETTINGS-FILL, implementer.

- **Sources:** `docs/verification/NATIVE_SETTINGS_FILL.md` (#450; analyst lane ANALYST-SETTINGS-FILL, decision
  2026-11-19-69, not this thread), read with NATIVE_ONLINE_GAME_SETTINGS.md. No native code or analysis output was
  opened. The installed script and declarations were read with `research/script_disasm.py` and the package exports.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** main 7556c254, after #448 and #450 landed (first coded on #448's branch with the #450 note merged in).

**What changed** (`crates/ow-world/src/world/datastore.rs`, Off only; Reference unchanged).

- **At store creation** (the client making its global stores): for an online game settings store, each
  `GameSettingsCfgList` entry gets:
  - a new object of its `GameSettingsClass` from that class's defaults (none for an entry without a class);
  - a new provider, of the store's `SettingsProviderClass` when that is `UIDataProvider_Settings` or a subclass, else
    `UIDataProvider_Settings`. It is bound at once: its `Settings` is the new object and its `DataClass` that object's
    class.

  `SettingsArrayProviders` stays empty, and `Registered` is not run.
- **The plan is read before anything is made** (the hub's ruling): the store's class defaults (the list, each entry's
  class and fields, and `SettingsProviderClass`), the provider class and its two fields. A refusal there leaves
  nothing.
- **The provider refusal:** a call on the settings provider answers the note's named `NotYet` ("the settings
  provider's list providers are not built"). That covers the natives declared on `UIDataProvider`,
  `UIDynamicDataProvider` and `UIDataProvider_Settings` (called on another provider they answer a `NotYet` of kind
  `NotRead` naming the native), and the settings provider's own script functions, which are overridden. **Open gap:**
  script functions it inherits from those base classes are not overridden. They run as script until their first native
  call.
- **The entry read:** the installed `GetOnlineGameSettingsViaDatastore` (it reads `GameSettingsCfgList[0].GameSettings`
  and casts it) is no longer overridden. It now reads the filled entry, and the refresh goes on.
- **`Settings.SetIntProperty`:** the first `Properties` entry with the id, when its data is the integer type, gets the
  value. The integer type is `SDT_Int32`, found as the entry name of the enum that `SettingsData.Type` declares.
  Otherwise nothing happens. **Implementer reading, UNVERIFIED:** the value goes to `SettingsData.Value1`, the data's
  one int field; the note says only "its value".
- **`GetCurrPlaythrough`:** the replication info's `CurrentPlaythrough`.
- **`GameEngine.GetDLCManager`:** None (the hub's ruling), so the two DLC writes are skipped.
- **Still `NotYet { NotRead }`:** `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber`, and the DLC manager's
  two queries.
- **Terminal after a write:** a refresh begins at its data store client lookup, which clears a write mark. The store's
  fill and a setting write set it. A `NotYet` from the four unread natives, or from the provider, after a write is
  terminal: "stopped after partial writes", its kind kept, nothing rolled back. Other errors propagate unchanged.

**Verified (automated, invented packages).** `ow-world`: 11 data store tests, 5 of them new on the `ToyFill` packages.
- **The fill at creation:** with the provider class fallback both ways; an entry without a class stays empty.
- **A fill that cannot be planned** (no `UIDataProvider_Settings` class) leaves nothing.
- **`SetIntProperty`:** the first integer match only; a float entry and an unlisted id are left alone.
- **The engine reads and the provider refusal**, for a native and for the provider's own script.
- **The partial-writes mark** after a setting write, and none on a refresh that wrote nothing.
- **The override is gone:** the toy's own script answers.

Gate numbers are in `docs/verification/GATES.md`.

**Installed (Off; outputs under ignored `local/p2/settings-fill/`).** `--slice-gate` strict: 0 ok / 14 `NotYet` / 0
errors.
- Links 10 to 12 pass 3 of their 4 driver steps (`stage:8`, `player:8:21000`, `use`) and stop in `accept`.
- They stop no longer at the settings entry but at
  `WillowPlayerController.GetActiveMissionNumber`, marked "stopped after partial writes". This is the refresh's next
  unread native, after the entry read and the level write.

**Open.**
- Binding `GetActiveMissionNumber` (NATIVE_CONTROLLER_HELPERS.md) and `GetActivePlotCriticalMissionNumber`
  (NATIVE_SAVE_LOAD.md) in Off.
- The note's open questions 1 (when the stores are initialised, and `Registered`) and 2 (the DLC manager in the
  shipped game).
- The inherited provider scripts.
