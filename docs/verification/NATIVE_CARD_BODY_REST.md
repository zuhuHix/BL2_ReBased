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
NATIVE_CACHE_WEAPON_CARD_BODY.md section 3.2 (weapon type, then parts, then the damage type, unchanged). When a
presentation comes up again, its priority grows in place, and its position stays.

**Before** the threshold filter, the map's elements are put in order by the sort below, over **all** entries,
including those below the threshold. Then the map is walked in the sorted order, and each entry whose priority is at
least 0.0001 (equality kept) is appended (section 3). **An implementation must sort the full map in first-insertion
order and only then filter.** Sorting only the entries that pass can order a tie differently (example H below).

### 2.1 The sort's rules

The comparison is "a is lower than b", that is `p(a) < p(b)`, where `p` is the entry's single-precision priority. For
two ordered values the engine's difference test gives exactly this. Any test that involves a NaN is false (section
2.3).

For ordered priorities the result is **descending**. The sort is not stable; ties are left where these rules put
them:
- **A range of 8 or fewer entries:** repeatedly, the lowest priority in the range goes to the range's last place, and
  the range shrinks by one from the right. Among equal lowest priorities, the **first** (leftmost) one is taken. It
  is exchanged with the last entry, and that exchange can move a tie partner to the left.
- **A range of 9 or more:**
  - the **pivot** is the entry at offset `count / 2` (integer division) from the range's start. It is exchanged with
    the range's first entry;
  - **the partition:** a left cursor looks for entries strictly lower than the pivot. A right cursor, starting from
    the end, looks for entries that are not lower than the pivot. Each pair found is exchanged, as long as the left
    cursor has not passed the right one. The left cursor stops at the range's end at the latest, and the right cursor
    at the pivot's place;
  - the pivot is then exchanged with the entry where the right cursor stopped. The entries before that place form one
    part, and the entries from the left cursor's stop onward form the other. Each part is sorted by the same rules.

The parts do not overlap, so the order in which they are processed does not change the result.

### 2.2 Worked examples (synthetic; letters are first-insertion order, numbers are priorities)

| | Input | Sorted | Appended (≥ 0.0001) |
|---|---|---|---|
| A, ties, small | a2 b5 c2 d2 e5 | b5 e5 d2 c2 a2 | all |
| B, hidden, small | a1 b0 c1 d0.00005 e1 | e1 c1 a1 d0.00005 b0 | e c a |
| C, ties, large | a3 b1 c3 d2 e3 f1 g2 h3 i1 j2 | h3 c3 a3 e3 d2 g2 j2 i1 b1 f1 | all |
| D, hidden, large | a1 b0 c1 d0 e2 f1 g0 h1 i2 j0 k1 | i2 e2 c1 k1 a1 h1 f1 d0 j0 b0 g0 | i e c k a h f |
| H, filter after the sort | a2 b0 c2 d0 e2 f0 g2 h0 i2 | i2 c2 g2 a2 e2 d0 h0 b0 f0 | i c g a e |
| H', filter first (wrong) | a2 c2 e2 g2 i2 | c2 e2 g2 i2 a2 | c e g i a: differs from H |
| J, all equal, large | a1 b1 ... j1 (10 entries) | b c d j a g h i e f | all |
| K, at the cutoff | a0.0001 b0.0002 c0.0001 d0.00009 | b c a d | b c a |
| E, NaN | a8 b7 c6 d5 eNaN f4 g3 h2 i1 | f4 g3 h2 i1 eNaN a8 b7 c6 d5 | refused (section 4) |
| I, NaN pivot | a1 b2 c3 d4 eNaN f5 g6 h7 i8 | i8 h7 g6 f5 eNaN d4 c3 b2 a1 | refused (section 4) |
| F, NaN, small | a3 bNaN c1 d2 | bNaN a3 d2 c1 | refused (section 4) |

### 2.3 Unordered priorities (NaN)

Every comparison with a NaN is false. In the partition, the left cursor therefore **stops** at an entry whose
difference from the pivot is NaN, and so does the right cursor. A NaN entry is never "lower", so in a small range it
is never the one moved to the right. A NaN does more than end up somewhere in the list. As example E shows, a NaN
pivot splits the range without ordering it, and the result is **not descending**, even among the ordered entries:
4 to 1 come before 8 to 5. Filtering the NaN out afterwards does not repair that.

How the engine's compare treats an unordered result was read from the decompiled form only, not at the instruction
level: **UNVERIFIED**. A NaN priority needs NaN data (a NaN `BasePriority`, or opposite infinities summed). Section 4
refuses it rather than port this.

## 3. The displayed value, and `bShouldDisplay`

