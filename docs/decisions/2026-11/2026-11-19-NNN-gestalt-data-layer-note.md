## 2026-11-19: The gestalt component data layer, an own-words note for one implementer

AI-assisted (Claude Code)

Lane ANALYST-GESTALT-DATA-LAYER, analyst only, with Claude Opus 5.5. The file number is a placeholder until the
maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_GESTALT_DATA_LAYER.md`, a new note, not an extension (#588's and #591's notes
  are not on main). It comes from a local, read-only Ghidra reading under the machine lock, the packages' struct
  layouts, and a search of all script. There is one NATIVE_INDEX row. Raw output stays under ignored
  `local/analysis/gdl/`, and no addresses, offsets or listing structure are in tracked files.
- **Findings** (all UNVERIFIED in game):
  - (a) the fill's first write is `PartMeshNames` := the name list;
  - (b) the socket mappings are appended per name in list order, from the definition's mappings whose fragment name
    matches;
  - (c) the bounds start at a constant (zero by fit). The first matching part's bounds are copied, and later ones
    unioned with the stock bounds sum;
  - (d) every write and copy falls at the offset of one declared `GestaltDataContainer` member, so the match is by
    offset, not by position.

  The fill leaves the accessory list and the matrices object untouched. It loads and draws nothing. No script reads
  any member.
- **Proposed Off answer:** the component's `GestaltData` heap slot as the layer. A write-free, load-free and draw-free
  preflight runs first: the declarations, the slot held whole (F1), and the definition's arrays decoded (F2). Then
  one whole-slot write for the fill and one for the copy. Reference is unchanged, and #596's two `LayerAbsent`
  refusals are removed once the layer exists.
- **Probe:** installed Off `--slice-gate`, strict, on main 3ad58b08: ok 2, notyet 10, error 2. Link 8 passes 1 of 5
  steps and stops at `init` on `CacheWeaponCard`'s slot count, before these natives.
- **Clean room:** a different lane implements this note.
