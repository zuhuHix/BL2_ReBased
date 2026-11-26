## 2026-11-26: The component template on links 6 and 7, and the touch path after the bounds

AI-assisted (Claude Code)

Lane ANALYST-LINK67-TOUCH-TEMPLATE, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** links 6 and 7 stop in `touch` at `skeletal_bounds::TEMPLATE` (NATIVE_COMPONENT_BOUNDS.md 9.5, option B
  as revised in review r1 of #707). The pickup's component lacks its template's values because the port's `new`
  ignores the template.
- **What:**
  - the new note `docs/verification/NATIVE_TOUCH_COMPONENT_TEMPLATE.md`;
  - two NATIVE_INDEX rows and one GATES row;
  - pointers in NATIVE_COMPONENT_BOUNDS.md 9.6 and NATIVE_PICKUP_SAVE_ROTATION.md section 1.
- **Evidence:**
  - `research/script_disasm.py`, and a local copy of it with a five-operand `New` for a census (12,968 of 12,978
    functions decode with either table; 189 of the 225 `New` expressions are followed by a bare `Nothing` under four
    operands);
  - a local read-only Ghidra reading under the machine lock (the `new` handler, the construction and property
    initialisation, `Pickupable_IsEnabled` and `WillowPickup`'s pickup-interface slot);
  - the layout tool.

  Raw output stays under ignored `local/analysis/touchtpl/`.
- **Findings** (UNVERIFIED in game):
  - `New` has five operands (outer, name, flags, class, template);
  - with a template, every property value is copied from it, transient fields are reset from the class default, and
    the archetype is the template. Nothing is loaded;
  - option B alone leaves links 6 and 7 at the same stop, because the fixture's template has no mesh. With the gun's
    init run too, the next stop is source f's stored-bounds `LayerAbsent` for the run-time composite mesh;
  - after the bounds, `touch` reaches `Pickupable_IsEnabled`, unbound. `WillowPickup` answers its `bPickupable`;
    the field fits under a fitted 16-byte alignment of `RBState`.
- **Proposed Off answer:**
  - the `new` template rule as an interpreter change, with the decoder change (both the hub's call), with a
    write-free and load-free preflight;
  - `Pickupable_IsEnabled` as a plain read;
  - tests listed.
- **Next** (a prediction, not a run): unchanged today. After option B and the gun's init, source f's `LayerAbsent`.
  Then `Pickupable_IsEnabled`.
- **Review r1 (#743):**
  - the component-instancing predicate is named (class flag bit 23 in the store's class flags) and refused until
    read; links 6 and 7 trip it;
  - a class default that is not resident is refused, every refusal comes before the first write, and a failure after
    the object is made is named;
  - the operand order is the handler's, all five before any check, replacing main's;
  - the structural oracles cannot tell four operands from five; an argument count against declared parameters does
    (5 overflows under four, none under five);
  - the `Quat` alignment question is widened to every struct holding one;
  - a template must be exactly the new class;
  - keeping the "template has no mesh" refusal is the hub's call, and it stays refused.
- **Clean room:** a different lane implements this note.
