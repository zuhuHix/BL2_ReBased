## 2026-11-19: ParticleSystemComponent activation and initialisation, an own-words note and a proposed Off answer

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-PARTICLE-ACTIVATION, analyst only. The file number is a
placeholder until the maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_PARTICLE_ACTIVATION.md`, from a local, read-only Ghidra reading of the executable
  under the machine lock. It covers what `SetTemplate` does around its two branches, the activation and the
  initialisation: their script-visible writes, the one random draw, what they load, and what only rendering,
  simulation or audio reads. There are three NATIVE_INDEX rows. Raw output stays under ignored `local/analysis/ppa/`.
  No addresses, offsets or listing structure are in tracked files.
- **Findings** (all UNVERIFIED in game):
  - The activation draws **once, from the C runtime's `rand()`**, and only when the template's `bUseDelayRange` is
    set.
  - The warm-up ticks are particle simulation and run only when `WarmupTime` is not 0.
  - The emitter instances are engine data. The rest is a list of field writes in a fixed order.
  - `SetTemplate` also swaps a PhysX-level template when one is set, and runs part 1's conditional component update
    when the component has an owner.
- **The slice** (two local probes on a never-pushed merge of #593 with refusal E stood in):
  - the template has no warm-up, no delay range, no audio events, one LOD and no PhysX swap;
  - the component default auto-activates;
  - with the stand-in, links 6 and 7 pass `make` and stop at `touch` on the unbound `Actor.GetBoundingCylinder`.
- **Proposed Off answer:** the slice's branch as a write plan, with UNVERIFIED port inputs for the engine flags and an
  empty particle layer for the instances. The PhysX swap, the LOD for location with more than one LOD, the delay-range
  draw, audio events, the warm-up and existing instances stay typed refusals.
- **Hub rulings on #597:** the engine flags are harness inputs; `bUseDelayRange` is `NotPorted`; the conditional
  update runs never a second time; `EmitterInstances` stays empty while no script reads it (the script search found
  none).
- **Clean room:** this lane implemented #593 from NATIVE_SPAWN_PICKUP_PARTICLES.md before reading these natives. A
  different lane implements this note.

## Edits: review r1 of #597

- Dates: 2026-11-19 throughout; the note cites this file by name.
- The audio fields are named, the template each stop-audio step reads is given (the reset reads the old template),
  and `LoopingAkPlayingInfo`'s type is given. The reset writes `bStartEventPlayed` and `LoopingAkPlayingInfo` on the
  slice.
- The declarations the preflight type-checks are listed.
- A script search found no reader of `EmitterInstances`. That is hub ruling 4.
- Each step has its own preflight entry. Every new refusal has a full text and record; #593's E and F are removed.
- The draw is single precision and shares one C-runtime state with script `FRand` and the loot draws.
- Choice 3 has a consequence line per option.
- The shift's size is dropped, and `LODMethod` is compared as a raw byte (exempt from the reordering rule).
