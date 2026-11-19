# Native rules of a pawn's friendliness test: Pawn.IsFriendly on link 9 (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-IS-FRIENDLY (decision 2026-11-19-55). The natives were read from the game
executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock), and the script that calls them from
the installed packages with `research/script_disasm.py`. Written in our own words, as behaviour only: no decompiler
output, pseudo-code, quoted script statements, addresses, offsets or call structure is reproduced here. Field names
come from the cooked packages. **Every rule is UNVERIFIED in the running game.** The last section lists the live
observations that would confirm them.

**Clean room.** This lane implements nothing. `Pawn.IsFriendly` stays unbound in Off until a fresh implementer codes
it from this note alone.

**Why it matters.** With sub-object instancing (decision 2026-11-19-44), the Off Phaselock cast of slice link 9 gets
past `GetSMComponent` and stops at the unbound `Pawn.IsFriendly`. The script asks it in `LiftActionSkill`'s
can-be-phaselocked test (NATIVE_SPECIAL_MOVE_PLAY.md section 0): the caster's pawn is asked whether it is friendly to
the target, and a friendly target cannot be phaselocked.

This note extends NATIVE_COMBAT_GROUNDWORK.md section 6, which gave the generic opinion rule and a low-confidence
reading of the `WillowPawn` layer. The generic rule is confirmed here; the layer is now read and named.

## Summary

| Piece | Script declaration (from the package) | Confidence | Status |
|---|---|---|---|
| `Pawn.IsFriendly(Object)` | native, not redeclared by any class on link 9's path | high | UNVERIFIED |
| `Pawn.GetOpinion(Object)` | native, the generic rule | high | UNVERIFIED |
| `WillowPawn.GetOpinion(Object)` | native, the charm layer over the generic rule | high | UNVERIFIED |

- `IsFriendly` is "my opinion of the other is Friendly" (the opinion enum: Enemy 0, Neutral 1, Friendly 2). It asks
  the pawn's own opinion rule, so a class that replaces `GetOpinion` changes `IsFriendly` with it.
