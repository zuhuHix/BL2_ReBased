# Delegated tasks: in-game map selector, performance profiling, docs sync

Prepared 2026-09-14 for a separate worktree/session. None of these touch a
sensitive area (`src/package.cpp`, `src/container.*`, struct/array property
decoding, texture/mesh serialization, `CMakeLists.txt`, `THIRD_PARTY.md`,
`LICENSE`). No format guessing, no new cooked-resource bytes, no offsets. Read
`CLAUDE.md` and the "Is this legal?" sections of `README.md` / `docs/LEGAL.md`
before starting regardless.

The cooked-material-resource work (remaining 13 opaque Sanctuary materials,
Ash/Southpaw re-measurement) is intentionally **not** in this doc — that one
needs explicit approval per commit before extending the unspecified Material
layout, and is being handled separately on `fix/missing-materials`.

---

## 1. In-game map selector

**Why:** ROADMAP.md's Phase 1 checklist has `[ ] In-game map selector` open.
Today, map choice is entirely command-line (`tools/viewer.py --map <name>
--action view`), which calls `tools/run_ue_level.ps1` → launches
`UnrealEditor.exe` with `-ViewOnly` against a pre-imported scene. Only 3 maps
are currently importable/walkable (`Ash_P`, `Sanctuary_P`,
`SouthpawFactory_P`), so the in-game selector only needs to offer whichever
scenes are already prepared under `local/<map>/scene.json` — it is a UI
convenience layer, not a new loading pipeline.

**Relevant existing code:**
- `tools/viewer.py` — `catalog()` / `select()` show how map names are
  discovered and validated (`local/<name-lower>/scene.json` must exist and
  its `map` field must match).
- `host/ue5/OpenWillow/Source/OpenWillow/OpenWillowGameMode.cpp/.h` — game
  mode, the natural place to add a selector widget/level-open call.
- `host/ue5/OpenWillow/Source/OpenWillow/OpenWillowWalker.cpp/.h` — the
  spectator/walker pawn; a selector would coexist with this, not replace it.
- `host/ue5/import_level.py` — invoked by `run_ue_level.ps1`; shows how a
  scene manifest maps to a `.umap`.

**Suggested approach (pick the simplest that works, this is UI plumbing):**
1. At startup (or via a bound key, e.g. Tab), show a simple UMG widget or
   even a debug on-screen list (`GEngine->AddOnScreenDebugMessage` is fine
   for a first pass) enumerating scenes with a `scene.json` present under
   `local/`.
2. Selecting one calls `UGameplayStatics::OpenLevel` with the level path
   already used by `run_ue_level.ps1` (`/Game/OpenWillow/<Map>/<Map>`) —
   only for maps whose `.umap` already exists under
   `Content/OpenWillow/<Map>/`. Do not attempt to trigger `prepare`/`import`
   from inside the running editor; those stay CLI-only for now.
3. If a `.umap` isn't present for a listed scene, gray it out / show "not
   imported" rather than erroring.
4. Keep it minimal — this does not need to be pretty, just functional and
   reversible (a widget you can delete without touching the importer).

**Definition of done:** launching the game (`-ViewOnly`, no `-Scene` forced)
lets you pick among the 3 working maps in-editor and it opens the right one.
Update ROADMAP.md's checkbox and the Phase 1 "Map coverage" bullet. No
DECISIONS.md entry needed (no parsing-behavior change) — a short ROADMAP note
is enough.

---

## 2. Performance profiling (Sanctuary ~8-9 FPS)

**Why:** ROADMAP.md flags "Sanctuary ran at ~8-9 FPS during automation
startup on the development machine. Not yet profiled." This is a
measurement task, not a format/reasoning task.

**How:**
1. Prepare and import Sanctuary if not already present locally:
   `python tools/viewer.py --game <BL2 path> --map Sanctuary_P --action prepare`
   then `--action import --engine <UE5 path>`.
2. Launch with `--action view --engine <UE5 path>` (use `-LowEnd` via
   `run_ue_level.ps1` directly if profiling the low-end path too — see
   `tools/run_ue_level.ps1`'s `-LowEnd` switch).
3. Use UE5's built-in tools rather than guessing:
   - `stat fps`, `stat unit` (CPU/GPU/draw/game/render thread split) from
     the in-game console.
   - `stat scenerendertargets`, `stat gpu` for GPU-side breakdown.
   - Unreal Insights (`-trace=cpu,gpu,frame` launch arg, or
     `UnrealInsights.exe`) for a proper capture if `stat unit` doesn't make
     the bottleneck obvious.
4. Capture both the default and `-LowEnd` runtime paths since they take
   different rendering routes (Nanite/VSM vs DX11 SM5).
5. Write up findings: which stage dominates (draw calls, Nanite, shadows,
   material count, etc.), a number for both paths, and the hardware used.
   This is diagnostic only — do not change renderer settings/importer
   behavior based on a guess; if a concrete fix is obvious (e.g. an
   accidentally unbounded draw distance), note it separately for review
   rather than changing it in the same pass.

**Definition of done:** a short section in
`docs/verification/PHASE1_VIEWER_VERIFICATION.md` (or a new
`docs/verification/PERFORMANCE.md` if that file is getting crowded) with
FPS numbers, `stat unit` breakdown, hardware spec, and both render paths.
Update the ROADMAP.md bullet from "Not yet profiled" to reference the
record.

---

## 5. Docs sync (README / ROADMAP / docs/TOOLING.md vs DECISIONS.md and verification records)

**Why:** CLAUDE.md explicitly lists this as "safe to work on without
ceremony" and asks to keep these in agreement. Docs drift after each fast
round of commits (materials, DLC scoping, low-end switch were all recent).

**How:**
1. Read `DECISIONS.md` top-to-bottom (newest entries especially — the
   2026-09-14 DLC and material-inference entries) and the files under
   `docs/verification/`.
2. Cross-check every claim in `README.md`, `ROADMAP.md`, and
   `docs/TOOLING.md` against those sources:
   - Map coverage counts (currently 3/82 importable; 82 discoverable names).
   - `--include-dlc` / DLC scoping behavior description.
   - Material inference rule names/counts (`sole_cooked_resource_texture`,
     `constant_diffuse`, current no-diffuse material counts for Sanctuary).
   - `-LowEnd` switch existence and what it actually changes (runtime only).
   - Any tool flag mentioned in docs that no longer matches actual
     `argparse`/`param()` definitions in `tools/*.py` and `tools/*.ps1`.
3. Fix mismatches. Prefer trimming stale claims over adding new ones — this
   task is reconciliation, not new writing.
4. Keep the existing hedged, evidence-first tone (see current README/
   ROADMAP phrasing: "Caveat:", "UNVERIFIED", counts instead of "works").

**Definition of done:** README.md, ROADMAP.md, docs/TOOLING.md make no claim
that DECISIONS.md or docs/verification/*.md doesn't support, and no
recent (post-2026-09-10) decision is missing from those three files if it's
user-facing.

---

## Every change (all three tasks)

Per CLAUDE.md, before calling any of these done:
- Run `ctest --test-dir build -C Release --output-on-failure` and
  `python tools/verify_packages.py --reader build/Release/ow-package.exe`;
  paste the output.
- Report automated checks separately from in-game checks.
- Task 1 (in-game selector) needs an actual in-editor check (open the
  editor, use the widget) since it's UI — say explicitly if that wasn't
  possible in the delegated environment.
