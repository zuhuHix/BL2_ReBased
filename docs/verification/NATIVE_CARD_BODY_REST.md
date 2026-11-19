# CacheWeaponCard's body, the rest: the order of equal priorities and the displayed value (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-CARD-BODY-REST (decision 2026-11-19-NNN, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed packages, through `tools/ghidra/class_layout.py` (the presentation class and its structs);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- the script has no part in these steps: the card body and the presentation's conversion are native.

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/cardrest/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** `WillowWeapon.CacheWeaponCard`'s body (NATIVE_CACHE_WEAPON_CARD_BODY.md, implemented in Off by
#521) still refuses four cases with `NotYet { NotRead }`. With #521, and the slot-count rule in progress in another
lane, link 8's next stops are two of them: the card order of equal-priority entries, and the presentation's displayed
value. This note settles both and places the other two.

## 1. The four refusals, and which this note settles

| Refusal (#521's text, record NATIVE_CACHE_WEAPON_CARD_BODY.md) | Here |
|---|---|
| "the card's slot count" | not here: another lane reads it |
| "the card order of equal-priority entries" | **settled**: section 2 |
| "the presentation's displayed value" | **settled**: section 3 |
| "the card visit of a definition that is neither a weapon type nor a weapon part" | **not on link 8's path**: every definition the card visits comes from a weapon-type or a weapon-part field, and the name parts (`WeaponNamePartDefinition`) derive from `WeaponPartDefinition`. It stays `NotRead` |

## 2. The order of the card entries

The card keeps its candidate entries in a map keyed by presentation. An entry is added the first time its
presentation is met, so **the map's element order is first-insertion order**: the order of the visits in
NATIVE_CACHE_WEAPON_CARD_BODY.md section 3.2. When a presentation comes up again, its priority grows in place, and its
position stays.

**Before** the threshold filter, when the map holds at least one entry, its elements are put in order by this sort,
over **all** entries: those below the 0.0001 threshold too, and those that will not be appended. Then the map is walked
in the sorted order, and each entry that passes the filter is appended (section 3).

**The sort** is in place and not stable. Here `p(x)` is entry `x`'s priority. "x goes after y" is the single-precision
test `p(y) − p(x) > 0`, which is false whenever a NaN is involved. On the element sequence `E[0..n−1]`, with `n` at least
2:
1. A stack of index ranges starts with `[0, n−1]`.
2. Take the top range `[lo, hi]` off the stack, and repeat:
   - **Small range** (`hi − lo + 1` is 8 or fewer): while `lo < hi`, find `m`: start at `m = lo`, and for `k` from
     `lo + 1` to `hi`, set `m = k` when `p(E[m]) − p(E[k]) > 0`. So `m` is the **first** of the lowest priorities. Swap
     `E[m]` and `E[hi]`, then decrease `hi` by 1. The range is then done.
   - **Large range** (more than 8):
     1. Swap `E[lo]` with `E[lo + c/2]`, where `c = hi − lo + 1` and the division is integer. The pivot is now `E[lo]`.
     2. Set `i = lo` and `j = hi + 1`, then repeat:
        - **`i` steps right:** add 1 to `i` until `i > hi`, or `p(E[lo]) − p(E[i]) > 0` is true (the element is
          below the pivot). A NaN difference also stops it: `p(E[lo]) − p(E[i]) ≤ 0` is the test to keep going;
        - **`j` steps left:** subtract 1 from `j` until `j ≤ lo` (then `j = lo`), or `p(E[lo]) − p(E[j]) > 0` is
          false (the element is not below the pivot);
        - if `i ≤ j`, swap `E[i]` and `E[j]`, and carry on both scans from where they stand; otherwise stop.

        Said plainly: `i` walks right over elements whose priority is at least the pivot's, and `j` walks left over
        elements whose priority is below it. A pair found out of place is swapped.
     3. Swap the pivot `E[lo]` with `E[j]`. The left part is `[lo, j − 1]` and the right part is `[i, hi]`.
     4. If the left part is shorter (`j − 1 − lo < hi − i`): push the right part when it has two or more elements;
        then, if the left part has two or more elements, carry on with it; otherwise take the next range off the
        stack. Otherwise: push the left part when it has two or more elements; then, if the right part has two or
        more, carry on with it; otherwise take the next range.
3. Stop when the stack is empty.

The result is in descending priority, with ties in whatever order these exact steps leave them. That order depends on
every entry in the map, so **an implementation must sort the full map, in first-insertion order, before it filters.**
Sorting only the entries that will be appended can order a tie differently.

## 3. The displayed value, and `bShouldDisplay`

For each entry the walk appends (a presentation set, priority at least 0.0001), the card computes the base value B and
the value V (NATIVE_CACHE_WEAPON_CARD_BODY.md section 3.3, unchanged). Then:
- `bShouldDisplay` is **true when the entry has no modifier record**;
- otherwise it is true when the presentation's **displayed form of V**, computed as below, is at least 0.00000001 in
  size.

The card asks for the displayed form in its "card" mode (one of three modes the conversion takes), with the "no
minimum of 1" option set, and with no context object. Only the size of the result matters to the card, so the sign
steps, which change only the sign, are listed but cannot change `bShouldDisplay`.

**The conversion, in the card's mode** (fields of `AttributePresentationDefinition`, in this order):
1. **Zero snap.** When |V| < 0.00000001, V is taken as 0 (and noted as "zero" for step 6).
2. **Scale or remap:**
   - with `bDisplayAsPercentage`: V is multiplied by 100 (unless it is "zero");
   - without it: the **remap**, below.
3. **Sign** (by `SignStyle` and `bBiggerIsBetter`): V may be negated. This changes only the sign, never the size, so
   it cannot change `bShouldDisplay`, and it is not detailed here.
4. **Inverse:** not in the card's mode (`bDisplayAsInverse` applies only in the plain mode).
5. **Rounding** of V, by a rounding mode and a precision:
   - **The mode:** the presentation's `RoundingMode`, except that in the card's mode, with `bDisplayAsPercentage` set,
     `RoundingMode` = `ATTRROUNDING_Float` (0) and `bDisplayPercentAsFloat` clear, the mode is
     `ATTRROUNDING_IntRound` (1).
   - **The precision** `P` is `FloatPrecision`, taken as 0 when below 0 and as 10 when above 9.
   - **By mode:**
     - 0, `Float`: the value rounded half up at `P` decimals: `floor(10^P × V + 0.5) / 10^P`. Here 10^P is computed in
       double and used in single precision, and the floor goes through a 32-bit integer.
     - 1, `IntRound`: `floor(V + 0.5)`;
     - 2, `IntFloor`: `floor(V)`;
     - 3, `IntCeil`: `ceil(V)`;
     - any other value: V unchanged.
   - Each whole-number result goes through a signed 32-bit integer. A value outside that range is not defined here
     (open question 3).
6. **No minimum of 1:** the card's option is set, so a nonzero V that rounds to 0 stays 0. In other callers, it would
   become ±1.

**The remap** (step 2, without `bDisplayAsPercentage`):
- Without `bValueRemappingEnabled`, V is unchanged.
- With it, the four initialization-data fields of `RemappingData` are evaluated, in this order, as
  attribute-initialization data (the port's `evaluate_init`, with no override source): `InputValueMn` as `a`,
  `InputValueMx` as `b`, `OutputValueMn` as `c`, `OutputValueMx` as `d`.
  - **The source** is the call's context object when `RemappingData.bUseContextOverride` is set. The card passes
    none, so the source is None.
  - Otherwise, the source is a global: the engine's first local player, through one of its fields. By fit that field
    is the local player's controller; **UNVERIFIED**, open question 1.
- The slope `s` is `(d − c) / (b − a)` when `a < b`, else 0.
- V maps to:
  - `c` when `V < a`;
  - `(V − a) × s + c` when `V < b`;
  - else `(b − a) × s + c`.

  The engine does these steps in a wider float than single precision (open question 2).

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces two of #521's refusals; nothing else in #521 changes.

1. **The order:** drop "the card order of equal-priority entries". Build the entry list in first-insertion order, as
   #521 does. Sort the **whole** list (all entries, before the threshold filter) with section 2's steps, then filter.
   The sort writes nothing script-visible (the list is the native's own).
2. **The display:** drop "the presentation's displayed value". For an appended entry with a record, compute section
   3's displayed form and set `bShouldDisplay` from its size. Preflight, before the native's first write, these
   refusals, each `VmError::NotYet` of kind `NotRead`, record this note:
   - `bValueRemappingEnabled` set, `bDisplayAsPercentage` clear, `bUseContextOverride` clear: "CacheWeaponCard: the
     remap's default context (the first local player's controller, by fit)". Alternatively, a hub ruling lets the
     port use its world's first local player controller, labelled UNVERIFIED;
   - a whole-number rounding whose input is outside the signed 32-bit range: "CacheWeaponCard: a displayed value
     outside the integer range";
   - the remap's four evaluations refuse as the attribute layer refuses (typed, passed through unchanged).
3. **Unchanged:** "the card's slot count" (another lane) and "the card visit of a definition that is neither a weapon
   type nor a weapon part" (not on link 8's path).
4. **Writes:** none new. The conversion reads only. The remap's evaluations may load definitions, and under the
   write-tracker (decision of review r2 of #518) a load counts as a write, so a refusal after one is marked.

**Reference** keeps the stub policy.

## Open questions

1. **UNVERIFIED:** the remap's default source: which field of the engine's first local player it takes (by fit the
   controller). The card reaches it only through a presentation with remapping enabled and no context override.
2. **UNVERIFIED:** the remap's interpolation and the rounding's division are done by the engine in a wider float. A
   single-precision port can differ in the last bit; this matters to `bShouldDisplay` only right at the 0.00000001
   threshold.
3. **UNVERIFIED:** what a whole-number rounding does with a value outside the signed 32-bit range.
4. Unchanged from NATIVE_CACHE_WEAPON_CARD_BODY.md: the slot count's definition (another lane), and the class test of
   the presentation lookup.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. **The tie order.** Find a weapon whose card has two entries of equal priority (two presentations with the same
   `BasePriority`, listed by the same definitions). Read `WeaponCardModifierStats` after the drop, and compare it with
   section 2's sort run on the visit order. A weapon with more than 8 card candidates also exercises the large-range
   path.
2. **The hidden entries.** On a weapon with a tie and an entry below the 0.0001 threshold, check that the order of the
   tie matches the sort run over all entries.
3. **`bShouldDisplay`.** On an entry with a modifier record and a small value, for example a percentage presentation
   (`IntRound` by the rule above) with |V| below 0.005, expect `bShouldDisplay` false. For the same value on a
   presentation with `RoundingMode` `Float` and `FloatPrecision` 2, expect it true at 0.005 or more.
4. **The remap.** On a presentation with `bValueRemappingEnabled`, read the displayed text for two values and fit the
   linear map. Compare with the four fields evaluated for the local player's controller.
