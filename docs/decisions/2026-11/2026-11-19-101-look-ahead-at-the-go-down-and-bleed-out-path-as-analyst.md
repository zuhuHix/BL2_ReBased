## 2026-11-19: Look ahead at the go-down and bleed-out path as analyst

AI-assisted (Claude Code, Opus 5.5). Lane MAIN-READING + LOOKAHEAD-13-14, analyst only. The number NNN is a placeholder:
the day's two-digit numbers are used up, and the hub gives the number once three digits are allowed. This lane reads
and writes docs only; another lane implements from the note alone.

- **The main reading.** `--slice-gate` in Off, strict, on main 0235937 (#462, #464, #467, #475), under the machine
  lock: ok 0, notyet 12, error 2 of 14. Links 6 and 7 are `error` on the unbound `WillowGlobals.GetWillowGlobals`
  (the globals natives are not on main yet). The per-link table is in the note, section 6. The full rows are saved
  outside the repository, in the main checkout's ignored `local/hub-runs/`.
- **How the look-ahead list was made.** The combat driver's steps of links 1, 4, 13 and 14 were run in Reference,
  whose stub policy counts every unbound native, and the counts were diffed step by step. This is a lower bound: a
  Reference zero can send the script past a branch that Off would take.
- **New record:** `docs/verification/NATIVE_GO_DOWN_PATH.md`, with nine `NATIVE_INDEX.md` rows.
  - **Read, with an Off rule:** `Pawn.IsLocallyControlled`, `WillowPlayerController.GetSkillManager`,
    `Engine.GetCurrentWorldInfo`, `Engine.IsSplitScreen`, `InteractionProxy.IsInteractionDebugEnabled`,
    `WillowGlobals.GetCustomizationManager`, `WillowPlayerPawn.SetShadowCasting` (no entry in the headless pawn).
  - **Partly read:** `GetTechDeathIndexForDamageType` (the -1 cases and the list walk; its status-effect test is
    unread, so Off answers `NotRead` there), `GetVehicleLifetimeManager` (the field; its lazy creation is `NotRead`).
  - **Outline only, Off `NotRead`:** `StatusEffectsComponent.ClearAllStatusEffects`, `Actor.SetCollision`,
    `Actor.SetCollisionSize`, `RecordCharacterKilledEventForPlayer`.
  - **Not resolved, Off `NotRead`:** twelve natives that each hand straight to an overridable implementation the
    class-table lookup could not resolve (the limit NATIVE_CACHE_WEAPON_CARD.md, #484, also met). The thirteenth,
    `WillowPlayerPawn.InitRagdoll`, is a separate key from the bound `Engine.Pawn.InitRagdoll` and takes the death
    chain's presentation answer, true (COMBAT_DEATH.md), as a stand-in (review r1 of #489).
  - **Unbound and not read:** `Actor.Trace` (349 Reference stub calls on link 14; Off `NotYet` `LayerAbsent`, the
    world's collision), `Controller.IsUsingVehicle` (`NotRead`) and the native iterators (named by the first Off run
    past link 4).
  - **The prefix:** each `NotRead` is a plain one at entry; the layer that has written adds "stopped after partial
    writes".
  - **Already recorded elsewhere:** the vehicle, dialog, anchor, AI reset, effect coordinator and revived-event
    natives (NATIVE_RESPAWN_STATIONS.md, NATIVE_DIALOG.md, NATIVE_DOWN_TIMERS.md).
- **The point for the gate.** In the combat run's Off stub policy an unbound native is an `error`, not a `NotYet`, so
  every native on the list needs an Off binding, even one that only answers a named `NotYet`.
- **Not verified:** everything above in the running game; the identity of two `WorldInfo` fields that the layout tool
  cannot lay out (named by fit), and of the two shadow flags.
