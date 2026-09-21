# BL2_ReBased agent instructions

This is the tool-neutral project brief. Read it before starting work, then read
`CLAUDE.md` for repository-specific safety rules when that file is present.

## Strategic direction

BL2_ReBased is a clean-room Borderlands 2 engine reimplementation targeting
Unreal Engine 5. The original game must be present on the user's machine; the
project does not ship Gearbox files or Gearbox code.

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
9. Preserve clean-room boundaries, licenses and parser safety rules. Never
   loosen bounds checks or invent serialization offsets.

## Current priority

The current priority is a pipeline proof of concept: one end-to-end playable
slice on **Sanctuary** with one Vault Hunter (**Maya**), a few missions or one
hand-picked mission, a handful of guns, her Phaselock action skill and the
relevant skill-tree path. The slice should prove spawn, movement, fighting,
looting, equipping, using a skill, completing a mission, dying and respawning.

Use external extraction to accelerate the asset side of that slice, but treat
the Phase 0.5 benchmark in `ROADMAP.md` as a supporting gate, not as a new
breadth-first roadmap. This may shorten asset preparation from months to weeks
or months; it does not by itself implement gameplay, scripting, AI, UI, saves,
networking or campaign parity. Broad map and character coverage is deferred
until the vertical-slice gate passes.

When the maintainer asks for an implementation task, state which external-tool
path is being used, what remains owned by this project and what acceptance
check will prove the slice. Then execute that bounded slice without reopening
settled architecture decisions.
