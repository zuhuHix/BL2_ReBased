# OpenWillow: Borderlands 2 engine reimplementation plan

Working title ("Willow" is Gearbox's internal name for the BL2 engine branch). Companion to
`BL2_REMASTER_ANALYSIS.md`, the research this plan is built on.

**What this is:** a new engine that runs Borderlands 2 from the player's own, legitimately purchased install.
It reads Gearbox's files. It ships none of them. It contains no Gearbox code.

**What it unlocks:** every item on the analysis's "blocked" list, new maps, modern renderer, relighting, 64-bit,
ultrawide, split-screen, *your own co-op netcode* (the #1 complaint), platform freedom, preservation, plus The
Pre-Sequel and standalone Dragon Keep, which run on the same engine branch.

**Who this is written for:** someone directing an AI-heavy build without (yet) being a professional programmer.
That's exactly why every step ends in something you can *check against the real game* yourself. That check is the
actual job here, and it's not a lesser one: code that can't be checked against reality doesn't get merged, no matter
who or what wrote it.

---

## 0. Ground truth: the numbers this plan rests on

All measured directly from the installed game (see analysis §4).

| Fact | Value |
|---|---|
| Engine | UE3, Gearbox "Willow2" branch, package version **832 / licensee 46**, MSVC 2008-era, D3D9 / SM3 |
| Code packages | Core, Engine, GameFramework, GearboxFramework, WillowGame, GFxUI, IpDrv, OnlineSubsystemSteamworks, AkAudio, fully LZO-compressed |
| Total functions | **20,119** |
| Inherited (UnrealScript bytecode) | **12,978 (64.5%)**, executed by our VM as-is |
| Rebuilt (native C++) | **7,141 (35.5%)** |
| …of which trivial builtins | 286 |
| …of which online/save/DLC (replace, don't replicate) | 609 |
| …of which Scaleform UI bridge | 512 |
| …of which stock UE3 engine (contracts public via UDK headers) | 1,914 |
| …of which **Gearbox-specific, undocumented** | **3,803 across 443 classes** |
| Concentration | top 50 classes = 52% of natives; top 200 = 78% |
| Behavior system (skills, weapon effects, missions) | 302 classes, **268 pure script → inherited** |
| Stat/skill/item core | ~120 natives, zero script (`SkillDefinition` 44, `AttributeDefinition*`, `WeaponPartDefinition`, `InventoryBalanceDefinition`) |
| Animation | data inherited; player rebuilt (`SkeletalMeshComponent` 127 natives, ~30 `AnimNode*` types) |
| Levels | 82 persistent maps (37 base + 45 DLC), each split into `_P / _Dynamic / _Combat / _Light / _FX / _Audio / _Skybox / _Px` |
| Textures | streamed from `.tfc` caches (Textures 1 GB, CharTextures 772 MB, Lighting 154 MB; UHD pack 7.5 GB) |
| Audio / video / UI | Wwise `.pck` / Bink `.bik` / Scaleform GFx (Flash SWF) |

**Already built in this project:** `research/native_count.py`: a working pure-Python LZO1X decompressor and UE3
package reader (names, imports, exports) for version 832. It parsed every code package with zero errors and matched
Gearbox's own `.uncompressed_size` files byte-for-byte. That is the seed of the package loader.

---

## 1. Hard rules (legal hygiene: non-negotiable)

1. **Never redistribute a Gearbox file.** Not a texture, not a sound, not a UPK, not a decompressed dump. The engine
   reads them from the user's install at runtime. Test fixtures in the repo must be synthetic or generated.
2. **Never use, read, or accept leaked source code.** Not UE3's, not Gearbox's. If someone offers, refuse in writing.
   Clean-room only: file formats, observed behaviour, public documentation.
3. **No decompiled C++ from `Borderlands2.exe` in the repo.** Observing behaviour by running the game (with
   unrealsdk instrumentation) is fine. Disassembling the exe and transcribing it is the re3 line; don't cross it.
4. **Never sell anything.** Donations to the *engine project* only. No paid builds, no "premium" anything.
5. **Require the original game.** The engine must refuse to start without a valid BL2 install.
6. **License hygiene for references.** Legendary Explorer is GPL-3 (copying its code makes your project GPL);
   umodel's source has its own terms; check every license before copying a single line. Referencing a *format* is
   always fine; copying *code* depends on the license.
7. **Credit and transparency.** State plainly that the project is AI-assisted. It's a strength, not a secret.

This list isn't original. It's how OpenMW, OpenRCT2, Daggerfall Unity, OpenGothic and Ship of Harkinian all stayed alive long enough to finish, so it's how this one runs too.

---

## 2. Architecture

```
                 ┌──────────────────────────────────────────────────────┐
                 │  HOST ENGINE (renderer, physics, audio, input, UI)   │
                 └───────────────▲──────────────────────▲───────────────┘
                                 │                      │
   ┌─────────────────────────────┴───┐    ┌─────────────┴──────────────────┐
   │  NATIVE LAYER (C++)             │    │  ASSET PIPELINE                │
   │  the 7,141 rebuilt functions:   │    │  package loader (UPK/TFC)      │
   │  Actor/Pawn/Controller, traces, │    │  textures (DXT → GPU)          │
   │  movement, anim player, particles│   │  static/skeletal meshes        │
   │  navmesh/AI, stat core, weapons,│    │  animations, physics assets    │
   │  Scaleform bridge, audio bridge │    │  materials (node graph → host) │
   └─────────────────────────────▲───┘    │  levels (actors + transforms)  │
                                 │        │  lightmaps, Kismet, Matinee    │
   ┌─────────────────────────────┴───┐    │  Wwise / Bink / SWF            │
   │  UNREALSCRIPT VM                │    └────────────────────────────────┘
   │  object model (UObject, FName,  │
   │  classes, CDOs, properties),    │
   │  bytecode interpreter, states,  │
   │  latent funcs, timers, delegates│
   │  native dispatch table          │
   └─────────────────────────────────┘
        runs the 12,978 inherited functions unchanged
```

Three layers. The VM and asset pipeline are engine-agnostic C++. The native layer is where the host engine shows up.

### 2.1 Host engine decision: decide by end of Phase 0, never after

**Recommendation: Unreal Engine 5.** Reasons, in order of weight:
1. **The conceptual mapping is nearly 1:1.** UE3 material node graphs → UE5 material graphs. AnimTree → AnimBP.
   Matinee → Sequencer. Cascade → Niagara. Kismet → Blueprint. UObject/FName/reflection still exist. Every UE3
   system you must rebuild has a modern sibling to translate *into* rather than invent.
2. **AI knows UE5 better than any other engine**: the deepest public corpus. Your engineer is strongest here.
3. **The editor is free.** New maps, the original dream, arrive the day levels load.
4. Chaos physics, navmesh, networking, and a production renderer are already there.

**Costs, honestly:** heavy toolchain, slow C++ compile loops for a beginner, big install, source-available (not
open), Epic EULA (fine for a free project). If Phase 0 shows the weight is crushing your iteration speed, **Godot**
(MIT, tiny, fast compiles, C++ via GDExtension) is the alternative. Switching engines after Phase 1 is fatal, 
decide once.

### 2.2 Language
C++ for the VM, native layer and asset pipeline; non-negotiable for performance (thousands of actors running
interpreted script). Python for tooling and analysis (like `native_count.py`).

### 2.3 External extraction accelerator

The asset pipeline may use mature community exporters, with UModel / UE Viewer
as the first candidate, to shorten local visual-asset preparation for the
Sanctuary/Maya vertical slice. This is an implementation tactic, not a change
to the runtime architecture. External exports remain untrusted payloads that
need object-path, reference, material, LOD, animation and UE5 import checks.
The OpenWillow reader remains responsible for package identity, metadata,
cross-package resolution, scene manifests and verification, and it remains the
fallback for exporter gaps.

The external path must be benchmarked on representative BL2 packages before it
becomes a default. See the Phase 0.5 gate in `ROADMAP.md`; the measured export
time may reduce the asset-preparation part of the vertical slice, but does not
reduce the later VM, native behavior, gameplay or campaign work.

---

## 3. Sources of truth: where "correct" comes from

Every native you rebuild needs a definition of correct. There are exactly three sources, and knowing which one
applies to a given function is half the work.

| Source | Covers | How |
|---|---|---|
| **A. UDK** (free, runs on your PC) | The 1,914 stock UE3 natives | UDK ships `Engine/Classes/*.uc` with the exact native declarations *and doc comments*. Better: UDK is a **running UE3**: write a test script, run it, observe what `Trace()` or `MoveSmooth()` actually does. Ground truth for bucket 5, free. |
| **B. The original game + unrealsdk** | The 3,803 Gearbox natives | Hook a native with unrealsdk, log every call's inputs and outputs during play, save as a "golden file." Implement until your VM produces the same outputs for the same inputs. **This is the core methodology of Phase 4 and it is exactly the loop you described, go in, observe, report.** |
| **C. Community documentation** | Stat math, part weights, drop pools, population scheduling | BLCM wiki, bl2.parts, Lootlemon, a decade of modder notes. Written specs for the ~120-function stat core. |

Rule: a native without a golden file is a guess. Guesses are labelled `// UNVERIFIED` and tracked.

---

## 4. What is going to be hard: ranked, honestly

1. **The 3,803 undocumented Gearbox natives.** No spec except the running game. Every one needs instrumentation,
   golden data, implementation, comparison. This is 60% of the project's total effort and almost all of its calendar
   time. Concentration helps (200 classes carry 78%), but the long tail is long.
2. **Script VM correctness in the corners.** The opcode set is documented, but states, latent functions, struct
   copy semantics, delegates, `out` parameters, default-property loading from CDOs, and replication flags are
   where interpreters go subtly wrong, and subtle VM bugs surface as inexplicable gameplay weirdness far away.
3. **Materials.** UE3 material node graphs must become host-engine materials. Most nodes translate; SM3-era tricks
   and Gearbox's custom nodes (the cel-shade edge, the ink lines) need hand-crafting. The *look* of BL2 lives here.
4. **Cascade particles.** Dozens of module types with precise semantics. "Close enough" is achievable; exact is a
   project of its own. Muzzle flashes, elemental effects and explosions are BL2's feel.
5. **Animation fidelity.** ~30 AnimNode types, blend timing, root motion, physics assets and ragdoll. Data is
   inherited; making it *move* identically is not.
6. **AI feel.** GearboxFramework's `AIComponent`, `GearboxMind`, `GearboxCoverStateManager`, navmesh, cover.
   Enemies that shoot back is M3; enemies that *behave like BL2 enemies* is M4.
7. **Scaleform.** The entire UI is Flash (AS2) running Gearbox's ActionScript, plus 512 bridge natives. Needs an
   SWF VM (Ruffle-class) and handling of Scaleform's `.gfx` extensions. Skippable early (debug UI), unavoidable
   for M4.
8. **Performance.** UE3 ran thousands of scripted actors fine because its VM was tight C++. A sloppy VM will not.
9. **Scope and burnout.** The actual cause of death for 90% of these projects. Mitigated only by the phase gates
   below and by shipping M1 publicly.
10. **Fidelity vs modernisation tension.** Every system has a choice: replicate SM3 lightmaps or use Lumen?
    Replicate Cascade or approximate with Niagara? Rule: **fidelity first where it defines feel (guns, movement,
    skills); modernisation first where it defines look (lighting, resolution).**

---

## 5. Phases

Times assume one person, near-full-time, AI-assisted, learning as they go. Halve the pace for part-time. These are
honest ranges, not promises; no project of this shape has been completed AI-first yet.

### 5.0 Priority: the vertical slice

Adopted 2026-09-18 from outside feedback (see `DECISIONS.md`). Porting all 82 maps is the *easier* part of this
project; the harder, unproven part is Phases 2-4 below. Finishing map breadth first, only to discover in Phase 2-4
that the approach doesn't hold, would waste most of the project's calendar time on the easy part. So the phases
below are scoped to prove the hard parts on **one map and one character** before spending time on breadth:

1. Finish **Sanctuary** (already the most complete map) to visual/walkable parity.
2. Get **Maya** (chosen 2026-09-18) working end-to-end: movement, a few weapons, animations, her action skill
   (Phaselock). Her skill trees (Motion, Harmony, Cataclysm) are the target for Phase 4's skill-tree work.
3. Phases 2 and 3 below, scoped to what Sanctuary and that one character need, not full native coverage.
4. One hand-picked simple mission and a handful of guns, end-to-end: spawn, fight, loot, equip, use a skill,
   complete the mission, die, respawn. That's Phase 4's gate.

Broad map coverage (the remaining ~79 maps) and the remaining Vault Hunters move to Phase 5, after the Phase 4 gate
is met.

### Phase 0: Foundation & spikes · 3–6 weeks

Goal: a repo, a build, a package loader, and a decided host engine.

**Steps**
1. Repo + license (choose GPL-3 or MIT deliberately, GPL if you'll borrow from Legendary Explorer), README with
   the hard rules from §1, `CONTRIBUTING.md`, issue tracker.
2. Dev environment: Visual Studio, CMake, the host engine, Git. AI walks you through it; you screenshot errors.
3. **Port the package loader to C++** from `native_count.py`: LZO1X, chunk container, names/imports/exports.
   Verify: identical export counts to the Python tool for every code package.
4. **Extend to all 914+ base packages and DLC.** Catalog every export by class. Output: a JSON census, "BL2
   contains N StaticMeshes, N Texture2Ds, N SkeletalMeshes, N Materials, N AnimSequences…" Verify: totals are
   stable across runs and plausible against umodel's view of the same package.
5. **Tagged-property stream reader.** Every UObject's properties (int/float/bool/name/object/struct/array). This is
   the single most-reused piece of code in the project. Verify: dump a known object (e.g. a gun's
   `WeaponPartDefinition`) and compare fields against what BLCMM shows for the same object.
6. **Spike: one texture.** Decode a `Texture2D` (DXT1/DXT5, mips, TFC-streamed high mips) to PNG.
   Verify: open it; it's the texture.
7. **Spike: one static mesh.** Vertex/index buffers, UVs, sections, materials refs → export OBJ.
   Verify: opens in Blender; it's the mesh.
8. **Decide the host engine.** Build the hello-world in it. Load the OBJ + PNG into it. Decision made; write it down.

**Gate:** the census exists, one mesh and one texture from BL2 render inside the host engine.

### Phase 1: World viewer (M1) · 2–4 months

Goal: walk around any BL2 map in a modern 64-bit renderer. **Ship this publicly.** It's the proof, the recruiting
poster, and the thing that tells you whether the loop holds.

**Steps**
1. Static mesh importer for all meshes (LODs, collision hulls from `RB_BodySetup`).
2. Texture importer with TFC streaming (base + CharTextures + Lighting; UHD pack optional, the 4 GB wall is gone).
3. **Material translation v1**: diffuse/normal/specular/emissive approximation for every material.
   v2 later: real node-graph translation, cel-shade and ink lines.
4. **Level loader**: parse `_P` and its sublevels; instantiate every `StaticMeshActor` with transform; skybox;
   terrain if present; `_Dynamic` props as static for now.
5. **Lighting: two options, do both eventually:**
   - *Modern first (recommended):* Lumen/dynamic GI. Zero lightmap parsing, immediate wow, but not BL2's look.
   - *Fidelity:* parse SM3 directional lightmaps from `Lighting.tfc` and apply via a custom material.
6. Free-fly camera → then a placeholder character controller with collision.
7. Map selector for all 82 levels; loading times; memory profile.

**Verification:** side-by-side screenshots against the real game from the same spot, per map. Keep a folder.
Your eye is the test.

**Gate (rescoped 2026-09-18):** Sanctuary loads, is walkable, and is visually verified against the real game →
move on to Phase 2 for the vertical slice. Full 82/82 map coverage remains this phase's eventual completion target
but is deferred until the Phase 4 vertical-slice gate is met (§5.0); see Phase 5. If at 4 months you cannot load
*one* map, stop and reassess honestly; the loop isn't holding.

### Phase 2: UnrealScript VM (M2) · 3–6 months

Goal: Gearbox's own gameplay code executing, scoped to what Sanctuary and the chosen first Vault Hunter need (§5.0),
not full native coverage yet.

**Steps**
1. **Object model**: `UObject` with class, outer, name, property bag; `UClass` with hierarchy and default objects
   loaded from package CDOs; `FName` table; object references resolved across packages (imports).
2. **Bytecode loader**: read `Script` from every `UFunction`/`UState`; opcode table (documented, UE Explorer's
   source and UDK docs enumerate them; UE Explorer's "Borderlands 2 [Status]" notes flag a Gearbox quirk: a
   local-variable array at function start, handle it).
3. **Interpreter**: expressions, locals, `out` params, structs, dynamic arrays, casts, `foreach` iterators,
   `switch`, `goto`, delegates, `super` calls, states and state transitions, latent functions (`Sleep`,
   `FinishAnim`), timers.
4. **Native dispatch table**: every one of the 7,141 natives registered as a stub that logs `UNIMPLEMENTED name(args)`.
   Now every missing native announces itself.
5. **Implement the 286 Core builtins** (operators, math, string, name, object). Trivial; unblocks everything.
6. **Test harness**: run pure-script classes in isolation. The 268 script-only `Behavior_*` classes are ideal, 
   feed inputs, compare outputs to hand-computed or UDK-computed results.

**Verification:** a corpus of script functions with known outputs; the VM matches them. Ground truth from UDK
(source A), run the same UnrealScript there.

**Gate:** VM runs `Behavior_*` chains and the stat-free parts of `WillowWeapon` without crashing; every unmet
native is a logged stub, not a mystery.

### Phase 3: Stock engine natives (M3a) · 6–12 months

Goal: the UE3 natives the vertical slice needs (movement, collision, animation playback), not all 1,914 up front
(§5.0). Ground truth = UDK (source A).

**Order, by dependency:**
1. `Actor` (166): spawn/destroy, transforms, timers, `Trace`/`FastTrace`, `SetLocation`/`Move`, attachment,
   tick, `bStatic`/`bHidden` semantics, `Touch`/`Bump` events (the 2,453 script events are *called by* these).
2. `PrimitiveComponent` / collision (47) → host physics (Chaos).
3. `Pawn` (126) + `Controller` (45) + `PlayerController` (73): movement physics (walking, falling, jumping,
   slopes, UE3's `physWalking` semantics matter to gunplay feel), input, camera, possession.
4. `SkeletalMeshComponent` (127) + the ~30 `AnimNode*` types + `PhysicsAssetInstance` (19): the animation player.
   Verify against UDK's own AnimTree behaviour.
5. `ParticleSystemComponent` (46) + Cascade modules → Niagara translation.
6. `WorldInfo` (56), `NavigationHandle` (44), `Settings`, `Camera`, `Light`, `Sound` stubs.

**Verification:** each native gets a UDK-derived golden test. A "generic UE3 pawn" walks, jumps and falls in your
engine exactly as it does in UDK.

**Gate:** a scripted UE3 pawn moves and animates on a BL2 map with correct collision.

### Phase 4: Willow natives (M3b) · 12–24 months · **the mountain**

Goal: **Maya**, the vertical slice's first Vault Hunter (§5.0), walks, shoots a handful of real guns, uses
Phaselock and her skill trees, and enemies on Sanctuary fight back. Ground truth = the original game instrumented
with unrealsdk (source B) + community docs (source C).

**Method: the golden-file loop (your loop, formalised):**
1. Pick a native from the priority list.
2. Hook it in the real game with unrealsdk; play; log every call (args → return, plus relevant object state).
3. That log is the spec. AI implements. Your engine replays the same inputs; outputs must match.
4. Mismatch → you go back in-game, reproduce, extend the log. Repeat.

**Priority order (by "what gets a playable character soonest"):**
1. **The stat core (~120):** `AttributeDefinition*`, `AttributeInitializationDefinition`, `SkillDefinition`,
   `WeaponPartDefinition`, `InventoryBalanceDefinition`, `WillowDamageTypeDefinition`. Community math (source C)
   gives you the spec for most of it before you instrument anything. Small, dense, decisive.
2. `WillowPawn` (246) + `WillowPlayerPawn` (65): health/shield, Fight-For-Your-Life, damage intake.
3. `WillowPlayerController` (298): input, aiming, ADS, skills activation, HUD hooks, interaction.
4. `WillowWeapon` (121) + `WillowProjectile` (95): fire, reload, spread, recoil, elemental procs, projectile
   behaviours. **This is where "does it feel like BL2" lives, the highest-stakes fidelity in the project.**
5. `WillowInventory` (50) + `WillowItem` (39) + item generation: guns actually generate from parts.
6. `WillowAIPawn` (121) + `WillowMind` (47) + `WillowAIComponent` (45) + GearboxFramework AI (`AIComponent` 48,
   `GearboxMind` 32, `GearboxCoverStateManager` 48): enemies.
7. `WillowInteractiveObject` (134): doors, chests, vendors, switches.
8. `WillowVehicle` (156): Runners and Technicals.
9. Scaleform bridge (512) + `WillowHUDGFxMovie` (98) + `WillowUIInteraction` (41): the real UI via an SWF VM.
   Until then, a debug HUD.
10. Wwise (17 + `AkAudio`) via wwiser/vgmstream decoding; Bink via FFmpeg.

**Verification:** you, playing, with a checklist per system: "Jakobs recoil recovery matches," "Maya's Phaselock
duration matches," "Bee shield amp matches." Golden files for numbers; your hands for feel.

**Gate:** a full loop on Sanctuary with Maya: spawn, fight enemies, loot a gun, equip it, use Phaselock, complete
one hand-picked mission, die, respawn. This is the vertical slice (§5.0); once met, broad map and character
coverage (Phase 5) becomes the priority again.

### Phase 5: Campaign completable (M4) · 12–24 months

0. **Broad map coverage** (the remaining ~79 maps) and **remaining Vault Hunters** (5 of 6), both deferred from
   Phases 1 and 4 by the 2026-09-18 vertical-slice rescoping (§5.0).
1. **Kismet interpreter**: sequences are data in the map packages; implement the node types they use
   (`MissionTracker` 81 natives sits here).
2. **Matinee → Sequencer** for cutscenes/scripted moments.
3. Mission system, objectives, fast travel, vending, dialogue/echo system, Bink cutscene playback.
4. **Saves**: Gibbed's save format is open source; read/write real BL2 saves (huge win: players keep characters).
5. Full Scaleform UI. Menus, inventory, skill trees, map.
6. All 82 maps' `_Combat` populations, spawn schedules, bosses, raid mechanics.

**Gate:** Claptrap to Warrior, single player, with a real save file.

### Phase 6: Parity and beyond (M5) · ongoing

- **Co-op netcode: yours.** Fixes the #1 complaint permanently.
- DLC coverage; The Pre-Sequel; standalone Dragon Keep.
- **Mod compatibility**: BLCMM text mods are `set Object Property Value` on the object graph; your VM *has* that
  object graph; support them natively from day one of M4. SDK-mod compatibility layer later. A decade of mods
  carries over; the community doesn't split.
- Editor for new maps. Modern lighting toggle. VR. 8-player. Everything on the daydream list.

---

## 6. Milestone summary

| Milestone | What you can show | Full-time + AI | Part-time |
|---|---|---|---|
| Phase 0 | A mesh and a texture from BL2 inside the host engine | 3–6 wks | 2–3 mo |
| **M1** | Sanctuary walkable and verified, modern renderer, 64-bit (rescoped 2026-09-18; all 82 maps moves to M4) | 2–4 mo | 6–12 mo |
| M2 | Gearbox's script running in your VM, scoped to Sanctuary + one Vault Hunter | +3–6 mo | +12 mo |
| M3a | A UE3 pawn moves/animates correctly on Sanctuary | +6–12 mo | +2 yr |
| M3b | Maya shoots real guns on Sanctuary, uses Phaselock, enemies fight back, one mission completable: **the vertical slice** | +12–24 mo | +3–4 yr |
| M4 | Campaign completable with real saves, remaining maps and Vault Hunters added | +12–24 mo | +3 yr |
| M5 | Co-op, DLC, TPS, mods, editor | ongoing | ongoing |
| **Cumulative to M4** | | **~3–5 years** | **~8–10 years** |

---

## 7. The working loop: how a non-programmer drives this

1. **Task size:** one native, one node type, one importer at a time. Never "implement the animation system."
2. **Every task ends in a check you can perform.** A screenshot comparison, a golden-file diff, a number from
   BLCMM, a behaviour in UDK. If the AI can't tell you how you'll verify it, the task isn't ready.
3. **Golden files are sacred.** Instrumented logs from the real game live in a `golden/` folder (they're your
   observations, not Gearbox assets, but keep them free of raw asset data). The test suite replays them on every
   build. A change that breaks a golden test doesn't merge.
4. **Weekly gate:** the build compiles and loads Sanctuary. If it doesn't, nothing else happens until it does.
5. **Git discipline:** small commits, a branch per task, `main` always builds. AI can run this; you own it.
6. **Keep a `DECISIONS.md`.** Every architectural choice with its reason. Future-you and future contributors need it.
7. **Read the code the AI writes, even before you understand it.** You will understand it sooner than you think.
   Ask it to explain any line you can't. That's the "learning by accident"; it is not optional and it is the reason
   this can work.
8. **Publish early.** M1 in public. Contributors arrive for things that already run.
9. **Debug it yourself before asking the AI to fix it** (adopted 2026-09-18 from outside feedback). Read the
   error, check documentation, Stack Overflow, GitHub issues, and understand what's actually wrong before bringing
   it to an AI assistant, and only once genuinely stuck. Throwing an error message straight back at an AI teaches
   you nothing and lets technical debt pile up quietly; AI is a tool here, not the developer.

---

## 8. Reference projects (formats and behaviour: check licenses before copying code)

| Project | Use it for |
|---|---|
| **umodel / UE Viewer** (Gildor) | The definitive reference for UE3 mesh/texture/anim serialization, incl. BL2 |
| **Legendary Explorer** (ME3Tweaks, GPL-3) | Package format, property streams, level/Kismet parsing; the proof that a LEX-class toolkit is buildable |
| **UE Explorer** (Eliot) | UnrealScript bytecode decompilation; opcode reference; BL2 status notes |
| **unrealsdk / pyunrealsdk** (bl-sdk) | Live instrumentation of the real game; your golden-file generator |
| **Gibbed.Borderlands2** | Save file format |
| **UDK** (Epic, free) | Running UE3 for stock-native ground truth; the `.uc` headers with doc comments |
| **Ruffle** | SWF/ActionScript VM for the Scaleform UI |
| **wwiser / vgmstream** | Wwise bank decoding |
| **FFmpeg** | Bink video |
| **BLCM wiki / bl2.parts / Lootlemon** | Stat math, part weights, drop pools; written specs for the stat core |

---

## 9. Kill criteria: be honest with yourself

- Phase 0 not gated in **3 months** → the environment/tooling loop isn't working; fix that before anything else.
- M1 cannot load a single map by **month 6** → the loop isn't holding for this project. Stop, reassess honestly,
  and don't keep pushing on hope alone.
- Nobody but you has contributed by **M2** → still fine, but stop planning M5 and plan M3 only.
- You stop reading the code → the project has become something you can't maintain. Pause and fix that.

---

## 10. This week: the first ten tasks

1. Create the repo, README with §1's rules, choose the license.
2. Install Visual Studio + CMake + Git. Build a C++ hello-world.
3. Install UE5 (or Godot, if choosing early). Open an empty project. Press play.
4. Port `lzo1x_decompress` from `research/native_count.py` to C++. Test: decompress `Core.upk`; size = 234,397.
5. Port the container unwrapper and header/name/import/export reader. Test: 1,621 exports in `Core.upk`;
   11,945 functions counted in `WillowGame.upk`, same numbers as the Python tool.
6. Write the tagged-property reader. Test: dump one `WeaponPartDefinition` and check a field against BLCMM.
7. Run the census over all packages in `CookedPCConsole` and `DLC/`. Save the JSON.
8. Decode one `Texture2D` to PNG. Look at it.
9. Export one `StaticMesh` to OBJ. Open it in Blender.
10. Load both into the host engine. Screenshot it. That screenshot is Phase 0's gate and the first thing you post.