For each entry the walk appends, the card computes the base value B and the value V (NATIVE_CACHE_WEAPON_CARD_BODY.md
section 3.3, unchanged). Then:
- `bShouldDisplay` is **true when the entry has no modifier record**;
- otherwise it is true when the presentation's **displayed form of V**, computed as below, has a size of at least
  0.00000001.

The card asks for the displayed form in its "card" mode (one of three modes), with the "no minimum of 1" option set,
and with no context object.

**The conversion in the card's mode** uses fields of `AttributePresentationDefinition`, in this order:
1. **Zero snap.** Unless |V| is at least 0.00000001, V is taken as 0 and noted as "zero". A NaN fails that test, so a
   NaN V is snapped to 0 as well.
2. **Scale or remap:**
   - with `bDisplayAsPercentage`: V × 100, unless it is "zero";
   - without it: the **remap** (below), applied to the snapped V, "zero" included.
3. **Sign**, by `SignStyle` (s) and `bBiggerIsBetter`, on V as step 2 left it:
   - s = 3: V unchanged;
   - s = 4: V negated;
   - s = 1: V negated when V < 0; otherwise the card rule;
   - s = 2: V negated when V > 0; otherwise the card rule;
   - s = 0 or any other value: the card rule.

   **The card rule:** V is negated when `bBiggerIsBetter` is clear, and left unchanged when it is set.
   For s = 1 and s = 2, "otherwise" includes V = 0, and negating 0 changes nothing that follows.
4. **Inverse:** not in the card's mode.
5. **Rounding**, by a mode and a precision:
   - **The mode:** the presentation's `RoundingMode`, except in one case. When `bDisplayAsPercentage` is set,
     `RoundingMode` is `ATTRROUNDING_Float` (0) and `bDisplayPercentAsFloat` is clear, the mode is
     `ATTRROUNDING_IntRound` (1).
   - **The precision** `P` is `FloatPrecision`, taken as 0 when below 0 and as 10 when above 9.
   - **By mode**, with W the value after step 3:
     - 0, `Float`: `n / 10^P`, with `n = floor(10^P × W + 0.5)`;
     - 1, `IntRound`: `n = floor(W + 0.5)`;
     - 2, `IntFloor`: `n = floor(W)`;
     - 3, `IntCeil`: `n = ceil(W)`;
     - any other value: W unchanged, and the size test is on W itself.
   - **The integer:** n goes through a signed 32-bit integer. The engine converts twice the value, plus or minus a
     half, so the range limit applies to that doubled value (open question 3).
6. **No minimum of 1:** the card sets this option, so an n of 0 stays 0.

**Why the sign matters.** It runs before rounding, and rounding is not symmetric. A W of 0.2 shows under `IntCeil`
(n = 1) and hides under `IntFloor` (n = 0); −0.2 does the opposite. Under `IntRound` and `Float`, an exact half rounds
up: 0.5 gives 1, and −0.5 gives 0. So steps 3 and 5 must run in this order, on the signed value.

**Precision, all the way to the cutoff:**
- V and the four remap values are single precision. The remap's arithmetic runs in the engine's wider float, and its
  result is stored back to single precision. The rule: compute in double, then round to single.
- In modes 1 to 3, n is exact for any single-precision W inside the integer range; no intermediate rounding can move
  it.
- In mode 0, `10^P` is exact in single precision for P from 0 to 10, and the product `10^P × W` is exact in double and
  in the engine's wider float. If that float ran at single precision, a product within half a single-precision step
  of ±0.5 could round onto ±0.5 and change n. Whether it does is **UNVERIFIED** (open question 2), and section 4
  refuses that case.
- **The cutoff.** In modes 1 to 3 the size is |n|, which is 0 or at least 1, so the test is `n ≠ 0`. In mode 0 the
  size is |n| / 10^P: for P up to 7, any n ≠ 0 passes (at least 0.0000001). For P of 8 or more, a small |n| sits at or
  below the cutoff, where the division's precision decides. Section 4 refuses n ≠ 0 there; n = 0 hides.
- With an unknown mode, the size test is on W itself, in single precision, against 0.00000001.

