## 2026-11-16: Rust AI hold behavior through the VM, the hold kinds read as analyst

AI-assisted (Claude Code). Lane FIX-AIHOLD (SLICE_RUN_PLAN.md step 3). Number 74 was given by the hub. It adds
`crates/ow-ai/src/awareness/holds.rs` with its tests, changes the Fire slice's `Behavior_AIHold` handler in Off
(`crates/ow-mission/src/slice.rs` and its tests), and adds the analyst note `docs/verification/NATIVE_AI_HOLDS.md` with
its rows in `NATIVE_INDEX.md`. No dependency, no CLI change. There was no real-game session; every rule here is
**UNVERIFIED** in the running game.

- **The behavior is script, so it runs in the VM.** `Behavior_AIHold.ApplyBehaviorToContext` is installed UnrealScript
  (read with `research/script_disasm.py`): it asks its context for `IAIInterface`; with none, or a `Reason` of None, it
  does nothing; otherwise `Action` 0 holds and any other value releases, and `Type` 0 to 5 picks the base, target,
  movement, demigod, god or dialog kind, called on the interface's `GetAIComponent()` with the reason and the context as
  the tag. The script's switch settles one open point of COMBAT_GOLDEN_TRACE.md question 8: the Fire dummy's
  `Behavior_AIHold_59` (type 3) is `HoldDemigod`, not "Target plus Movement" bits.
- **Off, the Fire slice.** The dummy's `Behavior_AIHold` handler, `NotYet { NotPorted }` since decision 2026-11-16-38,
  now runs that script on each context through the VM (`ow_ai::awareness::holds::apply`), with the context as
  `ContextObject`; the event's other five parameters are not read by the script and keep their defaults, as
  `ow-combat`'s class provider does. Without a dummy pawn there is no context and nothing runs.
- **The pawn question, `NotYet`.** The installed class tables list `IAIInterface` for `GearboxMind` and `WillowMind` and
  for no class of `WillowAIPawn`'s chain (probed on the installed packages), so on the dummy's pawn the script answers
  None and places no hold. The live dummy, though, became demigod in that call. How a pawn context reaches its AI is not
  read (the note's "Not read"). So a context whose `Controller` or `MyWillowMind` is set is `NotYet { NotRead }` rather
  than a no-op the game contradicts; a context with neither can reach no component either way (the component is the
  mind's), and runs.
- **Reference (R3)** keeps the C++ record: the handler is a no-op and the script does not run.
- **Clean room.** This lane read `HoldMovement`, `HoldDemigod`, `HoldGod`, `HoldDialog`, their `Release` and `OnHold`
  forms and `GearboxMind.GetAIComponent` in the executable as analyst, and wrote NATIVE_AI_HOLDS.md in its own words:
  each kind is the base list rule on its own `AIHoldData` array (`MovementHolds`, `DemigodHolds`, `GodHolds`,
  `DialogHolds`), and `HoldMovement` alone also sets the controller's pawn's `Velocity` and `Acceleration` to an engine
  vector constant when it adds a pair. It does not implement them: `ow_ai::awareness::holds::Pack` binds the twelve as
  `NotYet { NotPorted }` in Off (a key another pack bound first keeps that binding) and nothing under Reference. Nothing
  was implemented from `local/analysis/`. The base and target kinds stay part 2's bindings (decision 2026-11-16-66,
  from G26).
- **Not changed, for the hub.** `ow-combat`'s class provider (`--combat-run`) answers the pawn question with the C++
  stand-in `ai_interface_for` (pawn to its mind) in both modes, and its `HoldDemigod` / `ReleaseDemigod` /
  `DemigodOnHold` are the C++ stand-in (a list of reasons, "any hold") in both modes. The note's rule differs (name and
  tag pairs; a None tag refused; no duplicate pair). Both belong to the fresh implementer of the note, with an
  Off/Reference split under R3.
- **Verified.**
  - Synthetic tests in both modes: `ow-ai` (the script runs with the context, a context with a controller is `NotYet`
    with nothing run, a class without the event is an error; the twelve kinds `NotYet` in Off, unbound in Reference, a
    bound key kept) and `ow-mission` (an invented `Behavior_AIHold` script sees the dummy's pawn in Off and is not run
    in Reference; no pawn, nothing run; a pawn with a controller is `NotYet { NotRead }` in Off).
  - Installed `--slice-run` in Off, the step list of `tools/verify_fire_event_order.py`: before, the run stopped at
    `spawn` on `Behavior_AIHold` (exit 3, no complete output). After, all 22 steps run and the output is complete; the
    first stop is at `accept`, the unbound `Actor.LocalPlayerControllers` (refused until read, decision
    2026-11-15-34), followed by the unbound `Trigger.TriggerDetachSprites` and `AIComponent.ActivateEvent(name)` and
    the lent weapon's region-stage condition (`NotYet { NotRead }`), so the exit is still 3. The tool's Off count is 12
    of 33 rules. Most of the 12 are absence rules that hold because the accept failed; the ones the change makes true
    are that the run reaches its end and "AIHold_57 runs right after SpecialMove_58". Reference: 33 of 33, as before.
- **Live reads that would settle it** (bl-sdk; a session the maintainer grants): hook
  `Behavior_AIHold.ApplyBehaviorToContext` on the Fire dummy and log the class of `ContextObject`, whether
  `QueryInterface(IAIInterface)` on it is None, and the component's `DemigodHolds` before and after `Behavior_AIHold_59`.
