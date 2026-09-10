# OpenWillow — working notes for AI assistants

Read README.md "Project rules" first. They are non-negotiable: no game files or
asset dumps in the repo, no leaked/decompiled source, record provenance and
licenses, disclose AI assistance, keep verification claims honest.

## Sensitive areas — stop and warn before editing

Before changing any of these, tell the user explicitly that the area is
sensitive, say what you intend to change, and wait for confirmation:

- `src/package.cpp` — the `Reader` struct, `require()`, `limit`, size and
  terminator checks. Never loosen a bounds check to "make it work".
- `src/container.cpp` / `src/container.hpp` — LZO container validation.
- Struct/array property decoding, texture (`Texture2D`) and mesh
  (`StaticMesh`) serialization — the version 832/46 layout has no public spec;
  do not invent offsets from memory. Label guesses `UNVERIFIED`.
- `CMakeLists.txt`, `THIRD_PARTY.md`, any `LICENSE` file — dependency and
  license/provenance decisions. Never copy code from GPL tools (Legendary
  Explorer, umodel, etc.) without the user deciding on licensing first.
- The host-engine choice (plan §2.1) is a user decision, not a code task.

A `PreToolUse` hook in `.claude/settings.json` enforces a confirmation prompt
on those files and blocks writes of `.upk/.tfc/.pck/.bik` files.

## Safe to work on without ceremony

Export census over packages, synthetic tests using the existing `package()` /
`tag()` fixture helpers in `tests/`, CLI/JSON output flags, documentation,
`tools/verify_packages.py`.

## Every change

- Run `ctest --test-dir build -C Release --output-on-failure` and
  `python tools/verify_packages.py --reader build/Release/ow-package.exe`;
  paste the output. Report automated checks separately from in-game checks.
- Add a DECISIONS.md entry when parsing behavior changes; mark what is verified
  and what is not. Do not present count agreement as full compatibility.
- Fixtures are synthetic only. Match the hedged, evidence-first tone of the
  existing docs.
