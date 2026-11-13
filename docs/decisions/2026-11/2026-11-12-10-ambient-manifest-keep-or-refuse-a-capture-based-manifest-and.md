## 2026-11-12: Ambient manifest: keep or refuse a capture-based manifest, and the suite names its look

AI-assisted (Claude Code). Tools, a suite launcher and their tests; no host change and no parsing change. The follow-up to
`2026-11-12-09-generated-data-guards-prep-steps-keep-or-refuse-never-drop.md`. Record: `docs/TOOLING.md`, the ambient citizens' steps,
and the ambient line in `docs/verification/GATES.md`.

What happened: on the desktop's test worktree, `local/slice/ambient_world.json` was a capture-based manifest (observed, 35 spawns, 89
worn pieces). The documented bare call `python tools/prepare_ambient_world.py` replaced it without a word by a census-only one (39
spawns, no worn pieces, exit 0). The capture it came from was not in that worktree, so it could not be rebuilt there. `--compose`
without `--observed` also wrote no pieces. The behaviour is older than the previous decision.

- **`tools/prepare_ambient_world.py` keeps or refuses, never degrades.** When the output file exists and is capture-based (`observed`
  true, or any spawn with worn pieces), a run without the capture, or with fewer worn pieces in total, stops before writing (exit 1).
  The message names the file's counts and the new ones, and the ways on: pass the capture, pass `--replace-observed` if the census-only
  world is wanted, or write elsewhere with `--output`. A file that exists but cannot be read as a manifest is not overwritten either
  (what it held is unknown), unless `--replace-observed` is given. A path that does not exist yet is written as before.
  `--compose` or `--stops-from` without `--observed` stops: they only apply to observed citizens.
  Review r1: a run without `--observed` is refused before the reader work; an odd shape (an `attachments` that is not a list) or an
  empty file counts as unreadable; an allowed replacement writes a temporary file next to the target and replaces it only when
  complete, so a failure part-way keeps the old manifest; the refusal names the whole capture call, `--compose` and `--stops-from`
  included, and `--replace-observed` covers a smaller capture that is meant as well as the census-only world.
- **The `ambient` suite would have passed on the census-only manifest.** Read, not run: the host's `OWAMBIENT SUMMARY` gives PASS when
  every pawn reached a node (or is held) and one walked 50 cm, and `tools/suites.txt` matches only that line; nothing looks at the
  citizens' look. `tools/test_ambient.ps1` now runs `prepare_ambient_world.py --check-manifest` first. Its result line ends with
  `manifest observed=... spawns=... worn=...`, and a manifest that is not capture-based fails before the editor starts unless the run
  passes `-CensusWorld`. A missing Python gives `OWAMBIENT SUMMARY result=FAIL manifest check: python not found` (exit 1).

Checked: synthetic tests in `tests/ambient_transform_test.py` (an observed manifest is not replaced by a census one, fewer worn pieces
are refused, worn pieces alone count as capture-based, the flag writes, a missing or census file is written, an unreadable file is
refused, `--compose` without `--observed` stops, and the preflight line). On this worktree's real data the bare call stopped with the
counts above (observed, 35 spawns, 89 pieces against 39 spawns and none) and left the file byte-identical; `--output` to a new path
wrote the census-only world; the launcher refused that file before taking the run lock. The in-engine suite has not run on this change.

UNVERIFIED: that every capture-based manifest on the maintainer's machines carries `observed` or worn pieces;
a hand-made manifest with neither fails the suite's preflight.
