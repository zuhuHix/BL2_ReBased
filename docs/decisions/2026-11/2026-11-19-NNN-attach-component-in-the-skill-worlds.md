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
