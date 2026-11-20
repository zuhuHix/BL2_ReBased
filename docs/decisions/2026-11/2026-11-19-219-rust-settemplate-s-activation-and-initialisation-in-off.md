## 2026-11-19: Rust SetTemplate's activation and initialisation in Off: SpawnPickupParticles writes its plan

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PARTICLE-ACTIVATION. The file number is a placeholder until the
maintainer gives one. Coded from `docs/verification/NATIVE_PARTICLE_ACTIVATION.md` alone (analyst lane
ANALYST-PARTICLE-ACTIVATION, #597, accepted at 3df9cd88, with the hub's four rulings in its section 7) and, for the
steps around it, `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md` (#581). This lane did not read the natives
(clean room). No real-game session was run. No dependency was added and no bounds check was loosened. **Every rule is
UNVERIFIED in the running game.**

**Base.** Stacked on #593 (`fix/rust-spawn-pickup-particles` @ 9777c2cc), not rebased: #593 and #587 are in the
landing run and bring the `globals.rs` resolution.

- **What changed** (Off only; Reference binds nothing):
  - `crates/ow-inventory/src/attach_mesh/particle_activation.rs` (new) plans `ParticleSystemComponent.SetTemplate`,
    the activation (`ActivateSystem(false)`) and the initialisation, the note's section 7.3, over an overlay. Each
    planned write is checked against the heap's write rules, and later reads see it. Nothing is applied there.
  - `crates/ow-inventory/src/attach_mesh/pickup_particles.rs`: `SpawnPickupParticles` now runs its whole preflight as
    one plan, then applies it in order. #581's refusals E and F are removed. Steps 5.3 to 10 are coded: `bAcceptsLights`,
    the reattach request (C1, `REATTACH_SCENE`), the absolute flags and `Rotation`, the colour, the "RColor"
    `InstanceParameters` entry (`ParamType` 3), `SetHidden(false)` and the transform update (C2, D).
  - For a new component the allocation comes first, with the pickup as its outer and the slots captured from the
    preloaded default object. Planned writes on the default object land on the new one.
- **The hub's rulings, as coded:**
  1. The engine flags are a harness input (`ParticleEngineFlags`). A world without them is `NotRead`, "the engine
     flags (a harness input the world's set-up did not supply)", before any write.
  2. `bUseDelayRange` set is `NotPorted`, "the delay range draw (the C runtime's rand)". The slice makes no draw.
  3. The conditional component update runs where the note puts it and nowhere else. "Attached" means `bAttached`,
     or in its `Owner`'s `Components`, where part 1's attach put it.
  4. `EmitterInstances` stays empty (4 in the game on the slice). Existing instances refuse the reset as the
     particle simulation layer's `LayerAbsent`.
- **The note's refusals,** each before any write, with its record: the PhysX swap; the existing instances; the old
  template's and T's stop audio events (the audio layer; a recorded looping sound counts); the LOD for location with
  more than one LOD distance (the camera layer); the warm-up; the world's time with no world clock; #520's `ATTACH`;
  part 1's `REATTACH_OWNER` and `REATTACH_SCENE`; #581's C2 and D. `LODMethod` is compared as the raw byte 1.
- **Two guards of this lane's own,** not note refusals, both `NotPorted` (a third is listed below):
  - engine flags outside the slice's kind of session (the editor, a non-game session, the forced lowest LOD, a
    non-zero LOD bias). The note reads branches for them that are not coded;
  - an LOD level that would change. The note's step 16 reattach and the instances' LOD are not modelled.
- **A third guard, and two known gaps (review r1 of #609; follow-ups, not fixed here):**
  - the template reset refuses with refusal B's text when the old `Template` is unloaded or a stand-in, and
    type-checks its three audio fields, before `stop_audio`'s own gate. On a held component with `AudioEnabled`
    clear, the note never reads the old template, so this refuses `NotRead` where the native would plan. It is
    stricter than the note, never looser;
  - the engine-flags guard also covers the forced-lowest LOD and the LOD bias, which the note does give (section 3,
    steps 11 and 16). They are deferred, not unknown;
  - the "looping sound recorded" check counts any non-zero member of the playing-info struct, and the
    `InstanceParameters` value is built in the apply phase from lookups the plan has already checked.
- **What is not told apart:** a template object is a class default on the component's outer chain
  (`Origin::Default`, or a `Default__` name). Archetypes are not recognised (UNVERIFIED).
- **Review r1 of #593, folded in:**
  - no globals definition held skips the rarity read (opaque white);
  - every `RarityLevelColors` entry is type-checked, its `Color` included;
  - a rating outside its enum is a data error;
  - data-error tests for the held component's class, `InventoryRarityLevel`, `RarityRating`, `WarmupTime`,
    `LODSettings` and `bLit`;
  - #593's GATES count is corrected to 2,490.
- **Harness inputs** (`ow-cli` `equip_run::particle_preload`, UNVERIFIED): the hub's slice flags, logged on one
  "harness input: ParticleSystemComponent engine flags" line: GIsGame true, GIsEditor false, particles allowed,
  game-thread LOD set, force-lowest clear, LOD bias 0, system detail mode 0.
- **What a second call sees.** The first call requests a reattach (`bNeedsReattach`, T changed). The port runs no
  end-of-frame reattach, so a second call on the same pickup finds it pending and refuses part 1's `REATTACH_OWNER`
  before any write. A test records this.
- **Tests** (`attach_mesh/pickup_particles_tests.rs`, 20, on invented packages): the note's tests 1 to 9 (the slice's
  case with every planned write in order; the initialisation, with and without attachment; the initialisation's
  auto-activation; each refusal before any write; a template object; particles not allowed; the detail-mode skip; no
  clock; Reference), the colour override, refusals A, B and G, the attach plan, and the data errors.
- **Slice gate** (installed, Off, strict): links 6 and 7 pass `make` and stop at `touch` on the unbound
  `Actor.GetBoundingCylinder(float,float)` (1 of 3, 1 of 4), the next stop the note's probe showed. This is what the
  host did, not parity.
