# Native rules of a weapon's name parts and card cache: ChooseRandomNameParts and CacheWeaponCard (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-CACHE-WEAPON-CARD (decision 2026-11-19-96).
- **Script first:** `WillowWeapon.InitializeInternal` was read in the installed packages with
  `research/script_disasm.py`. Its jump targets were converted to statement starts in the in-memory form
  (SCRIPT_BYTECODE_DISASM.md).
- **The two natives** were then read from the game executable with Ghidra (`tools/ghidra/run.ps1`, one headless run
  at a time under the machine lock).
- **Field names** come from the cooked packages (`tools/ghidra/class_layout.py`).
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here.
  Raw output stays under ignored `local/analysis/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.** The last section lists the bl-sdk
  observations that would settle each one.

**Clean room.** This lane implements nothing. Both natives stay unbound (or `NotYet`) in Off until a fresh implementer
codes them from this note alone.

**Implemented in Off** by lane IMPL-CACHE-WEAPON-CARD (decision 2026-11-19-118, placeholder number), from this note
alone: `ChooseRandomNameParts` in full, `CacheWeaponCard`'s call-time `NotRead`, and the equip driver's preflight of
section 5.1. Still UNVERIFIED in the running game.

**Why it matters.** Slice link 8 (equip): once `ApplyInternalSlotEffectModifiers` is bound (lane H6's note, decision
-87, #471), a weapon's init (`InitializeInternal`, run by its spawn) reaches the name-part branch's
`ChooseRandomNameParts`, or `CacheWeaponCard` when the branch is skipped. Neither native is bound.

## Summary

| Native | Does | Writes | Confidence | Status |
|---|---|---|---|---|
| `WillowWeapon.ChooseRandomNameParts()` | picks the prefix and the title from the weapon type's and the parts' name-part lists, by level range, priority and expressions; no random draw | `DefinitionData.PrefixPartDefinition`, `DefinitionData.TitlePartDefinition` | high | UNVERIFIED |
| `WillowWeapon.CacheWeaponCard()` | rebuilds the weapon card's modifier list from the activated attribute slots and the card attributes of the type and parts; publishes up to 5 values for replication on the authority | `WeaponCardModifierStats`; a slot's `bIncludeInModifierText`; `ReplicatedWeaponCardModifierValues`; in the game, possibly the globals definition's first load (never in Off) | medium (outline), low (identity of the body) | UNVERIFIED |

## 1. Where they are called: the script of InitializeInternal

`WillowWeapon.InitializeInternal(bForceSelectNameParts)` runs, after the parts and the first slot effects
(NATIVE_INVENTORY_VALUE.md section 5):
1. `RecomputeNormalizedDamageValues` (first call).
2. **The name-part branch**, taken when `bSelectRandomPartsOnInitialization` is true **or** the parameter
   `bForceSelectNameParts` is true:
   1. `ChooseRandomNameParts`;
   2. `ComputeValueOfParts` (its second call);
   3. `InitializeAttributeSlotsForNameParts`;
   4. `ApplyInternalSlotEffectModifiers`, with the flag false and the weapon type definition;
   5. `ApplyNamePartWeaponAttributeEffects` (script);
   6. `RecomputeNormalizedDamageValues` (second call).
3. **Always**, whether the branch ran or not:
   1. `ComputeValueOfParts` (its third call). The branch's skip lands here, so this call runs on both paths;
   2. `CacheWeaponCard`;
   3. `RecomputeAttributeBaseValues`, then `ReplicatedZoomedEndFOVBaseValue` is set from `ZoomedEndFOVBaseValue`,
      then `ClientInitializeFromWeaponTypeDefinition` (script) and `GetFiringModeDefinition`.

So on a path without the branch, the next unread native after `ApplyInternalSlotEffectModifiers` is `CacheWeaponCard`,
reached after `ComputeValueOfParts`. NATIVE_INVENTORY_VALUE.md section 5 lists "`RecomputeNormalizedDamageValues` and
`ComputeValueOfParts` (its third call) run again". Read with the converted jump target, the second
`RecomputeNormalizedDamageValues` belongs to the branch, and the third `ComputeValueOfParts` does not.

`CacheWeaponCard` is an overridable native: what runs is the weapon class's own implementation, so a subclass of
`WillowWeapon` could replace it (none was looked for).

## 2. ChooseRandomNameParts

**Reads:** `DefinitionData.WeaponTypeDefinition` and its `PrefixList` and `TitleList`; the nine part slots of
`DefinitionData` and each part's `PrefixList` and `TitleList`; on each name part, `MinExpLevelRequirement`,
`MaxExpLevelRequirement`, `Priority` and `Expressions`; the weapon's `ExpLevel`.

**Steps:**
1. With no `WeaponTypeDefinition` (None), it does nothing: both name-part fields keep what they hold.
2. **The best entry of one list** (used for every list below), with the weapon as the expressions' context:
   - the entries are walked in order, and a None entry is skipped;
   - an entry qualifies when `MinExpLevelRequirement` <= the weapon's `ExpLevel` <= `MaxExpLevelRequirement` (both
     ends included), its `Priority` is above 0, and its `Expressions` are empty or all hold;
   - the expressions are evaluated in And mode (the first false one ends the check) with the weapon as context and no
     override context. That is `AttributeExpression.EvaluateExpressions`, NATIVE_ATTRIBUTES.md section 11;
   - a qualifying entry replaces the best so far when there is none yet, or when the best's `Priority` is at most the
     entry's. So on equal priority the later entry wins;
   - the result is the best entry, or None when none qualifies.
   - The priorities are compared as floats.
3. The prefix starts as the best of the type's `PrefixList`, and the title as the best of its `TitleList`.
4. **The parts**, in this slot order: `BodyPartDefinition`, `GripPartDefinition`, `BarrelPartDefinition`,
   `SightPartDefinition`, `StockPartDefinition`, `ElementalPartDefinition`, `Accessory1PartDefinition`,
   `Accessory2PartDefinition`, `MaterialPartDefinition`. A None slot is skipped. The name-part slots themselves
   (`PrefixPartDefinition`, `TitlePartDefinition`) are not walked. For each part:
   - its best prefix (step 2 on its `PrefixList`) replaces the current prefix when the current one is None, or when
     the part's best is not None and the current one's `Priority` is at most the part's best's;
   - the same for the title, from its `TitleList`.
   - So a later slot wins a priority tie with an earlier slot or with the type. A part whose list has no qualifying
     entry never clears a pick.
5. **Writes** the final prefix into `DefinitionData.PrefixPartDefinition` and the final title into
   `DefinitionData.TitlePartDefinition`. Both are written, None included, once the type is not None.
   - Nothing is written before the end: the picks are computed first.
   - No event, no other field, no random draw, no definition load.

This agrees with NATIVE_WEAPON_RULES.md section 5, where the rule was fitted to observed cards. It adds what the cards
could not show: a None type leaves the fields alone; a part with no qualifying entry keeps the earlier pick; the
comparison is "at most", so later wins ties; and the expressions run in And mode with the weapon as context.

## 3. CacheWeaponCard

**Identity of the body (low confidence).** Because the native is overridable, the script name does not lead straight
to one body, and the project's class-table lookup could not resolve `WillowWeapon`'s implementation. The body described
here was chosen by its behaviour: it is the one function in the executable that
- rebuilds `WeaponCardModifierStats` and fills `ReplicatedWeaponCardModifierValues`;
- walks the weapon's attribute slots and its parts;
- has no fixed caller, which fits an overridable native.

Confirming that it is the `CacheWeaponCard` slot is open question 1.

**Reads:** the globals (`GetWillowGlobals`) and the globals definition (`GetGlobalsDefinition`), including its
`WeaponAttributePresentationOverride`; `DefinitionData.WeaponTypeDefinition` and `DefinitionData.BalanceDefinition`
(its `DlcItemSet`); the weapon's `AttributeSlots` (the first N, below) with each slot's `bActivated`,
`bIncludeInModifierText`, `AttributeToModify`, `ConstraintAttribute` and `ComputedModifierValue`; the elemental part's
`CustomDamageTypeDefinition`, else the type's `DefaultDamageTypeDefinition`, and that damage type's
`WeaponCardPresentations`; `Role`.

**Steps, as read:**
1. **The globals definition:** the globals object, then its definition through `GetGlobalsDefinition`. In the game
   that call can be the definition's first load (NATIVE_WILLOW_GLOBALS.md section 4). That load writes script-visible
   state: it stores `TheGlobalsDefinition` and numbers the challenge list. Off never runs it; Off's getter answers
   `NotRead` while the field is unset (NATIVE_WILLOW_GLOBALS.md section 6.3, step 3). The port's loading of a package
   or a definition into its handle is not meant here: that is not a write (NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md
   section 3). With no globals, no definition, or no `WeaponTypeDefinition`, the native does nothing more.
2. **Clear:** `WeaponCardModifierStats` is emptied (its storage released).
3. **The slots:** a slot count is read and clamped. Which field or getter supplies it was **not read** (the reading
   did not tie it to a name). A value from 1 to 19 is used as it is; anything else (0, negative, or above 19) counts
   as 19, the size of `AttributeSlots`. That clamp differs from `ApplyInternalSlotEffectModifiers`' Off assumption B1
   (NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md section 3), where a count of 0 or less visits no slot and only a count above
   the dimension stops at 19. For each of those slots, in order:
   - a slot that is not `bActivated` is skipped;
   - the presentation of its `AttributeToModify` and of its `ConstraintAttribute` are looked up (step 4);
   - when the slot is `bIncludeInModifierText`: if the presentation of `AttributeToModify` is None, the slot's
     `bIncludeInModifierText` is **cleared** (a write to the slot). Either way an entry is appended to
     `WeaponCardModifierStats`: `AttributePresentation` = that presentation (possibly None),
     `OptionalConstraintPresentation` = the constraint's, `ModifierValue` = the slot's `ComputedModifierValue`,
     `bShouldDisplay` = true;
   - the number of entries added here is kept (call it S).
4. **A presentation lookup** for an attribute:
   - when the balance definition's `DlcItemSet` is set, two presentation lists of that set are tried first, in a
     fixed order;
   - otherwise, or when neither answers, the lookup uses the globals definition's
     `WeaponAttributePresentationOverride`;
   - a found presentation of a certain kind is then replaced by a damage-type-specific one, using the damage type
     above.
   The list lookups, the "certain kind" test and the replacement were **not read** (open question 2).
5. **Further card entries (not read):**
   - a further step uses the weapon and the globals definition;
   - then the weapon type definition and every part, the name parts included, each get a chance to contribute,
     through behaviour each definition class may supply for itself;
   - then each presentation in the damage type's `WeaponCardPresentations` is processed.
   These presumably add the type's and parts' card attributes (for example `WeaponCardAttributes`) to
   `WeaponCardModifierStats`. That is an inference: their bodies were not read (open question 3).
6. **Ordering (not read):** `WeaponCardModifierStats` is then processed by a routine that weighs each entry's value.
   It is probably a sort or a filter; it was not read (open question 3).
7. **Replication:** only when `Role` is 3 (authority):
   - the entries from index S onward are walked in order, so the slot entries of step 3 are excluded;
   - each entry with `bShouldDisplay` fills the next of the 5 `ReplicatedWeaponCardModifierValues`, with
     `ModifierValue` = the entry's value and `ModifierStatIndex` = the entry's index in `WeaponCardModifierStats`;
   - the walk stops after 5 have been filled.
   - Entries not reached are left as they were. In particular, a call that fills fewer than 5 does not clear the
     remaining ones; that is the reading, and it is UNVERIFIED. With any other role, nothing is written there.

## 4. The BulletSpeed weapon (lane H6's report)

H6 reported that a weapon with a "BulletSpeed" part stops before these natives: an attribute names a property that
the installed `WillowWeapon` does not declare. **That is a real gap of the installed data, not a fixture problem.**

**Which weapon carries which part.**
- **Link 8's weapon** (`EQUIP_WEAPON` in `crates/ow-cli/src/cmd/slice_gate.rs`) is a Bandit assault rifle: Bandit
  barrel, body and material, Jakobs grip and stock, no sight, the prefix `Prefix_Grip_Jakobs` and a Bandit title. It
  has no accessory and no BulletSpeed part, so it does not meet this gap.
- **The stopping weapon** was another one, from H6's own probe. NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md section 4 calls
  it a Jakobs sniper rifle that stops in `ApplyAllWeaponAttributeEffects`. **Mismatch, flagged:** the BulletSpeed
  accessory found here, `AR_Accessory_Box_BulletSpeed`, is an assault-rifle accessory, and the reviewer's local census
  puts it on a Jakobs assault rifle. So "sniper" is probably a misnaming. This lane did not re-run that census, and
  it stays UNVERIFIED.
- **The prefix is not the cause of that stop** on a weapon whose name parts are picked at random: `ChooseRandomNameParts`
  runs after `ApplyAllWeaponAttributeEffects` (section 1). Whether `ApplyAllWeaponAttributeEffects` walks a prefix that
  the definition data already holds was not read here.
- NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md's open question 4 (which attribute names `BulletSpeed`, and on which class) is
  answered below, as far as the packages go.
- The installed attribute `D_Attributes.WeaponBullet.WeaponBulletSpeed` resolves its context through a
  `WeaponAttributeContextResolver` (the weapon), and its value through an `ObjectPropertyAttributeValueResolver` whose
  `PropertyName` is `BulletSpeed`.
- No class in the installed `WillowGame`, `Engine` or `GearboxFramework` packages declares a property named
  `BulletSpeed`: their name tables do not hold the name. `WillowWeapon` declares `ProjectileSpeedMultiplier`, with its
  base value and modifier stack, instead.
- The assault-rifle accessory `AR_Accessory_Box_BulletSpeed` and the six `Prefix_Box_BulletSpeed` name parts (the
  generic list and five manufacturer lists) ship with the game and use that attribute.
- So the game itself meets a resolver whose property lookup fails. NATIVE_ATTRIBUTES.md section 3 says the resolver
  looks the property up by name on the context's class and caches it. **What it does when the lookup fails** (most
  likely a value of 0, and add or remove answering false, so the modifier has no effect) **was not read.**
- **Off answer: the existing `NotRead` stays; no change.** It is `ow-attr`'s `value_resolvers/object_property.rs`
  `target()`, which answers `NotYet { NotRead }`, "ObjectPropertyAttributeValueResolver on a <class> context, which
  has no property <name>". That is the right answer until the rule is read. Inventing a
  `BulletSpeed` field in a fixture, or mapping it to `ProjectileSpeedMultiplier`, would be a fitted rule.
- For the slice, choose a link 8 weapon without these parts, or read the rule (open question 4).

## 5. For an implementer: the Off answers, step by step

These follow the hub's rulings:
- preflight what is cheap and certain before any write;
- after the run's first write, a refusal keeps its typed outcome. A `NotYet` stays `VmError::NotYet` with its kind
  and record, its text gains the "stopped after partial writes" prefix, and every write already made stays (no Off
  rollback). Other errors pass through unchanged;
- the game's first load of the globals definition writes script-visible state (it stores `TheGlobalsDefinition` and
  numbers the challenge list, NATIVE_WILLOW_GLOBALS.md section 4), and Off never runs it: Off's getter answers
  `NotRead` while the field is unset (that note's section 6.3, step 3). The port's package and definition loads are
  not writes (NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md section 3);
- the gate grades by the `VmError` variant, never by text.

Both natives run inside the spawn's `InitializeInternal`, after the weapon was made and after the parts and the first
slot effects were written. So every call-time `NotYet` below carries the prefix.

### 5.1 Preflight (the equip driver, before the weapon is made)

**First, the CacheWeaponCard refusal.** It is cheap and certain: every path through `InitializeInternal` reaches the
native (section 1), and Off answers `NotRead` whatever the data (section 5.3). So the equip driver answers it here,
before the weapon is made: `VmError::NotYet`, kind `NotRead`, the text of section 5.3, record this note, **with no
prefix**, since nothing has been written. This is the preflight rule above, applied as NATIVE_WILLOW_GLOBALS.md
section 6.2 applies it to the pickup drivers.
- The refusal names `CacheWeaponCard` even where a call-time stop earlier in the init would have come first (an
  expression `NotYet`, or the BulletSpeed lookup of section 4). The run cannot finish on either path, so the outcome
  is the same variant; the gate row names the preflight's stop instead of the earlier one.
- The native's own call-time answer (section 5.3) stays, for a run that skips this preflight.
- When `CacheWeaponCard` is read, this refusal goes, and the globals preflight of section 5.3, step 1 replaces it.

**Then the name-part schema,** for `ChooseRandomNameParts`. It is checked now so that it holds once the refusal above
is gone. On the declared classes, checked once, each failure a `VmError::Script` naming the field:
- `WillowWeapon.DefinitionData` is a `WeaponDefinitionData` struct with object fields `WeaponTypeDefinition`, the nine
  part slots of section 2, `PrefixPartDefinition` and `TitlePartDefinition` (the last two typed
  `WeaponNamePartDefinition`);
- `PrefixList` and `TitleList` on `WeaponTypeDefinition` and on `WeaponPartDefinition` are dynamic arrays of
  `WeaponNamePartDefinition` references;
- on `WeaponNamePartDefinition`: `MinExpLevelRequirement` and `MaxExpLevelRequirement` are ints, `Priority` is a float
  and `Expressions` is a dynamic array of `AttributeExpressionData`;
- `ExpLevel` on the weapon is an int.

**Name-part class defaults.** Cooked data omits a field that holds its class default, so a name part with no stored
value for one of these fields takes the default of `WeaponNamePartDefinition`: `MinExpLevelRequirement` 1,
`MaxExpLevelRequirement` 100, `Priority` 1 (NATIVE_WEAPON_RULES.md section 5). The implementer reads the field through
the class defaults, not as 0.

### 5.2 ChooseRandomNameParts at call time

The native can be implemented in Off as section 2 says.
1. Read `WeaponTypeDefinition`. If it is None, return with no write.
2. Compute both picks (section 2, steps 2 to 4), writing nothing.
   - Each expression is evaluated by the port's `EvaluateExpressions` (NATIVE_ATTRIBUTES.md section 11).
   - A `NotYet` from an expression returns as it is, prefixed (the run has written before). The native has written
     nothing itself at that point.
3. Write both fields.

There are no `NotYet`s of its own: every rule it uses is read.

### 5.3 CacheWeaponCard at call time

**Off answers `NotYet` of kind `NotRead`**, naming "WillowWeapon.CacheWeaponCard: the card presentation lookup and
the card attributes of the type and parts", record this note. In the equip driver this answer comes from the
preflight (section 5.1), with no prefix. At call time, for a run that skipped that preflight, it answers before
anything of its own:
- before the `GetGlobalsDefinition` call. In Off that getter never loads; it answers `NotRead` when the field is
  unset. So the order decides which `NotRead` the run reports, not whether a write happens;
- before the clear of step 2;
- before the slot flag change of step 3.
At call time the text carries the "stopped after partial writes" prefix, since `InitializeInternal` has written
before.

Steps 4 to 6 are not read. An Off answer that ran steps 1 to 3 and then refused would leave a half-built card (the list
cleared, slot flags changed). Refusing at entry leaves the card as the earlier steps left it.

When the rest is read, the order to keep is section 3's:
1. the definition, through the getter. The equip driver must then run the globals preflight of
   NATIVE_WILLOW_GLOBALS.md section 6.2, steps 1 to 3 (the globals instance, its class, `TheGlobalsDefinition`
   holding a `GlobalsDefinition`), before the weapon is made, on a world set up per that note's section 6.1. Off's
   getter then answers the preloaded object, and no first load is ever asked for;
2. the clear;
3. the slot entries, with the flag clear;
4. the further entries;
5. the processing;
6. replication on the authority (a headless world with no network role should hold `Role` 3, as the server does;
   that is UNVERIFIED for the port's worlds).

## Open questions

1. **Is the body read in section 3 `WillowWeapon`'s `CacheWeaponCard`:** confirming it needs a way to resolve the
   weapon class's implementation of the overridable native (the class-table lookup failed for `WillowWeapon`), or a
   live trace.
2. **The presentation lookup:**
   - the two lists of the DLC item set and their order;
   - how an attribute is matched in `AttributePresentationListDefinition`;
   - which presentations are replaced by a damage-type-specific one, and how.
3. **The further card entries and the processing:**
   - the step that uses the weapon and the globals definition, what the type and each part contribute, and the
     processing of `WeaponCardPresentations`;
   - the routine that orders or filters `WeaponCardModifierStats`.
4. **A property lookup that fails** (section 4): what `ObjectPropertyAttributeValueResolver` answers for a name the
   context class does not declare, for read, add, remove and set-base.
5. Whether any `WillowWeapon` subclass overrides `CacheWeaponCard` or `ChooseRandomNameParts`. The second is final in
   script, so it cannot be overridden there.

## Live checks that would settle each rule (bl-sdk, the RTSE way)

- **The branch.** Hook `ChooseRandomNameParts` and `CacheWeaponCard` and log their order on:
  - a dropped gun (expect both, the name parts first);
  - a gun loaded from a save, if its init runs without part selection (expect `CacheWeaponCard` only, unless the
    name parts are forced).
- **The name-part picks.** On guns whose parts carry name parts:
  - read `DefinitionData.PrefixPartDefinition` and `TitlePartDefinition`, and recompute them from the type's and the
    parts' lists with the weapon's `ExpLevel`, the priorities and the expressions;
  - expect agreement, including a gun where two slots offer equal priority (the later slot wins);
  - include one where an entry's level range excludes the gun's level.
- **A None type.** Not reachable on a real gun; it stays a read rule.
- **The card list.** Read `WeaponCardModifierStats` on a gun right after it drops:
  - expect the activated slots marked for modifier text first, in slot order, with their `ComputedModifierValue`;
  - then the type's and parts' card entries;
  - compare the order with the card shown in the inventory.
- **The cleared flag.** On a gun with a slot whose attribute has no presentation, expect that slot's
  `bIncludeInModifierText` false after the call, with an entry still present for it.
- **Replication.** On the host:
  - expect the 5 `ReplicatedWeaponCardModifierValues` to be the first displayed non-slot entries (value and index);
  - on a gun with fewer than 5, read the unused ones, to see whether they keep old values;
  - on a client, expect no change by the call.
- **The first load.** Hook `GetGlobalsDefinition` during the first weapon init of a session, to see whether
  `CacheWeaponCard` is ever the call that loads it.
- **BulletSpeed.** On an assault rifle with `AR_Accessory_Box_BulletSpeed` or a `Prefix_Box_BulletSpeed` prefix:
  - read the `WeaponBulletSpeed` attribute's value and `ProjectileSpeedMultiplier`;
  - expect a value of 0 and no change to `ProjectileSpeedMultiplier`, if the failed lookup does nothing;
  - compare the projectile speed with the same gun without the part.
