# OpenWillow

**A clean-room engine reimplementation for Borderlands 2 that runs the game from your own installed copy.**

[![CI](https://github.com/zuhuHix/BL2_ReEngine/actions/workflows/ci.yml/badge.svg)](https://github.com/zuhuHix/BL2_ReEngine/actions/workflows/ci.yml)
![Status: pre-alpha, Phase 1 of 6](https://img.shields.io/badge/status-pre--alpha%20%C2%B7%20Phase%201%20of%206-orange)
![License: pending](https://img.shields.io/badge/license-pending-lightgrey)

> **Read this first.** There is nothing to play yet. OpenWillow currently reads
> every package in a Borderlands 2 install and loads two maps as frozen,
> approximately-textured geometry inside the Unreal Engine 5 editor. No
> gameplay, no script execution, no characters. This repository is public so
> the work is visible from the start, not because it is usable.
>
> OpenWillow is not affiliated with, endorsed by, or supported by Gearbox
> Software, 2K, or Take-Two Interactive. It ships **no game files** and
> contains **no Gearbox code**. You need your own legitimately purchased copy
> of Borderlands 2 for any of it to do anything. See [Legal](#legal).

---

## What this is

Borderlands 2 is a 2012 game on a 32-bit, Direct3D 9 build of Unreal Engine 3.
Its editor was stripped before shipping, its engine source has never been
public, and its multiplayer runs through a backend the community cannot
touch. Everything people have wanted for a decade that mods *can't* deliver —
working co-op, 64-bit, a modern renderer, new maps — is blocked by that
executable.

OpenWillow replaces the executable, not the game. It is a new engine that:

1. **reads Gearbox's own data files** (packages, textures, meshes, levels,
   UnrealScript bytecode) straight from the player's install, exactly as they
   shipped;
2. **runs the game's own gameplay code** — 64.5% of Borderlands 2's logic is
   UnrealScript bytecode inside those files, which a script VM can execute
   unchanged;
3. **rebuilds only the native C++ layer** that lived inside `Borderlands2.exe`,
   on top of a modern host engine (Unreal Engine 5).

This is the [OpenMW](https://openmw.org/) / [OpenRCT2](https://openrct2.org/) /
[Ship of Harkinian](https://www.shipofharkinian.com/) model applied to a UE3
game. It is **not** a remake (no assets are re-authored), **not** a remaster
(nothing is redistributed), and **not** a mod (it does not patch the original
executable).

The name: "Willow" is the internal codename of Gearbox's UE3 branch —
`WillowGame`, `WillowEngine.ini`, `WillowPawn`.

## What it would unlock

Every item below is on the "blocked by the executable" list in our
[feasibility research](docs/BL2_REMASTER_ANALYSIS.md#45-what-is-genuinely-blocked):

| Unlock | Why it matters |
|---|---|
| **Co-op that you control** | Broken multiplayer is the #1 complaint in Borderlands 2's negative Steam reviews (24.6% mention co-op; 14.3% call it broken outright). A new engine owns its netcode. |
| **64-bit** | Removes the ~4 GB address ceiling behind most "out of video memory" crashes and the Ultra HD texture pack problems. |
| **Modern renderer** | Native ultrawide, unlocked resolution and framerate, optional modern lighting. |
| **New maps** | No BL2 level editor was ever released. A UE5 host gets one for free the day levels load. |
| **Preservation** | The game keeps working when the backend doesn't. |
| **Mod continuity** | BLCMM text mods are `set Object Property Value` on an object graph the VM will have. A decade of mods should carry over. |
| **The Pre-Sequel and standalone Dragon Keep** | Same engine branch. |
| Split-screen, 8-player, VR, platform freedom | The daydream list — reachable once the engine exists. |

## Where we are (2026-09-13)

The project started on 2026-09-09. Progress so far, with what each result
does and does not prove:

| Milestone | Status | Evidence |
|---|---|---|
| **Phase 0 — Foundation** | ✅ Gated 2026-09-10 | Package reader reads **2,008 of 2,008** packages in a full install (base + DLC): 4,751,329 serialized exports. Nine code packages decode **byte-for-byte identical** to an independent Python reader. Tagged properties on a real weapon part match BLCMM's dump. One mesh and one texture extracted and rendered in UE 5.8. |
| **Phase 1 — World viewer** | 🔄 In progress | `Ash_P` (5,059 placements) and `Sanctuary_P` (4,430 placements, 9 sublevels) load as frozen scenes in the UE5 editor with a four-channel material approximation, an inspection lighting rig and a free-flight camera. Saved scenes reopen with zero verification errors. **Not done:** 80 more maps, sky (black), terrain/BSP, skeletal meshes, lightmaps, real material graphs, walking collision, performance. |
| Phase 2 — UnrealScript VM | ⬜ | — |
| Phase 3 — Stock UE3 natives | ⬜ | — |
| Phase 4 — Gearbox natives | ⬜ | — |
| Phase 5 — Campaign completable | ⬜ | — |
| Phase 6 — Co-op, DLC, mods, editor | ⬜ | — |

Every number above comes from a dated verification record:
[Phase 0 decisions](DECISIONS.md),
[Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md),
[Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md),
[cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md).
Automated checks are always reported separately from in-game or visual
checks, and anything we could not verify is labelled `UNVERIFIED` in the code
and docs. Screenshots of loaded maps are game-derived and stay out of the
repository; contributors with a copy of the game can reproduce them with the
commands in [docs/TOOLING.md](docs/TOOLING.md).

The live task list is in [ROADMAP.md](ROADMAP.md).

## How long will this take?

Honestly: **years.** The plan's estimates, for one person working near
full-time with an AI assistant, are ranges rather than promises — no project
of this shape has been completed AI-first before, and the estimates will be
revised as real data comes in.

| Milestone | What you can show | Estimate (full-time + AI) | Part-time |
|---|---|---|---|
| Phase 0 | A mesh and a texture from BL2 inside the host engine | 3–6 weeks — *actual: about a day* | 2–3 months |
| **M1** | Walk all 82 maps in a modern 64-bit renderer — **first public release** | 2–4 months | 6–12 months |
| M2 | Gearbox's own script running in our VM | +3–6 months | +12 months |
| M3a | A generic UE3 pawn moves and animates correctly | +6–12 months | +2 years |
| M3b | A Vault Hunter shoots real guns; enemies fight back | +12–24 months | +3–4 years |
| M4 | Campaign completable with real save files | +12–24 months | +3 years |
| M5 | Co-op, DLC, TPS, mod compatibility, editor | ongoing | ongoing |
| **Cumulative to M4** | | **~3–5 years** | **~8–10 years** |

Phase 0 finished far ahead of its estimate, but it was mostly porting an
existing Python reader. The mountain is Phase 4: **3,803 undocumented
Gearbox-specific native functions** across 443 classes, each of which has to
be reverse-engineered by observing the running game. The plan puts that at
60% of total effort and most of the calendar time. Don't extrapolate from
Phase 0.

The plan also states **kill criteria** — conditions under which we stop and
say so publicly — in [the engine plan, §9](docs/OPENWILLOW_ENGINE_PLAN.md#9-kill-criteria--be-honest-with-yourself).

## How we're building it

**The numbers it rests on.** Measured directly from the installed game:
20,119 functions across the nine code packages. 12,978 (64.5%) are
UnrealScript bytecode and will run in the VM as-is. 7,141 (35.5%) were native
C++ and must be rebuilt — of which 286 are trivial builtins, 609 are
online/save/DLC plumbing we replace rather than replicate, 512 bridge the
Scaleform UI, 1,914 are stock UE3 natives whose contracts are public, and
3,803 are Gearbox's own. Full breakdown in
[the engine plan, §0](docs/OPENWILLOW_ENGINE_PLAN.md#0-ground-truth--the-numbers-this-plan-rests-on).

**Architecture.** Three layers. The script VM and the asset pipeline are
engine-agnostic C++; the native layer is where the host engine shows up.

```
                 ┌──────────────────────────────────────────────────────┐
                 │  HOST ENGINE (UE5: renderer, physics, audio, UI)     │
                 └───────────────▲──────────────────────▲───────────────┘
                                 │                      │
   ┌─────────────────────────────┴───┐    ┌─────────────┴──────────────────┐
   │  NATIVE LAYER (C++)             │    │  ASSET PIPELINE  ◄── Phase 1   │
   │  the 7,141 rebuilt functions    │    │  package loader (UPK/TFC)  ✅  │
   │  Actor/Pawn/Controller, traces, │    │  textures, static meshes   ✅  │
   │  movement, animation, particles,│    │  materials (approximation) 🔄  │
   │  AI, stat core, weapons, UI     │    │  levels (actors+transforms)🔄  │
   └─────────────────────────────▲───┘    │  skeletal, anim, lightmaps,    │
                                 │        │  Kismet, Wwise, Bink, SWF  ⬜  │
   ┌─────────────────────────────┴───┐    └────────────────────────────────┘
   │  UNREALSCRIPT VM  ◄── Phase 2   │
   │  UObject model, bytecode        │
   │  interpreter, states, latents,  │
   │  native dispatch table          │
   └─────────────────────────────────┘
        runs the 12,978 inherited functions unchanged
```

**Clean room, strictly.** We work from file formats and observed behaviour.
No leaked source, no decompiled executable code, ever. Public reference
implementations (UE Viewer, UDK headers) are read for serialization *order*
and never copied; every reference and its license is recorded in
[THIRD_PARTY.md](THIRD_PARTY.md). The full rules are in [Legal](#legal).

**Three sources of truth.** Every rebuilt native needs a definition of
"correct": (A) **UDK**, a free running UE3, for the 1,914 stock natives;
(B) **the original game, instrumented** with [unrealsdk](https://github.com/bl-sdk),
for the 3,803 Gearbox natives — hook a function, log its inputs and outputs
during play, and implement until our engine reproduces the log; (C)
**community documentation** (BLCM wiki, bl2.parts, Lootlemon) for stat math.
A native without a golden file is a guess, and guesses are labelled.

**Evidence first.** Every change ends in a check that can be performed against
the real game, and the check is written down. Synthetic tests run in CI;
differential checks run against a real install; visual checks are done by a
human. [DECISIONS.md](DECISIONS.md) records every architectural choice, what
was verified, and what wasn't.

**AI-assisted, human-verified.** OpenWillow is developed by one person working
with an AI coding assistant ([Claude Code](https://claude.com/claude-code)).
The AI writes most of the code; the human owns every verification against the
real game, every architectural decision, and every license and provenance
call. Guard hooks in [`.claude/`](.claude/) force a confirmation before the
AI can touch bounds-checking code, dependency wiring or license files. We say
this plainly because the evidence trail is what makes it trustworthy, and the
evidence trail is public.

**Host engine.** Unreal Engine 5. UE3's material graphs, AnimTrees, Cascade,
Matinee and Kismet all have direct UE5 descendants to translate *into*, which
matters enormously for a reimplementation. The plan required this decision by
the Phase 0 gate and rules out switching later; all Phase 1 work targets
UE 5.8. Reasoning and the alternative considered (Godot) are in
[the engine plan, §2.1](docs/OPENWILLOW_ENGINE_PLAN.md#21-host-engine-decision--decide-by-end-of-phase-0-never-after).

## The plan in one screen

| Phase | Goal | Gate |
|---|---|---|
| **0 · Foundation** | C++ package loader, full-install census, property reader, one texture and one mesh in the host engine | ✅ Census exists; a real mesh and texture render in UE5 |
| **1 · World viewer (M1)** | Import every static mesh, texture and level; approximate materials; walk any map | 82/82 maps load and are walkable → **first public release** |
| **2 · UnrealScript VM (M2)** | UObject model, bytecode interpreter, states, latents, delegates; every missing native is a logged stub | Pure-script `Behavior_*` chains execute with results matching UDK |
| **3 · Stock natives (M3a)** | The 1,914 documented UE3 natives: Actor, Pawn, collision, animation, particles | A generic UE3 pawn walks, jumps and falls on a BL2 map as it does in UDK |
| **4 · Willow natives (M3b)** | The 3,803 Gearbox natives via the golden-file loop: stat core → pawn → controller → weapons → items → AI → interactives → vehicles → UI → audio | Spawn, fight, loot a gun, equip it, use a skill, die, respawn — on one map |
| **5 · Campaign (M4)** | Kismet, Matinee, missions, saves (Gibbed format), Scaleform UI, all 82 maps populated | Claptrap to the Warrior, single player, real save file |
| **6 · Beyond (M5)** | Own co-op netcode, DLC, The Pre-Sequel, mod compatibility, editor, modern lighting | ongoing |

Full step lists, dependency order and verification method per phase:
[ROADMAP.md](ROADMAP.md) (tracker) and
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md) (rationale).

## Try it

You need: Windows, CMake, Visual Studio 2022 C++ build tools, Python 3, and
an installed Borderlands 2. Unreal Engine 5.8 is only needed for the map
viewer.

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure          # synthetic, no game needed
python tools/verify_packages.py --reader build/Release/ow-package.exe   # needs the game
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

Then, to inspect properties, run the full-install census, extract a texture or
mesh, or prepare and open a map in the UE5 viewer, see
[docs/TOOLING.md](docs/TOOLING.md). Everything the tools produce lands under
`local/`, which is git-ignored — extracted assets never enter the tree.

## How to help

You don't need to write C++ to move this project. The most valuable
contributions right now are, in order:

1. **Verification with your own copy.** Run the census and the differential
   check on your install (different DLC sets, Epic vs Steam, with/without the
   UHD pack) and report the numbers. Load a map, compare a viewpoint against
   the real game, and report what's wrong. Use the *Verification report* issue
   template.
2. **Format findings.** If you know something about version 832/46
   serialization — a property offset, a struct layout, a bulk-data quirk —
   file a *Format finding* with how you observed it and where it came from.
   Provenance matters as much as the finding.
3. **Code.** Current open work is listed in [ROADMAP.md](ROADMAP.md#now--next),
   including small, well-bounded items (e.g. `PF_A8R8G8B8` texture decoding,
   which is what's blocking the sky).

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request: it
covers the clean-room certification every contributor makes, the sensitive
areas of the code, and the check-and-report format. Note that until the
project license is chosen, external *code* contributions cannot be merged;
findings and reports are welcome now.

## FAQ

**Can I play Borderlands 2 in this?** No. See [Where we are](#where-we-are-2026-09-13).
The first thing you'll be able to do is walk through maps (M1). Shooting is
M3b, the full campaign is M4.

**Is this legal?** We believe so, on the same basis as OpenMW, OpenRCT2,
Daggerfall Unity and Ship of Harkinian: the engine is original code, reads
files from a copy you bought, and redistributes none of them. We do not use
leaked source or decompiled code, we will never sell anything, and we require
the original game. Details in [Legal](#legal).

**Why not just mod the game?** We did the research first:
[docs/BL2_REMASTER_ANALYSIS.md](docs/BL2_REMASTER_ANALYSIS.md). Mods can
deliver roughly 80% of what players ask for — and there is a full
[overhaul-mod design](docs/DESIGN_OVERHAUL_MOD.md) in this repo as the
fallback plan. But the two things people want most, working co-op and a
modern engine, live inside the executable and are unreachable by modding.

**Why UE5 and not a custom engine or Godot?** See
[How we're building it](#how-were-building-it). Short version: UE3's systems
have direct UE5 descendants, and translating into them is a much smaller
problem than inventing replacements.

**Will my mods and saves work?** That's the intent. Text mods edit the same
object graph the VM will hold, so they should work from the day the VM runs
gameplay (M4). Save files use the open Gibbed format and reading them is a
Phase 5 task. SDK mods will need a compatibility layer, later.

**Isn't "AI-written engine" a red flag?** It's a fair concern, which is why
every claim here comes with the check that supports it, why the synthetic
tests run publicly in CI, why nothing merges without a human running it
against the real game, and why the human reads the code. Judge the evidence
trail, not the tool.

**Why "Phase 0 in a day" but "years" overall?** Because Phase 0 was porting
a working Python reader to C++ — the thing AI is best at. Phase 4 is
reverse-engineering 3,803 undocumented functions from a running game, one
golden file at a time. Different work entirely.

## Legal

OpenWillow is an independent, non-commercial, fan-made project. Borderlands,
Borderlands 2, Gearbox and related marks are trademarks of their respective
owners. This project is not affiliated with, endorsed by, sponsored by, or
supported by Gearbox Software, 2K Games, or Take-Two Interactive.

The non-negotiable rules every contributor and every AI assistant works under:

1. **Never distribute game files.** No packages, textures, sounds, decompressed
   dumps, or extracted assets — not in the repository, not in releases, not in
   issues. Test fixtures are synthetic. A hook refuses to write `.upk`, `.tfc`,
   `.pck`, `.bik`, `.umap`, `.gfx` and `.swf` files into the tree.
2. **Never use leaked source or decompiled code.** Not Unreal Engine 3's, not
   Gearbox's. Clean room only: file formats, public documentation, and the
   observed behaviour of a running game.
3. **Record provenance.** Every reference implementation consulted and every
   dependency is listed with its license in [THIRD_PARTY.md](THIRD_PARTY.md).
   Referencing a *format* is always fine; copying *code* depends on the license
   and is a human decision.
4. **The original game is required.** The engine refuses to start without an
   installed copy.
5. **No money.** No paid builds, no premium features, no monetization. Any
   donations, if ever accepted, go to engine development.
6. **Disclose AI assistance and keep verification claims honest.**

**License: not yet selected.** Until a project-wide license is granted, all
rights are reserved by the author; the code is public for transparency, not
yet for reuse. The candidates are MIT and GPL-3, and the choice is gated on a
provenance review recorded in [THIRD_PARTY.md](THIRD_PARTY.md) and
[DECISIONS.md](DECISIONS.md). Vendored third-party code keeps its own license
(lzokay: MIT).

The full policy, including the contributor certification and the takedown
contact, is in [docs/LEGAL.md](docs/LEGAL.md).

## Documents

| | |
|---|---|
| [ROADMAP.md](ROADMAP.md) | Phase-by-phase tracker: what's done, what's next, what's blocked |
| [DECISIONS.md](DECISIONS.md) | Dated log of every architectural and parsing decision and its evidence |
| [docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md) | The plan: numbers, architecture, sources of truth, phases, estimates, kill criteria |
| [docs/BL2_REMASTER_ANALYSIS.md](docs/BL2_REMASTER_ANALYSIS.md) | Research: what players actually want, what modding can and cannot reach |
| [docs/DESIGN_OVERHAUL_MOD.md](docs/DESIGN_OVERHAUL_MOD.md) | The fallback: an in-engine overhaul mod design, if the engine route fails |
| [docs/TOOLING.md](docs/TOOLING.md) | Every tool, flag and command, with what each check proves |
| [docs/verification/](docs/verification/) | Dated verification records for each shipped slice |
| [docs/LEGAL.md](docs/LEGAL.md) | Clean-room policy, non-affiliation, contributor certification |
| [THIRD_PARTY.md](THIRD_PARTY.md) | Dependency and reference provenance |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute, and the sensitive areas |
| [research/](research/README.md) | The community-demand corpus and the original Python package reader |
