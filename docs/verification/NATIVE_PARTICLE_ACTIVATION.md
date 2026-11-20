# ParticleSystemComponent activation and initialisation, as SetTemplate runs them (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-PARTICLE-ACTIVATION (decision
`docs/decisions/2026-11/2026-11-19-206-particle-activation-note.md`, number pending). Analyst only: a different lane implements it (clean room). This lane implemented #593
(`SpawnPickupParticles`' preflight) from NATIVE_SPAWN_PICKUP_PARTICLES.md before it read these natives. It does not
implement this note.

**Sources:**
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock):
  - the script natives `ParticleSystemComponent.SetTemplate` and `ActivateSystem`;
  - the engine routines they reach, each matched to its stock UE3 role by what it does;
  - the component and template virtuals, found by their vtable slots;
  - the script natives `IsGame`, `IsEditor`, `IsPlayInEditor` and `GetDetailMode`, to name the engine flags read here;
  - two flags' initial values, read from the image's data section;
- the script, read with `research/script_disasm.py` (every function of the nine code packages searched for field
  names; section 7, hub ruling 4);
- the installed packages: `tools/ghidra/class_layout.py` for `ParticleSystemComponent`, `PrimitiveComponent`,
  `ActorComponent` and `ParticleSystem`;
- two local probes of the installed slice gate (section 5), on a merge that is never pushed.

