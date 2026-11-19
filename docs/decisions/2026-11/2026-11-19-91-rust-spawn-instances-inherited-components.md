## 2026-11-19: Rust spawn instances a component inherited from a parent class's default

AI-assisted (Claude Code, Claude Opus 5.5). Lane INHERITED-COMPONENT-INSTANCING, implementer. Number 91 was given by the
hub. The lane read no native code and opened no `local/analysis/`, Ghidra or REA output. There was no real-game
session. It adds no dependency, CLI flag or parsing change, and Reference is unchanged (`instance_components` runs in
Off only). The branch is stacked on `fix/rust-link2-fight-area` (#470, decision 2026-11-19-76), whose driver it
changes and needs for its installed proof.

- **Why.** Decision 2026-11-19-44 refused, with a named `NotYet`, a template slot that names a component of a
  parent class's default object. On the installed game that refusal stopped link 2's target dummy
  (`SpecialMoveComponent0` of `Default__WillowAIPawn`) and stands in front of the enemy spawns of link 3.
- **What the packages show** (structural oracle, the export tables only, outputs under ignored `local/p2/inherited/`):
  - The dummy archetype (`WillowGame.WillowAIPawn`) has 13 sub-object exports of its own. Five of them have an
    export archetype, each the same-named component of `Default__WillowAIPawn`: `SpecialMoveComponent0`,
    `GearboxDialogComponent0`, `StatComp`, `MyLightEnvironment` and `CoverComponent0`.
  - Loaded, the archetype's slots name both sets: the five class-default components, inherited from the class
    defaults because the archetype's export does not serialize those fields, and its own sub-objects.
- **The rule (public stock UE3, a port decision, UNVERIFIED in this game).** UE3's loader settles such a reference
  when it loads the template: the template's own sub-object whose archetype is that component stands for it, since
  the instancing step matches existing sub-objects by archetype. The port does the same in `plan_components`
  (`crates/ow-world/src/actors/instancing.rs`), by export identity, never by name:
  - the template's own sub-objects are the exports directly outered to its export identity, and each one's export
    archetype is resolved with `CodeIndex::resolve_ref`;
  - a slot naming a class default's component whose export is one of those archetypes stands for that sub-object.
    The sub-object is loaded after the template's own slots are judged (the plan's pass 1 still loads nothing), and
    must be a component the template owns, by handle or export identity. It is copied for the new actor like any
    other template component, and the inherited reference is re-pointed to that copy at any depth;
  - kept from decision 44: the transitive search, the slot rules, the refusals, and the error on a field outside the
    object's slots. Where the template has no such sub-object, the reference stays a refusal (now citing this
    record), and so does a sub-object another file holds or that has another outer.
- **The driver** (`ow-cli` `cmd::slice_gate::fight_area`):
  - The dummy archetype is now the store's own object for its path (`DefStore::find`, the identity policy's holder).
    It was a copy instantiated from `Sanctuary_Dynamic`'s export. Under the policy the copy's slots resolve to the
    holder's components, so the copy owned none of them, and decision 44's rules shared them silently as another
    archetype's.
  - After the spawn, the step checks that the dummy's `SMComponent` and `CylinderComponent` are its own: outered to
    it, and neither the template's nor the class default's. Otherwise it is an `error`.
