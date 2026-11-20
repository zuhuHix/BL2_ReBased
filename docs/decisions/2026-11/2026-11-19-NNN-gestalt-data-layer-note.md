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

## Edits: review r1 of #603

- **F1 is split into #596's three sub-cases.** (i) undecoded is `NotRead` "a decode is a write"; (ii) a stand-in that
  never loads is `STAND_IN` `NotRead` once the layer exists (hub ruling), and `LayerAbsent` until then; (iii) an
  incomplete slot is a data error. The note names the four `LayerAbsent` sites in `gestalt.rs` that go; `MERGE_LAYER`
  stays.
- **F2's signal is the package walk** (no `PropValue::Raw` in any of the three arrays' trees, as `ow-items` uses),
  because `to_value` fills defaults over raw nested fields. There is a test with a raw nested `Parts`.
- **The bounds union in operation order**, single precision throughout: the code's roots are single-precision square
  roots. The port's root goes through `sqrt_f64` and is cast back. The binary's rule is a normalised per-axis box.
  Bit parity is UNVERIFIED.
- **Smaller fixes:**
  - the declared member order is noted against #588's and #591's;
  - the None branch is reached only by a direct call;
  - #596's text prefixes are used;
  - six tests are added;
  - the bounds field's name is given.
