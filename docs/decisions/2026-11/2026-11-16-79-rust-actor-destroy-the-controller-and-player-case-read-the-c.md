## 2026-11-16: Rust actor destroy: the controller and player case read, the C++ stand-in under Reference

AI-assisted (Claude Code). Lane FIX-ACTOR-DESTROY of the Rust port (docs/rust-port/SLICE_RUN_PLAN.md, step 3 of the
lane list; section 3.1 lane 4). Number 79 was reserved for this lane by the hub. There was no real-game session:
every rule below is read from the executable or is the C++ reference's, and is **UNVERIFIED** in the running game.
No parsing behaviour changes.

- **What changed.**
  - `crates/ow-world/src/actors.rs`: the destroy routine's controller branch, plus a `RECORD_CONTROLLER` constant.
  - `crates/ow-world/src/world/tests.rs`: a parity-mode variant of the fixture and one test in both modes.
  - The analyst note `docs/verification/NATIVE_ACTOR_DESTROY.md`.
  - No dependency, no other crate edited.
- **The analysis** (the note, own words, clean-room scan clean). The case NATIVE_ENGINE_CORE.md left "not read in
  detail" is a **player controller** case, not a controller one:
  - The routine asks the actor for the player controller it is.
  - Any other actor, an AI controller such as a `WillowMind` among them, takes the generic routine, which the port
    already has.
  - A local player's controller is not destroyed: `bPendingDestroy` is set, the engine is asked to drop the player,
    and `Destroy` answers false.
  - A network child connection's controller has its child connections closed, then the generic routine runs.
  - The engine classes involved are read by role, not named.
- **Off mode.** The case stays `NotYet { NotRead }`, now naming the note (`actors::RECORD_CONTROLLER`). By the
  clean-room rule the analyst does not implement what it read; a fresh implementer builds the Off rule from the note
  and NATIVE_ENGINE_CORE.md. For the slice it reduces to: a non-player controller goes through the generic routine,
  and a local player's controller answers false after `bPendingDestroy`.
- **Reference mode** reproduces the C++ stand-in for a controller (architecture R3; `src/combat_death.cpp`): the
  actor is marked `bDeleteMe` and `Destroy` answers true. Nothing else runs: no `Destroyed` event, no `SetOwner`, and
  it stays in the level list. Every other actor keeps the world's routine in Reference, as decision 2026-11-15-26 set,
  so nothing another lane pins changes. The C++ marks every actor that way; the port keeps it to the case it has no
  rule for. The C++'s `Destroy <name>` event line is the combat log's and is not written by `ow-world`.
- **The installed game** (local `--combat-run` over `Sanctuary_Dynamic.upk`; outputs under ignored
  `local/p2/fix-actor-destroy/`). The runs use this lane together with FIX-ATTR-RESOLVERS (PR #372, not yet merged),
  cherry-picked into a temporary local build only, because without it the loot run stops before the destroys.
  - **W6-LOOT's bullymong loot run, Reference:** before (with #372 alone) 197 of 197 rows equal to the C++, with 300
    errors, all this case (each kill's destroy of the victim's mind). After: 197 of 197 equal, **0 errors**.
  - **S-f with the respawn fee, Reference:** 17 of 17 equal, 0 errors, before and after.
  - **Off:** 0 rows before and after. The next stops, before any destroy, are not this lane's:
    - the unbound `Actor.LocalPlayerControllers` (the mission bridge's script);
    - the bridge's hand-built health pool (`NotYet` in Off: FIX-POOLS-OFF);
    - the spawn's template archetype `Default__WillowAIPawn.SpecialMoveComponent0` (FIX-TEMPLATE-PREFIX).
- **The bl-sdk reads that would confirm the rule** are in the note's section 3.
