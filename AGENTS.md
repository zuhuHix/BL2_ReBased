# BL2_ReBased agent brief

The brief for AI assistants on this repository, whatever the tool (`CLAUDE.md` imports it).

**Lanes, sub-agents and short tasks:** also read [docs/LANE_BRIEF.md](docs/LANE_BRIEF.md) (lane procedures and the
clean room) and [docs/verification/GATES.md](docs/verification/GATES.md) (gate commands and the current baseline, at
its top), then open longer documents only for the part your task touches.

BL2_ReBased (code prefix `OpenWillow` / `ow`) is a Borderlands 2 engine reimplementation in Rust: the C++ core in
`src/` is being ported crate by crate (`crates/`), and the host is our own engine on wgpu + winit (`crates/ow-host`).
The original game must be installed on the user's machine. The project ships no Gearbox files and no Gearbox code;
its code is written from an understanding of what the game does. That code and the project's documents are
MIT-licensed, which covers no third-party component and no game content. The maintainer, zuhu, is a solo, largely
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
   licence or provenance change. The language is chosen: runtime code is Rust, which replaced the UE5 host
   (`docs/decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md`); do not reopen it.
   The engine route is picked too: an own engine on wgpu + winit, the maintainer's pick from the Phase 0R benchmark
   (`docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md`); do not reopen it, and
   add no engine crate to the tree before its `THIRD_PARTY.md` entry.
7. **Honest verification.** A rule that has not been confirmed in the running game stays `UNVERIFIED` in code and
   docs, and a confirmed one says how it was confirmed. A passing host suite means "the host did this", not parity.
   Count agreement is not compatibility. Never report a result you did not see.
8. **Provenance and disclosure.** Record the provenance and licence of anything brought in, and disclose AI assistance
   (records and decision files open with "AI-assisted (<tool>)").
9. **Synthetic fixtures only.** Tests use invented values, built with the existing `package()` / `tag()` helpers in
   `tests/`.
10. **Process safety.** Several lanes share each machine. Never kill a process by image name (`taskkill /IM`,
    `Stop-Process -Name`, `pkill`, `killall`); stop only a PID your own run started, after checking its command line.
    Never delete a lock you do not hold. Put this rule in every worker and sub-agent brief.
11. **Nothing is sold, and the original game stays required.** No paid builds, premium features or early-access
    tiers. The engine reads the player's own installed game and is never made to work without it.

## Priority: prove the vertical slice first

