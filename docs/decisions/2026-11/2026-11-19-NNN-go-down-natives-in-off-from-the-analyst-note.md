## 2026-11-19: The go-down natives in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-GO-DOWN-NATIVES, implementer. This lane coded the natives that slice
links 13 (`go_down`) and 14 (`bleed_out_respawn`) reach from `docs/verification/NATIVE_GO_DOWN_PATH.md`, and from the
notes it points to: NATIVE_RESPAWN_STATIONS.md, NATIVE_DIALOG.md and NATIVE_DOWN_TIMERS.md. The go-down note is
analyst lane MAIN-READING + LOOKAHEAD-13-14, PR #489, read at its review-r1 head 27beafc. This lane opened no analysis
output, Ghidra or REA. There was no real-game session, so every rule stays **UNVERIFIED** in the running game. No
dependency or crate edge was added, no bounds check was loosened, and Reference is unchanged. The decision number is
a placeholder until the hub assigns one.

- **`ow-combat` `go_down.rs` (new), Off only.** It is registered last in the combat pack, so every existing binding
  keeps its key.
  - **Rules:**
    - `Pawn.IsLocallyControlled` (note 3.1): the pawn's `Controller`, else its `DrivenVehicle`'s; with none, false.
      With a controller it is true on a standalone world. A world whose `NetMode` is not `NM_Standalone` is
      `NotRead`. The enum value is found by name, never as a constant.
    - `WillowPlayerController.GetSkillManager` (3.5): the world's `Game` when it is a `WillowGameInfo`, asked through
      its own `GetSkillManager`; otherwise None.
    - `WillowPawn.GetTechDeathIndexForDamageType` (3.4): -1 for a pawn that is gibbed, has no death definition or no
      status-effect component; -1 for a damage type that is None, not a `WillowDamageTypeDefinition`, or of a kind
      outside {1, 2, 4, 7}. Otherwise it is `NotRead`, for the unread status-effect test.
    - The 4.2 getters:
      - `Engine.GetCurrentWorldInfo`: the world's `WorldInfo`. With none, `LayerAbsent`.
      - `Engine.IsSplitScreen`: whether there is more than one local player.
      - `InteractionProxy.IsInteractionDebugEnabled`: false.
      - `WillowGlobals.GetCustomizationManager` and `GetEffectCoordinator` (NATIVE_RESPAWN_STATIONS.md 3.2): the
        field.
      - `GetVehicleLifetimeManager`: the field, or `NotRead` for the lazy creation when it is None.
    - `WillowPlayerPawn.SetShadowCasting` (4.4): nothing with an empty `InstanceDataState.Data`, else `NotRead`.
    - `WillowPlayerPawn.InitRagdoll` (3.2): `death.rs`'s presentation answer, true. It is one shared function bound to
      a second key; `Engine.Pawn.InitRagdoll` keeps its binding.
    - `Controller.IsUsingVehicleEx` (NATIVE_RESPAWN_STATIONS.md 3.1): with no `Pawn`, false and a None vehicle.
      With a `Pawn`, `NotRead` for the pawn's own answer.
    - `WillowLeviathanService.RecordCharacterRevivedEventForPlayer` (that note's 3.5, telemetry only): nothing.
    - `GearboxDialogComponent.StopTalking` (NATIVE_DIALOG.md): nothing when there is no live line (no `EventData`,
      or its `LiveTalkAction` is None) and the flag is clear. Otherwise `NotPorted`.
  - **Named `NotYet`s**, each plain at entry. The layer that wrote earlier adds "stopped after partial writes"
    (note section 1).
    - **`NotRead`:**
      - the sixteen 3.2 keys, "<Class>.<Function>: the class's own implementation is not resolved". This includes
        `Engine.PlayerController.IsLocalController` beside the `Controller` key;
      - `ClearAllStatusEffects`, `SetCollision`, `SetCollisionSize`, `SetLocation`, `SetRotation` and
        `IsUsingVehicle`;
      - `RecordCharacterKilledEventForPlayer`, `WillowGameInfo.ResetAI` and
        `WillowDialogGlobalsDefinition.StaticTriggerTemplateEvent`.
    - **`NotPorted`:** `Pawn.SetAnchor`. Its note reads it as bookkeeping, and this lane does not port it.
    - **`LayerAbsent`:** `Actor.Trace`, "Actor.Trace: the world's collision".
  - **Not on `READ_ONLY_QUERIES`.** `PlayerController.IsLocalController` is not added there: it answers `NotRead`,
    and no preflight calls it.
  - **The native iterators** (4.5) cannot be bound by name. The VM's own Off refusal names the key, and the lane
    that binds one answers `NotRead`, "the engine iterator <key>".
- **Tests:** eight in `go_down/tests.rs`, on invented classes, a toy Core and a toy WillowGame. They grade by
  `VmError` variant and `NotYet` kind. The shared combat fixture is untouched.
- **The slice gate does not move links 13 and 14 yet.** Both are still blocked by link 1, which stops at `maya:8` on
  the unbound `GameEngine.GetOnlineSubsystem()`, which is not on this lane's list. The installed Off gate on this
  branch gave 0 ok, 5 notyet, 9 error of 14. Its JSONL is byte-identical to that of main d72b96d, run back to back
  under the machine lock.

**Review r1 of #500 (five findings, fixed).**
1. **The actor's world.** `GetSkillManager` and `IsLocallyControlled` read the receiver's own `WorldInfo` field
   first, as the behavior kernel's `CanPlayBehaviorData` does, and fall back to the combat state's world. With
   neither, the answer is `NotYet { LayerAbsent }`, "the world's WorldInfo", not a None skill manager. The None
   answer stays for a game info that is not a `WillowGameInfo`.
2. **`ow-behavior`'s kernel documentation** now says that `Controller.IsLocalController` is bound and allow-listed.
   `PlayLocal` gets the native's `NotRead` in the preflight, and nothing is written.
3. **The module documentation** says that binding is first-wins. A lane that gives one of these keys a real answer
   replaces the placeholder entry here, or registers its pack before the combat pack.
4. **Tests added:**
   - `IsLocallyControlled`: the own-controller path, a listen server, a driven vehicle with no controller, and the
     actor's own world;
   - `GetTechDeathIndexForDamageType`: every kind from 0 to 8, and a missing status-effect component;
   - `IsSplitScreen` with two players;
   - `IsUsingVehicleEx`'s out vehicle;
   - `StopTalking` with event data but no live act;
   - `SetAnchor`'s `NotPorted`, and `SetDefaultStance` and `SetHidden`.
5. **The record** for `SetLocation` and `SetRotation` is NATIVE_RESPAWN_STATIONS.md, as the note's table says.
