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
- **What it does.** It makes and attaches a `ParticleSystemComponent` when there is none, with `Translation` := the
  existing `GetPickupParticleOffset` rule. It picks the template: `PickupParticles`, or, for `RARITY_Rainbow`, a field
  of an item-set definition it loads by name. Then `SetTemplate`, part read: field writes, an activation, the
  `bAcceptsLights` copy and the deferred reattach. Then the absolute-transform flags and `Rotation`, a colour from the
  rarity entry or the definition's override, a `SetVectorParameter` read in full (an `InstanceParameters` update),
  `SetHidden(false)`, and the deferred transform update.
- **Loads and random numbers.** It draws no random numbers. It loads only for the Rainbow rarity, plus a template
  post-load when one is needed.
- **What links 6 and 7 read.** Nothing they read depends on the effect. The only reachable write is the component in
  `Components`, through component bounds, which the headless world does not have. So a minimal answer is enough, with
  the particle system's own work as an empty particle layer (a hub ruling).
- **Proposed Off answer.** A write-free preflight, the field writes in order, and typed refusals:
  - the Rainbow template load;
  - an unloaded template;
  - the owner and ownerless transform updates;
  - the activation, unless the hub rules it into the empty layer.
- **The slice's data** (a local probe, loaded-only reads): a new component is made; the rarity is not Rainbow; the
  offset and the override are zero; **the template handle is not loaded in the port**. So the answer stops on the
  unloaded-template refusal unless the pickup world's set-up preloads `PartPickup` (an UNVERIFIED harness input, the
  smaller of the two ways; the hub's call).
- **Next stop** (the probe, a merge of origin/main with #520, never pushed, with both natives stood in): links 6 and 7
  pass `make` and error at `touch` on the unbound `Actor.GetBoundingCylinder`. `SetAnimTreeTemplate` does not come
  first on this base; that stop is from the #573 stack.

Method: the script with `research/script_disasm.py`; the layouts with `tools/ghidra/class_layout.py`; the enum value
names from `WillowGame.upk`; the native with Ghidra 12.1.4 headless, read-only, under the machine lock. Raw output
stays under ignored `local/analysis/spp/`. No bl-sdk session was granted and no recorded evidence covers this native,
so every rule is UNVERIFIED. The note lists four bl-sdk checks.

Open: `DestroyPickupParticles`; `SetTemplate`'s activation, initialisation and post-load; the two transform updates;
the item-set field; the parameter name.