- **The bl-sdk check that would confirm it** (no session was run): on a spawned target dummy, its
  `SpecialMoveComponent` (`SMComponent`) is a distinct object from the archetype's and the class default's, outered
  to the pawn. Compare a field that the archetype's own `SpecialMoveComponent0` sets differently from
  `Default__WillowAIPawn.SpecialMoveComponent0` (picked from that export's own tags): the spawned copy is expected to
  hold the archetype's value, not the default's. Matching the default's values would not tell the two readings apart
  (review r1 of #478, item 5).
- **Verified (automated, local, desktop).**
  - **Synthetic tests** (ow-world, invented classes and exports, rule 9):
    - `an_inherited_component_is_instanced_from_the_templates_own_sub_object`: the plan pairs the inherited reference
      with the template's own sub-object; the spawned actor's copy is outered to it and holds the sub-object's values;
      the template and the parent default's component are unchanged; a second actor gets another copy;
    - `an_inherited_component_without_a_stand_in_is_refused`: `NotYet { NotRead }` citing this record, with the heap
      and level-list counts unchanged.
  - **Gates:** `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,190 passed, 0 failed, 46
    ignored; clippy flags nothing new in the changed files; `crate_edges_test.py` 19 OK;
    `check_slice_run_off_test.py` OK.
  - **Installed, Off, `--slice-gate --cooked <CookedPCConsole>` (strict), under the machine run lock:**
    - before (#470 at 6268e6d): link 2 `notyet`, 3 of 4 steps passed, stopped at `fight_area` with decision 44's
      inherited-component `NotYet`;
    - after: link 2 **`ok`**, 4 of 4 steps (load, spawn, walk, fight_area). Maya's walk ends 9.5 uu from the centre,
      the floor and reach checks pass, and the dummy stands 0.0 uu from the centre and owns its components;
    - link 3 is unchanged: `notyet`, 0 steps, stopped at `maya:8` (`ApplyCharacterClassStartingValues`'s unbound
      `InitPlayerSkillTree`). Its enemy spawns are not reached, so this rule's effect on them is not seen yet;
    - the other 13 rows are byte-identical; 1 ok, 13 notyet, 0 error.
- **Open.**
  - Link 3's enemy spawns, once `maya:8` clears: a class-default template's own sub-objects come from its
    `Default__<Class>` export, so the same match applies, but no installed run reached it.
  - Other drivers that spawn from a copy of a cooked archetype, rather than the store's holder, share the holder's
    components (decision 44's "another archetype's component" rule). This lane changes only link 2's driver.
  - A real-game reading (the bl-sdk check above) would confirm the rule.
- **Review r1 of #478** (on 07792e4, rebased onto #470 r2 at 22cc2b9):
  - **A unique stand-in.** A template with two sub-objects made from the inherited component's export is ambiguous,
    and the plan refuses it with `NotYet { NotRead }` instead of taking the first.
  - **The stand-in's identity.** The object the store resolves for the selected child must be that child by
    canonical export identity (its file and export index). Another file holding the same path is refused. This is a
    code check; no synthetic test covers it, because the fixture would need a second file of the same package name.
  - **Nested in an inherited component.** A slot naming an object inside a class default's sub-object (for example
    an object outered to an inherited component) is refused, instead of shared into the parent default's graph.
  - **Typed and ordered in the driver.** The definition store's errors convert with `VmError::from`, so a `NotYet`
    stays one. The template's and class default's components are read before the spawn, and the new actor's own
    components after it. The dummy's cylinder is read from the policy's holder file (the first in scan order).
  - **The commit boundary.** In Off, once the actor is in the level list, a `NotYet` from the writes and events after
    it is terminal, with "stopped after partial writes: " at the start and its kind and record kept. Reference is
    unchanged. On main this boundary came with review r1 of #463 (`after_insertion`), so the rebase keeps main's code
    and this branch adds only the test. Review r2 of #478 moved the boundary earlier: a lazy definition load is a write, and an Off spawn can
    load before it makes the actor (the archetype, a template component in the plan's second pass, an inherited
    component's stand-in). Once one of those loads has run, every `NotYet` the spawn returns gets the same prefix. A
    stand-in resolution counts as a load whether or not its handle was loaded before, because the store does not say
    (conservative). Test: `a_refusal_after_the_spawns_lazy_load_is_prefixed`.
  - Synthetic tests: `an_inherited_component_with_two_stand_ins_is_refused`,
    `an_object_inside_an_inherited_component_is_refused` and `a_not_yet_after_registration_is_prefixed_in_off`.
