## 2026-11-19: The auto state flag is bit 1, and Auto walks the class chain (analyst note)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-AUTO-STATE, analyst only (clean room: another lane implements it).
The number NNN is a placeholder; the hub gives it. Docs only: no code changed and no dependency added. There was no
real-game session, so **every rule is UNVERIFIED** in the running game.

- **What.** New note: `docs/verification/NATIVE_AUTO_STATE.md`. It covers:
  - which state flag marks an auto state;
  - how `GotoState('Auto')` finds the state;
  - when an actor enters it;
  - what the slice's auto states hold;
  - what the port does today;
  - a proposed Off answer with typed refusals.
- **Why.** `ow-vm`'s `auto_state` refuses unless every flag in the chain is 0. That refusal is the bring-up's largest
  stop (597, decision 2026-11-19-20). It also blocks the game info's `InitGame` and `PreBeginPlay`, and so the
  script-made GRI and tracker the map-load design needs (SLICE_MAPLOAD.md, #530).
- **Findings:**
  - **Bit 1 (value 2) is the auto flag**, from a package oracle over the nine code packages:
    - on states, it is set on 12 states, never twice in one class;
    - on classes, it is set exactly when no state in the chain has it, 3,339 of 3,339.

    The executable's state change routine tests that same bit.
  - **The lookup:**
    - a class with the bit set gets no state;
    - otherwise, the first state with the bit set in the class's own states, then each super class's, most derived
      first;
    - otherwise, no state.

    So `WillowCoopGameInfo` resolves to `WillowGameInfo.PendingMatch`.
  - **When it is entered.** `Actor.SetInitialState` calls `GotoState('Auto')`. It is raised right after
    `PostBeginPlay`, by the bring-up's routing and by a spawn after play began. `BeginState` runs inside it, and the
    state code runs only at the next state-code step.
  - **The state code the slice reaches** (the game info's, the player controller's, the pickups') uses only `Begin:`,
    a branch, a call and `Stop`, with no latent calls. The port runs all of these already.
  - **The next stop after this:** the world tick's state-code hook (`World::set_state_step`) is unset in every
    production world, so an actor in a state stops with `LayerAbsent` at its first tick. The note proposes that the
    map route sets it.
  - **Links today:** no link driver reaches `GotoState('Auto')`, because the drivers' worlds never begin play. Link
    12's next stop is an unrelated unbound native.
- **Raw output** stays under ignored `local/analysis/auto-state/`. The note is in our own words, with no addresses,
  offsets or listing structure.
