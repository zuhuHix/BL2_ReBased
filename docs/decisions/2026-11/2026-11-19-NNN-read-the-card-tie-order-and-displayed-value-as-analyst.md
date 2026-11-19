## 2026-11-19: Read the card's tie order and displayed value as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-CARD-BODY-REST, analyst only, for slice link 8. The decision number is
pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_CARD_BODY_REST.md`, and two `NATIVE_INDEX.md` rows. No code
  changes.
- **Sources:**
  - the installed packages (`tools/ghidra/class_layout.py`) for the presentation class and its structs;
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock), for the
    card body's sort and the presentation's displayed-value conversion. Neither is script.

  Raw output stays under ignored `local/analysis/cardrest/`. The note has no addresses, offsets, listing structure or
  decompiler names. No bl-sdk session took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:** this lane's author wrote the card-body note (NATIVE_CACHE_WEAPON_CARD_BODY.md) as analyst and did not
  implement #521. Neither note may be implemented by this author.
- **Findings:**
  - **The tie order** of #521's "card order of equal-priority entries" refusal is settled. It is an in-place,
    non-stable quicksort over **all** of the card's candidate entries, in first-insertion order, before the 0.0001
    threshold filter, described step by step. Sorting only the entries that will be appended (as #521 does) can order
    a tie differently.
  - **The displayed value** of #521's "the presentation's displayed value" refusal is settled for the card's use. It
    covers the zero snap, the percent scale or the value remap, the rounding by `RoundingMode` (with the percentage
    override to `IntRound`) at `FloatPrecision` clamped to 0 to 10, and no minimum of 1. The sign steps change only the
    sign.
  - **The visit refusal** is not on link 8's path: the name parts derive from `WeaponPartDefinition`. The slot count
    is another lane's.
  - **Left open:** the remap's default source (by fit, the first local player's controller), the wider-float steps,
    and integer-range overflow, each with a proposed typed refusal.
