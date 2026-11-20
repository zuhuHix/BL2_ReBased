## 2026-11-19: Actor.AttachComponent bound in the skill worlds, the pickup worlds' rule reused

AI-assisted (Claude Code)

Lane BIND-ATTACHCOMPONENT-SKILL-WORLDS, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. No new rule, no dependency, and no bounds check changed. Clean room: this lane did not write or analyse
NATIVE_ATTACH_MESH_TO_PICKUP.md; it reuses that note's existing implementation.

- **Why:** with the probe's stand-ins (NATIVE_ACTOR_TRACE.md section 4), link 9's `cast` stopped at the unbound
  `Actor.AttachComponent(object)`: `LiftActionSkill.BeginLifting` attaches the phaselock light to the target. The
  record and the binding existed only in the pickup worlds (`ow_inventory::attach_mesh::register`, decision
  2026-11-19-123). `phaselock_world` registers the Core, skill-tree and action-skill packs only.
- **What:**
  - `ow_inventory::attach_mesh::attach_native` is now public, so there is one implementation.
  - `ow_skills::action_skill::Pack::register` binds it in Off under `Engine.Actor.AttachComponent` through the
    keep-existing `ow_combat::hit::bind`.
  - `attach_mesh::register` now skips the key when it is already bound, so a world that binds the action-skill pack
    first is not refused as a duplicate.
  - Reference is unchanged: neither site binds anything in Reference.
- **Preflight check (for the skill-world receiver, a light component on a pawn):** `attach_plan` takes `&Vm`, so it
  can neither write, load nor draw. It reads only the heap, the class declarations and `check_value`, and every
  refusal comes before the first write. A refusal after the first write is marked through #518's counters as before.
- **The scene layer:** the rule still needs the world's scene layer (`attach_mesh::SceneLayer`). `phaselock_world`
  installs none, so in that world the bound rule refuses layer-absent, before any write. Whether the link 9 world
  should take the pickup worlds' empty-scene ruling is the hub's call; this lane does not add it.
- **Tests:** `crates/ow-skills/src/action_skill/attach_component_tests.rs`, synthetic, in a world built with the
  action-skill pack:
  - the light attach to a pawn, with no draw and no load;
  - no scene layer gives layer-absent, and an attached light gives not-read, each with #518's three counters and the
    fields unchanged;
  - both combined registration orders keep one binding;
  - Reference binds nothing.
- **UNVERIFIED** in the running game, as the rule itself is.

## Edits: review r1 of #623

- **The reuse is narrowed.** `attach_mesh::register` skips the key only when its single binding is
  `bind_engine_attach`'s. That function is in `ow-inventory`, binds `Engine.Actor.AttachComponent` alone, and records
  that it did. Any other binding of the key, such as a conflicting handler on another package's function, keeps the
  duplicate-registration error. `attach_native` is private again, and the action-skill pack calls
  `bind_engine_attach`.
- **New read-only accessor:** `ow-vm` `Vm::package_bindings(key)`.
- **The orders differ by design:**
  - inventory first binds the key VM-wide;
  - the action-skill pack first binds `Engine`'s function alone, and inventory then reuses it.

  The order test covers both with a second invented package, and a test with a conflicting handler keeps the
  duplicate error.
- **The success test** asserts exactly four committed writes.

## Edits: review r2 of #623

- **The key is checked.** `bind_engine_attach` binds, and records the reuse, only when `Engine.Actor.AttachComponent`
  is declared under `Actor.AttachComponent(object)`, the rule's key. A function declared under another signature gets
  nothing, so a conflicting handler that holds the key elsewhere still gives inventory its duplicate error. A test
  covers the mismatched signature.
- **A known order difference, left in the code.** With the action-skill pack first, inventory reuses the `Engine`
  binding and binds no other package. A same-keyed function in another package then stays unbound, where inventory
  first would bind it VM-wide.
  - Promoting the binding to VM-wide would need a new `ow-vm` operation that replaces a package binding. That is more
    than a small change.
  - The slice's worlds use `Engine`'s `Actor`. Whether any other installed package declares a same-keyed
    `AttachComponent` was not checked.

Landing (hub ruling, 2026-11-20): #610 makes `World::new` install the empty scene layer in Off, so a skill world built by `World::new` can no longer lack it; the no-scene-layer refusal is covered by #610's test in `ow-world/src/actors/component_attach_tests.rs`, and this lane's test now asserts that the attach answers in the skill world (`the_skill_world_s_own_scene_layer_lets_the_attach_answer`).
