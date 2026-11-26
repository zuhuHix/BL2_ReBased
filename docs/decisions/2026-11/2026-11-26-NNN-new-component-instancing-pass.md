## 2026-11-26: Script new's component-instancing pass and per-object configuration read

AI-assisted (Claude Code)

Lane ANALYST-NEW-COMPONENT-INSTANCING, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** NATIVE_TOUCH_COMPONENT_TEMPLATE.md (#743) refuses the component-instancing step behind class-flag bit 23
  because it was not read, and links 6 and 7 trip that refusal.
- **What:**
  - the new note `docs/verification/NATIVE_NEW_COMPONENT_INSTANCING.md`;
  - two NATIVE_INDEX rows and one GATES row;
  - a pointer in NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.4.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock: the instancing graph, the class pass, the property
    methods of four property classes, and the graph's lookup with its creation;
  - a local census of every package's property exports: 857 component fields, and 23 object properties of a
    component class, all of them function parameters or locals;
  - `ow-package --def-dump` of the weapon's third-person mesh template, and the layout tool.

  Raw output stays under ignored `local/analysis/newinst/` and `local/analysis/touchtpl/`.
- **Findings** (UNVERIFIED in game):
  - with bit 23, after the property copy and the constructor, each component field, array element and struct member
    is visited. None is left alone; a component the template owns is re-made with the source as its template and the
    new object as outer; a reference to another owner's component is kept. Nothing is loaded;
  - bit 10 reads the per-object configuration; without the bit, nothing happens;
  - for links 6 and 7 every component field of the template is None, so the pass does nothing.
- **Proposed Off answer:**
  - #743's blanket bit-23 refusal narrows to "any would-be component value non-None";
  - component fields are found by a stand-in (an object field of a `Component` class), which matches the census;
  - refusals before the made object, so no prefix;
  - tests listed.
- **Next** (a prediction, not a run): with option B and this rule, `new` passes on links 6 and 7, then the `TEMPLATE`
  `NotRead` again (the fixture's template has no mesh); with the gun's init, source f's `LayerAbsent`.
- **Clean room:** a different lane implements this note.
