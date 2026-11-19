## 2026-11-19: Rust spawn gives each actor its own copy of its template's components

AI-assisted (Claude Code). Lane SUBOBJECT-INSTANCING. It answers open question 1 of
`NATIVE_SPECIAL_MOVE_COMPONENT.md` (decision 2026-11-19-33's finding): a pawn made from class defaults shared the
class default's `SpecialMoveComponent0` instead of owning a copy. The rule is **public stock UE3 component
instancing**, not a rule read from this game. No native code or analysis output was opened, and there was no
real-game session, so it is **UNVERIFIED** in the running game. No dependency was added and no bounds check was
loosened. The `NotYet` guards changed are listed under "Guards" below.

- **What was added** (`crates/ow-world/src/actors/instancing.rs`, `instance_components`, Off only):
  - an actor made from a template (its class default object, or an archetype) gets its own copy of each component
    the template owns;
  - a copy has the same class and name, is outered to the new actor, and takes its slots from the template's
    component;
  - every reference to a copied component is re-pointed to the copy, at any depth (arrays, structs), in the actor's
    slots and in the copies' own slots. This covers `Components` and the named field holding the same object, as
    `SMComponent` and `Components` do.
- **Which slots count.** A slot counts when its declared class, or its array's inner class, is `Core.Component` or a
  subclass. That class is found by name. In public UE3 such a slot is a `ComponentProperty`, which the port's
  reflection reads as an object property of its component class. **No property flag is read**: the "instanced" bits
  are not identified in this port, and none is invented (rule 4).
  - Copied: an object in such a slot, of a component class, whose outer is the template.
  - Left as it is (a plain shared reference): an object whose outer is anything else, such as another archetype's
    component or an asset.
- **The template is matched by path.** The definition store holds a class default object (`Origin::Default`) and the
  handle of its `Default__<Class>` export (a `Resource` stand-in) as two heap objects of the same name and class. A
  CDO sub-object's outer is the export's handle, so the outer is compared by name and class up the outer chain. A
  candidate the store has not loaded is loaded first (its outer is set on load); a load that fails stops the spawn
  with the store's error. The two-object identity is the store's (`ow-defs`), and is left for its owner.
- **UNVERIFIED, left shared** (the rule that would cover them is not read):
  - a component the template holds only inside a struct;
  - a non-component object outered to the template (public UE3's `instanced` objects, which need the unidentified
    flags);
  - a component's own sub-objects (nested).
- **Spawn** (`crates/ow-world/src/actors.rs`, `spawn`):
  - In Off the actor is made from the template's slots, then `instance_components` runs.
  - An archetype (a template other than the class default) is now spawned in Off, as the brief asks and the hub
    confirmed in review: public UE3's spawn copies its template's properties. **UNVERIFIED** in this game. The collision
    pre-checks still read the class defaults; for an archetype whose flags differ, that is UNVERIFIED.
  - Under Reference nothing changes: `instantiate_from_defaults` as before, and an archetype stays
    `NotYet { NotRead }`, because the C++ stand-in does not copy a template's sub-objects (R3).
- **Guards:**
  - Off's archetype refusal in `spawn` is lifted, as the brief's "Do" states.
  - `GearboxPawn.GetSMComponent` keeps its check (`crates/ow-skills/src/action_skill.rs`). A component outered to the
    pawn is answered, and a spawn-instanced copy is one. A shared template component is still
    `NotYet { NotRead }`, now saying the pawn was not made by a spawn that instances, with question 1 labelled as
    answered by stock UE3.
  - No other guard changed.
- **The installed Phaselock test** (`phaselock::installed::installed_game_maya_skills_duration_and_lift`): its cast
  part makes the controller, Maya's pawn and the target as the spawn does in Off (`spawned_of`: class defaults, then
  `instance_components`). The other parts are unchanged.
- **Tests** (invented classes, rule 9):
  - `actors::instancing::tests`, 4 tests:
    - own components copied with class, name and values;
    - the named field, `Components` and a copy's sibling reference re-pointed;
    - another archetype's component, a non-component sub-object and a nested sub-object left shared;
    - the template unchanged;
    - each actor gets distinct copies;
    - no `Core.Component`: nothing copied;
    - the template matched by path, not by a same-named object of another class.
  - `world::tests::an_archetype_template_spawns_from_its_slots_in_off_only`: Off takes the archetype's value;
    Reference stays `NotYet { NotRead }`.
  - `GetSMComponent`'s two tests are unchanged and pass.
- **Installed, before and after** (pc-hummer, main e59bff01; outputs under ignored `local/p2/subobject-instancing/`):
  - Off: one line changes. The cast (`LiftActionSkill.OnActionSkillStarted`) stopped at `GetSMComponent`'s
    question-1 `NotYet`. It now stops at the unbound `Pawn.IsFriendly(object)`, the first native of
    `CanPhaseLockTarget`, as the note foresaw. `SpecialMoveComponent.Play` is not reached yet.
  - Reference: byte-identical.
- **`--slice-gate`** (merge-base e59bff01's binary against this branch's): the rows are byte-identical, strict
  0/11/3 and past-errors 0/14/0. Every Off spawn there stops at `WillowPawn.SetDefaultPhysics` first, and link 9 has
  no headless driver.
- **The note:** `NATIVE_SPECIAL_MOVE_COMPONENT.md` open questions 1 and 3 get status lines (port decision by stock UE3;
  the seen next stop).
