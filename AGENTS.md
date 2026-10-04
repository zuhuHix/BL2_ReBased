# BL2_ReBased agent instructions

This is the tool-neutral project brief. Read it before starting work, then read
`CLAUDE.md` for repository-specific safety rules when that file is present.

## Strategic direction

BL2_ReBased is a Borderlands 2 engine reimplementation targeting Unreal Engine 5.
The original game must be present on the user's machine; the project does not ship
Gearbox files or Gearbox code. Since 2026-10-01 the game executable may be analysed
locally to learn what its native code does (`docs/LEGAL.md`, "Analysing the
executable"; `docs/NATIVE_ANALYSIS.md`); the repository still never contains game
data, decompiler output or code transcribed from it, and project code is written from
the understanding of what the game does.

Use mature community tools as external extraction backends whenever they can
save substantial time. Investigate UModel / UE Viewer first for supported UE3
assets. Use UPK Explorer or other community tools as secondary inspection or
conversion tools only when their actual BL2 support, distribution terms and
output quality have been checked.

External tools are used locally against the user's legitimately obtained game
installation. Never commit, distribute or package game-derived assets,
packages, textures, meshes, audio, level dumps or extracted manifests in this
repository. Generated output belongs under the ignored `local/` directory;
third-party tools and their binaries belong outside the repository.

## Required workflow

When asked to work on a new asset or game-system capability:

1. Check whether a mature community tool already supports the required
   extraction or inspection.
2. If a tool has not been tested for this project, run a small representative
   benchmark before changing the architecture.
3. Record the tool version, source or download URL, command/options, elapsed
   time, output counts, failures, unsupported types, duplicates and output
   location.
4. Treat external exports as payloads, not unquestionable truth. Verify object
   identity, package references, materials, LODs, animations, collision and map
   placement before using them in the host engine.
5. Keep our own code responsible for package identity, object-path resolution,
   cross-package references, scene manifests, provenance and reproducible
   verification.
6. Report automated extraction, visual validation and runtime/gameplay
   validation separately.
7. Do not claim that "all assets work" unless successes, failures,
   unsupported categories and duplicates have been enumerated.
8. Prefer the smallest bounded implementation slice that advances the current
   roadmap. Do not redesign the whole project unless benchmark evidence shows
   that the architecture must change.
9. Preserve the repository boundary (no game data, no decompiler output, no
   transcribed code), licenses and parser safety rules. Never loosen bounds
   checks or invent serialization offsets; recover them from the packages or
   from analysis of the executable, and label unconfirmed ones `UNVERIFIED`.

## Sensitive files

Since 2026-09-30 you may edit `src/package.cpp`, `src/container.*` and `CMakeLists.txt` without
asking first. Afterwards tell the maintainer, in your final message, which of them you touched,
what changed and what is verified versus `UNVERIFIED`. Never loosen a bounds check; keep `ctest`
and `tools/verify_packages.py` green. `THIRD_PARTY.md`, `LICENSE` and dependency/license
decisions still need the maintainer. Details in `CLAUDE.md`.

## Current priority

The current priority is a pipeline proof of concept: one end-to-end playable
slice on **Sanctuary** with one Vault Hunter (**Maya**), a few missions or one
hand-picked mission, a handful of guns, her Phaselock action skill and the
relevant skill-tree path. The slice should prove spawn, movement, fighting,
looting, equipping, using a skill, completing a mission, dying and respawning.

Use external extraction to accelerate the asset side of that slice, but treat
the Phase 0.5 benchmark in `ROADMAP.md` as a supporting gate, not as a new
breadth-first roadmap. This accelerates asset preparation; it does not by itself
implement gameplay, scripting, AI, UI, saves, networking or campaign parity. Broad map and character coverage is deferred
until the vertical-slice gate passes.

When the maintainer asks for an implementation task, state which external-tool
path is being used, what remains owned by this project and what acceptance
check will prove the slice. Then execute that bounded slice without reopening
settled architecture decisions.

## Where things stand (updated 2026-10-02)

Read this first when picking work up; it is the short version of ROADMAP.md.

- **Phase 1 (Sanctuary):** loads and walks; visual parity still open.
- **Maya prototype:** in UE5 with Infinity and Phaselock as host prototypes (not stock logic).
- **Ambient citizens (2026-10-04):** male and female Sanctuary Citizens spawn behind `-owambient`, with stock perch
  clips and node chains under stand-in movement rules (UNVERIFIED). Round 2 added heads, hair and hats copied from
  what the real game's live citizens carried, Maya's ink line and the perch clips' root motion (critic 5.5/10). Not
  done: body garment variants, `Master_NPC` zone colours, Resistance patrols. See
  `docs/verification/SANCTUARY_AMBIENT_NPCS.md`.
- **Inventory menu:** the real StatusMenu movie runs under Ruffle inside UE5 with a host adapter
  (`tools/hud_overlay/inventory.js`). Keyboard traversal follows observation of the real game.
  Since 2026-10-04 the stock sort list (ALL/TYPES/BRANDS/ITEMS/VALUE with sub-headers), the
  backpack focus layout, red `bad` cells in compare view and a full-screen Inspect exist, and
  the Skills page is preloaded hidden. Round 10 added sub-header rows, a full-width selection
  band, the narrowed compare layout and Q "Toggle Overview" on the Skills page; round 11 fixed the list's scroll
  origin and the compare and Skills layout (critic 7.3/10).
  Open: the perspective tilt and glass sheen (3D transforms Ruffle ignores), selectable empty
  backpack cells, gear compare (not yet observed in the real game), white flavour lines on cards.
  State and evidence: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (last sections).
- **Phase 2 (script VM):** Python and C++ loaders structurally decode 12,968 of 12,978
  script functions; record in `docs/verification/SCRIPT_BYTECODE_DISASM.md`. C++ object
  model, interpreter, Core natives, default-state sweep and scalar trace replay exist.
  Current evidence and next bounded state-faithful comparison:
  `docs/verification/SCRIPT_VM_PROTOTYPE.md`. Full runtime semantics remain UNVERIFIED. Finding to remember:
  the inventory sort logic is **native** code, but menu navigation/equip logic is readable script.
  First host connection: ordinary item-only backpack Up/Down executes the installed
  `InventoryListPanelGFxObject.MoveDelta`; equipment, transfers and sorting remain in
  the host adapter. Build the CMake Release libraries before building the UE module.
- **First world-object bridge:** a bounded Sanctuary Matinee door adapter loads
  placed actor/action state and runs installed mover notification scripts in the VM.
  `tools/prepare_mover.py` prepares its binding/curves; `tools/test_mover.ps1` checks
  repeated host movement/collision. Developer E activation, mission gating and audio
  are separate from stock behavior parity. Evidence:
  `docs/verification/SANCTUARY_MOVER_PROTOTYPE.md`.
- **Mission/Kismet/behavior executors (2026-10-01):** MissionTracker, BehaviorKernel and Kismet activation are native in
  this build, so `src/kismet.*`, `src/behavior.*`, `src/mission.*`, `src/slice.*` execute their installed *data*. The stock
  door opens from its installed remote events, and `tools/test_quest.ps1` plays the Fire mission end to end in the host
  on stock world data: placed Marcus whose walk is started by the installed Kismet, the stock range cylinder, the stock
  dummy and target Matinee, formula-based health, decoded respawn selection, a lent stock mission pistol whose item
  damage type goes to the dummy's check, candidate XP into the skills component, Phaselock read from the manifest
  (lift rule, valid-target rule and cast gate from script and data), progression (level, XP, skill grades) in the quest
  save, health recomputed on level change, the dummy's Transform/RegisterTargetable behaviours and holder socket, and a
  labelled turn-in loot stand-in (stock data drops no item for this mission). Recorded checks: quest suite 73/73 and
  resume 10/10, door suite 16/16, CTest 10/10, packages 9/9; inventory suite 47 PASS / 2 KNOWN_DIVERGENCE (sort order)
  on the fully seeded worktree, 45 PASS / 2 NOT_RUN / 2 KNOWN_DIVERGENCE on a regenerated one with a short backpack.
  This is host behaviour with many documented stand-ins and **no original-game parity capture yet** (needs exclusive
  screen/keyboard); open items (native auto-aim, constraint evaluation, weapon paint and decal readings, audio) are in
  `docs/verification/SANCTUARY_RPG_MISSION.md`. Hand play: `tools/run_quest.ps1 -Fresh`.
- **Native analysis and dispatch rules (2026-10-02):** Ghidra drives the local analysis of the executable
  (`tools/ghidra/`, `docs/NATIVE_ANALYSIS.md`; raw output stays under ignored `local/analysis/` or the private store).
  Three own-words note sets exist, all `UNVERIFIED`: mission/behavior/Kismet dispatch
  (`docs/verification/NATIVE_MISSION_DISPATCH.md`), progression and Phaselock targeting
  (`NATIVE_PROGRESSION.md`, `NATIVE_PHASELOCK_TARGETING.md`). The dispatch notes are implemented in
  `src/behavior.*`, `src/kismet.*` and `src/mission.*` (the host fires link ids per the note; the Fire mission's first set
  now starts from the kickoff, not from accept). After the dispatch change: quest 73/73 and resume 10/10, door 16/16, CTest
  10/10, packages 9/9, inventory suite 45 PASS / 0 FAIL / 2 NOT_RUN / 2 KNOWN_DIVERGENCE (regenerated worktree). The
  progression notes are implemented in the tools and host (mission XP 395 at stage 8, single-precision level curve,
  cap 50; quest 75/75 and resume 11/11). The Phaselock targeting/constraint rules and the stock presentation are written
  in the host but have not passed a suite: a rebuilt module DLL was blocked by Windows Application Control on
  2026-10-02 (DECISIONS 2026-10-02).
- **Weapon paint (2026-10-01/02):** the slice guns are painted from Master_Gun's recovered colour model
  (`tools/weapon_paint_model.py`, `tools/material_static_parameters.py`, our own SM3 token reader
  `research/d3d9_bytecode.py`; listings stay under `local/`). Three passes recorded in DECISIONS; the whole reading and the
  shading stand-ins are `UNVERIFIED`. First real captures (2026-10-02): an independent critic agent scored the host guns
  3-4.5/10 against them (far too dark, wrong Maliwan orange hue; `docs/verification/REALGAME_GROUND_TRUTH.md`).
  Since 2026-10-04 the guns draw the parts the running game draws, the paint is an Unlit material that undoes UE5's
  tone mapper (measured curve), the arms use per-type clip sets and the game's foreground FOV 45 is the default
  (`-owfpfov=0` opts out); critic 7.2 then 6.8/10, round 2 closer on every pair
  (`docs/verification/WEAPON_VISUALS.md`).
- **Inventory open time (2026-10-02):** first open 443 ms to 234 ms by moving one-time work to level start; repeat opens
  about 60 ms; opening in the first ~5 s of play still waits about 5.5 s for the movie player. One PC. The original game
  shows its page ~126-156 ms after the key press (screen capture, different method;
  `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`, 2026-10-02 sections).
- **Phaselock effects:** `research/particle_system.py` reads cooked `ParticleSystem` templates (layouts checked by
  oracles, nothing rendered yet); see `docs/verification/PHASELOCK_STOCK_DATA.md`. Since 2026-10-04 the effect materials
  follow own-words notes on their compiled shaders (Round 6); an independent critic scored round 11 at 6.3/10 and round 13 at 6/10. The
  Sanctuary dummy stands on the lane floor; the earlier "kneeling" was an origin-placement error (DECISIONS 2026-10-01).
- **Tests:** `ctest --test-dir build -C Release`, `python tools/verify_packages.py ...`,
  `node tests/inventory_navigation_test.js`, `python tests/script_disasm_test.py`, `python tests/weapon_paint_test.py`,
  `python tests/particle_system_test.py`, and the
  in-engine suites `tools/test_inventory_actions.ps1`, `tools/test_mover.ps1` (door) and
  `tools/test_quest.ps1` (Fire mission slice); all need a seeded worktree, see below.

### Setting up a fresh worktree

Ignored data (`host/ue5/OpenWillow/Content`, `local/`) does not come from git. See
`tools/worktree-assets.md` and `tools/seed_inventory_demo.py`, `tools/seed_inventory_assets.ps1`
(set `OPENWILLOW_BL2`, `OPENWILLOW_UMODEL`), `tools/render_weapon_previews.py`,
`tools/prepare_skill_tree.py`. The slice needs further local data: `tools/prepare_mover.py`,
`tools/prepare_slice_world.py`, `tools/prepare_action_skill.py`, `tools/seed_slice_npc_assets.ps1` and
`tools/seed_slice_player_assets.ps1` (see `docs/TOOLING.md` and the verification records they name).
Check what already exists before re-seeding. Preferred for a new worktree that continues earlier work:
`tools/provision_worktree_assets.ps1 -SourceWorktree <previous worktree>` copies Content and all of `local/`
(missing files only; see `tools/worktree-assets.md`), then build `build/` and run the suites so the new
worktree is ready to test and screenshot without redoing decodes. Two machine notes from regenerating the slice on a second PC
(2026-10-01): configure CMake with the MSVC toolset Unreal uses (`cmake -S . -B build -T version=14.50` for UE 5.8
there; a newer default toolset made the UE module fail to link against `ow-core.lib`), and
`tools/slice_npc_assets.py` needs a Python with both numpy and Pillow.

### Working rules that have paid off

- Observe the real game for behaviour before inventing it; decode its bytecode for logic
  (Phase 2 tooling) where the function is script, and observe where it is native.
- Keep guesses labelled `UNVERIFIED` in code and docs; never copy game data, item names or stats
  from a player's save into the repository.
- Only one UE editor at a time per machine: `tools/test_inventory_actions.ps1` holds
  `local/ue_run.lock`. The page loads `tools/hud_overlay/*.js` from disk at start, so do not edit
  them while a run is in progress.
