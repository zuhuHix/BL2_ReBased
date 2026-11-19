# Native rules of an item's internal slot effects: ApplyInternalSlotEffectModifiers (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-APPLY-SLOT-EFFECT-MODIFIERS (decision 2026-11-19-87).

**How this was read.**
- The native was read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`), under the machine
  lock.
- Its callers were read from the installed `WillowGame.upk` with `research/script_disasm.py`.
- The structures it touches were checked against the installed layouts with `tools/ghidra/class_layout.py`.
- This note is in our own words and records behaviour only. It reproduces no decompiler output, pseudo-code, quoted
  script statements, addresses, offsets or call structure.
- Raw output stays under ignored `local/analysis/apply-slot-effect-modifiers/`.

**Every rule here is UNVERIFIED in the running game.** No bl-sdk evidence about this native was on record; section 6
lists the checks that would settle each rule. No real-game session was run.

**Clean room.** This lane implements nothing. The native stays unbound in Off until a fresh implementer codes it from
this note alone.

**Why it matters.** With `InitializeAttributeSlots` bound (lane IMPL-INIT-ATTRIBUTE-SLOTS, #468, decision
2026-11-19-82), slice link 8 (`equip_gun`) stops in `init` at this native. The installed `--equip-run` stops there for
49 of its 50 weapons.

This note details the outline in NATIVE_ATTRIBUTES.md section 9 and corrects one point of it (section 1, step 2).

## Summary

| Function | Kind (from the package) | Confidence | Status |
|---|---|---|---|
| `WillowInventory.ApplyInternalSlotEffectModifiers(bBackupSlotEffectsApplied, MaxSlotsActivated, out AttributeModifiers)` | native, final; a bool, an int and an out array of `AppliedAttributeEffect`; no return value | high | UNVERIFIED |

**What it reads:**
- the item's `AttributeSlots`, a static array of 19 `AttributeSlotData`;
- the item's `AppliedAttributeSlotEffects`, a dynamic array of `AppliedAttributeEffect` records (`Context`,
  `Attribute`, `Modifier`);
- for each slot it applies, that slot's attribute, to resolve the context.

**What it writes:**
- the attribute modifier stacks of the contexts it adds to or removes from;
- `AppliedAttributeSlotEffects`;
- the caller's out array;
- new `Core.AttributeModifier` objects.

## 1. What it does, in order

1. **Remove the recorded slot effects, only when `bBackupSlotEffectsApplied` is false.** For each record of
   `AppliedAttributeSlotEffects`, in list order, whose `Context` and `Attribute` are both set: the record's attribute
   removes the record's modifier from the record's context. Notifications are not suppressed, and the result is
   ignored. A record with a None context or attribute is skipped.
2. **Empty `AppliedAttributeSlotEffects`, always,** whatever the flag. With the flag true, the modifiers it recorded
   are therefore forgotten **without being removed**.
   - *Correction to NATIVE_ATTRIBUTES.md section 9,* which read the emptying as part of the removal: the removal
     depends on the flag, and the emptying does not.
3. **Apply the internal slots.** Nothing happens when `MaxSlotsActivated` is 0 or less. Otherwise the native visits
   that many slots of `AttributeSlots`, from index 0 and in index order.
   - **The native does not clamp the count.** The installed callers pass the definition's
     `GetAttributeSlotMaxActivated()`, which is 1 to 19 (decision 2026-11-19-03).
   - **A slot qualifies** when it is `bActivated`, not `bExternalSlot`, and its `AttributeToModify` is set.
     `bRunEffectsAsSkill` is not looked at, so an internal slot marked "run as skill" is applied here too.
   - **For a qualifying slot:**
     1. It makes a new `Core.AttributeModifier` (transient outer, no name). It sets the modifier's `Type` to the
        slot's `ModifierType` and its `Value` to the slot's `ComputedModifierValue`, as stored; nothing is recomputed
        here.
     2. It resolves the slot attribute's context **from the item itself**: the attribute's context resolver chain
        (NATIVE_ATTRIBUTES.md section 1).
     3. When a context results, the attribute adds the modifier to it, with notifications not suppressed.
     4. When the add succeeds, it appends a record (the context, the attribute and the modifier) to the out array,
        **after whatever the array already holds**: the native does not empty it. When the flag is true, it appends
        the same record to `AppliedAttributeSlotEffects` as well.
     5. When the context is None or the add fails, nothing is recorded. The new modifier object is left unreferenced.
4. It returns nothing.

**What it refuses.** Nothing. There is no error path: a slot that does not qualify, a None context and a failed add are
all silent.

## 2. How the installed callers use it

`WillowWeapon.InitializeInternal` (the weapon's spawn) calls it twice. `WillowItem.InitializeInternal` does the same
with its own definition and modifier list. Both calls pass the definition's `GetAttributeSlotMaxActivated()` and the
weapon's `WeaponAttributeModifiers` as the out array.

1. **After the base slots are built** (`InitializeAttributeSlots`) and the weapon's own effects are applied
   (`ApplyAllWeaponAttributeEffects`), it calls this native with true. The slot effects are recorded in the backup
   list.
2. **Only on the name-part branch,** when the weapon chose random parts or the name parts are forced: after
   `InitializeAttributeSlotsForNameParts`, it calls it again with false.
   - That second call removes the first call's slot modifiers through the backup list, empties the list, and applies
     the current slots again without recording them in it.
   - `WeaponAttributeModifiers` then still holds the first call's records (whose modifiers are now removed), followed
     by the second call's records. This follows from the two rules above and is UNVERIFIED.

## 3. The Off answer, step by step (for an implementer, under the hub's write rule)

Every case has exactly one Off answer. Where the native or the game leaves a case open, the answer is marked
**Off assumption**, and it is UNVERIFIED like everything else here.

**The hub's write rule.**
- What is cheap and certain is checked before the first write.
- After a write there is no rollback.
- A `NotYet` raised after a write is terminal and keeps its kind. Its text gets the prefix "stopped after partial
  writes: ".
- Every other error propagates unchanged, before a write or after one.

**Off assumptions.**
- **B1, the count.** A `MaxSlotsActivated` above the declared dimension of `AttributeSlots` stops at the dimension, as
  the external native's port does (decision 2026-11-19-49). The native would read past the array. No installed caller
  passes more than 19.
- **B2, contexts before writes.** Off resolves every qualifying slot's context in the preflight, in slot order, before
  the removals in step 1. The Off assumption is that nothing this call writes changes a later slot's context: resolvers
  walk object references, not the modifier stacks this call changes. The game resolves each slot's context just
  before its add.
- **B3, no orphan modifiers.** Off makes a modifier object only for a slot whose context resolved. In the game an
  unrecorded modifier is created anyway and left unreferenced; nothing script-visible holds it.

**Preflight**, in this order, with no writes:

| # | Check | Outcome |
|---|---|---|
| P1 | the out argument is an array of `AppliedAttributeEffect` | otherwise a data error |
| P2 | the item's class declares `AttributeSlots` as a static array of `AttributeSlotData`, the struct declaring `bActivated` and `bExternalSlot` as bools, `AttributeToModify` as an object, `ModifierType` as a byte and `ComputedModifierValue` as a float | otherwise a data error (the shape check of decision 2026-11-19-49, without `bRunEffectsAsSkill`, which this native does not read) |
| P3 | the item's class declares `AppliedAttributeSlotEffects` as a dynamic array of `AppliedAttributeEffect`, whose `Context`, `Attribute` and `Modifier` are objects | otherwise a data error |
| P4 | the class `Core.AttributeModifier`, declaring `Type` as a byte and `Value` as a float | not found: the store's own error, unchanged; a field missing or of another kind: a data error |
| P5 | the slot count | 0 when `MaxSlotsActivated` is 0 or less; otherwise the smaller of it and the dimension (B1) |
| P6 | read the first P5 slots, and the five fields of each | a stored value of the wrong kind is a data error naming the slot index and the field |
| P7 | read `AppliedAttributeSlotEffects` | a record field holding something other than an object or None is a data error naming the record index |
| P8 | for each qualifying slot (activated, not external, attribute set), resolve its context from the item (B2) | an object: the slot will be added there; None: the slot is skipped; a `NotYet` or another error: returned unchanged, before any write |
| P9 | for each removal step 1 will make (flag false; context and attribute set) and each add P8 planned: the attribute layer's own test, without writing, of whether that remove or add can be carried out on that context (the test `ow-attr` applies before it touches a stack) | a `NotYet` or another error: returned unchanged, before any write. A "cannot" that the attribute layer answers as false (a refused add, NATIVE_ATTRIBUTES.md section 1) is not an error: the add will fail and record nothing |

If the attribute layer has no test that writes nothing, the implementer adds one that reads only. P9 must not be
skipped.

**Writes**, in order:

| # | Write | Off answer |
|---|---|---|
| W1 | flag false only: each removal of P9, in list order | the attribute layer's remove, notifications not suppressed; its bool is ignored |
| W2 | `AppliedAttributeSlotEffects` emptied | always |
| W3 | for each slot P8 resolved, in index order: a new `AttributeModifier` (B3) with `Type` and `Value` from the slot, added to the context | on a true add: one record appended to the out array, and also to `AppliedAttributeSlotEffects` when the flag is true; on a false add: nothing recorded |

The native returns nothing. A heap error propagates unchanged. With P8 and P9 passed, no `NotYet` should be possible
after W1. If one is raised anyway, by a resolver or an attribute layer that behaves differently on the second look,
the write rule applies: it is terminal, keeps its kind, gets the prefix, and nothing is rolled back.

**The slice minimum** is the whole of the above. Nothing in it is optional for link 8. The `init` step needs the
call to return and the out array to carry the slot records, which the weapon's later removals use.

## 4. What link 8's `init` reaches next (read cheaply, script and the native list only)

After the first call (`WillowWeapon.InitializeInternal`, section 2), in order:

1. `RecomputeNormalizedDamageValues`: native, and bound in Off since #446 (decision 2026-11-19-56).
2. **On the name-part branch** (random parts or forced name parts):
   - `ChooseRandomNameParts`: native, and **not bound**, so this is the next stop on that branch;
   - after it, `ComputeValueOfParts` (bound) and `InitializeAttributeSlotsForNameParts` (bound by #468);
   - this native again, with false;
   - `ApplyNamePartWeaponAttributeEffects` (script) and `RecomputeNormalizedDamageValues`.
3. **Then on either branch:**
   - `ComputeValueOfParts` (bound);
   - `CacheWeaponCard`: native, and **not bound**, so this is the next stop off the name-part branch;
   - `RecomputeAttributeBaseValues` (native, not bound);
   - `ClientInitializeFromWeaponTypeDefinition` (script);
   - `GetFiringModeDefinition` (native, not bound);
   - the firing mode's explosion override, script.
   - Which branch the equip run's fixed weapons take depends on their `bSelectRandomPartsOnInitialization`, which this
     lane did not read for them.

**The one weapon that went further** (a Jakobs sniper rifle) does not reach this native. It stops earlier in the same
function, in `ApplyAllWeaponAttributeEffects`, on an `ObjectPropertyAttributeValueResolver` `NotYet`: its effect's
attribute names a `BulletSpeed` property on the weapon context. The installed `WillowWeapon` declares no `BulletSpeed`
property; its nearest is `ProjectileSpeedMultiplier`. So the attribute's context is not the weapon, or the resolver
names a property of another class. Which attribute, and on what class `BulletSpeed` lives, is open question 4; it is
not this native's.

## 5. Open questions

1. Whether the game's per-slot context resolution can differ from B2's up-front resolution: a resolver whose answer
   depends on a modifier stack this call changes.
2. Whether a count above 19 can ever reach the native (B1).
3. Whether the stale records `WeaponAttributeModifiers` keeps after the second call (section 2) are ever used in a way
   that matters: a later removal through them removes nothing, because the modifiers are already gone.
4. Which attribute names `BulletSpeed`, and on which class that property is declared (section 4).
5. How `ComputedModifierValue` is computed (NATIVE_ATTRIBUTES.md section 9, "Open", now `InitializeAttributeSlots`'
   note). This native only copies it.

## 6. Live checks that would settle these rules (bl-sdk, the RTSE way; none run, no session granted)

Use a weapon spawn (a vendor or a drop) and a spare weapon in the backpack. The **slot snapshot** is, for each of
the first `MaxSlotsActivated` slots:
- `bActivated`, `bExternalSlot` and `bRunEffectsAsSkill`;
- `AttributeToModify`, `ModifierType` and `ComputedModifierValue`.

1. **The order and the records** (section 1).
   - Hook `ApplyInternalSlotEffectModifiers` before and after. Record the flag, the count, the slot snapshot, and
     `AppliedAttributeSlotEffects` and the out array, both before and after.
   - Each new record's context should be the weapon, or what the attribute's resolver gives for it. Its attribute
     should be the slot's, and its modifier should carry the slot's type and value.
   - The out array should grow by exactly the new records and keep what it held (step 3, point 4).
2. **The removal and the emptying** (steps 1 and 2).
   - For each attribute a slot names, read the modifier stack of the property it modifies on the context before and
     after each call.
   - With false, the backup list's modifiers should leave the stacks before the new ones arrive.
   - With true, the backup list should be emptied, and the stacks should keep its old modifiers.
   - On the spare weapon, call the native twice with true through the SDK: the second call should leave the first
     call's modifiers on the stacks (doubled effects).
3. **The filter** (step 3). Find a weapon or item with an internal slot marked `bRunEffectsAsSkill` and check that
   this native applies it. Check that an external slot is never applied here.
4. **Silent failures** (step 3, point 5): a qualifying slot whose context resolves to None, or whose add fails,
   should leave no record.
5. **Notifications.** Hook the context's attribute notification during the add and the removal; it should fire,
   because nothing is suppressed.
6. **The caller** (section 2): which installed weapons take the name-part branch at spawn, and what
   `WeaponAttributeModifiers` holds after it.
7. **Not to be tested:** a count above 19. It could read past the slot array.

## Cross-references

- NATIVE_ATTRIBUTES.md, sections 1 (resolve, add and remove) and 9 (the slot natives; one correction, section 1
  step 2).
- Decisions 2026-11-19-03 (the equip natives in Off), 2026-11-19-49 (the slot read and its shape check) and
  2026-11-19-82 (`InitializeAttributeSlots`).
- ROADMAP.md, Phase 4 (a handful of guns end to end): a weapon's spawn must finish `InitializeInternal`.
