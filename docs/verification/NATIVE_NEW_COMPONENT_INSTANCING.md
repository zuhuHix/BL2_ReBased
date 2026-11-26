# Script `new`: the component-instancing step and the per-object configuration read (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-NEW-COMPONENT-INSTANCING (decision 2026-11-26-08), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/newinst/`
and `local/analysis/touchtpl/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_TOUCH_COMPONENT_TEMPLATE.md (#743) proposes a five-operand `new` with a template. It refuses the
component-instancing step that the construction runs for a class with class-flag bit 23, because that step was not
read. Links 6 and 7 trip that refusal: `AttachMeshToPickup` makes a `SkeletalMeshComponent`, and that class has the
bit. This note reads the step, and the configuration read behind bit 10.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The natives:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - the object construction that script `new` calls (#743), and its creation and disposal of an instancing graph;
  - the graph's set-up and its pair registration;
  - the class-level instancing pass. It walks the class's list of component-holding properties and calls one
    property virtual on each;
  - that virtual's bodies for `ComponentProperty`, `ArrayProperty`, `StructProperty` and `ObjectProperty`, found in
    each property class's primary table as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - the graph's "instanced component" lookup and the creation it makes.
- **A census of component fields** (local, over every package's property exports):
  - 857 `ComponentProperty` exports, each pointing at a class that descends from `Core.Component` (a structural
    check of the reading);
  - 23 `ObjectProperty` exports point at a component class. **All 23 are function parameters or locals**, none a
    class or struct field;
  - so over the shipped packages, "a class or struct field whose class descends from `Component`" picks out exactly
    the declared `ComponentProperty` fields.
- **The port:** `ow-object` `layout.rs` folds a `ComponentProperty` into `PropKind::Object` of its class (its
  `PropKind` documentation). So the port cannot see the declared kind, and needs the census's stand-in (section 3).
- **Maya's case** (`ow-package --def-dump` and the layout tool, local): the component fields of
  `SkeletalMeshComponent` and its bases are `AttachedToSkelComponent`, `ParentAnimComponent`,
  `CachedFaceFXAudioComp`, `FogVolumeComponent`, `OverrideLightComponent`, `LightEnvironment`,
  `PreviousLightEnvironment`, and the array `DecalsToReattach`. On
  `WillowGame.Default__WillowWeapon.ThirdPersonWeaponMesh` every one is None, and the array is empty, as is
  `Attachments`.

## 2. What the game does

### 2.1 When the step runs

The construction makes an **instancing graph** for the call, unless the caller passed one, when the new class has
**bit 23** set. That is the "has components" class flag, by fit. Then:
1. at allocation, the graph is told its roots. The source root is the template (or the class default object, with
   no template), and the destination root is the new object. The pair is registered. **Nothing is written to the
   new object by this;**
2. the property copy of #743 section 3.2 runs (values from the template, transient fields from the class default);
3. the class's own constructor runs;
4. **then the instancing pass**, unless the new object is a class default object. The graph's "component instancing
   enabled" switch defaults to on, so the pass runs. It walks the new class's **list of component-holding
   properties**:
   - each declared component field;
   - each dynamic array whose element is a component;
   - each struct field whose struct holds one (the struct runs the same pass over its own list).

   For each, it calls the property's instancing method, with the new object's value, the archetype's value at the
   same place (when the archetype's class is large enough to have it), the new object as owner, and the graph;
5. the graph is disposed when the construction made it.

`ObjectProperty`'s instancing method does nothing. `ArrayProperty`'s calls its element property's method for each
element. `StructProperty`'s runs the struct's own pass.

### 2.2 `ComponentProperty`'s instancing method

A component field **flagged native** (the property flag by fit for natively serialised fields) is skipped. Otherwise,
for each element of the field:
1. **the field is None: nothing.** No write, no lookup;
2. the field holds a component `c`. The **source** is the archetype's value at the same place when that is set;
   otherwise it is `c` itself. With no archetype value, the field is first checked for a component that belongs to
   one of the owner's archetypes: when `c`'s `Outer` is one of them (walking the owner's archetype chain), the answer
   is **None**, written to the field;
3. otherwise the graph is asked for the **instanced component** for that source and `c`:
   - with component instancing off, or no source: the field is left alone;
   - **a component not owned by the source root** (its `Outer` is not the graph's source root, by the graph's own
     test): left alone. A reference to someone else's component is kept as a reference;
   - one already instanced in this graph: that instance;
   - when `c` is not the source and `c`'s `Outer` is already the owner: `c` itself, re-pointed at the source as its
     archetype (some of its header fields are refreshed from the source);
   - in some engine states, a component that is not needed on this side (client or server) gives None (not read in
     detail);
   - **otherwise a new component is made:**
     - with the source's class;
     - with the instanced counterpart of the source's `Outer` as its outer (for a component owned by the template,
       that is the new object);
     - with the source's name;
     - **with the source as its template**, so it goes through the same construction (#743 section 3.2), with this
       graph;
     - and the pair is registered.

   The answer is written to the field, unless it is "leave alone".
4. **Loads: none.** Every class and component involved is already in memory. A class lookup by name, for a test, does
   not load.

### 2.3 The per-object configuration read (bit 10)

After the constructor, unless the object is a class default object (and outside one engine mode), a new object whose
class has **bit 10** set (by fit "per-object config") reads its configuration and its localised values. That is an
ini read and a localisation read (by fit; the readers were not opened). **For a class without bit 10 nothing
happens.** `SkeletalMeshComponent` does not have the bit (#743 section 2).

## 3. The Off answer (for a separate implementer)

This replaces #743 section 3.3's blanket refusal for bit 23 and refines its bit-10 refusal. On main (#746) those are
`ow-vm` `interp/eval.rs`'s `NEW_INSTANCING` and `NEW_CONFIG` in `template_refusal`. It is part of option B's
interpreter change, **the hub's call**. **Every rule is UNVERIFIED.**

**Both paths of `new`.** The pass runs whether or not a template is given (section 2.1): with a template its source is
the template, and with none it is the class default object. So the rule below applies to both:
- **with a template:** the would-be values are the template's, and the class default's for transient fields;
- **with no template:** the would-be values are the class default object's, all of them.

Main's no-template path (`new_object`, straight to `instantiate_from_defaults`) checks neither bit 23 nor bit 10 today.
So a bit-23 class whose class default holds a non-None component value gets that component shared, where the game
makes a new one. A bit-10 class skips its configuration read. **That is a gap against main's code**, which the
implementer closes with the same preflight on that path.

**The object fields the rule looks at:** every field declared on the new class **or any super class**, that is:
- an object reference;
- a dynamic array of object references;
- a struct holding such fields (recursively).

Class and struct fields only: parameters and locals do not count.

**The walk order** (fixed, so the first failing check names the refusal):
1. super class first, down to the new class;
2. within a class or struct, the fields in declaration order;
3. within an array, element index order;
4. within a struct field, its members by the same rules, recursively.

**Preflight** (write-free and load-free, before the object is made, together with #743's checks). With bit 23 set on
the new class, for each object field's would-be value, in the walk order:
1. **the value is None** (or the array is empty): nothing to check. **A None value needs no classification**, so its
   field's declared class is never walked;
2. **the value is not None:** classify the field. Its declared class is a component class when its chain reaches
   `Core.Component` (the census stand-in of section 1):
   - **the chain cannot be walked without a load** (a super class the store has not loaded):
     `NotYet { NotRead }`, "new: classifying an object field whose class is not loaded (a load is a write)";
   - **a component class:** `NotYet { NotRead }`, "new: instancing a referenced component". The graph's lookup is
     read, but its engine-state branches (section 2.2, step 3) are not, and making a nested component would also
     recurse through `new`'s whole rule;
   - **any other class:** not a component field. Nothing, and the walk goes on;
3. **a struct or array value that is not resident** (a store handle not loaded): `NotYet { NotRead }` (a load is a
   write).

Then **bit 10** set on the new class: `NotYet { NotRead }`, "new: a per-object configuration read", as in #743. With
the bit clear: nothing, and no refusal.

**The native-field skip** (section 2.2) needs no port check while every component value is None.

**Links 6 and 7:** every component field of the template is None (section 4). Its other non-None object fields
(such as a material or a mesh reference) are declared with engine classes the store has loaded, so they classify as
non-component without a load. So no refusal is expected there (an expectation, not a run).

**Commit:** with every component value None, the pass writes nothing. The made object and its interned name stay the
only writes (#743). Since the refusals all come before the object is made, none carries the partial-writes prefix.

**The game's constructor step** (section 2.1, step 3) has no port counterpart. A class whose native constructor sets
a component field would change the inputs above. None was read for `SkeletalMeshComponent` (UNVERIFIED).

## 4. Links 6 and 7

`AttachMeshToPickup`'s `new(Pickup) class'SkeletalMeshComponent'(ThirdPersonMesh)`:
- **the new class** has bit 23 and not bit 10;
- **the template** is the fixture weapon's `ThirdPersonMesh`. Under the subobject-instancing ruling (#435) that is
  the weapon's own copy, or the class-default subobject. Either way, as dumped, its seven component fields are None
  and `DecalsToReattach` is empty (section 1). The fixture runs no step that sets one (UNVERIFIED: not checked in a
  run);
- **so the pass does nothing, and option B's `new` makes the component.** That is a prediction: a fixture step that
  writes a component reference into the weapon's mesh before the drop would make the value non-None, and give the
  `NotRead`;
- **the next stop** is then `skeletal_bounds`' source e again: the made component holds the template's values, and
  the fixture's template has no `SkeletalMesh`. That is the `TEMPLATE` `NotRead`, which the hub keeps refused
  (NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.4, item 2). With the gun's init run as well, source f's
  stored-bounds `LayerAbsent` (item 3 there).

In the game the weapon has run its init, so its `ThirdPersonMesh` may hold a light environment. Whether that
component is owned by the weapon's mesh (instanced) or by the weapon actor (kept as a reference) follows the
source-root test in 2.2. Not read for the game's live values.

## 5. Tests (synthetic fixtures)

1. a toy component class `C` with bit 23, one component field, and a template whose field is None: the object is made,
   the field is None, and no refusal;
2. the same with the template's field holding a toy component: the `NotRead`, no object made, no name interned;
3. a component array field, empty on the template: made. With one non-None element: the `NotRead`;
4. a struct field whose struct has a component field set on the template: the `NotRead`;
5. a transient component field set on the template but None on the class default: made (the would-be value is the
   class default's);
6. a class without bit 23 whose template holds a component reference: made, with the reference copied as is (no
   pass);
7. bit 10 set: the configuration `NotRead`. Clear: nothing;
8. a parameter or local of component type (not a field) is not a component field: it never takes part.
9. **the no-template path:** a bit-23 class whose class default holds a toy component in a component field: the
   `NotRead`, no object made. The same class with that field None on the class default: made, as today;
10. a field of a class whose super chain is not loaded: holding None, nothing; holding an object, the classification
    `NotRead`;
11. a field declared on a super class with a non-None component value, and one on the new class: the super class's is
    met first, and names the refusal;
12. an object field of a non-component class holding an object: no refusal.

## 6. Predicted next stop

Against main after #746:
- **today:** `new` refuses with `NEW_INSTANCING`, "new: instancing a template's components" (the blanket bit-23
  refusal; links 6 and 7 show it with the partial-writes prefix of the step that ran before `new`);
- **with this rule:** the pass answers nothing on links 6 and 7, so `new` goes on to main's next refusal,
  `NEW_TEMPLATE_COPY` (`NotPorted`, "new: copying a template's values"), until option B's template copy lands;
- **with option B's copy as well:** `new` makes the component, then `skeletal_bounds`' source e gives the same
  `TEMPLATE` `NotRead`, because the fixture's template has no mesh (NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.4);
- **with the gun's init as well:** source f's stored-bounds `LayerAbsent`.

*Corrected 2026-11-26 in NATIVE_LINK67_PICKUP_GUN_INIT.md section 5 (decision 2026-11-26-15): that `LayerAbsent` holds
only while the pickup world lacks the mesh layer. With link 8's layer, the stop is the bounding cylinder's
`WORLD_TRANSFORM` (the copied scale 0.92).*

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the graph's engine-state branches (not needed on this side, editor and loading modes);
   - the configuration and localisation readers behind bit 10;
   - native constructors' writes;
   - the header fields refreshed when a component is re-pointed.
3. **By fit:** bit 23 as "has components", bit 10 as "per-object config", and the native-field flag that the
   component method skips.
4. **The census stand-in** holds for the shipped packages only. A field declared `ObjectProperty` of a component class
   would be treated as a component field by the port, but not by the game. The census found none.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Drop a gun. On the pickup's mesh component, read `LightEnvironment` and its `Outer`, and the same on the weapon's
   `ThirdPersonMesh`: expect either the same component (kept as a reference) or a distinct one whose `Outer` is the
   pickup's component (instanced).
2. Read the pickup's mesh component's `ParentAnimComponent` and `AttachedToSkelComponent`: expect None.
