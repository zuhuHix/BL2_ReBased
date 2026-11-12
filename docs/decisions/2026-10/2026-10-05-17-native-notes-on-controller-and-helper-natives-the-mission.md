## 2026-10-05: native notes on controller and helper natives the mission script calls (UNVERIFIED)

AI-assisted (Claude), analyst lane G4, Ghidra; own-words note `docs/verification/NATIVE_CONTROLLER_HELPERS.md`, no listing
text. Covers the census's top helpers: `GetCurrentPlaythrough` (GRI's `CurrentPlaythrough`, else 0),
`NativeGetMissionIndex` (first `MissionList` entry with the same `MissionDef`, else -1; matches the provisional
implementation of script swap 1), `MissionTracker.IsDataValid` (the tracker's `bDataValidated` bit set by `ValidateData`,
not a constant: the provisional `true` is a stand-in), `GetHUDMovie`, `CanAffordToUseUsableObject` / `PayForUsedObject`
and the currency caps, `GetPawnInventoryManager`, `IsPrimaryPlayer`, `WorldInfo.IsMenuLevel`, the globals getters,
`WillowAIPawn.IsComponentUsable`, `Object.Localize` (search roots, `INT` fallback, `?INT?Package.Section.Key?` for a
missing entry) and `Object.QueryInterface`; presentation-only natives are marked as such. All UNVERIFIED in game.

