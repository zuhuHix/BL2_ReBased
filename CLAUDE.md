# BL2_ReBased (code prefix OpenWillow/ow): working notes for AI assistants

Read `AGENTS.md` first. It is the tool-neutral project brief and defines the
vertical-slice priority plus the external-extraction support strategy. This
file adds repository-specific safety rules and sensitive areas.

Read the "Is this legal?" section of README.md and docs/LEGAL.md first. The rules are
non-negotiable: no game files or asset dumps in the repo, no leaked/decompiled
source, record provenance and licenses, disclose AI assistance, keep
verification claims honest.

## Sensitive areas: stop and warn before editing

Before changing any of these, tell the user explicitly that the area is
sensitive, say what you intend to change, and wait for confirmation:

- `src/package.cpp`: the `Reader` struct, `require()`, `limit`, size and
  terminator checks. Never loosen a bounds check to "make it work".
- `src/container.cpp` / `src/container.hpp`: LZO container validation.
- Struct/array property decoding, texture (`Texture2D`) and mesh
  (`StaticMesh`) serialization: the version 832/46 layout has no public spec;
  do not invent offsets from memory. Label guesses `UNVERIFIED`.
- `CMakeLists.txt`, `THIRD_PARTY.md`, `LICENSE`: dependency and
  license/provenance decisions. The project license is MIT (2026-09-13). Never
  copy code from GPL tools (Legendary Explorer, UE Explorer, UPKUtils, etc.).
  UModel / UE Viewer may be used as an external binary or format reference;
  copying its MIT-licensed code still requires a provenance entry first.
- The host-engine choice (docs/OPENWILLOW_ENGINE_PLAN.md §2.1) is made (UE5) and
  is not reopened by an AI assistant.

A `PreToolUse` hook in `.claude/settings.json` enforces a confirmation prompt
on those files and blocks writes of `.upk/.tfc/.pck/.bik` files.

## Safe to work on without ceremony

Export census over packages, synthetic tests using the existing `package()` /
`tag()` fixture helpers in `tests/`, CLI/JSON output flags, documentation,
`tools/verify_packages.py`. Keep README.md, ROADMAP.md and docs/TOOLING.md in
agreement with DECISIONS.md and the verification records under docs/verification/.

## Development priority: prove the vertical slice first

Porting all 82 maps is the *easy* part of this project. The hard, unproven
part is Phases 2-4 (script VM, stock UE3 natives, Gearbox's undocumented
natives). Porting 80 maps only to discover in Phase 2-4 that the approach
doesn't hold would waste most of the project's calendar time. So, adopted
2026-09-18 from outside feedback (see DECISIONS.md), the current priority
order is:

1. Finish **Sanctuary** to visual/walkable parity (Phase 1). It's already
   the most complete map; don't start over on a different one.
2. Get **Maya** (chosen 2026-09-18) working end-to-end: movement, a few
   weapons, animations, her action skill (Phaselock). Her skill trees
   (Motion, Harmony, Cataclysm) are the target for Phase 4's skill-tree work.
3. Prove core engine/gameplay integration (Phase 2's VM, Phase 3's stock UE3
   natives) scoped to what Sanctuary + that one character actually need. Not
   the full native surface, not every `AnimNode` type.
4. One hand-picked simple mission and a handful of guns, end-to-end: spawn,
   fight, loot, equip, use a skill, complete the mission, die, respawn. That's
   Phase 4's gate in ROADMAP.md.

Only after that slice is proven does broad map coverage (the remaining ~79
maps) and additional Vault Hunters become the priority again; that work is
in Phase 5/6 of ROADMAP.md. If asked to "port map N" or "add character X"
before the slice gate is met, say so and confirm with the user first instead
of just doing it; it's a priority-order question, not a technical blocker.

## AI is a tool, not the developer

zuhu is a solo, largely self-taught developer directing an AI-heavy build.
Outside feedback (2026-09-18, see DECISIONS.md) set the working norm:

- Don't hand an error message straight back to an AI assistant for a fix.
  Read the error, check the relevant docs/Stack Overflow/GitHub issues first;
  bring in AI once genuinely stuck. This is how zuhu learns the codebase well
  enough to keep directing it.
- Generated code should be something zuhu can explain, not just accept. Prefer
  smaller, explainable diffs over large ones.
- AI can generate a lot of technical debt very quickly. When in doubt, favor
  the simpler, more obviously correct implementation over the clever one.

This doesn't relax any other rule in this file: sensitive areas are still
sensitive, and everything still needs a check against the real game.

## External extraction policy

Use UModel / UE Viewer as the first external extraction candidate for supported
Borderlands 2 UE3 assets needed by the Sanctuary/Maya proof of concept. Its
exported meshes, textures, animations and sounds are local payloads for the UE5
import pipeline; they are not a replacement for our package identity,
object-path resolution, cross-package reference handling, scene manifests or
verification. UModel material files are heuristic and do not prove complete
UE5 material-graph compatibility.

Before introducing another external tool, timebox a representative benchmark
and record its version, provenance, license, command line, elapsed time,
successes, failures, unsupported types, duplicates and output size. Put tools
outside the repository and generated output under ignored `local/`. Never copy
game-derived output into the repository and never assume a batch command
successfully exported every object without an inventory.

The current verified local candidate is UModel build 1590 from the official
`gildor2/UEViewer` repository. The actual executable path is machine-specific;
use an explicit `-UModel` path or an environment variable rather than adding a
binary to this repository.

## Every change

- Run `ctest --test-dir build -C Release --output-on-failure` and
  `python tools/verify_packages.py --reader build/Release/ow-package.exe`;
  paste the output. Report automated checks separately from in-game checks.
- Add a DECISIONS.md entry when parsing behavior changes; mark what is verified
  and what is not. Do not present count agreement as full compatibility.
- Fixtures are synthetic only. Match the hedged, evidence-first tone of the
  existing docs.