The hard, unproven part is Phases 2-4 (the script VM, the stock UE3 natives, Gearbox's undocumented natives), not
the 82 maps: porting 80 maps and then finding that the approach does not hold would waste most of the project's
time. So the order (adopted 2026-09-18, kept for the Rust switch on 2026-11-14) is:

1. The Rust core, until each crate matches `src/`: the same synthetic fixtures give the same results, and
   `tools/verify_packages.py --reader` the same counts and class bodies. Until then the C++ reader is the reference.
2. **Sanctuary** at visual and walkable parity in the Rust host (Phase 1). It is the most complete map; do not start
   over on another.
3. **Maya** end to end: movement, a few weapons, animations, her action skill (Phaselock). Her skill trees (Motion,
   Harmony, Cataclysm) are the target of Phase 4's skill-tree work.
4. Engine and gameplay integration (Phase 2's VM, Phase 3's stock natives) only as far as Sanctuary and Maya need:
   not the full native surface, not every `AnimNode` type.
5. One hand-picked simple mission and a handful of guns, end to end: spawn, fight, loot, equip, use a skill, complete
   the mission, die, respawn. That is Phase 4's gate in `ROADMAP.md`.

The other maps (about 79) and the other Vault Hunters come after that gate (Phases 5 and 6). Asked to port another
map or add another character before it is met, say so and confirm with the user first: it is a question of order,
not a technical blocker.

External extraction speeds up the asset side of the slice. It does not by itself implement gameplay, scripting, AI,
UI, saves, networking or campaign parity, and the Phase 0.5 benchmark in `ROADMAP.md` is a supporting gate, not a new
breadth-first roadmap. For an implementation task, say up front which external-tool path you are using, what stays
owned by this project and which acceptance check will prove the result, then do that bounded slice: the smallest that
advances the roadmap. Do not redesign the project unless benchmark evidence shows the architecture has to change.

## Finding out what the game does

Observe before inventing, in this order. The tools line goes in every worker and sub-agent brief: bl-sdk live reads
first, REA (the `rea` MCP server) for decoding.

1. **The running game**, for any value, native input or output, state change or event order it can show: bl-sdk live
   reads and hooks, the RTSE way (`tools/real_game/`; method in `docs/verification/REALGAME_GROUND_TRUTH.md`). A
   real-game session takes over the maintainer's screen, keyboard and saves, so run one only when the maintainer or
   your brief explicitly asks for a session; a feature request is not that. Without one, work from the evidence
   already recorded and label what stays unconfirmed `UNVERIFIED`. In a session, back up the saves first and leave
   saves, config and SDK settings byte-identical afterwards (restore from the backup what the game rewrote, and say
   so).
2. **The script**, for logic that is UnrealScript: `research/script_disasm.py`
   (`docs/verification/SCRIPT_BYTECODE_DISASM.md`). Menu navigation and equip logic are script; the backpack sort is
   native.
3. **The native code**, for what is neither observable nor script (next section).

Before writing a one-off decoder, check `docs/TOOLING.md`, "Main tools at a glance": bl-sdk, REA, UModel,
`ow-package`, Ruffle and the rest.

## Analysing the executable (allowed since 2026-10-01)

Disassembling and decompiling `Borderlands2.exe` locally is allowed, as the fastest way to settle native behaviour.
Policy: `docs/LEGAL.md`, "Analysing the executable". Practice: `docs/NATIVE_ANALYSIS.md`.

- Raw output (listings, pseudo-code, recovered headers, analysis databases) stays under ignored `local/analysis/` or
  the maintainer's private store. It never goes into a tracked file, a commit message, a PR description, an issue, a
  `docs/verification/` record or anything public (a gist, a chat).
- Write what a native function does in your own words first, then implement from that note. Constants and formulas
  needed for correct behaviour may be recorded; the structure of a listing may not.
- The tool is Ghidra 12.1.4 with a portable Temurin JDK 21 (`THIRD_PARTY.md`), both outside the repository and
  driven headlessly by `tools/ghidra_import.ps1`. Any other decompiler or disassembler is a new tool: it needs a
  `THIRD_PARTY.md` entry and a home outside the repository. Do not download or install one unasked.
- `tools/private_sync.ps1` moves game-derived files that cannot be regenerated between the maintainer's machines.
  Never commit them, and never point the store inside the repository.

## External tools and extraction

`docs/TOOLING.md`, "External tools", holds the rules in full; read it before extracting assets or relying on a tool.
In short:

- **UModel / UE Viewer comes first** for UE3 meshes, textures, animations and sounds. Any extraction backend runs
  locally and only against the user's own legitimately obtained installation.
- A tool this project has not tested gets a small, recorded benchmark before anything relies on it, and adding it is
  still a maintainer decision (rule 6).
- Exports are payloads, not truth, and identity, references, manifests, provenance and verification stay in our own
  code.
- Never assume a batch exported every object without an inventory, and do not claim "all assets work" unless
  successes, failures, unsupported categories and duplicates are enumerated.

## Sensitive files: edit when needed, then warn

`src/package.cpp` (the `Reader`, `require()`, `limit`, size and terminator checks), `src/container.cpp` and
`src/container.hpp` (LZO container validation), `CMakeLists.txt`, their Rust ports `crates/ow-container/` and
`crates/ow-package/`, and the workspace files `Cargo.toml` and `rust-toolchain.toml` need no confirmation (since
2026-09-30). Rules 4 to 6 apply here more than anywhere else, and a new third-party crate is still a dependency
under rule 6. What you owe afterwards:

- **Warn the maintainer in your final message:** name each of these files you touched, what changed and why, and what
  is verified versus `UNVERIFIED`.
- Keep `ctest`, `cargo test --workspace` and `tools/verify_packages.py` green, and add a decision file when parsing
  behaviour changes.

## Working with the maintainer

zuhu has to be able to explain what gets merged, so:

- Keep diffs small and explainable. When in doubt, take the simpler, more obviously correct implementation over the
  clever one. AI can produce a lot of technical debt very quickly.
- Never hand an error back unread, and never claim more than you checked. When something fails, explain the cause
  and how you found it, not only the fix: zuhu's own norm is to read an error and check the docs before handing it
  to an assistant.
- Match the hedged, evidence-first tone of the existing docs.

None of this relaxes a rule above, and nothing counts as verified until it has been checked against the real game.

## Every change

- Run `ctest`, `tools/verify_packages.py`, `cargo build --workspace` and `cargo test --workspace`, then the gates for
  the part you touched, and paste the output. The command lines, the build steps and the current baseline are in
  `docs/verification/GATES.md`: the general gates at its top, the Rust host's gates after the dated rows. Not every
  Python or JS test is in ctest, so also run the ones for the part you touched (`python tests/<name>_test.py`,
  `node tests/<name>_test.js`).
- Report automated extraction and tests, visual validation, and runtime or gameplay validation (host runs, and
  real-game checks where there are any) separately, with pass and fail counts as they came out, including steps that
  fail only because local data is missing.
- Menu work: re-observe the real game (`docs/TOOLING.md`, "Tracing the real game's UI code") instead of guessing. It
  is the reference for menu behaviour until the Rust host has its own check.
- Heavy runs are one per machine: the real game, and the Rust runs that `GATES.md` and `docs/TOOLING.md` call heavy.
  Take the machine-wide lock first (`tools/ue_lock.ps1`; `docs/TOOLING.md`, "The run lock"); the real-game helpers
  take it themselves.
- A change in parsing behaviour needs a decision file saying what is verified and what is not: one new file under
  `docs/decisions/` (`python tools/decisions_index.py --name "<title>"` prints its name and `--check --baseline
  origin/main` checks it; rules in `DECISIONS.md`). Keep `README.md`, `ROADMAP.md` and `docs/TOOLING.md` in agreement
  with the decision files and the verification records; a `ROADMAP.md` change lands only with the maintainer's OK.

## Where things are

This file carries no status on purpose: the copy that used to live here went stale within weeks, and assistants
believed it. Look status up where it is maintained.

| Need | Where |
|---|---|
| What works, what is next | `ROADMAP.md`, "How it's going" and "Now / next" |
| Current test numbers | `docs/verification/GATES.md` |
| Evidence and open items for one area | its record in `docs/verification/` (`HUD_*`, `COMBAT_*`, `WEAPON_*`, `SANCTUARY_*`, `SLICE_*`, and the own-words `NATIVE_*` notes); the slice gate is `docs/verification/SLICE_GATE.md` (its recorded runs are the retired UE5 host's: history, not a gate result; the Rust `--slice-gate` results are rows in `GATES.md`) |
| A past decision | `python tools/decisions_index.py --tail N` or `--grep WORD` (one file per decision under `docs/decisions/`) |
| A native function | `python tools/native_index_find.py <name>` (do not open `docs/verification/NATIVE_INDEX.md` whole) |
| Tools, build, data preparation | `docs/TOOLING.md` |
| Landing PRs, and the standing approval with its exceptions | `docs/TOOLING.md`, "Landing PRs in a batch" |

**A fresh worktree** gets no ignored data (`local/`) from git. `tools/worktree-assets.md` says what the post-checkout
provisioner has already seeded (look before you fill anything) and how to copy the rest from an earlier worktree.
Then configure and build `build/`, run the checks above, and regenerate only what is missing or older than the code
that produces it (`docs/TOOLING.md`, "What a new worktree gets" and "Regenerating local data on a fresh machine").
