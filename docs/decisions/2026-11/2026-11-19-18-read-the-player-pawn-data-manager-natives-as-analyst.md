## 2026-11-19: Read the player pawn data manager natives as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane PAWN-DATA-MANAGER, analyst only. With the skill manager natives bound (decision 2026-11-19-14), Off
`--combat-run maya:8` stops on `WillowGlobals.GetPlayerPawnDataManager()`. That is slice link 1, and links 3, 4, 5, 13
and 14 wait behind it. The new own-words note `docs/verification/NATIVE_PAWN_DATA.md` and two `NATIVE_INDEX.md` rows
record:

- `GetPlayerPawnDataManager` is a plain read of `WillowGlobals.ThePlayerPawnDataManager`. The field is filled by a
  native globals routine, not lazily: it reuses an existing manager object or constructs one and roots it. When that
  routine runs was not found (its only path is the globals' C++ vtable), and the note asks it as an open question.
- The next native on the same statement, `WillowPlayerPawnDataManager.FindAlreadyLoadedObject(Path)`, looks the path
  up among objects already in memory (any class) and never loads. On the maya path it is the player class's
  `PawnArchetypePath`, whose `BodyClass` goes to `BodySelectClass`.

Raw output stays under ignored `local/analysis/pawn-data/`; the Ghidra runs were read-only and took the machine lock.
Nothing is implemented: both natives stay unbound in Off, so the stub policy refuses them, until a fresh lane codes
them from the note. Every rule is UNVERIFIED in the running game; the note lists the bl-sdk reads that would confirm
it.
