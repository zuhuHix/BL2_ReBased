# Contributing

Thanks for looking. Read the [Is this legal?](README.md#is-this-legal) section of the README
and [docs/LEGAL.md](docs/LEGAL.md) first. The clean-room rules are
non-negotiable and every pull request certifies compliance with them.

**License.** The project is [MIT](LICENSE). By opening a pull request you
agree your contribution is licensed under the same terms (inbound = outbound;
see [docs/LEGAL.md](docs/LEGAL.md#license)). There is no CLA.

## Ways to help

### 1. Verify with your own copy of the game

This is the most valuable thing a non-programmer can do, and it is the
project's primary correctness method. Build the reader (README, *Build and run it*) and:

- Run `python tools/verify_packages.py --reader build/Release/ow-package.exe`
  and `python tools/census.py ...` against your install. Report the totals,
  your platform (Steam/Epic), which DLC you own, and whether the UHD pack is
  installed. Different installs are different test cases.
- If you have Unreal Engine 5.8: prepare and open a map
  ([docs/TOOLING.md](docs/TOOLING.md)), stand in the same spot in the real
  game, and compare. Report what's missing, mirrored, mis-coloured or
  misplaced, with both screenshots.

Use the **Verification report** issue template. Never attach package files or
extracted assets; screenshots are fine.

### 2. Contribute format findings

If you know something about version 832 / licensee 46 serialization (an
object prefix, a struct layout, a bulk-data quirk, a native-tail size), open
a **Format finding**. State how you observed it (which tool, which object,
which counts) and where it came from (your own observation, a public document,
a reference implementation and its license). A finding with provenance is
usable; one without is a rumour.

### 3. Code

Open items are listed in [ROADMAP.md](ROADMAP.md#now--next). Before starting
anything larger than a bounded task, open an issue so the approach can be
agreed. This matters most for anything that touches the sensitive areas below.

## Working rules

- **One observable capability per change.** Describe the input provenance,
  the expected behaviour, and a reproducible check.
- **Synthetic fixtures only.** Tests generate their own data with the
  existing `package()` / `tag()` helpers in `tests/`. Never commit game
  files, decompressed data, or extracted assets. Everything the tools write
  goes under `local/`, which is ignored; never move their output into the
  tree.
- **Report checks honestly and separately.** Automated checks (CTest,
  `tests/level_test.py`, `tools/verify_packages.py`) are one thing; in-game
  or visual checks are another. Say which you ran and paste the output. Do
  not present count agreement as full compatibility.
- **Label guesses.** Behaviour inferred rather than verified against the
  real game is marked `UNVERIFIED` in code and docs.
- **Record decisions.** Any change to parsing behaviour gets a dated entry in
  [DECISIONS.md](DECISIONS.md) stating what is verified and what is not.
- **Record provenance.** Any reference implementation consulted goes in
  [THIRD_PARTY.md](THIRD_PARTY.md) with its license, before code is written.
  Changing anything under `third_party/` means re-verifying the recorded
  hashes.
- **Match the tone.** The docs are hedged and evidence-first on purpose.

## Sensitive areas

Changes here need explicit discussion first and a DECISIONS.md entry:

- `src/package.cpp`: the `Reader` struct, `require()`, `limit`, size and
  terminator checks. Never loosen a bounds check to make something work.
- `src/container.cpp` / `.hpp`: LZO container validation.
- Struct/array property decoding, `Texture2D` and `StaticMesh` serialization,
  the cooked `Material` resource reader: the 832/46 layout has no public
  spec; do not invent offsets.
- `CMakeLists.txt`, `THIRD_PARTY.md`, any `LICENSE` file: dependency and
  license decisions are the maintainer's.
- The host-engine choice is made and is not reopened.

Safe to work on without ceremony: census and CLI output, synthetic tests,
JSON output flags, tooling scripts, documentation.

## Every pull request

The PR template asks for:

1. What changed and why, in one paragraph.
2. Output of `ctest --test-dir build -C Release --output-on-failure` and
   `python tests/level_test.py`.
3. For reader changes: output of `tools/verify_packages.py` against your own
   install. For census/property/asset/level changes: the relevant tool's
   numbers against your install.
4. In-game or visual checks, reported separately, or "none".
5. Confirmation that no game-derived data is included and that the
   contributor certification in [docs/LEGAL.md](docs/LEGAL.md) holds.

`main` always builds; CI runs the synthetic suites on every PR.

## AI coding tools

This is a solo project and AI coding tools are part of its workflow; the
project rules require saying so. If you use them too, the same rules apply to
their output: [CLAUDE.md](CLAUDE.md) is the working brief, a pre-write hook
enforces the sensitive-area prompts, and you are responsible for what you
submit. Read what it writes.
