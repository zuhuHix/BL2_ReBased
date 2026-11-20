## 2026-11-19: CacheWeaponCard's card order and displayed value in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-CARD-BODY-REST, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_CARD_BODY_REST.md` (#532 at 5301a80d, which holds the band ruling;
  analyst lane ANALYST-CARD-BODY-REST, not this thread) and NATIVE_CACHE_WEAPON_CARD_BODY.md section 3.3 for the
  value V. No native code or analysis output was opened.
- **Base:** #521 (fix/rust-cache-weapon-card-body at 83d4e909). Only #521's two remaining refusals change, the
  equal-priority order and the displayed value. The rest of #521 stays as it is.
- **No real-game session:** every rule is **UNVERIFIED** in the running game, and #532 is still in its hub read.
- **Unchanged:** no dependency was added and no bounds check was loosened. Reference binds nothing, as before.

**What changed** (`crates/ow-inventory/src/equip/card.rs`, Off only):
- **The order** (the note's section 2):
  - a NaN priority anywhere in the list is the typed refusal "an unordered card priority", checked before the sort;
  - otherwise `sort_entries` sorts the whole list, in first-insertion order, by the note's section 2.1 rules;
  - only then are entries of at least 0.0001 appended.
  - The tie refusal is gone.
- **The values** (body note section 3.3, the rest note's section 3): an appended entry with a record gets V and
  `bShouldDisplay` from the displayed form. This covers the zero snap, the percentage or the remap, the sign, and the
  rounding by mode and clamped precision on the doubled sum, with no minimum of 1. The display refusal is gone.
  - V is computed in single precision. The doubled sum is computed in double, and the remap in double, then rounded to
    single. The double sum is not exact for a small X: it rounds, but that moves n only below about 1e-16.
- **The typed refusals** of the note's section 4, item 3, each `NotYet` of kind `NotRead` with this note as the
  record, raised in the card step before the first card write:
  - the remap without `bUseContextOverride` (no hub ruling for the default source);
  - a converted value that is NaN, infinite or outside the signed 32-bit range;
  - single-precision steps that change the outcome. In mode 0 at precision 8 or more the outcome is hidden, in the
    band or shown; elsewhere it is whether n is 0. This follows the hub's ruling on #532, since single rounding can move
    n by one into or out of the band without crossing 0.
  - mode 0 at precision 8 or more with |n| strictly between half and twice 10^(P−8).

  The remap's four evaluations pass the attribute layer's refusals through unchanged.
  - **Range check:** it is applied to the converted integer, the doubled sum rounded to even, not to the doubled sum
    itself as the note words it. A sum just past ±2^31 that rounds into range is accepted. Which reading the game
    follows is the note's open question 3 (UNVERIFIED); the port keeps the converted-integer reading.
  - **Refusal texts:** each is the note's text with "WillowWeapon." before it, as #521's texts are.
- **The preflight's card schema** also checks `bValueRemappingEnabled`, `bDisplayPercentAsFloat`, `bBiggerIsBetter`
  (bools), `SignStyle` and `RoundingMode` (bytes) and `FloatPrecision` (an int). It also checks `RemappingData`: the
  remap struct, with `bUseContextOverride` and the four initialization-data fields. The field names and kinds were
  checked against the installed class with `tools/ghidra/class_layout.py`.
- **The partial-writes prefix** is #521's own: the card runs under `marked_after_loads`, so a refusal is prefixed only
  when #518's counters moved (a remap evaluation that draws or loads, for one).

**Tests** (synthetic classes and values):
- the note's sort examples A, B, C, D, H, J and K, H' (filtering first gives another order), and H through the card;
- the NaN refusal with nothing written;
- V and `bShouldDisplay` for records from both maps, a zero value, the scale and the post-add;
- the percentage's division by B;
- the remap refused without the override, and applied with it;
- the sign and rounding interplay of section 3 (0.2 and −0.2 under IntCeil and IntFloor, the exact halves, each sign
  style);
- the zero snap, the percentage's IntRound switch and the precision clamp;
- the remap's three forms and a NaN bound;
- the range, precision and cutoff refusals, including the note's two precision examples;
- two new schema failures;
- #521's draw-then-refusal test, now driven by the NaN refusal.
- the hub's two band examples on the decision (P 8: double −2 shows, single −1 in the band; P 9: double 5 hides,
  single 6 in the band);
- their two mirrors through `shows`, from single values a search found. The ruling's exact pairs need a double sum just
  past a tie, which single precision cannot reach.

The sort examples come from the note's table and were not tuned: every one matched at the first run.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #534
at 8fc0420b, which contains #521 (deleted after):
- ok 0, NotYet 5, error 9 of 14;
- #534 alone (8fc0420b) gives the same rows and the same stops.
- **Link 8** (equip-run) passes 1 of its driver's 5 steps (`make`) and stops at `init` on the unbound
  `WillowWeapon.RecomputeAttributeBaseValues()`. The script calls it right after `CacheWeaponCard`, so the card passes
  on the slice's weapon, and it does on #534 alone too. The slice's weapon does not exercise this lane's rules.
