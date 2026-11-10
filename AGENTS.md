# BL2_ReBased agent brief

The working brief for AI assistants on this repository, whatever the tool. `CLAUDE.md` imports this file and adds only
what is specific to Claude Code.

**Sub-agents and short tasks:** read [docs/LANE_BRIEF.md](docs/LANE_BRIEF.md) (the binding rules on one page) and
[docs/verification/GATES.md](docs/verification/GATES.md) (gate commands and the current baseline), then open longer
documents only for the part your task touches.

BL2_ReBased (code prefix `OpenWillow` / `ow`) is a Borderlands 2 engine reimplementation on Unreal Engine 5. The
original game must be installed on the user's machine: the project ships no Gearbox files and no Gearbox code, and
project code is written from an understanding of what the game does. The project's own code and documents are
MIT-licensed; that covers no third-party component and no game content. The maintainer, zuhu, is a solo, largely
self-taught developer directing an AI-heavy build.

## Rules that do not bend

This is the short form. `docs/LEGAL.md` and the "Is this legal?" section of `README.md` are the full text and win
where they say more. Read them before any work on native analysis, licences, provenance, reuse of third-party code,
releases or anything published, money, or anything that moves game-derived data.

1. **No game-derived payload in the repository, and none distributed or packaged by the project.** Not in a tracked
   file, a commit message, a PR, an issue or a verification record: no game files, assets, package or level dumps,
   extracted manifests or script listings, and no game data, item names or stats copied from a player's save.
   Generated output goes under ignored `local/`. Third-party tools and their binaries live outside the repository.
   What docs may hold: own-words behaviour notes, names of game objects, counts, and the short constants and formulas
   needed to describe behaviour, as `docs/verification/` already does.
2. **No leaked source. No decompiler or disassembler output, and no code pasted, translated or converted from it.**
   Analysing the executable locally is allowed; what may leave `local/` is in "Analysing the executable" below.
3. **No code from GPL tools** (Legendary Explorer, UE Explorer, UPKUtils and the like). A public reference such as
   UModel / UE Viewer may be run as an external binary or read to learn a format, once its source and licence are in
   `THIRD_PARTY.md`. Copying or translating its code, MIT-licensed or not, is the maintainer's licence decision and
   needs a provenance entry first.
4. **No invented offsets.** The version 832/46 layout has no public spec, which covers struct and array property
   decoding, `Texture2D` and `StaticMesh` serialization and UnrealScript bytecode. Recover offsets from the packages or
   from analysis of the executable, check them with structural oracles (sizes meet, jumps land on statements, totals
   match the census), and label what is fitted and not proven `UNVERIFIED`.
5. **No loosened bounds checks.** If a check fails, fix the cause.
6. **Maintainer decisions stay with the maintainer:** `THIRD_PARTY.md`, `LICENSE`, and any new tool, dependency,
   licence or provenance change. The host-engine choice (UE5, `docs/OPENWILLOW_ENGINE_PLAN.md` section 2.1) is made;
   do not reopen it.
7. **Honest verification.** A rule that has not been confirmed in the running game stays `UNVERIFIED` in code and
   docs, and a confirmed one says how it was confirmed. A passing host suite means "the host did this", not parity.
   Count agreement is not compatibility. Never report a result you did not see.
8. **Provenance and disclosure.** Record the provenance and licence of anything brought in, and disclose AI assistance
   (records and `DECISIONS.md` entries open with "AI-assisted (<tool>)").
9. **Synthetic fixtures only.** Tests use invented values, built with the existing `package()` / `tag()` helpers in
   `tests/`.
10. **Process safety.** Several lanes share each machine. Never kill a process by image name (`taskkill /IM`,
    `Stop-Process -Name`, `pkill`, `killall`); stop only a PID your own run started, after checking its command line.
    Never delete a lock you do not hold. Put this rule in every worker and sub-agent brief.
11. **Nothing is sold, and the original game stays required.** No paid builds, premium features or early-access
    tiers. The engine reads the player's own installed game and is never made to work without it.

## Priority: prove the vertical slice first

Porting all 82 maps is the easier part of this project. The hard, unproven part is Phases 2-4: the script VM, the
stock UE3 natives and Gearbox's undocumented natives. Porting 80 maps only to find in Phase 2-4 that the approach does
not hold would waste most of the project's time, so the order (adopted 2026-09-18, see `DECISIONS.md`) is:

1. Finish **Sanctuary** to visual and walkable parity (Phase 1). It is the most complete map; do not start over on
   another.
2. Get **Maya** working end to end: movement, a few weapons, animations, her action skill (Phaselock). Her skill trees
   (Motion, Harmony, Cataclysm) are the target for Phase 4's skill-tree work.
