# Contributing

Keep changes to one observable capability at a time. Describe input provenance,
expected behavior, and a reproducible check. Use synthetic package fixtures;
never commit game files, decompressed data, or extracted assets.

Read the project rules in README.md before contributing. Record external code
sources and licenses before copying or translating implementations. License
selection and the existing Python research decompressor's provenance are unresolved.
The optional native miniLZO dependency has recorded provenance in THIRD_PARTY.md;
its GPL license applies to enabled builds and must be addressed before distribution.

Build and run CTest using the README commands. For package-reader changes, also
run the local nine-package comparison with your own game installation. Report
automated checks separately from in-game or visual verification. Mark guessed
behavior UNVERIFIED; do not present count agreement as full compatibility.
