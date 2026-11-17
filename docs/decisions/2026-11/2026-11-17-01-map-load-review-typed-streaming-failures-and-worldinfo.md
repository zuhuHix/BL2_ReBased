## 2026-11-17: Map load review: typed streaming failures and WorldInfo validation

AI-assisted (Codex). Review round 1 of PR #376, following decision 2026-11-16-76.
The maintainer requested both corrections together, without rebasing the reviewed head.

- `MapError` retains the store, package-reader and identifier errors as typed causes.
  A streaming level is recorded as missing only for `StoreError::PackageNotFound` from
  loading that map file. Corrupt files, ambiguous package names, malformed names,
  missing level exports and wrong level classes stop the load. No error text is compared.
- Both persistent and streaming levels load and check their first entry against
  `Engine.WorldInfo` before it is placed or skipped. A streaming level whose first
  entry is an ordinary actor is refused before its remaining actors are placed.
  The existing list layout, bounds checks and valid placement order are unchanged.
- Synthetic regressions run in Off and Reference: genuinely absent streaming files,
  corrupt files, ambiguous names, malformed names, missing level exports, wrong level
  classes and ordinary actors in the first slot of either kind of level. The existing
  ordering regression still passes. `ow-world`: 80 passed, 0 failed, 3 ignored;
  crate edges: 19 passed. Formatting and diff whitespace checks pass.
- Clippy on all `ow-world` targets with `--no-deps` exits 0 with one pre-existing
  `type_complexity` warning in `query.rs`. Strict all-target Clippy fails on that warning;
  strict Clippy including dependencies first fails on three pre-existing findings
  in `ow-package`. These unrelated files are unchanged.
- Strict library-only Clippy with `--no-deps` passes. The decision check against
  reviewed head `b5e9601` passes (496 baseline files, 1 added, 0 edited). Against
  `origin/main` it fails because that newer branch contains decision 68 absent from
  this older PR head; checking unmodified `b5e9601` reproduces the same failure.
  The reviewed branch is intentionally not rebased for this fix.
- Previously passed workspace suites cover unchanged code; they are not rerun for
  this bounded review fix. No live game, host or native-analysis session was run.
  Begin-play, game info and local player remain pending in both modes; gameplay
  parity and streaming order remain UNVERIFIED. No dependency or layout was added.
