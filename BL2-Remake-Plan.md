# BL2_ReBased: vertical-slice pipeline plan

This is the working plan for the first playable proof of concept. It keeps the
community-tool acceleration path, but puts it in service of a deliberately
small end-to-end slice instead of treating full map extraction as the project
goal.

## The first proof of concept

Build one believable Borderlands 2 loop on **Sanctuary**:

- one playable Vault Hunter: **Maya**;
- movement, camera, animation and collision on the Sanctuary slice;
- a small set of real guns with loot, inventory and equip flow;
- Maya's Phaselock action skill and the relevant Motion, Harmony and Cataclysm
  skill-tree data needed to prove the skill pipeline;
- a few small mission flows, beginning with one hand-picked mission;
- enemies that can fight back;
- death and respawn.

The minimum gate is: spawn, move, fight, loot a gun, equip it, use Phaselock,
complete the hand-picked mission, die and respawn. The preferred proof of
concept expands that loop to a few missions and enough guns and skill choices
to prove the data-driven pipeline rather than a one-off scripted demo.

This is the project's first convincing answer to “can the whole pipeline
work?” Broad map coverage, the other Vault Hunters, the complete campaign,
multiplayer and DLC come after this gate.

## Division of responsibility

The project uses three cooperating paths:

1. **External extraction:** UModel / UE Viewer is the first candidate for
   quickly producing local meshes, textures, skeletal assets, animations and
   sounds where its BL2 support is proven.
2. **OpenWillow data path:** the project's reader remains authoritative for
   package identity, object paths, cross-package references, actor placement,
   scene manifests, provenance and reproducible verification.
3. **UE5 runtime:** this project owns import validation, material and collision
   decisions, the UnrealScript VM, native behavior, movement, weapons, skills,
   enemies, missions, UI, saves and the rest of the playable loop.

An external export is a payload, not proof that the object is correct or
playable. Exported meshes, materials, LODs, animations, collision, references
and placement must be checked before they become part of the Sanctuary slice.

## Work order

### 1. Prove the extraction path

- Keep UModel and any other third-party binaries outside this repository.
- Benchmark a texture, static mesh, skeletal mesh, animation, material and
  sound, then run a bounded multi-package batch.
- Record versions, source URLs, licenses, commands, elapsed time, output counts,
  failures, unsupported types, duplicates, warnings and output locations.
- Compare the export inventory to the `ow-package` census and Sanctuary scene
  manifests.
- Import representative results into UE5 and record automated, visual and
  runtime checks separately.

Do not run an unmeasured full-install dump or change the architecture just
because a tool lists a large number of objects. The benchmark gate is the
current supporting task; it does not replace the vertical-slice goal.

### 2. Make Sanctuary a usable test space

- Finish the bounded Sanctuary visual and walkable slice.
- Preserve actor transforms, sublevel ownership, package/object identity and
  unresolved-reference reporting in the OpenWillow path.
- Add only the map systems needed by the first mission and encounter.
- Verify the scene visually against the original game and verify collision in
  runtime; an automated import is not a visual or gameplay pass.

### 3. Make Maya move and animate

- Implement the player pawn, camera, movement, jump/fall and respawn anchor.
- Import the minimum Maya skeletal mesh, animations and material set required by
  the slice.
- Rebuild only the VM and UE3-native behavior that this slice exercises.

### 4. Prove guns, loot and combat

- Choose a bounded representative set of gun families and parts.
- Load weapon definitions and stats from the original data where possible;
  keep generation, inventory, equip, firing, damage, ammo and drops separate.
- Add one enemy family first, then enough encounter logic for it to fight back.
- Check numbers against data and check feel, animation and audio in the running
  game.

### 5. Prove skills and missions

- Load Maya's skill-tree data and implement a small, explainable path through
  Motion, Harmony and Cataclysm.
- Implement Phaselock end-to-end: input, cooldown, target effect, duration,
  damage/behavior consequences and UI feedback.
- Start with one hand-picked simple mission, then add the remaining small
  mission flows for the preferred proof of concept.
- Implement objectives, mission state, rewards, completion, death and respawn
  only to the extent exercised by those missions.

### 6. Close the vertical-slice gate

The gate passes only when a human can run the same reproducible checklist on a
clean local setup:

1. load Sanctuary;
2. spawn as Maya and move through the test route;
3. fight an enemy;
4. loot and equip a gun;
5. use Phaselock and a selected skill-tree effect;
6. complete the hand-picked mission, plus the additional small missions if
   they are included in the POC target;
7. die and respawn;
8. repeat the run after restarting, with automated logs and a visual/runtime
   verification record.

Only after this gate should the project spend its main effort on the remaining
maps, Vault Hunters, campaign breadth, co-op, DLC or editor features.

## What the community tools can and cannot save

Extraction can reduce the preparation of static payloads from a long custom
decoder effort to hours for a batch once the commands and failure policy are
proven. It may move useful asset preparation from years toward weeks or
months. It does not provide the UnrealScript VM, Gearbox's native behavior,
movement, AI, combat, missions, skill trees, UI, saves, networking or campaign
parity. Those remain the hard part of the POC and the full project.

## Legal and provenance boundary

- The user supplies a legitimately obtained Borderlands 2 installation.
- No game-derived package, texture, mesh, sound, map dump, extracted manifest
  or other payload is committed, distributed or packaged here.
- Generated output belongs under ignored `local/`; third-party tools belong
  outside the repository.
- No leaked or decompiled Gearbox source is used.
- Every external tool gets a version, source, license and usage record. The
  repository's MIT license covers only original project work.

The detailed tracker is [ROADMAP.md](ROADMAP.md). Tool commands and current
benchmark evidence are in [docs/TOOLING.md](docs/TOOLING.md) and
[docs/verification/EXTERNAL_TOOL_BENCHMARK.md](docs/verification/EXTERNAL_TOOL_BENCHMARK.md).
