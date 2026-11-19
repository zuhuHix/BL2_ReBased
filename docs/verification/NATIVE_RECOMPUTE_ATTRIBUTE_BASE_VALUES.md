# WillowWeapon.RecomputeAttributeBaseValues (analyst note, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5). Analyst lane ANALYST-RECOMPUTE-ATTRIBUTE-BASE-VALUES (decision
2026-11-19-NNN, a placeholder), for slice link 8 (equip). It corrects the short entry in NATIVE_ATTRIBUTES.md section 8.
- **Sources, in the brief's order:**
  - the script, read with `research/script_disasm.py` (every script function of the nine code packages searched for
    callers);
  - recorded live evidence: the verification records and the ignored local captures were searched, and none holds a
    weapon's base values or modifier stacks after init;
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

1. **It walks the stack attributes the weapon's class itself declares.** That is the field list of the object's runtime
   class, followed through each field's next link. Every property whose flags carry the stack-attribute bit (the port's
   `PropertyDecl::STACK_ATTRIBUTE`, the value side of a triple) is handled; other fields are skipped. For each one:
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

**Which attributes.** The walk starts from the runtime class's own field list. Read in the usual way, that list holds
only the fields that class declares, not inherited ones (UNVERIFIED: the reading rests on the engine's usual struct
layout, which was not confirmed for this build). For a weapon whose class is exactly `WillowWeapon`:
- **handled:** the 34 triples `WillowWeapon` declares, for example `ClipSize`, `ReloadTime`, `ZoomedEndFOV`,
  `MeleeDamage`, `StatusEffectDamage`, `ProjectilesPerShot`, `ShotCost`, `BurstInterval` and the barrel, body and
  magazine timings;
- **not handled:** the 7 triples `Engine.Weapon` declares (`FireInterval`, `Spread`, `InstantHitDamage`,
  `InstantHitMomentum`, `EquipTime`, `PutDownTime` and `WeaponRange`). Their stacks and bases are left as they are.

The prior note's "inherited ones were not confirmed" stands, with this reading leaning to "not included". A
`WillowWeapon` subclass would handle its own declared triples instead (none was looked for).

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
   - each stack-attribute property the class declares has its three companions (the class table already checks this
     at definition, `AttrCompanions`), its stack companion is an array, and its base companion holds a number of
     the value's own kind;
   - a failure is a `VmError::Script` naming the field.
3. **The writes, in the game's order:** for each stack-attribute property **declared by the weapon's runtime class
   itself** (`ClassTable::props` with `declared_in` equal to that class), in the class's declaration order:
   1. the stack companion is set to the empty array;
   2. the base companion is set to the value companion's stored value, unchanged.

   Then `WeaponAttributeModifiers` is set to the empty array. The per-attribute writes touch disjoint fields and nothing
   reads between them, so their order among attributes cannot be observed. Declaration order keeps the port
   deterministic.
4. **Not the port's set-base.** Do not route the base write through `ow-attr`'s set-base (`storage::set_base_of`): it
   recomputes the value and notifies, and the native does neither. A direct property write is the native's behaviour.
5. **Draws and loads:** none. The native touches only the weapon's own fields, so the counters move only through the
   heap writes of step 3.
6. **Refusals:** none are needed for link 8's weapon, which is a `WillowWeapon` with authority. If an implementer
   wants a guard for the open reading on inherited attributes, it would be a typed `NotYet { NotRead }` naming this
   record. But it would refuse every weapon, so this note does not ask for one: the reading above is the Off answer.

**What link 8 meets next.** Binding this removes the `RecomputeAttributeBaseValues` stop, and nothing more is claimed.
The script then runs `ClientInitializeFromWeaponTypeDefinition`, which reaches several natives (meshes, the behavior
kernel, the attribute slots and the weapon's damage type), and then `GetFiringModeDefinition`. The gate run shows the
next stop.

## 5. Open questions

1. Whether the walk covers inherited stack attributes (the `Engine.Weapon` triples, damage and fire rate among them).
   This reading says no; a live check settles it (check 2 below).
2. The direction of the copy (base takes value) is read from which companion receives the copy. A live check settles
   it (check 1).
3. `WillowItem`'s body and any `WillowWeapon` subclass were not read.
4. The integer comparison that gates the client's second `CacheWeaponCard` call was read as "less than".

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Hook `WillowWeapon.RecomputeAttributeBaseValues` (pre and post) on a freshly spawned weapon, for example one dropped
from a chest:
1. **The bake.** Before the call, capture `ClipSize`, `ClipSizeBaseValue` and the length of `ClipSizeModifierStack`, the
   same for `ReloadTime` and `ZoomedEndFOV`, and the length of `WeaponAttributeModifiers`. After the call, expect each
   value unchanged, each base equal to its value, each stack empty, and `WeaponAttributeModifiers` empty.
2. **The inherited triples.** In the same capture, `InstantHitDamage`, `InstantHitDamageBaseValue`,
   `InstantHitDamageModifierStack` and the same for `FireInterval` and `Spread`. Expect them untouched by the call (this
   note's reading). A cleared stack or a changed base would mean the walk covers inherited fields.
3. **No notification.** Hook the weapon's attribute-change handler during the call: expect no call.