- **Review r1 of #435.**
  - **Spawning from an archetype in Off:**
    - Before anything is made, the archetype is loaded and checked. A live actor (one in the level list) as the
      template, or a template whose slot count is not the class layout's (a stand-in with no properties loaded), is
      `NotYet { NotRead }`.
    - The collision pre-checks (spot search, encroachment, base search) refuse when either the class defaults or the
      archetype asks for them. Which of the two the game reads is not read: UNVERIFIED.
  - **Instancing:**
    - Components are found transitively: the actor's component fields first, then each found component's own, with
      the same outer test, until nothing new turns up. So no copy is left pointing at a template component.
    - Each field's slots come from its recorded first slot, and a field outside the object's slots is an error.
  - **Labelled UNVERIFIED** (in the code documentation):
    - a component inherited from a parent class's default stays shared;
    - references to the template itself, or to its `Default__` handle, are not re-pointed;
    - the actor-component reference loop is not freed by `release_unreferenced`;
    - a failure part-way through a spawn leaves the actor on the heap and in the level list.
  - **Tests added:**
    - an archetype that sets `bCollideWorld` or `bCollideActors`;
    - an archetype with no slots, and a live actor as the template;
    - a component reached only through another component;
    - an Off spawn from an archetype that has a component. This one uses a world fixture with a synthetic `Core.upk`
      for `Core.Component`.
- **Review r2 of #435.** The rule for Off is now: copy and re-point correctly, or refuse with a named `NotYet`; never
  share a reference silently. Three of round 1's UNVERIFIED labels are replaced by refusals: the inherited component,
  the reference to the template, and the actor left behind.
  - **Two steps.** `plan_components` runs every refusal before the actor is made. `apply_plan` makes the copies and
    re-points; if a copy cannot be made, the copies made so far are freed, and the spawn frees the actor. The fields
    the spawn writes are checked on the template before anything is made, so nothing is left on the heap or in the
    level list by a refusal before registration.
  - **Discovery.** A component is found by its run-time class (`Core.Component` by path along its own layout chain:
    no class lookup, so no lookup error turns into a spawn with shared references). It is found through any field
    type, struct, array, interface or delegate target.
  - **Delegates** bound to a copied component are rebound to the copy, at any depth.
  - **Ownership** is by handle, or by canonical export identity (package and export index), never by name.
    - For a class default object, that identity is the `Default__<Class>` export of its class's own package. The
      installed `WillowPlayerController`'s `CollisionCylinder` is outered to that export's handle, another heap object
      than the class default.
    - `DefStore::find` on that path gives the class default itself, so it cannot stand in for the export's handle.
  - **New `NotYet { NotRead }` refusals**, each naming the object:
    - a reference (or delegate) back to the template;
    - a non-component sub-object of the template;
    - a sub-object of a template component (nested instancing);
    - a component owned by another class default object;
    - a destroyed or pending-delete (`bDeleteMe`, `bPendingDelete`) template;
    - a template with `Owner` set: the spawn's owner rules for a template are not read.
  - **Tests (ow-world):**
    - delegates rebound, also inside a struct;
    - a component held through an `Object` field or a struct;
    - each refusal with the heap count unchanged;
    - ownership by handle, not by name;
    - no `Core.Component`;
    - templates that are destroyed, pending-delete, owned, or that hold a non-component sub-object, each with the heap
      and level-list counts unchanged.
    - The class-default export identity is covered by the installed run only: a hand-made object has no export.
  - **Installed** (pc-hummer):
    - Phaselock Off: the cast still stops at `Pawn.IsFriendly(object)`. Before the export-identity fix it stopped
      at the controller's `CollisionCylinder`, wrongly refused as another default's.
    - Reference: byte-identical.
    - `--slice-gate`: byte-identical, 0/11/3 and 0/14/0; link 9 is `notyet`, with no headless driver.
- **Review r3 of #435** (the delta review of round 2):
  - **Nested sub-objects** are judged on each object's whole outer chain, through the heap's outers and then the
    export table's. A sub-object of a template component, at any depth, is refused even when a template slot names it
    before the component is found.
  - **The level list** is borrowed for the registration before anything is allocated, and released right after the
    insertion, before any event. A list another caller holds refuses the spawn with nothing made.
  - **Loading.** The plan judges every reference in the template's own slots before it loads anything. Template
    components are loaded afterwards: they are the template's definition, read as the archetype itself is.
    - Residual: a refusal found inside a component's own fields comes after that component's load. The store's loads
      cannot be staged.
    - The reviewer's other option, refusing every component not yet loaded, would refuse every installed
      class-default spawn: their components are all lazy.
  - **The class-default export** is chosen by the decoder's own rule, now shared as `ow_object::cdo::default_export_in`
    and `is_default_export`: the exact name and the class reference, first in table order. Before this round the
    instancing took the first top-level, case-insensitive name match.
  - **A template component whose slot count is not its class layout's** (a stand-in) is `NotYet { NotRead }`, never
    copied as an object with no slots.
  - **Tests:**
    - a nested reference met first, and a deeper descendant;
    - a component stand-in;
    - a level list held during a spawn that would allocate an actor and a copy;
    - a lazy archetype export, whose self-reference is refused with its component still unloaded and nothing logged;
    - `default_export_in` passing over a same-name export of another class (ow-object).
