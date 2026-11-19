# CacheWeaponCard's slot count, and its slots past 0 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-CARD-SLOT-COUNT (decision 2026-11-19-NNN, a placeholder),
for slice link 8. A companion to NATIVE_CACHE_WEAPON_CARD.md (the outline, section 3, step 3) and
NATIVE_CACHE_WEAPON_CARD_BODY.md (PR #484's body note, on `research/rust-cache-weapon-card-body`; its open question 1).
That note is not on main yet, so this is a sibling note rather than a new section of it.
- **Sources:**
  - the game executable, read with Ghidra (`tools/ghidra/run.ps1` read-only queries plus a local, uncommitted
    call-site listing script, one headless run at a time under the machine lock);
  - field positions from `tools/ghidra/class_layout.py`;
  - the script function list (`tools/ghidra/script_natives.py`).
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure. Raw output stays
  under ignored `local/analysis/cardslot/`.
- **No live session** was run. **Every rule here is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. A fresh implementer codes the Off answer from this note.

## 1. The question

With #521 (the body note's implementation, stacked on #506), link 8 passes 1 of its 5 steps. It stops at `init` with
"stopped after partial writes: WillowWeapon.CacheWeaponCard: the card's slot count". The body note leaves the count's
definition unread (its open question 1). The installed weapon has an activated slot past slot 0, so the count can
change the result, and the implementation refuses.

## 2. Where the count comes from

- **The definition:** the weapon's own **`DefinitionData.WeaponTypeDefinition`**. That is the same object whose
  absence ends the native early (the outline's step 1: no weapon type, nothing more is done). It is not the balance
  definition, not a part, and not the inventory's own `InventoryDefinition`.
- **The read:** that definition's `AttributeSlotMaxActivated`, a field `WillowInventoryDefinition` declares (so every
  `WeaponTypeDefinition` has it), through the engine's own slot-count routine:
  - a stored value from 1 to 19 is the count as it is;
  - anything else (0, negative, or above 19) counts as **19**, the size of the weapon's `AttributeSlots`.
- **The same routine** answers the script native `WillowInventoryDefinition.GetAttributeSlotMaxActivated()`. So the
  count is exactly that getter's answer on the weapon type, and that clamp is now read from the engine routine itself.
  That settles the outline's note that the getter clamping was only a reading (NATIVE_INDEX.md).
- **Inputs:** only the weapon type and its one field. No attribute evaluation, no context, no random draw.
- **Writes:** none, for the count itself.
- **When:** after the native clears `WeaponCardModifierStats` (the outline's step 2), before the first slot is
  visited. So the count is read after the native's first write. It reads nothing that step 2 changed.

**Class defaults.** Cooked data omits a field that holds its class default. So a weapon type with no stored
`AttributeSlotMaxActivated` takes its class default, and a default of 0 gives 19 by the clamp. The implementer reads it
through the class defaults, never as 0 by absence.

## 3. The slots past 0

There is **no special case for slot 0.** The native visits the weapon's `AttributeSlots` from index 0 up to the count
minus 1, in index order, and runs the same step on each. That step is the outline's step 3 (NATIVE_CACHE_WEAPON_CARD.md
section 3), confirmed on every slot of the loop:
1. a slot whose `bActivated` is clear is skipped, and nothing is written for it;
2. otherwise the presentations of its `AttributeToModify` and its `ConstraintAttribute` are looked up, through the
   body note's section 2 (the damage-type replacement included);
3. when the slot's `bIncludeInModifierText` is set:
   - if the `AttributeToModify` presentation is None, the slot's `bIncludeInModifierText` is **cleared**, a write to
     the slot;
   - either way one entry is appended to `WeaponCardModifierStats`: `AttributePresentation` = that presentation
     (possibly None), `OptionalConstraintPresentation` = the constraint's presentation (possibly None),
     `ModifierValue` = the slot's `ComputedModifierValue`, `bShouldDisplay` = true;
   - the count of entries appended here (the outline's S) grows by one;
4. a slot with `bIncludeInModifierText` clear appends nothing and writes nothing.

Slots at or past the count are never read, whatever their flags.

**Script-visible state.** The reads (`AttributeSlotMaxActivated`, the slot fields) and the writes
(`WeaponCardModifierStats` and the cleared `bIncludeInModifierText`) are all script-declared fields. The slot-count
routine is the one behind the script native `GetAttributeSlotMaxActivated`.

## 4. For an implementer: the Off answer

**Off only; Reference is unchanged.** The hub's rulings apply:
- preflight what is cheap and certain;
- a lazy definition load counts as a write;
- a `NotYet` after a write is terminal, with its prefix, kind and record;
- grade by the `VmError` variant and the `NotYet` kind.

1. **Replace the slot-count refusal** ("WillowWeapon.CacheWeaponCard: the card's slot count") with the count of
   section 2: the weapon type's `AttributeSlotMaxActivated`, read through its class defaults when not stored, then
   clamped (1 to 19 as is, else 19). It is the same rule as the port's `GetAttributeSlotMaxActivated` binding, so the
   implementer may call one shared function for both rather than write a second copy.
2. **Where it is read:** with the rest of the card's call-time computation (the body note's section 4, step 2),
   before the native's own first write. Reading it early changes nothing: step 2's clear does not touch it.
3. **The weapon type** is the object the definition read already found. Under the lazy-load rule, a weapon type that
   is still an unloaded definition handle must not be loaded here. In that case the answer is
   `NotYet { NotRead }`, "WillowWeapon.CacheWeaponCard: the weapon type <object> is an unloaded definition handle",
   record this note. Expected not to occur on link 8, since the weapon type is read earlier in the init.
4. **The schema** (in the equip driver's preflight, with the body note's card schema): `WeaponTypeDefinition`
   (through `WillowInventoryDefinition`) declares `AttributeSlotMaxActivated` as an int. A missing or mistyped field is
   a `VmError::Script` naming it.
5. **Then the slots** 0 to count−1 run section 3's step, unchanged from slot 0's. The body note's other refusals (the
   tie order and the displayed value, its section 4, step 3) stay as they are.

**What link 8 meets next.** With the count read, the slot step no longer refuses. The card then runs on to the body
note's sections 3.1 to 3.3. The next stop is one of that note's two remaining refusals, if an appended entry ties on
priority or has a modifier record:
- "WillowWeapon.CacheWeaponCard: the card order of equal-priority entries";
- "WillowWeapon.CacheWeaponCard: the presentation's displayed value".

Otherwise the card completes, and link 8 goes past `init`. Which of these the installed weapon meets is for the gate
run to show; this note does not assume it.

## 5. Open questions

1. A subclass of `WillowWeapon` with its own card body was not looked at (the outline's open question 5 stands).
2. The installed weapon type's stored `AttributeSlotMaxActivated` and its class default were not read here. The gate
   run shows them through the slots the card lists.
3. The body note's open questions 2 to 5 stand (the display conversion, the tie order, the class test).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The definition.** On a weapon whose type's `AttributeSlotMaxActivated` (read with `GetAttributeSlotMaxActivated`
   on `DefinitionData.WeaponTypeDefinition`) is smaller than the number of activated slots, count the slot entries at
   the head of `WeaponCardModifierStats` after the drop. Expect only the slots below the type's count.
2. **The clamp.** On a weapon type whose stored value is 0 or above 19, expect every activated slot of the 19 to be
   listed.
3. **A slot past 0.** On a weapon with slot 1 activated and `bIncludeInModifierText` set, expect its entry right after
   slot 0's, with `bShouldDisplay` true and `ModifierValue` equal to the slot's `ComputedModifierValue`.
