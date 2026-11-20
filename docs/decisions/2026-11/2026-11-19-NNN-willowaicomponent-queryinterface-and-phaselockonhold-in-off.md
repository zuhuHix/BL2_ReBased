## 2026-11-19: The AI components take the component QueryInterface rule, and WillowAIComponent.PhaseLockOnHold, in Off

AI-assisted (Claude Code)

Opus 5.5. Lane IMPL-WILLOWAICOMPONENT-QUERYINTERFACE, slice link 9. The code follows
docs/verification/NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md and nothing else: #600 at c24a2a51, then its review r1
head e7dcdde, which adds the ruled steps. Clean room: lane ANALYST-WILLOWAICOMPONENT-QUERYINTERFACE read the
natives. This lane did not open the executable or the analysis output. The fixtures are synthetic.

- **The component rule (`ow-combat` `pawn_interface`, Off only; Reference unchanged).** `COMPONENT_RULE_CLASSES` now
  also lists exactly `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent`, so both answer by
  NATIVE_PAWN_AI_INTERFACE.md section 6.4. That covers the plain answer, the unloaded-receiver check, the `Owner`
  declaration and the owner's answer, refusals included. Their subclasses keep the open `NotYet { NotRead }`, because
  the test is on the exact class.
- **The unattached owner (the hub's ruling, 2026-11-19).** This applies only to those two classes. Once `Owner` reads
  None, the rule reads `bAttached` (declared on `ActorComponent`) with `vm.space.prop` and `vm.space.read`, loading
  nothing. Its declaration is judged at registration, beside `Owner`'s (`ComponentDeclarations`):
  - not declared, or not a bool: `VmError::Script`, "ActorComponent.QueryInterface: <class>.bAttached is not declared
    as a bool";
  - true: none;
  - false: `NotYet { NotRead }`, "the component <path> has no Owner and is not attached: the attach step that sets
    Owner is not ported (asked for <interface>)", record NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md.

  The port cannot tell a component the game never attached from one whose attach step the port does not run, so the
  rule refuses rather than answering none. `ActorComponent` and `SkeletalMeshComponent` keep "Owner None answers none",
  and they never read `bAttached`. #610 (#605's note) gives a spawned mind's component its `Owner`, so after it the refusal is
  reachable only for a map-placed mind's component. The tests cover both cases.
- **`WillowAIComponent.PhaseLockOnHold()` (`ow-combat` `holds`, bound in Off by the holds pack).** The steps, in the
  hub's order:
  1. no object: `VmError::Script`;
  2. a class other than exactly `WillowAIComponent`: `NotYet { NotRead }`, "PhaseLockOnHold on a <class> is not read";
  3. `PhaseLockHolds` undeclared, or not declared an array: `VmError::Script`. This is judged on the declaration,
     before the next step;
  4. an unloaded receiver: `NotYet { NotRead }`, and it is not loaded;
  5. `Bool(length > 0)`, read with `vm.space.prop` and `vm.space.read`. It does not reuse `holds::this` or
     `read_list`, which load.
- **Tests** (invented classes and packages, rule 9). Every question and call asserts that #518's three counters (heap
  committed writes, generator draws, load transitions) are unchanged.
  - `pawn_interface::tests`: the hand-defined AI classes sit under the invented `Engine.upk`'s decoded
    `ActorComponent`, which declares `bAttached` as a bool, an int or not at all. The cases:
    - an owned component, attached or not, answers its owner's answer;
    - a subclass gets the open NotRead;
    - a class the component derives from is its plain answer;
    - a declining owner gives none, and a question that comes back to the component gives none;
    - Owner None and not attached is the ruled NotRead, and Owner None and attached is none;
    - `ActorComponent`'s rule on a `SkeletalMeshComponent` with Owner None and `bAttached` false is still none;
    - an undeclared or int `bAttached` is the Script error when Owner is None, and is not read when Owner is set.
  - `holds::tests`, on an invented `WillowGame.upk`:
    - empty is false, and one entry is true;
    - no object is Script, a subclass is NotRead, and an unloaded receiver is NotRead and stays unloaded;
    - an undeclared or int `PhaseLockHolds` is the Script error for a resident receiver and for an unloaded one, so
      the field check comes first.
- **Slice probe** (installed Off `--slice-gate`, strict, under the lock, on a throwaway merge with #601 at e1c9f0e4 and
  #602 at 3e7c1294): 3 ok / 10 NotYet / 1 error. Link 9 runs both of its driver's 2 steps. `spawn` passes, and `cast`
  stops at "stopped after partial writes: NounAttributeValueResolver with ReplicationStrategy 2 on a provider"
  (NATIVE_NOUN_ATTRIBUTES.md). The same merge without this branch gives byte-identical output, so this branch moves
  no slice stop yet: the cast stops before it asks an AI component anything.
- **Landing notes** (corrected on review r1 of #614).
  - **#601** conflicts in `holds.rs` and `holds/tests.rs`. The `holds.rs` registration hunk is not a union: #601
    replaces the `GET_AI_COMPONENT` NotYet stub with `bind(vm, GET_AI_COMPONENT, Rc::new(get_ai_component))`, and
    this branch keeps the stub as context after its new `PHASE_LOCK_ON_HOLD` bind. Resolve it to this branch's
    `PHASE_LOCK_ON_HOLD` bind line plus #601's `GET_AI_COMPONENT` bind, and drop the stub. Never keep the stub with
    a `?;`: `bind` keeps the first binding, so the stub would silently shadow #601's native. The other two hunks
    are a union.
  - **#610** also conflicts in `holds/tests.rs`. Both sides append tests at the end of the file, so it is a union.
  - **#602:** `an_owned_ai_component_answers_its_owner_s_answer` must first resolve the controller's interface table
    (`resolve_interface_tables`), as #602 does in `a_direct_mesh_query_forwards_to_its_owner`, because #602's
    `Controller` rule reads only entries resolved at set-up.
- **Verified:** only the host tests and the slice probe. **UNVERIFIED in the running game:** every rule here. The
  note's live checks (a placed and a spawned mind's component, its `Owner` and `bAttached`) would confirm the
  attach-state reading.
