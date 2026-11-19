# WillowWeapon.RecomputeAttributeBaseValues (analyst note, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5). Analyst lane ANALYST-RECOMPUTE-ATTRIBUTE-BASE-VALUES (decision
2026-11-19-NNN, a placeholder), for slice link 8 (equip). It corrects the short entry in NATIVE_ATTRIBUTES.md section 8.
- **Sources, in the brief's order:**
  - the script, read with `research/script_disasm.py` (every script function of the nine code packages searched for
    callers);
  - recorded live evidence: AI_MELEE_TARGETING_LIVE.md section 6 (lanes L7 and L8) read the live
    `InstantHitDamageBaseValue` and `InstantHitDamage` stack of the five slice guns after they were built (section 3,
    "The recorded live evidence"). No record holds the other triples' bases or stacks;
  - the game executable, read with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, one headless run under the machine lock);
  - field positions from `tools/ghidra/class_layout.py`.
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure. Raw output stays
  under ignored `local/analysis/rabv/`.
- **No live session** was run. **Every rule here is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. A fresh implementer codes the Off answer from this note.

## 1. The question

With #534 (the card's slot count) on #521, link 8 passes 1 of its 5 steps and stops at `init` on the unbound native
`WillowWeapon.RecomputeAttributeBaseValues()`. The one-paragraph reading in NATIVE_ATTRIBUTES.md section 8 (medium
confidence) said each stack is emptied "and the value is reset to its base". Read that way, the native would throw away
every effect `InitializeInternal` has just applied. The body read here says the opposite direction: the base takes the
value.

## 2. Where it is called (script)

- **Callers:** `WillowWeapon.InitializeInternal` and `WillowItem.InitializeInternal`, and no other script function in
  the nine code packages. On the weapon it is a final (non-virtual) call.
- **In the weapon's init**, in order (NATIVE_CACHE_WEAPON_CARD.md section 1 has the full list):
  1. `CalculateWeaponBaseValues` sets the bases;
  2. the parts are chosen, the part-dependent bases are set, and `ApplyAllWeaponAttributeEffects` and
     `ApplyInternalSlotEffectModifiers` push their modifiers onto the stacks. Their records go into
     `WeaponAttributeModifiers`;
  3. the name-part branch, when it is taken;
  4. `ComputeValueOfParts`, then `CacheWeaponCard`;
  5. **this native**;
  6. `ReplicatedZoomedEndFOVBaseValue` is set from `ZoomedEndFOVBaseValue`;
  7. `ClientInitializeFromWeaponTypeDefinition` (script), then `GetFiringModeDefinition`.
- **What the script reads afterwards:** step 6 is the first read. It copies a base value, which only makes sense if the
  native has just made the bases meaningful. `ClientInitializeFromWeaponTypeDefinition` calls `CacheWeaponCard`
  again, but only when the weapon's `Role` is below `ROLE_Authority` (the comparison read as an integer "less than";
  UNVERIFIED). Link 8's weapon is the authority, so that second card call does not run there.

## 3. What the native does

**It bakes the modified values into the bases.** It reads no definition, evaluates nothing, draws no random number
and loads nothing. In order:

1. **It walks every stack attribute of the weapon's class, inherited ones included.** It starts from the object's
   runtime class and follows a linked chain from property to property. The recorded live evidence below shows that the
   chain reaches `Engine.Weapon`'s `InstantHitDamage`, so it is the class's whole property chain (every property of the
   class and its parents), not the list of fields the class itself declares. Every property whose flags carry the
   stack-attribute bit (the port's `PropertyDecl::STACK_ATTRIBUTE`, the value side of a triple) is handled; other
   properties are skipped. For each one:
   1. **its modifier stack** (the `XModifierStack` companion) is emptied: the length is set to 0, and when the array
      had storage that storage is released. The `AttributeModifier` objects are not destroyed; the stack simply
      stops referring to them;
   2. **its base** (the `XBaseValue` companion) is overwritten with **a copy of the value** (`X`), as the base
      property's own type copies (float to float, int to int). It is a plain property copy: the value is not
      recomputed, and no attribute-change notification runs.
2. **Then the weapon's `WeaponAttributeModifiers`** (the applied-effect records, an array of
   `AttributeEffect.AppliedAttributeEffect`; the layout tool places that field where the native clears) is emptied in
   the same way.

So afterwards every handled attribute keeps its value. Its base equals that value and its stack is empty, so a later
recompute from the base and the empty stack gives the same number. The effects stay in the numbers, but there is
nothing left to remove: their records are gone.

**Which attributes.** For a weapon whose class is `WillowWeapon`, all 41 triples:
- the 34 `WillowWeapon` declares, for example `ClipSize`, `ReloadTime`, `ZoomedEndFOV`, `MeleeDamage`,
  `StatusEffectDamage`, `ProjectilesPerShot`, `ShotCost`, `BurstInterval` and the barrel, body and magazine timings;
- the 7 it inherits from `Engine.Weapon`: `FireInterval`, `Spread`, `InstantHitDamage`, `InstantHitMomentum`,
  `EquipTime`, `PutDownTime` and `WeaponRange`.

The first version of this note (58d6b78) read the walk as the class's own fields only. The recorded live evidence
contradicts that. For the 34 own triples and for the 6 inherited ones other than `InstantHitDamage`, the inclusion
rests on the chain being one chain, not on a read of each.