There is no bl-sdk session and no recorded live evidence on these natives. Raw output stays under ignored
`local/analysis/ppa/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Field names.** The code's offsets were matched to the layouts the tool computes:
- `ActorComponent` fields need no adjustment;
- `PrimitiveComponent` fields after its transform need a fixed shift;
- `ParticleSystemComponent`'s own fields need a fixed shift;
- `ParticleSystem` fields need none.

Each shift is the same for every field it covers, and every field named below fits it. The fits themselves stay under
`local/analysis/ppa/`.

**Builds on** NATIVE_SPAWN_PICKUP_PARTICLES.md (#581 at 164b43d3; its section 2 step 5 and section 4, refusals E and
F) and NATIVE_ATTACH_MESH_TO_PICKUP.md (part 1's attach and deferred updates). Where this note reads further than
#581's section 2, this note wins.

## 1. The engine flags these natives test

| Flag (by what reads it) | Value in the game, as far as known |
|---|---|
| `GIsGame` (`IsGame` returns it) | true in a game session (set at start-up; zero in the image) |
| `GIsEditor` (`IsEditor` returns it) | false |
| `GIsPlayInEditorWorld` (`IsPlayInEditor` returns it) | false |
| the system detail mode (`WorldInfo.GetDetailMode` returns it) | the player's setting (set at start-up) |
| a "particles allowed" flag (by its use, the stock engine's) | **1** in the image's data |
| a "game-thread LOD" flag (by its use) | **1** in the image's data |
| a "force the lowest LOD" flag and an LOD bias | zero in the image; set at start-up from settings, not read |

The two flags read as 1 are written nowhere this lane looked. Their names are fits.

## 2. What `SetTemplate` does around the two branches (completes #581's step 5)

In order, for a template T:
1. **The PhysX swap.** In a game session (with other engine flags, not read), T may be replaced first. When T's
   `PhysxParticleSystemRef` is set and T has the `bLoadIfPhysXLevelN` flag for the machine's PhysX level N (0, 1 or
   2), that reference is used instead. Then T is post-loaded if it needs it.
2. **Particles not allowed, and not the editor:** `Template` := None, and nothing else.
3. `bIsViewRelevanceDirty` := true.
4. **The owner update.** When the component is not a template object (the class default or an archetype, checked
   through its outer chain), T is set, and `Owner` is set, part 1's conditional component update runs:
   - not `bAttached`: the conditional attach. That clears `bNeedsReattach` and `bNeedsUpdateTransform`, stores the
     scene and `Owner`, sets the transform, then makes the scene attach (whose body was not read; by its place, it
     sets `bAttached`);
   - `bAttached` with `bNeedsReattach`: detach and reattach;
   - `bAttached` with `bNeedsUpdateTransform`: the transform update.
5. `bWasCompleted` := false.
6. **The reset** (not for a template object). Each emitter instance is reset. With "changed" (T differs from the old
   `Template`) in a game session, the instances, `SMComponents` and `SMMaterialInterfaces` are freed and emptied. Then
   `bIsActive` := false, and the stop-audio step runs on the **old** template, since `Template` is not yet T
   (section 7.2).
7. `Template` := T, `WarmupTime` := T's `WarmupTime`.
8. **The choice:** the activation (section 3) when it is not a template object and either `bAutoActivate` is set, or
   it is `bIsActive` and not `bWasDeactivated` (as read before step 6 cleared `bIsActive`). Otherwise the
   initialisation (section 4).
9. `bAcceptsLights` := the `bLit` of `T.LODSettings[LODLevel]`, or false out of range.
10. The deferred reattach (part 1), when T changed or the component has a render proxy.

## 3. The activation (`ActivateSystem(bFlagAsJustAttached)`; from SetTemplate with false)

0. A template object: nothing at all.
1. **Particles not allowed, or the component's `DetailMode` above the system detail mode:** skip to step 15.
2. **The LOD for location.** In a game session with the game-thread LOD flag set, the LOD level is worked out for the
   component's world location (`DetermineLODLevelForLocation`, read):
   - T has at most one LOD distance: level 0;
   - otherwise: the nearest local player's view location, against T's `LODDistances`.

   A level that differs from `LODLevel` is applied (step 16). Outside a game session, `bLODUpdatePending` := true.
3. `bJustAttached` := true when the argument is set.
4. **The template's timings and the stop-audio step** (T set):
   - `EmitterDelay` := T's `Delay`. When T's `bUseDelayRange` is set, it is instead
     `DelayLow + (Delay − DelayLow) × r`, with r **one draw of the C runtime's `rand()`, scaled by 1/32767**. That is
     the C runtime's own generator, not a seeded engine stream;
   - `AudioEventDelay` := T's `fAudioDelaySeconds`;
   - the stop-audio step on T (section 7.2): when `AudioEnabled` is set and (a game session, or no `Owner`), it posts
     T's `StopAudioEvent` and, for a recorded looping sound, its `StopLoopingAudioEvent`, then `bStartEventPlayed` :=
     false and `LoopingAkPlayingInfo` := zero.
5. W := the old `bWasDeactivated`. Then `bWasCompleted`, `bSuppressSpawning` and `bWasDeactivated` := false, and
   `bIsActive` := true.
6. **No render proxy:** the deferred transform update:
   - `bNeedsUpdateTransform` := true;
   - with no `Owner`: the component's own transform update, when it is `bAttached`;
   - with an `Owner` that is `bStatic` or `bTickIsDisabled`, and not pending kill or a template object: the owner's
     component update.

   These are part 1's tests (#581's refusals C2 and D).
7. **The emitter instances.**
   - None yet (or, in the editor, a re-activation): the initialisation (section 4);
   - otherwise each instance is rewound and its spawning resumed (engine).
8. `bHasBeenActivated` := true.
9. **The LOD method.** "Automatic" is: `bOverrideLODMethod` set and the component's `LODMethod` is not the value 1,
   or `bOverrideLODMethod` clear and T's `LODMethod` is not 1 (the value names were not read).
10. **The owner part.**
    - No `Owner`: with W, the component's own transform update when it is `bAttached`.
    - An `Owner`: with W, `bNeedsUpdateTransform` := true; then part 1's conditional component update (section 2,
      step 4's three cases).
11. **The LOD level.**
    - With the force-lowest flag set: `bOverrideLODMethod` := true, `LODMethod` := 1, and the level := T's LOD count
      − 1.
    - Otherwise, with "automatic": the level for location (step 2), applied when the game-thread LOD flag is set.
12. **The warm-up**, when `WarmupTime` is not 0:
    - `bWarmingUp` and `bSkipUpdateDynamicDataDuringTick` are set;
    - each instance is readied for warm-up;
    - the component is ticked in steps of 0.032 s until the warm-up time is reached. That is particle simulation: it
      can spawn particles and draw random numbers;
    - then `WarmupTime` := 0, and both flags go back to what they were (`bWarmingUp` clear).
13. `AccumTickTime` := 0.
14. (Steps 2 and 11 apply the level through step 16.)
15. **Always (also after step 1):**
    - the component is registered with an engine-wide particle list (not script-visible; skipped under two engine
      flags);
    - `LastRenderTime` := the world's current time (the WorldInfo time field; by fit `TimeSeconds`).
16. **Applying an LOD level** (`SetLODLevel`'s code), only when T has LOD distances:
    - the level plus the LOD bias, clamped to 0 .. T's LOD count − 1;
    - when it differs from `LODLevel`: `bRecacheViewRelevance` := true and `LODLevel` := it;
    - a deferred reattach when the two levels' `bLit` differ, or their view relevance differs (with an `Owner`);
    - each instance is told the new level (engine).

## 4. The initialisation (the other branch)

Only when particles are allowed, the `DetailMode` fits, and it is not a template object:
1. **The instances' set-up** (a component virtual, read for its script-visible edge):
   - `WarmupTime` := T's;
   - `bSkipSpawnCountCheck` := T's.
   - With no instances yet, `SMComponents` and `SMMaterialInterfaces` are emptied. Then, unless hidden (in a game,
     `HiddenGame`, or the owner hidden without `bIgnoreOwnerHidden`; the exact owner test was not settled) or the
     detail mode is too low, one emitter instance is made per T emitter. Each is engine data behind an
     `EmitterInstances` pointer.
   - `LODLevel` is clamped to the emitters' LOD counts. `bWasCompleted` := false.
2. Then, when it is `bAttached`:
   - `AccumTickTime` := 0;
   - when not `bIsActive`, `bAutoActivate` set and not `bWasDeactivated`: the activation (section 3).

So the initialisation can itself end in the activation.

## 5. The slice's inputs and the probe

**Inputs** (the first probe, a stand-in for refusal E that writes nothing and reports; both links read the same):
- T is the globals definition's `PartPickup`, an `Engine.ParticleSystem`:
  - `WarmupTime` 0, `Delay` 0, `DelayLow` 0, `bUseDelayRange` false;
  - `LODMethod` 1, one LOD distance and one `LODSettings` entry (`bLit` false);
  - 4 emitters;
  - no audio events;
  - `PhysxParticleSystemRef` None and all three `bLoadIfPhysXLevel` flags false (the second probe).
- The `ParticleSystemComponent` class default: `bAutoActivate` true; `bIsActive`, `bWasDeactivated` and
  `bHasBeenActivated` false; `DetailMode` 0; `bOverrideLODMethod` false; `AudioEnabled` true; `HiddenGame` false;
  `LODLevel` 0; no instances; not attached.

**What that means for the slice's run** (a new component C, owned by the pickup after part 1's attach, not
`bAttached` in the port):
- no PhysX swap;
- particles allowed;
- the activation, since `bAutoActivate` is set (#581's refusal E);
- level 0 for location, the same as `LODLevel`, so no change;
- **no random draw**;
- the stop-audio step twice (the reset on the old template, None; the activation on T), each with no events, so
  only its two field writes each time;
- **no warm-up**;
- the instances made by the initialisation, then the activation;
- the owner part runs part 1's conditional update, which in the port finds C not `bAttached`. In the game, part 1's
  attach registered C, so this update is the transform branch at most.

**The probe**, under the machine lock, was a local merge of #593 (`fix/rust-spawn-pickup-particles` 9777c2cc) with
refusal E replaced by the stand-in, never pushed. Links 6 and 7 pass `make` and stop at `touch` with an `error`: the
unbound native `Actor.GetBoundingCylinder(float,float)`. Link 6 passes 1 of its 3 steps, link 7 1 of its 4. The gate as
a whole read ok 0, notyet 5, error 9 of 14; the other twelve links were as on #593. With #593 as it is, links 6 and 7
stop at refusal E in `make` (0 of 3, 0 of 4).

## 6. What only rendering, simulation or audio reads

- the emitter instances and everything inside them (particles, LOD state per instance, spawning, rewinds);
- the render proxy, view relevance, `bAcceptsLights` as a render flag, and `SMComponents` / `SMMaterialInterfaces`;
- the warm-up ticks;
- the audio events and `LoopingAkPlayingInfo`'s contents;
- the engine-wide particle list.

Script can still see the fields named in sections 2 to 4. That includes the length of `EmitterInstances`, which is 4
in the game on the slice.

## 7. A proposed Off answer, for the implementer

It replaces #581's refusals E and F: **#593's E and F refusals are removed** once this answer is coded. Every new
refusal below has the record `docs/verification/NATIVE_PARTICLE_ACTIVATION.md`; the refusals of other notes keep
their own texts and records.

**The hub's rulings on #597** (recorded here; all UNVERIFIED):
1. **Engine flags are harness inputs**, logged UNVERIFIED like the other harness inputs: `GIsGame` true, `GIsEditor`
   false, particles allowed, the game-thread LOD flag set, the force-lowest flag clear, LOD bias 0, and a system
   detail mode of at least the slice component's `DetailMode` (0).
2. **`bUseDelayRange` set is `NotPorted`.** The port has no stream for the C runtime's `rand()`. The draw is single
   precision (the stock `appFrand`, the port's `frand_f32`), and in the game it shares **one** C-runtime state with
   script `FRand` and the loot draws. The port splits that state into `CrtRand` and the named `CrtStream`s of
   `ow-core` `rand.rs`, so no existing stream can stand in for it. The slice makes no draw.
3. **The conditional component update runs only as this note says, never a second time** (section 7.3, step 3).
4. **`EmitterInstances` stays empty**, labelled UNVERIFIED (in the game its length on the slice is 4), because no
   script reads it. The search: every script function of the nine code packages was disassembled with
   `research/script_disasm.py` (12,968 of 12,978 decode; the 10 that do not decode were not searched) and searched for
   the property names `EmitterInstances`, `LoopingAkPlayingInfo` and `bStartEventPlayed`. There were **zero readers**
   of each. (Checks on the same search: `PickupParticles` gives 4 functions, `SetTemplate` 41, `bAutoActivate` 5.) If a
   reader on links 6 and 7's path is found later, the answer becomes `NotYet { LayerAbsent("particle emitter
   instances") }`, "ParticleSystemComponent.SetTemplate: the particle emitter instances", before any write.

### 7.1 The declarations the preflight type-checks

Beyond #593's three `ACTIVATION_FIELDS` (`bAutoActivate`, `bIsActive`, `bWasDeactivated`) and #581's preload list,
each is checked on the component C (or the template T) before any value is read. A missing or wrongly typed one is a
`VmError::Script` naming the field:
- **C, the activation and the reset:** `bJustAttached`, `bWasCompleted`, `bSuppressSpawning`, `bHasBeenActivated`,
  `bLODUpdatePending`, `bOverrideLODMethod`, `bWarmingUp`, `bSkipUpdateDynamicDataDuringTick`, `bRecacheViewRelevance`,
  `bSkipSpawnCountCheck`, `bIsViewRelevanceDirty`, `AudioEnabled` and `bStartEventPlayed` (bools); `EmitterDelay`,
  `AudioEventDelay`, `AccumTickTime` and `WarmupTime` (floats); `LODLevel` (an int); `LODMethod` (a byte);
  `LoopingAkPlayingInfo`, a struct `AkPlayingInfo` of `SourceComponent` (an `AkComponent` reference) and `AkPlayingId`
  (an int); `EmitterInstances` (an array of `Pointer` structs); `SMComponents` and `SMMaterialInterfaces` (arrays);
  `Template` (a `ParticleSystem` reference);
- **C, from `PrimitiveComponent` and `ActorComponent`:** `DetailMode` (a byte), `HiddenGame` and `bIgnoreOwnerHidden`
  (bools), `LastRenderTime` (a float), `Owner` (an `Actor` reference), `bAttached`, `bNeedsReattach` and
  `bNeedsUpdateTransform` (bools), and `Scene` (a `Pointer` struct);
- **T:** `PhysxParticleSystemRef` (a `ParticleSystem` reference); `bLoadIfPhysXLevel0`, `1` and `2`, `bUseDelayRange`
  and `bSkipSpawnCountCheck` (bools); `Delay`, `DelayLow`, `fAudioDelaySeconds` and `WarmupTime` (floats);
  `LODMethod` (a byte); `LODDistances` (an array of floats); `LODSettings` (an array of `ParticleSystemLOD`, each
  with a bool `bLit`); `Emitters` (an array); and the four audio fields `StartAudioEvent`, `StopAudioEvent`,
  `StartLoopingAudioEvent` and `StopLoopingAudioEvent` (`AkEvent` references).

**`LODMethod`.** The port compares the raw byte with 1 (the value names were not read; UNVERIFIED). This enum is
**exempt** from the tests' rule that enum values be declared in an order unlike the game's.

### 7.2 The audio steps (which template each reads)

The **stop-audio step** runs only when `AudioEnabled` is set and (a game session, or no `Owner`), and the component
is not being destroyed. It reads the component's `Template` **as it is at that moment**:
- it posts that template's `StopAudioEvent`, when set;
- when a looping sound is recorded in `LoopingAkPlayingInfo` (by its role; that test was not read), it posts that template's `StopLoopingAudioEvent` when
  set, and otherwise stops the recorded sound directly;
- then `bStartEventPlayed` := false and `LoopingAkPlayingInfo` := zero (both members).

It runs twice in `SetTemplate`'s path:
- **in the reset** (section 2, step 6), **before** `Template` := T, so it reads the **old** template. On a new
  component that is None: nothing is posted, and with `LoopingAkPlayingInfo` already zero nothing is stopped;
- **in the activation** (section 3, step 4), **after** `Template` := T, so it reads **T**.

"Audio events set" below means: the template the step reads has `StopAudioEvent` or `StopLoopingAudioEvent` not None,
or `LoopingAkPlayingInfo` holds a sound. (`StartAudioEvent` and `StartLoopingAudioEvent` are not posted by either
step; where the start is posted was not read.)

**On the slice:** `AudioEnabled` is set in the class default and the session is a game. So **the reset does write**
`bStartEventPlayed` (false) and `LoopingAkPlayingInfo` (zero), both already at those values. The activation writes
them again, with T's stop events None.

### 7.3 The preflight

It extends #593's at its step 5.2. It is write-free, load-free and draw-free and runs in this order; the first refusal
or data error met is the answer:
1. **Particles not allowed** (the port input, test switch only): the answer is the plan `Template` := None and nothing
   else. No refusal.
2. **The PhysX swap:** T's `PhysxParticleSystemRef` set and any `bLoadIfPhysXLevelN` set: `NotYet { NotRead }`,
   "ParticleSystemComponent.SetTemplate: the PhysX-level template swap (the machine's PhysX level)".
3. **SetTemplate's conditional update** (section 2, step 4), only for a component that is not a template object, with
   T and an `Owner`. By hub ruling 3 it is planned once:
   - **C attached by part 1 in this run** (C is in its `Owner`'s `Components`, where `Actor.AttachComponent` put it),
     or `bAttached` set: C counts as attached.
     - `bNeedsReattach` set: part 1's refusals by name: `REATTACH_OWNER` ("the deferred reattach: the owner's
       component update") or `REATTACH_SCENE` ("the deferred reattach: the component's detach and reattach"),
       record `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`.
     - `bNeedsUpdateTransform` set: the transform update, planned as `bNeedsUpdateTransform` := false (the scene
       layer's empty transform update; by its role, not read).
     - Neither: nothing.
   - **C never attached** (not in the `Owner`'s `Components` and not `bAttached`): the conditional attach's
     component-side writes, `bNeedsReattach` and `bNeedsUpdateTransform` := false and `Owner` := the owner. It does
     not append to `Components` (only `Actor.AttachComponent` does). Without a `SceneLayer` in the world: #520's
     `ATTACH`, `NotYet { LayerAbsent(SCENE_LAYER) }`, "Actor.AttachComponent: the component's attach to the world's
     scene", record `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`.
4. **The reset** (section 2, step 6), not for a template object:
   - existing emitter instances: `NotYet { LayerAbsent("a particle simulation layer (the port has none)") }`,
     "ParticleSystemComponent.SetTemplate: the emitter instances' reset";
   - the stop-audio step (section 7.2) on the **old** template, when its conditions hold: audio events set is
     `NotYet { LayerAbsent("an audio layer (the port has none)") }`, "ParticleSystemComponent.SetTemplate: the old
     template's stop audio events". Otherwise its two writes are planned;
   - `bIsActive` := false.
5. **The template-object case** (C is a class default or an archetype, through its outer chain): no conditional
   update, no reset, no activation or initialisation. The plan is SetTemplate's own writes (`bIsViewRelevanceDirty`,
   `bWasCompleted`, `Template`, `WarmupTime`, `bAcceptsLights`) and nothing else. No refusal. (A new component is
   never one; a held one could be.)
6. **The choice** (as #593 has it), then one of:
   - **the initialisation** (section 4):
     - `DetailMode` above the system detail mode: nothing in this branch;
     - otherwise `WarmupTime` and `bSkipSpawnCountCheck` from T; the instance set-up as hub ruling 4 says
       (`EmitterInstances` left empty, `LODLevel` unchanged); `bWasCompleted` := false; then, with `bAttached` or
       attached by part 1 (step 3's test), `AccumTickTime` := 0 and the auto-activation check, which leads into the
       activation below;
   - **the activation** (section 3), in its order:
     - **the DetailMode skip:** `DetailMode` above the system detail mode skips to the activation's step 15. Only
       `LastRenderTime` is planned, and the engine particle list is not modelled. No refusal;
     - **the LOD for location:** T with more than one LOD distance is `NotYet { LayerAbsent("a camera layer (the port
       has none)") }`, "ParticleSystemComponent.ActivateSystem: the LOD for location (the local players' view
       locations)". With at most one, level 0;
     - **the delay range:** `bUseDelayRange` set is `NotYet { NotPorted }`, "ParticleSystemComponent.ActivateSystem:
       the delay range draw (the C runtime's rand)" (hub ruling 2). Otherwise `EmitterDelay` := `Delay` and
       `AudioEventDelay` := `fAudioDelaySeconds`;
     - **the stop-audio step** on T (section 7.2): audio events set is `NotYet { LayerAbsent("an audio layer (the port
       has none)") }`, "ParticleSystemComponent.ActivateSystem: the template's stop audio events". Otherwise its two
       writes;
     - the four flags (section 3, step 5);
     - **step 6, no render proxy** (the port has none, so always): the deferred transform update.
       `bNeedsUpdateTransform` := true is planned. Then #581's refusals:
       - **C2:** with an `Owner` that is `bStatic` or `bTickIsDisabled` and not `bDeleteMe`: `NotYet { NotRead }`,
         "the deferred transform update: the owner's component update", record
         `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md`;
       - **D:** with no `Owner` and C `bAttached`: `NotYet { NotRead }`, "the deferred transform update: the
         component's own transform update", record `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md`;
     - **the instances:** none yet, so the initialisation as above (once: it does not activate again, since
       `bIsActive` is now set);
     - `bHasBeenActivated` := true;
     - **the owner part** (section 3, step 10): with an `Owner`, step 3's attached test again. On the slice C counts as
       attached, and the `bNeedsUpdateTransform` set by step 6 takes the transform branch, planned as
       `bNeedsUpdateTransform` := false. With no `Owner` and W, the own transform update is refusal D;
     - **the warm-up:** `WarmupTime` not 0 is `NotYet { LayerAbsent("a particle simulation layer (the port has
       none)") }`, "ParticleSystemComponent.ActivateSystem: the warm-up ticks";
     - `AccumTickTime` := 0;
     - the LOD level (no change on the slice: one LOD, level 0);
     - `LastRenderTime` := the world's time, from the world's clock (the clock NATIVE_ATTR_RESOLVERS.md names). A world
       with no clock is `NotYet { LayerAbsent("a world clock (the port's world has none)") }`,
       "ParticleSystemComponent.ActivateSystem: the world's time for LastRenderTime".
7. Then #581's steps 5.3 to 10, as that note gives them.

**Writes**, from the plan, with nothing that can refuse after the first, in the native's order:
- SetTemplate: `bIsViewRelevanceDirty`, the conditional update's writes, `bWasCompleted`, the reset's writes,
  `Template` and `WarmupTime`;
- the activation: `EmitterDelay`, `AudioEventDelay`, `bStartEventPlayed`, `LoopingAkPlayingInfo`, the four flags,
  `bNeedsUpdateTransform`, the initialisation's writes, `bHasBeenActivated`, the owner part's writes, `AccumTickTime`
  and `LastRenderTime`;
- then `bAcceptsLights` and the rest of #581's plan.

A late `NotYet` is marked only by #518's counters.

**On the slice** this preflight passes, and the native's first refusal is gone. The next stop is the `touch` step's
`Actor.GetBoundingCylinder`, as the probe shows.

**Choice 3's options and their consequences:**
- **"Never twice"** (the hub's ruling, coded above): a component part 1 attached counts as attached. The port takes the
  game's branches, and no write is repeated. On the slice it ends with `bNeedsUpdateTransform` false, the same as the
  game, where the transform update clears the flag step 6 set.
- **An empty scene that records `bAttached`** (not chosen): `Actor.AttachComponent` would set `bAttached`. That would
  change three things: #520's `ATTACH_DETACH` would refuse a second `AttachComponent` on the same component; the
  initialisation's step 2 would run its `AccumTickTime` and auto-activation check; and refusal D would apply to an
  ownerless attached component. The slice's end state would be the same.

**Tests, on invented classes** (rule 9; enum values declared in orders unlike the game's, except `LODMethod`, which is
compared as the raw byte 1):
1. the slice-shaped case:
   - auto-activate, one LOD, no audio events, no warm-up, no delay range, C attached by part 1;
   - every write in order: the reset's two audio writes and the activation's, `EmitterInstances` empty,
     `bNeedsUpdateTransform` ending false, no draw;
   - #518's three counters moved only by the writes;
2. the initialisation branch (`bAutoActivate` clear): its two copies and `bWasCompleted`, and no activation when C is
   neither `bAttached` nor attached by part 1;
3. the initialisation's auto-activation when C is attached;
4. each refusal before any write, with its exact text and record: the PhysX swap, the LOD for location, the
   delay-range draw (`NotPorted`), the old template's audio events (reset), T's audio events (activation), the warm-up,
   existing instances, C2, D, #520's `ATTACH` for a never-attached C with no `SceneLayer`, and part 1's
   `REATTACH_OWNER` and `REATTACH_SCENE` with `bNeedsReattach` set;
5. a template-object component: SetTemplate's own writes only;
6. particles-allowed false (the test switch): `Template` := None only;
7. `DetailMode` above the system's: the activation's steps skipped, `LastRenderTime` still written;
8. a world with no clock: the `LastRenderTime` refusal;
9. Reference binds nothing.

## Open questions

1. **Not read:**
   - the scene attach's body and whether it sets `bAttached`;
   - the post-load;
   - the engine particle list;
   - the instances' own work (reset, rewind, warm-up readiness, LOD);
   - the owner's component update;
   - the LOD method's value names;
   - the looping-sound test inside the stop-audio step, and where the start events are posted.
2. **UNVERIFIED:**
   - the flag names in section 1 and the game's values of the start-up flags;
   - the field fits, under the shifts named above;
   - that script reads none of what section 6 lists.
3. **Hub choices, ruled on #597** (section 7): the engine flags as harness inputs; `bUseDelayRange` as `NotPorted`;
   the conditional update never a second time; `EmitterInstances` left empty while no script reads it.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. **Field values.** Hook `ParticleSystemComponent.SetTemplate` on a weapon drop's pickup particle component and
   record, before and after: `Template`, `WarmupTime`, `bIsActive`, `bWasCompleted`, `bWasDeactivated`,
   `bHasBeenActivated`, `bIsViewRelevanceDirty`, `bAcceptsLights`, `EmitterDelay`, `AudioEventDelay`,
   `bStartEventPlayed`, `AccumTickTime`, `LastRenderTime`, `LODLevel`, `bAttached`, `bNeedsUpdateTransform`, and
   `EmitterInstances`' length. Expect sections 2 and 3, with 4 instances.
2. **The flags.** Read `WorldInfo.GetDetailMode()`, and `IsGame()` / `IsEditor()` from script.
3. **The draw.** Call `FRand` before and after `ActivateSystem` on a template with `bUseDelayRange` set, against a
   seeded expectation, to learn whether script `FRand` shares the C runtime's `rand()`.
