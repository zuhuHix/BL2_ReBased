## 2026-11-19: Rust SpawnPickupParticles in Off: the ordered preflight, refusals E and F kept, a harness preload

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SPAWN-PICKUP-PARTICLES. The file number is a placeholder until
the maintainer gives one. Coded from `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md` alone (analyst lane
ANALYST-SPAWN-PICKUP-PARTICLES, #581, accepted at 164b43d3), with the hub's rulings on #581. This lane did not read
the native (clean room). No real-game session was run. No dependency was added and no bounds check was loosened.
**Every rule is UNVERIFIED in the running game.**

**Base.** Stacked on #587 (`fix/rust-pickup-rb` @ 7ae53972, its r1 fix), which sits on #520 and #511, and carries
#518's counter files.

- **What changed** (`crates/ow-inventory/src/attach_mesh/pickup_particles.rs`, new; Off only; Reference binds nothing):
  `WillowPickup.SpawnPickupParticles()` no longer refuses with #520's particle-effects `LayerAbsent` (`PARTICLES`,
  `PARTICLES_LAYER` and the old function are removed). The preflight runs the note's section 4 in its order. It is
  write-free, load-free and draw-free, and the first refusal or data error wins:
  1. the pickup's seven declarations;
  2. `PickupParticles` None: #520's `PARTICLES_REMOVAL`;
  3. the component C:
     - a held one must be a `ParticleSystemComponent`, and its own values are the inputs;
     - for a new one, the preloaded class and default object (none is refusal G). Then the offset is planned
       (`GetPickupParticleOffset`'s reads, through the write-free `inventory_definition_of`), and part 1's
       `attach_plan` is run on the default's values, with its guards and #520's `ATTACH` and `ATTACH_DETACH`;
  4. the rarity choice from the prepared globals definition (`globals_definition_loaded`, #587 r1). `RARITY_Rainbow`,
     by the enum's value name, is refusal A;
  5. the template not loaded is refusal B. Its `WarmupTime` and `LODSettings[].bLit` are checked. Then
     `bAutoActivate`, or `bIsActive` and not `bWasDeactivated`, gives refusal E (the activation); otherwise refusal F
     (the initialisation).
- **One of E and F always applies, so the native writes nothing today**, as the note says. Steps 5.3 to 10 of the
  preflight (`bAcceptsLights`, refusals C1, C2 and D, the flags, the colour, `InstanceParameters`, the transform
  update) and the write plan are gated on E and F's answer and are not coded: they would be unreachable.
- **One guard of this lane's own,** not a note refusal: a new component's offset whose inventory definition is a
  handle the store has not loaded is a `NotYet { NotRead }` ("the particle offset of an inventory definition that is
  not loaded (a load is a write)"). Reading a stand-in's fields would be wrong, and loading it would be a write.
- **Harness inputs** (hub ruling 1 on #581, UNVERIFIED; `ow-cli` `equip_run::pickup_world`): before `make`, the set-up
  loads the globals definition's `PartPickup`, and the `ParticleSystemComponent` class with its code and its default
  object. It records them in `ParticlePreload` and logs each one.
- **Partial writes.** The native returns its refusals unmarked; #518's counters are snapshot at entry and would mark a
  late `NotYet` only if they moved. On the slice, the "stopped after partial writes" prefix is the pickup driver's,
  for `make`'s earlier writes.
- **Tests** (`attach_mesh/pickup_particles_tests.rs`, 10 new, on invented packages, with a rarity enum in an order
  unlike the game's):
  - the removal refusal;
  - a new component stopping at F, and at E when the default has `bAutoActivate`;
  - a held component choosing by its own flags, also without the preload;
  - refusal G;
  - the attach plan's `ATTACH`, its `bDeleteMe` skip, and `ATTACH_DETACH`;
  - refusal A in its place (after G, before B), and an unbanded level that is not Rainbow;
  - refusal B, with nothing loaded;
  - the LOD check;
  - Reference binds nothing.

  Every refusal compares the pickup, the default, the heap's object count and #518's three counters.
  `attach_mesh_tests.rs`: the old `LayerAbsent` test is replaced by a data error on that toy, whose pickup lacks the
  fields.
- **Installed slice gate (Off, strict):** see `docs/verification/GATES.md`. Links 6 and 7 stop at refusal E in
  `make`. This is what the host did, not parity.
- **Open:** the note's open questions, `ActivateSystem`'s script-visible writes above all (the next read for links 6
  and 7), and the initialisation.