**The recorded live evidence** (AI_MELEE_TARGETING_LIVE.md section 6, lanes L7 and L8):
- the five slice guns' live `InstantHitDamageBaseValue`, built at grade 8, was exactly their card damage (for
  example 63.633 on the Jakobs pistol). `tools/weapon_stats.py` builds that card damage by combining the type's base
  with the parts' damage modifiers;
- rerun on those five recipes for this note (local output under `local/analysis/rabv/l7/`), every one has a non-zero
  part term on damage. On the Jakobs pistol, for example, a pre-part base of 46.789 and a part scale of +0.36 give
  63.633. So the live base already holds the parts' effect: the value was copied into the base;
- the equipped gun's `InstantHitDamage` stack held one modifier, the player's Badass Rank, added after the build. No
  part modifier was left on it, so the stack had been emptied;
- backpack guns read a live value equal to their base.

So for `InstantHitDamage`, a triple `Engine.Weapon` declares, both halves of section 3 are seen: the base takes the
value, and the stack is emptied. That settles the walk's reach to inherited fields and supports the copy's direction.
The record does not say when in the build this happened. This native is the only step of the init that empties stacks
(section 2), so it is attributed here; that attribution is UNVERIFIED.

**The item variant.** `WillowItem.RecomputeAttributeBaseValues` is a thin call through the item's virtual table; its
body was not reached. It is not on link 8's path.

## 4. For an implementer: the Off answer

**Off only; Reference is unchanged.** The hub's rulings apply:
- the preflight is write-free and loads nothing; class and code loading happens in explicit setup;
- the three counters of #518 (`Heap::committed_writes`, `GameRng::draws`, `DefStore::load_transitions`) are recorded
  at the start, and a refusal after any of them moved is marked "stopped after partial writes";
- a typed `NotYet` keeps its kind and record; first wins on binding.

1. **The binding:** the native `WillowWeapon.RecomputeAttributeBaseValues` with no arguments and no return value, bound
   by its own key in Off.
2. **The preflight** (read-only, before the first write; it can sit with the card's schema in the equip preflight):
   - the weapon's class declares `WeaponAttributeModifiers` as a dynamic array;
   - each stack-attribute property of the class (inherited ones included) has its three companions (the class table
     already checks this at definition, `AttrCompanions`), its stack companion is an array, and its base companion
     holds the same numeric kind as the value: float, int or byte. An attribute kind and its plain kind count as the
     same, so `FloatAttribute` matches `Float`;
   - a failure is a `VmError::Script` naming the field.
3. **The writes, in the game's order:** for each stack-attribute property of the weapon's runtime class, inherited
   ones included (`ClassTable::props`, which lists inherited properties first, in slot order):
   1. the stack companion is set to the empty array;
   2. the base companion is set to the value companion's stored value, unchanged.

   Then `WeaponAttributeModifiers` is set to the empty array. The per-attribute writes touch disjoint fields and nothing
   reads between them, so their order among attributes cannot be observed. The slot order of `ClassTable::props`
   keeps the port deterministic.
4. **Not the port's set-base.** Do not route the base write through `ow-attr`'s set-base (`storage::set_base_of`): it
   recomputes the value and notifies, and the native does neither. It also takes the new base as an `f32`, so an int
   base above 2^24 would lose precision on the way. A direct property write of the stored value is the native's
   behaviour.
5. **Draws and loads:** none. The native touches only the weapon's own fields, so the counters move only through the
   heap writes of step 3.
6. **Refusals:** none. Link 8's weapon is a `WillowWeapon` with authority, and every rule above is read, so there is
   no case left for a `NotYet` of kind `NotRead`.

**What link 8 meets next.** Binding this removes the `RecomputeAttributeBaseValues` stop, and nothing more is claimed.
The script then runs `ClientInitializeFromWeaponTypeDefinition`, which reaches several natives (meshes, the behavior
kernel, the attribute slots and the weapon's damage type), and then `GetFiringModeDefinition`. The gate run shows the
next stop.

## 5. Open questions

1. The inherited triples other than `InstantHitDamage`, and the own triples, are covered by the chain reading, not
   by a live read of each (check 2 below).
2. The direction of the copy (base takes value) is read from the executable and supported by the live damage base
   (section 3). Check 1 confirms it on the other triples.
3. `WillowItem`'s body and any `WillowWeapon` subclass were not read.
4. The integer comparison that gates the client's second `CacheWeaponCard` call was read as "less than".

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Hook `WillowWeapon.RecomputeAttributeBaseValues` (pre and post) on a freshly spawned weapon, for example one dropped
from a chest:
1. **The bake.** Before the call, capture `ClipSize`, `ClipSizeBaseValue` and the length of `ClipSizeModifierStack`, the
   same for `ReloadTime` and `ZoomedEndFOV`, and the length of `WeaponAttributeModifiers`. After the call, expect each
   value unchanged, each base equal to its value, each stack empty, and `WeaponAttributeModifiers` empty.
2. **The inherited triples.** In the same capture, `InstantHitDamage`, `InstantHitDamageBaseValue`,
   `InstantHitDamageModifierStack` and the same for `FireInterval` and `Spread`. Expect the same bake as check 1, and
   expect the damage pair to agree with AI_MELEE_TARGETING_LIVE.md section 6.
3. **No notification.** Hook the weapon's attribute-change handler during the call: expect no call.