3. Prove engine and gameplay integration (Phase 2's VM, Phase 3's stock natives) only as far as Sanctuary and Maya
   need. Not the full native surface, not every `AnimNode` type.
4. One hand-picked simple mission and a handful of guns, end to end: spawn, fight, loot, equip, use a skill, complete
   the mission, die, respawn. That is Phase 4's gate in `ROADMAP.md`.

Broad map coverage (about 79 more maps) and the other Vault Hunters come after that gate (Phases 5 and 6). If you are
asked to port another map or add another character before the gate is met, say so and confirm with the user first.
It is a question of order, not a technical blocker.

External extraction speeds up the asset side of the slice. It does not by itself implement gameplay, scripting, AI,
UI, saves, networking or campaign parity, and the Phase 0.5 benchmark in `ROADMAP.md` is a supporting gate, not a new
breadth-first roadmap.

For an implementation task, say up front which external-tool path you are using, what stays owned by this project and
which acceptance check will prove the result. Then do that bounded slice. Prefer the smallest slice that advances the
roadmap, and do not redesign the project unless benchmark evidence shows the architecture has to change.

## Finding out what the game does

Observe before inventing, in this order:

1. **The running game.** If a value, a native's inputs and outputs, a state change or an event order can be seen in
   the running game, get it there with bl-sdk live reads and hooks (the RTSE way): `tools/real_game/`, method in
   `docs/verification/REALGAME_GROUND_TRUTH.md`. A real-game session takes over the maintainer's screen, keyboard and
   saves, so run one only when the maintainer or your brief explicitly asks for a session; a feature request is not
   that. Without one, work from the evidence already recorded and label what stays unconfirmed `UNVERIFIED`. In a
   session, back up the saves first and leave saves, config and SDK settings byte-identical afterwards.
2. **The script.** Logic that is UnrealScript is read with `research/script_disasm.py`
   (`docs/verification/SCRIPT_BYTECODE_DISASM.md`). Menu navigation and equip logic are script; the backpack sort is
   native.
3. **The native code**, for what is neither observable nor script. Rules in the next section.

`docs/TOOLING.md`, "Main tools at a glance", lists the go-to tools: bl-sdk for anything the running game shows, REA
(the `rea` MCP server) for decoding, then UModel, `ow-package`, Ruffle and the rest. Check there before writing a
one-off decoder, and put the bl-sdk and REA line in every worker and sub-agent brief.

## Analysing the executable (allowed since 2026-10-01)

Disassembling and decompiling `Borderlands2.exe` locally is allowed, because it is the fastest way to settle native
behaviour (mission tracker, behavior kernel, auto-aim, constraint evaluators, experience and loot rules, Gearbox's
natives). Policy: `docs/LEGAL.md`, "Analysing the executable". Practice: `docs/NATIVE_ANALYSIS.md`.

- Raw output (listings, pseudo-code, recovered headers, analysis databases) stays under ignored `local/analysis/` or
  the maintainer's private store. It never goes into a tracked file, a commit message, a PR description, an issue or a
  `docs/verification/` record.
- Write what a native function does in your own words first, then implement from that note. Constants and formulas
  needed for correct behaviour may be recorded; the structure of a listing may not.
- The tool is Ghidra 12.1.4 with a portable Temurin JDK 21 (`THIRD_PARTY.md`). Both live outside the repository, and
  `tools/ghidra_import.ps1` drives them headlessly. Any other decompiler or disassembler is a new tool: it needs a
  `THIRD_PARTY.md` entry and a home outside the repository. Do not download or install one unasked.
- `tools/private_sync.ps1` moves game-derived files that cannot be regenerated between the maintainer's machines.
  Never commit them, and never point the store inside the repository.

## External tools and extraction

- Use mature community tools as extraction backends when they save real time, locally and only against the user's
  own legitimately obtained installation. **UModel / UE Viewer comes first** for
  UE3 meshes, textures, animations and sounds. The verified local candidate is build 1590 from the official
  `gildor2/UEViewer` repository; its path differs per machine, so pass `-UModel` or an environment variable. UPK
  Explorer and other tools are secondary, and only once their BL2 support, distribution terms and output quality have
  been checked.
- Before relying on a tool this project has not tested, run a small, timeboxed, representative benchmark, before
  changing any architecture. Record version, source or URL, licence, command line, elapsed time, successes, failures,
  unsupported types, duplicates, output size and output location. Adding the tool is still a maintainer decision
  (rule 6).
- Exports are payloads, not truth. Check object identity, package references, materials, LODs, animations, collision
  and map placement before using them in the host. UModel material files are heuristic and do not prove UE5
  material-graph compatibility.