**The remap** (step 2, without `bDisplayAsPercentage`):
- Without `bValueRemappingEnabled`, V is unchanged.
- With it, the four initialization-data fields of `RemappingData` are evaluated as attribute-initialization data
  (the port's `evaluate_init`, with no override source), in this order: `InputValueMn` as `a`, `InputValueMx` as `b`,
  `OutputValueMn` as `c`, `OutputValueMx` as `d`.
  - **The source** is the call's context object when `RemappingData.bUseContextOverride` is set. The card passes
    none, so the source is None.
  - Otherwise, the source is a global: the engine's first local player, through one of its fields. By fit that field
    is the local player's controller. **UNVERIFIED**, open question 1.
- The slope `s` is `(d − c) / (b − a)` when `a < b`, else 0.
- V maps to:
  - `c` when `V < a`;
  - `(V − a) × s + c` when `V < b`;
  - else `(b − a) × s + c`.

  The comparisons are between single-precision values. With a NaN among `a` to `d`, every comparison is false, so the
  last form applies.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces two of #521's refusals; nothing else in #521 changes.

**Where each check runs:**
- **The equip preflight** (write-free, before init) holds only checks of data shape that need no initialised weapon
  state: the presentation fields' types and the `RemappingData` struct's shape, refused as #521 refuses shapes today.
  This note does not move the preflight.
- **The card step** (after init): the entry list, the sort, the filter, V and the displayed form all depend on the
  initialised weapon. They are computed in full **before the card's first field write**.
  - A refusal found there keeps its error kind and record.
  - It takes the "stopped after partial writes: " prefix only when the heap's `committed_writes()`, the RNG's
    `draws()` or the store's `load_transitions()` moved after the native began. These are #518's counters, as #521
    uses them.
  - The remap's evaluations can load definitions or draw from the RNG, so a refusal after them may carry the prefix.

**The changes:**
1. **The order:** drop "the card order of equal-priority entries". Build the entry list in first-insertion order, as
   #521 does. Sort the **whole** list by section 2.1, then filter with "at least 0.0001". The sort writes nothing
   script-visible: the list is the native's own.
2. **The display:** drop "the presentation's displayed value". For an appended entry with a record, compute section
   3's displayed form, sign step included, and set `bShouldDisplay` from it.
3. **Typed refusals:** each is `VmError::NotYet` of kind `NotRead`, with this note as the record, raised in the card
   step before any card-field write.
   - Any entry's priority is NaN (checked over all entries, before the sort): "CacheWeaponCard: an unordered card
     priority".
   - The remap is needed (`bValueRemappingEnabled` set, `bDisplayAsPercentage` clear) and `bUseContextOverride` is
     clear: "CacheWeaponCard: the remap's default context (the first local player's controller, by fit)".
     Alternatively, a hub ruling lets the port use its world's first local player controller, labelled UNVERIFIED.
   - In modes 0 to 3, the doubled value the engine converts is NaN, infinite or outside the signed 32-bit range:
     "CacheWeaponCard: a displayed value outside the integer range". That value is `2 × X ± 0.5`, computed in double,
     where X is W, or `10^P × W` in mode 0.
   - In mode 0, `10^P × W`, rounded to single, lands on the other side of ±0.5 than the exact product (giving a
     different n): "CacheWeaponCard: the display rounding's intermediate precision".
   - In mode 0, with P of 8 or more and n ≠ 0: "CacheWeaponCard: a displayed value at the display cutoff".
   - The remap's four evaluations refuse as the attribute layer refuses (typed, passed through unchanged).
4. **Unchanged:** "the card's slot count" (another lane), and "the card visit of a definition that is neither a weapon
   type nor a weapon part" (not on link 8's path).
5. **Writes:** none new. The conversion and the sort only read.

**Reference** keeps the stub policy.

## Open questions

1. **UNVERIFIED:** the remap's default source, that is, which field of the engine's first local player it takes (by
   fit, the controller). The card reaches it only through a presentation with remapping enabled and no context
   override.
2. **UNVERIFIED:** the precision of the engine's wider float in the rounding's product and in the remap. Section 3
   gives the double-precision rule. Section 4 refuses the one rounding case where running at single precision would
   change n. The remap is not refused: a last-bit difference there matters only at a rounding boundary, and no
   refusal is proposed for it.
3. **UNVERIFIED:** what the integer conversion gives for a doubled value that is NaN, infinite or out of range.
   Refused in section 4.
4. **UNVERIFIED:** how the sort's compare handles unordered results (section 2.3). Refused in section 4.
5. Unchanged from NATIVE_CACHE_WEAPON_CARD_BODY.md: the slot count's definition (another lane), and the class test of
   the presentation lookup.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. **The tie order.** Find a weapon whose card has two entries of equal priority. Read `WeaponCardModifierStats`
   after the drop, and compare it with section 2.1 run on the visit order. A weapon with more than 8 card candidates
   also exercises the large-range rule.
2. **The hidden entries.** On a weapon with a tie and an entry below the 0.0001 threshold, check that the tie's order
   matches the sort run over all entries.
3. **`bShouldDisplay` and the sign.** On an entry with a modifier record, a small value and `bBiggerIsBetter` clear,
   compare an `IntFloor` presentation with an `IntCeil` one: the negation decides which one shows.
4. **The remap.** On a presentation with `bValueRemappingEnabled`, read the displayed text for two values and fit the
   linear map. Compare it with the four fields evaluated for the local player's controller.
