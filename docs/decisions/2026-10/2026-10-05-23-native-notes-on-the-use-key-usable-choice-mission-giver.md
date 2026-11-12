## 2026-10-05: native notes on the use key: usable choice, mission giver, accept screen (UNVERIFIED)

AI-assisted (Claude), analyst lane G8, Ghidra; own-words note `docs/verification/NATIVE_USE_INTERACTION.md`, no listing text.
The current usable object comes from one view ray of `PlayerInteractionDistance` (350 uu in the stock globals), first
usable hit nearest-first, a blocking hit ends the search, no angle cone or type priority, at most 30 updates per second
(so the host's 250 cm reach and sweep are not the stock rule). The use key runs script (`Use` → `ServerUse` →
`PerformedUseAction` → `CanAffordToUseUsableObject` → `UseObject` → `PayForUsedObject`); `WillowAIPawn.UseObject` fires
the AI `OnUsed` events (Generic, then HasMissions/NoMissions); Marcus's `Brain` behavior falls through his story checks to
`Behavior_ShowMissionInterface`, which opens the mission screen with Marcus as context; the screen's buttons call the
`AcceptMission` / `ServerCompleteMission` scripts already running on the VM (script swap 1). Talk state: `BeginUse` /
`EndUse` at screen open/close, 30 s linger, look-at, focus camera. The trace flag word is recorded as a number with a
labelled guess at its bits. All UNVERIFIED in game.

