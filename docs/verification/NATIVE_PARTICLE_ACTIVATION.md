# ParticleSystemComponent activation and initialisation, as SetTemplate runs them (draft, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-PARTICLE-ACTIVATION (decision 2026-11-19-NNN,
number pending). Analyst only: a different lane implements it (clean room). This lane implemented #593
(`SpawnPickupParticles`' preflight) from NATIVE_SPAWN_PICKUP_PARTICLES.md before it read these natives. It does not
implement this note.

**Sources:**
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock):
  - the script natives `ParticleSystemComponent.SetTemplate` and `ActivateSystem`;
  - the engine routines they reach, each matched to its stock UE3 role by what it does;
  - the component and template virtuals, found by their vtable slots;
  - the script natives `IsGame`, `IsEditor`, `IsPlayInEditor` and `GetDetailMode`, to name the engine flags read here;
  - two flags' initial values, read from the image's data section;
- the installed packages: `tools/ghidra/class_layout.py` for `ParticleSystemComponent`, `PrimitiveComponent`,
  `ActorComponent` and `ParticleSystem`;
- two local probes of the installed slice gate (section 5), on a merge that is never pushed.

There is no bl-sdk session and no recorded live evidence on these natives. Raw output stays under ignored
`local/analysis/ppa/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Field names.** The code's offsets were matched to the layouts the tool computes:
- `ActorComponent` fields need no adjustment;
- `PrimitiveComponent` fields after its transform need a fixed shift;
- `ParticleSystemComponent`'s own fields need a fixed shift of 16 bytes;
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
   `bIsActive` := false, and the stop-audio step runs (section 3, step 4).
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
   - the stop-audio step, when `AudioEnabled` is set and (a game session, or no `Owner`). It posts T's stop audio
     events (their pairing to T's four audio fields was not settled), then `bStartEventPlayed` := false and
     `LoopingAkPlayingInfo` := zero.
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
- the stop-audio step with no events, so only its two field writes;
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

It replaces #581's refusals E and F. **Port inputs** (a hub ruling, UNVERIFIED): `GIsGame` true, `GIsEditor` false,
particles allowed, the game-thread LOD flag set, the force-lowest flag clear, LOD bias 0, and a system detail mode at
least the slice component's `DetailMode` (0). These constants belong in the world's set-up, logged like the other
harness inputs.

**Preflight**, extending #593's at its step 5.2, write-free, load-free and draw-free, in this order:
1. **The PhysX swap:** T's `PhysxParticleSystemRef` set with any `bLoadIfPhysXLevelN` set: `NotYet { NotRead }`, "the
   PhysX-level template swap (the machine's PhysX level)". Otherwise T stays.
2. **SetTemplate's owner update** (section 2, step 4): plan part 1's conditional update.
   - Not `bAttached`: the conditional attach's component-side writes, `bNeedsReattach` and `bNeedsUpdateTransform`
     := false and `Owner` := the owner, with the hub's empty scene. It does **not** append to the actor's
     `Components`: only `Actor.AttachComponent` does that, so part 1's whole `attach_plan` must not be reused here. The
     scene and transform writes are layer state, and `bAttached` stays as the scene layer leaves it. This keeps the
     slice going where the game would take the transform branch: **UNVERIFIED**, a port answer the hub must rule on.
   - `bAttached` with either flag: part 1's existing refusals.
3. **The reset** (section 2, step 6) on a new or instance-free component: `bIsActive` := false, and the stop-audio
   step's planned writes.
   - With audio events set and `AudioEnabled`: `NotYet { LayerAbsent }`, "the template's audio events", layer "an
     audio layer (the port has none)".
   - Instances present: `NotYet { LayerAbsent }`, "the emitter instances' reset", layer "a particle simulation layer
     (the port has none)".
4. **The choice** as #593 has it. Then:
   - **the initialisation:** `WarmupTime` and `bSkipSpawnCountCheck` from T, and the instance set-up as an empty
     particle layer: no instances made, `EmitterInstances` left empty, `LODLevel` unchanged (UNVERIFIED: the game has
     4 instances on the slice). `bWasCompleted` := false. Then, with `bAttached` set: `AccumTickTime` := 0 and the
     auto-activation check;
   - **the activation**, in section 3's order:
     - T with more than one LOD distance: `NotYet { LayerAbsent }`, "the LOD for location (the local players' view
       locations)", layer "a camera layer (the port has none)". With at most one, level 0;
     - `bUseDelayRange` set: one draw, but which `GameRng` stream stands for the C runtime's `rand()` is a hub choice.
       Until it is made, that case is `NotYet { NotPorted }`, "ParticleSystemComponent.ActivateSystem: the delay
       range draw (the C runtime's rand)";
     - audio events with `AudioEnabled`: the audio `LayerAbsent` above;
     - `WarmupTime` not 0: `NotYet { LayerAbsent }`, "the warm-up ticks", layer "a particle simulation layer (the
       port has none)";
     - no instances: the initialisation, as above (once: it does not re-activate, since `bIsActive` is now set);
     - the owner part: #581's refusals C2 and D (the owner and transform tests), and the conditional update of step 2;
     - the LOD level (no change on the slice: one LOD, level 0);
     - `LastRenderTime` := the world's time, from the world's clock (the same clock NATIVE_ATTR_RESOLVERS.md
       names), else `NotYet { LayerAbsent }` when the world has none.
5. Then #581's steps 5.3 to 10, as that note gives them.

**Writes**, from the plan, with nothing that can refuse after the first, in the native's order:
- SetTemplate: `bIsViewRelevanceDirty`, the owner update's writes, `bWasCompleted`, the reset's writes, `Template`,
  `WarmupTime`;
- the activation: `EmitterDelay`, `AudioEventDelay`, `bStartEventPlayed`, `LoopingAkPlayingInfo`, the four flags,
  `bNeedsUpdateTransform`, the initialisation's writes, `bHasBeenActivated`, the owner part's writes, `AccumTickTime`,
  `LastRenderTime`;
- then `bAcceptsLights` and the rest of #581's plan.

A late `NotYet` is marked only by #518's counters.

**On the slice** this preflight passes. The native's first refusal is gone. The next stop is the `touch` step's
`Actor.GetBoundingCylinder`, as the probe shows, provided the hub rules that the port's repeated attach (step 2) is an
answer.

**Tests, on invented classes** (rule 9; enum values in orders unlike the game's):
1. the slice-shaped case:
   - auto-activate, one LOD, no audio, no warm-up, no delay range;
   - every write in order, `EmitterInstances` empty, no draw;
   - #518's three counters moved only by the writes;
2. the initialisation branch (`bAutoActivate` clear): its two copies and `bWasCompleted`, and no activation without
   `bAttached`;
3. the initialisation's auto-activation when `bAttached` is set;
4. each refusal before any write: the PhysX swap, the LOD for location, the delay-range draw, audio events, the
   warm-up, existing instances;
5. a template-object component: no activation writes;
6. particles-allowed false (a test switch on the port input): `Template` := None only;
7. `DetailMode` above the system's: the activation's steps skipped, `LastRenderTime` still written;
8. Reference binds nothing.

## Open questions

1. **Not read:**
   - the scene attach's body and whether it sets `bAttached`;
   - the post-load;
   - the engine particle list;
   - the instances' own work (reset, rewind, warm-up readiness, LOD);
   - the owner's component update;
   - the LOD method's value names;
   - which audio fields the stop step posts.
2. **UNVERIFIED:**
   - the flag names in section 1 and the game's values of the start-up flags;
   - the field fits, under the shifts named above;
   - that script reads none of what section 6 lists.
3. **Hub choices:**
   - the port inputs in section 7;
   - the stream for the C runtime's `rand()` (the slice does not draw);
   - whether the port's repeated attach in step 2 is an answer, or the empty scene layer should record `bAttached`
     so the game's transform branch is taken.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. **Field values.** Hook `ParticleSystemComponent.SetTemplate` on a weapon drop's pickup particle component and
   record, before and after: `Template`, `WarmupTime`, `bIsActive`, `bWasCompleted`, `bWasDeactivated`,
   `bHasBeenActivated`, `bIsViewRelevanceDirty`, `bAcceptsLights`, `EmitterDelay`, `AudioEventDelay`,
   `bStartEventPlayed`, `AccumTickTime`, `LastRenderTime`, `LODLevel`, `bAttached`, `bNeedsUpdateTransform`, and
   `EmitterInstances`' length. Expect sections 2 and 3, with 4 instances.
2. **The flags.** Read `WorldInfo.GetDetailMode()`, and `IsGame()` / `IsEditor()` from script.
3. **The draw.** Call `FRand` before and after `ActivateSystem` on a template with `bUseDelayRange` set, against a
   seeded expectation, to learn whether script `FRand` shares the C runtime's `rand()`.
