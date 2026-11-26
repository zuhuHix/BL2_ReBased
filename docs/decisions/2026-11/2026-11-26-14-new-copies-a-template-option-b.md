## 2026-11-26: `new` copies a template (option B) and runs the component-instancing rule, Off only

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-NEW-TEMPLATE-COPY, the implementer half of the clean room
(lane F wrote NATIVE_TOUCH_COMPONENT_TEMPLATE.md and NATIVE_NEW_COMPONENT_INSTANCING.md; this lane did not read the
executable), on main 8fa9839d. **Every rule is UNVERIFIED in the running game.**

**What changed.** In `ParityMode::Off`, `ow-vm`'s `new_object` no longer refuses a template that passes section 3.3's
checks with `NotYet { NotPorted }` ("copying a template's values"). It copies, from section 3.2:
1. every slot from the template. A slot holds its value, so a string or a dynamic array is the new object's own copy:
   arrays are copy-on-write (`Rc<Vec<Value>>`, `Value::array_mut`), and the object references in them are shared;
2. every slot of a property declared transient from the class default object the preflight found;
3. the archetype: a new heap field, `Object::archetype: Option<ObjRef>` in `ow-object` (the struct is
   `#[non_exhaustive]`, made only by `Object::new`, which sets `None`). It is set to the template. When the class
   declares `ObjectArchetype` as an object (as `Core.Object` does in the store), that slot is also set to the template.
   Objects made any other way keep `None`, as before (the archetype is then the class default, not modelled).

The preflight is the same as before, and nothing is loaded. One change: it now also returns the class default it
found, and a default with fewer slots than its class (a stand-in) counts as not built. The made object and its
generated name are the first writes; a `NotYet` after them carries the partial-writes prefix. Reference is unchanged.

**The transient flag.** The note names no bit. `PropertyDecl::TRANSIENT` = 0x2000 (bit 13 of the low flag word) comes
from the installed packages (`tools/ghidra/class_layout.py`'s property reader, counts and names only). It is set on
`Engine.PrimitiveComponent`'s run-time state (`Tag`, `Bounds`, `SceneInfo`, the local-to-world matrices) and on none
of its editable fields. That is a fit, UNVERIFIED.

**The component-instancing rule** (NATIVE_NEW_COMPONENT_INSTANCING.md section 3, #749). It replaces the blanket
bit-23 refusal, on both paths of `new`, read only and before anything is made:
- the would-be values: with a template, the copy above; with none, the class default's slots. On the no-template path
  the rule runs only for a class with bit 23 or bit 10. That class's default must then be resident, else
  `NEW_DEFAULT_NOT_BUILT` (a new refusal on that path: building it is a write). A class whose body did not decode keeps
  the old unchecked path there. With a template it keeps `NEW_INSTANCING`, because its flags cannot rule the step out;
- bit 23: every object field, object array and struct holding them, on the class and its super classes (super first,
  declaration order, elements in order, struct members recursively). A None value (an empty array) needs nothing. A
  non-None value classifies its declared class by its chain: not defined in the class table gives "new: classifying an
  object field whose class is not loaded (a load is a write)"; reaching `Core.Component` gives "new: instancing a
  referenced component"; any other class passes. Both are `NotYet { NotRead }`. With every component value None the
  pass writes nothing;
- then bit 10: "new: a per-object configuration read". Without the bit, nothing.

Struct and array values sit inline in their slot, so the note's "a struct or array value not resident" case cannot
arise in the port. The field kind is the layout's `PropKind`, which folds a `ComponentProperty` into an object
reference. The `Core.Component` chain is the note's census stand-in for it (UNVERIFIED beyond the shipped packages).

**Still open:** which `Core.Object` native fields (`Outer`, `Name`, and the like, when declared) the game's copy
skips. The port copies every declared slot, as the note says, and keeps the heap's own outer and name.

**Measured.** Installed Off strict `--slice-gate` (release build, under the machine lock), before (main 8fa9839d):
6 ok / 8 NotYet, 42.9%, with links 6 and 7 at `new`'s instancing `NotRead` (0/3 and 0/4 steps). After: 6 ok / 8
NotYet, 47.0%. Links 6 and 7 pass `make` (1/3 and 1/4) and stop in touch at "Actor.GetBoundingCylinder: the
component's template (option B) is not read" (after partial writes), as predicted: the fixture's template has no mesh.
