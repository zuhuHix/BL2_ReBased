# Contributing

Keep changes to one observable capability at a time. Describe input provenance,
expected behavior, and a reproducible check. Use synthetic package fixtures;
never commit game files, decompressed data, or extracted assets.

Read the project rules in README.md before contributing. Record external code
sources and licenses before copying or translating implementations. License
selection and the existing Python research decompressor's provenance are unresolved.
The vendored lzokay decoder (MIT) has recorded provenance in THIRD_PARTY.md;
update that file and re-verify hashes if you change anything under `third_party/`.

Build and run CTest using the README commands. For package-reader changes, also
run the local nine-package comparison with your own game installation. For
census, property or asset changes, rerun `tools/census.py` and
`tools/prepare_probe.py` against your installation and report the numbers.
Everything those tools write goes under `local/`, which is ignored; never move
their output into the tree. Report automated checks separately from in-game or
visual verification. Mark guessed behavior UNVERIFIED; do not present count
agreement as full compatibility.
