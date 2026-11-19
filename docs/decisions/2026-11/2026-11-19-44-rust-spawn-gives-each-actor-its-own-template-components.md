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