- Package identity, object-path resolution, cross-package references, scene manifests, provenance and reproducible
  verification stay in our own code.
- Never assume a batch exported every object without an inventory, and do not claim "all assets work" unless
  successes, failures, unsupported categories and duplicates are enumerated.

## Sensitive files: edit when needed, then warn

Since 2026-09-30 these need no confirmation: `src/package.cpp` (the `Reader` struct, `require()`, `limit`, size and
terminator checks), `src/container.cpp` / `src/container.hpp` (LZO container validation) and `CMakeLists.txt`. The
C++ port of the script VM may touch the `Reader` and `CMakeLists.txt` on the same terms. What you owe afterwards:

- **Warn the maintainer in your final message:** name each of these files you touched, what changed and why, and what
  is verified versus `UNVERIFIED`.
- Keep `ctest` and `tools/verify_packages.py` green, and add a `DECISIONS.md` entry when parsing behaviour changes.

Rules 4 to 6 apply here more than anywhere else.

## Working with the maintainer

zuhu has to be able to explain what gets merged, so:

- Keep diffs small and explainable. When in doubt, take the simpler, more obviously correct implementation over the
  clever one. AI can produce a lot of technical debt very quickly.
- zuhu's own norm is to read an error and check the docs before handing it to an assistant. Support that: when
  something fails, explain the cause and how you found it, not only the fix.
- Match the hedged, evidence-first tone of the existing docs.

None of this relaxes a rule above, and nothing counts as verified until it has been checked against the real game.

## Every change

- Run `ctest --test-dir build -C Release --output-on-failure` and
  `python tools/verify_packages.py --reader build/Release/ow-package.exe`, and paste the output. The gates for the
  part you touched, the build commands and the current baseline are in `docs/verification/GATES.md`. Not every Python
  or JS test is in ctest, so also run the ones for the part you touched (`python tests/<name>_test.py`,
  `node tests/inventory_navigation_test.js`).
- Report automated extraction and tests, visual validation, and runtime or gameplay validation (host runs, and
  real-game checks where there are any) separately, with pass and fail counts as they came out, including steps that
  fail only because local data is missing.
- Menu work: re-observe the real game (`docs/TOOLING.md`, "Tracing the real game's UI code") instead of guessing, and
  rerun `tools/test_inventory_actions.ps1` after any change to `tools/hud_overlay/inventory.js` or the page report in
  `OpenWillowMayaHUD.cpp`.
- Unreal: build the CMake Release libraries before the UE module. Only one editor or game runs per machine; the suite
  scripts take the machine-wide lock themselves (`tools/ue_lock.ps1`). The overlay page loads `tools/hud_overlay/*.js`
  from disk at start, so do not edit those files while a run is in progress.
- Add a `DECISIONS.md` entry when parsing behaviour changes, saying what is verified and what is not. Keep `README.md`,
  `ROADMAP.md` and `docs/TOOLING.md` in agreement with `DECISIONS.md` and the verification records; a `ROADMAP.md`
  change lands only with the maintainer's OK.

## Where things are

This file carries no status on purpose: the copy that used to live here went stale within weeks, and assistants
believed it. Look status up where it is maintained.

| Need | Where |
|---|---|
| What works, what is next | `ROADMAP.md`, "How it's going" and "Now / next" |
| Current test numbers | `docs/verification/GATES.md` |
| Evidence and open items for one area | its record in `docs/verification/` (`HUD_*`, `COMBAT_*`, `WEAPON_*`, `SANCTUARY_*`, `SLICE_*`, and the own-words `NATIVE_*` notes); the slice gate is `docs/verification/SLICE_GATE.md` |
| A past decision | `python tools/decisions_index.py --tail N` or `--grep WORD` (do not open `DECISIONS.md` whole) |
| A native function | `python tools/native_index_find.py <name>` |
| Tools, build, data preparation | `docs/TOOLING.md` |
| Landing PRs, and the standing approval with its exceptions | `docs/TOOLING.md`, "Landing PRs in a batch" |

**A fresh worktree** gets no ignored data (`host/ue5/OpenWillow/Content`, `local/`) from git. Where the shared
post-checkout provisioner is installed, the default folders are already seeded, so look before you fill anything. To
continue earlier work, `tools/provision_worktree_assets.ps1 -SourceWorktree <previous worktree>` copies the missing
files. Then configure and build `build/` and run the checks above, and regenerate only what is missing or older than
the code that produces it: `tools/worktree-assets.md`, and `docs/TOOLING.md`, "What a new worktree gets" and
"Regenerating local data on a fresh machine".