- On link 9 both pawns are `WillowPawn`s (Maya's `WillowPlayerPawn`, a `WillowAIPawn` target). Neither class, nor
  `GearboxPawn`, replaces `IsFriendly`. `WillowPawn` replaces `GetOpinion`, and its subclasses keep that replacement.

## 1. IsFriendly(Other)

True exactly when the pawn's opinion of `Other` (section 2 for a `WillowPawn`, section 3 for any other pawn) is
Friendly. Nothing else is read.

## 2. WillowPawn.GetOpinion(Other): the charm layer

In this order, first match wins:
1. **`Other` is this pawn's `CharmedChild`** (set, and the same object): **Enemy**.
2. **`Other` is this pawn's `CharmedParent`** (set, and the same object): **Friendly**.
3. **This pawn has no `CharmedParent`, `Other` is a `WillowPawn` with a `CharmedParent`, and that `CharmedParent` is
   friendly to this pawn** (its own `IsFriendly` with this pawn as the argument, by these same rules): **Enemy**.
4. Otherwise **the generic rule** (section 3).

What the two fields mean in play (who is whose child or parent when Maya's skills charm a target) was not read here
(UNVERIFIED). Without a charm, both fields are empty on every pawn, and the answer is the generic rule.

## 3. Pawn.GetOpinion(Other): the generic rule

As in NATIVE_COMBAT_GROUNDWORK.md section 6, confirmed here:
1. **No `Allegiance` on this pawn:** Neutral. For a `WillowPawn` this applies only once section 2 has reached its
   step 4: a charm match in steps 1 to 3 answers first, with or without an allegiance.
2. **The other side's allegiance B:**
   - when `Other` implements the targetable interface (every pawn does), B is what it reports as its allegiance. For
     every pawn class on link 9 that is its `Allegiance` field;
   - otherwise, when `Other` is itself a `PawnAllegiance`, B is `Other`;
   - otherwise there is no B.
3. **No B:** Neutral.
4. **B is this pawn's own allegiance A** (the same object): A's `SelfOpinion`.
5. Else the first entry of A's `MyOpinions` whose `Allegiance` is B: that entry's `Opinion`.
6. Else the first entry of B's `OtherOpinions` whose `Allegiance` is A: that entry's `Opinion`.
7. Else B's `ForcedOtherOpinion` when B has `bForceAllOtherOpinions`, otherwise A's `DefaultOpinion`.

Allegiances are compared as objects: the same object, not an equal one.

## 4. What it answers for a pawn, a controller and None

- **Another pawn:** sections 2 and 3, with that pawn's `Allegiance`.
- **This pawn itself:** with an `Allegiance`, that allegiance's `SelfOpinion`; without one, Neutral (section 3 step
  1). The charm steps do not match, as long as no charm field points at the pawn itself (UNVERIFIED that none ever
  does).
- **A dead or dying pawn, on either side:** the rules above read no health, death or liveness state, so a dead pawn
  is answered like a live one from its current `Allegiance`, `CharmedChild` and `CharmedParent`. Whether death
  changes those fields first (NATIVE_DAMAGE_DEATH.md lists allegiance bookkeeping at death) is UNVERIFIED.
  `CanPhaseLockTarget` tests liveness separately (`IsAliveAndWell`).
- **A controller:** `Controller` does not implement the targetable interface and is not a `PawnAllegiance`, so there
  is no B: **Neutral**, so `IsFriendly` is false. The controller is **not** read through its pawn.
- **None:** no B: Neutral, so false.
- **A `PawnAllegiance` object:** the generic rule with that object as B.

## 5. For an implementer, in the port's terms

- `IsFriendly` = opinion is Friendly, with the opinion from the pawn's class: section 2 for `WillowPawn` and its
  subclasses, section 3 for any other pawn.
- The third step of section 2 asks the other pawn's `CharmedParent` for its friendliness toward this pawn. That is a
  second `IsFriendly` call, by these same rules.
- Today's Reference binding in `ow-combat` (`hit.rs`) differs from this note in two ways an implementer should not
  copy into Off: it reads a controller through its pawn (section 4: the game answers Neutral), and it has no charm
  layer. It also treats two allegiances with the same path as the same, where the game compares objects; with one
  object per path that gives the same answer.
- Not in scope and not read: `WillowAIPawn`'s own `IsEnemy` and `IsNeutral`. That class replaces those two tests, so
  on an AI pawn they need not equal "opinion is Enemy" or "opinion is Neutral". `IsFriendly` is not replaced there.

## Open questions

1. **UNVERIFIED:** which `Allegiance` Maya's pawn and link 9's target carry in the installed data (a data question,
   already open in NATIVE_COMBAT_GROUNDWORK.md section 6). With none on Maya's pawn, every answer that reaches the
   generic rule is Neutral. The charm steps of section 2 still answer first when one matches.
2. **UNVERIFIED:** what sets `CharmedChild` and `CharmedParent`, and on which pawn, when a target is charmed.
3. **UNVERIFIED:** whether death changes a pawn's `Allegiance` or charm fields before a later `IsFriendly` reads
   them (section 4).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Cast Phaselock at a bullymong and hook `Pawn.IsFriendly`: record the caller (Maya's pawn), the argument (the
  target) and the result. Read both pawns' `Allegiance`, `CharmedChild` and `CharmedParent`.
- Self target: call `IsFriendly` and `GetOpinion` on a pawn with itself as the argument, once with its `Allegiance`
  set and once with it cleared. Expected: `GetOpinion` returns `SelfOpinion` with an allegiance and Neutral without
  one, and `IsFriendly` returns true exactly when that opinion is Friendly (section 4).
- Death: call `GetOpinion` of Maya's pawn on a bullymong just before and just after it dies, and record both pawns'
  `Allegiance`, `CharmedChild` and `CharmedParent` each time (open question 3).
- Charm transitions: hook the writes of `CharmedChild` and `CharmedParent` (and whatever script or native sets them
  during a charm), and record both fields on every pawn involved before and after each change (open question 2).
- Three pawns for step 3 of section 2: a querying pawn Q, a target T and T's `CharmedParent` P. T is neither Q's
  `CharmedChild` nor Q's `CharmedParent`, and Q's generic opinion of T is Neutral to begin with (read it before the
  charm), so an Enemy answer cannot come from the generic rule. Vary P's opinion of Q (Friendly, then not Friendly)
  and whether Q has a `CharmedParent`. Record `Q.GetOpinion(T)` each time. The expected answer is Enemy only when P
  is friendly to Q and Q has no `CharmedParent`; otherwise the Neutral baseline of the next rule.
- Call `IsFriendly` on Maya's pawn with her own controller as the argument, and with None. Both should be false.
- After a charm, read both pawns' `CharmedChild` and `CharmedParent`, and their `GetOpinion` of each other.
