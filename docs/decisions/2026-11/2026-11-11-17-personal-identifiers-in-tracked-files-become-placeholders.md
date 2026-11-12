## 2026-11-11: Personal identifiers in tracked files become placeholders (lane-identifiers)

AI-assisted (Claude Opus 5.5). Policy change for tracked text and script defaults; no parser, tool output or host behaviour changes.

- **Why.** The repository is public, and an audit on 2026-11-11 found a Windows user-profile path on 27 lines in 18 files, the Windows account name on 39 lines in 20 files, and the private store's repository named in `docs/NATIVE_ANALYSIS.md`. The maintainer decided the same day: from now on such identifiers are placeholders. History is not rewritten; only the tree changes.
- **What.** Documentation and records use `%USERPROFILE%\...` or `<you>` for profile paths, and the zuhu PC's label in place of the account name. The private store is described without its name or URL, and the instruction never to link it stays. Script defaults no longer hard-code a profile path: the Ghidra and JDK defaults in `tools/ghidra_import.ps1` and `tools/ghidra/run.ps1` are built from `$env:USERPROFILE` (the `OPENWILLOW_GHIDRA` / `OPENWILLOW_JDK` overrides are unchanged), and the `-HubLock` default of eight capture and import scripts is derived from the git common directory, as the newer capture scripts already did. On this PC both resolve to the same paths as before.
- **Old entries edited.** As an exception to this log being append-only, three older entries were edited, each only to swap the identifier for a placeholder, with meaning unchanged: the 2026-10-01 entry (the private store's name) and two 2026-09 entries in `docs/decisions/2026-09.md` (two tool paths).
- **Left for the maintainer.** `THIRD_PARTY.md` still holds four profile paths; it is the maintainer's file, so the lane proposed replacements instead of editing it.
- **Verified (automated):** the defaults resolve to the old paths on the zuhu PC; the gates are in the lane's report.
- **Not verified:** no capture or Ghidra run was started with the new defaults.

