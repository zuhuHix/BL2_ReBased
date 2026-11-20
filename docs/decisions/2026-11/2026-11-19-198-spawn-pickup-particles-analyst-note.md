## 2026-11-19: Read the pickup's particle effect as analyst

AI-assisted (Claude Code, Opus 5.5).

Lane ANALYST-SPAWN-PICKUP-PARTICLES, analyst only. Another lane implements it. The number is a placeholder (NNN) for
the hub to assign.

With #574's answer for the rigid-body set-up stood in, links 6 and 7 stop at `make` on #520's refusal of
`WillowPickup.SpawnPickupParticles` (particle layer absent). The new own-words note
`docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md` and one `NATIVE_INDEX.md` row record, as behaviour only:

- **Inputs.** On the slice, the script reaches the native through `InitializeFromInventory`, `SetPickupability` and
  `TryToSetInteractParticles`, with `PickupParticles` = the globals definition's `PartPickup`. The native reads:
  - the pickup's `PickupParticleComponent`, `InventoryRarityLevel` and `Inventory`;
  - the definition's `PickupFlagOffset` and `LootBeamColorOverride`;
  - the globals definition's `RarityLevelColors`;
  - the template's `WarmupTime` and `LODSettings`.
- **What it does:**
  - makes and attaches a `ParticleSystemComponent` when there is none, with `Translation` := the existing
    `GetPickupParticleOffset` rule;
  - picks the template (`PickupParticles`, or a load by name for `RARITY_Rainbow`);
  - calls `SetTemplate`;
  - sets the absolute-transform flags and `Rotation`;
  - sets a colour parameter in `InstanceParameters`;
  - calls `SetHidden(false)`;
  - runs the deferred transform update.
- **The activation (hub ruling 2 on #581), read for its script-visible writes.** `SetTemplate` activates or initialises
  the system. The activation writes script-visible fields: `EmitterDelay`, which can be a random draw from the
  template's delay range, `AudioEventDelay`, `bIsActive` and the related flags, `LOD`, `bHasBeenActivated`,
  `AccumTickTime`, and `WarmupTime` after warm-up ticks. So it is not render-side only and keeps its refusal. The
  initialisation has its own refusal: its body is unread, and it writes `AccumTickTime`.
- **The proposed Off answer:**
  - harness inputs: the pickup world preloads `PartPickup` (hub ruling 1) and the particle component class with its
    defaults and declarations, all UNVERIFIED;
  - since one of the activation and initialisation refusals always applies, **today the native writes nothing**.
    The write sequence applies only once both are resolved;
  - a write-, load- and draw-free preflight in the native's own order (the offset before the attach, the rarity
    choice before `SetTemplate`, the colour and parameter after it). It checks concrete values and slots, and
    allocates a new component from defaults captured once. It plans everything before any write: the new
    component from its class defaults, the attach from actor guards, the colour and parameter, and each deferred
    step's owner test with its own text;
  - eight typed refusals, plus #520's four kept as they are;
  - a late `NotYet` is prefixed only when this native's counters moved.
- **What links 6 and 7 read.** Nothing they read depends on the effect. But the activation or initialisation refusal
  comes first, so with the answer and the preload the expected stop is that refusal at `make`. That is UNVERIFIED,
  because no probe ran the answer.
- **The probes** (a local merge of origin/main with #520, never pushed): both natives were stood in with no-write
  answers. The observed stop is at `touch`: the unbound `Actor.GetBoundingCylinder`. `SetAnimTreeTemplate` does not
  come first on this base.

Method: the script with `research/script_disasm.py`; the layouts with `tools/ghidra/class_layout.py` (the fits stay
under ignored `local/analysis/spp/`); the enum value names from `WillowGame.upk`; the native with Ghidra 12.1.4
headless, read-only, under the machine lock. No bl-sdk session was granted and no recorded evidence covers this
native, so every rule is UNVERIFIED. The note lists four bl-sdk checks.

Open: `DestroyPickupParticles`; the activation's engine steps and the initialisation's body; the delay draw's stream;
the two transform updates; the item-set field; the component default's `bAutoActivate`; the parameter name.
