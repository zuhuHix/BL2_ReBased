## 2026-11-25: The deferred reattach's owner rule in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-DEFERRED-REATTACH-OWNER, on main f29f2c8e. Coded from
NATIVE_ATTACH_MESH_TO_PICKUP.md section 2.1 alone, as amended in #709 (decision 2026-11-25-45, analyst lane
ANALYST-DEFERRED-REATTACH-OWNER). Clean room: this lane is not its analyst, and no native code or analysis output was
opened. Every rule is UNVERIFIED in the running game.

- **What was decided:** one owner rule for every caller of the deferred reattach, `ow-world`
  `component_setters::owner_rule`. With `Owner` set (read through the caller's slots, so a template plan sees the
  attach's stand-in):
  1. the owner must be resident, else `NotYet { NotRead }`, "the deferred reattach: an owner the store has not loaded
     (a load is a write)", with nothing loaded;
  2. its `bStatic`, `bTickIsDisabled` and `bDeleteMe` are read by name from the instance, each declared as a bool,
     else a data error naming it;
  3. `bStatic` or `bTickIsDisabled` set, with `bDeleteMe` clear: the existing `REATTACH_OWNER` refusal, its text and
     record unchanged. The owner's pending-kill and template-object tests are not modelled, so the port refuses on the
     side where the game may update, as the note says;
  4. otherwise no refusal: the calling native writes its field and then `bNeedsReattach`.

  The no-owner branch (`REATTACH_SCENE` for a non-zero `Scene`) and Reference are unchanged.
- **The sites, as the note's table lists them:**
  - `component_setters::reattach_check`, behind `set_flag` (`SetHidden`, `SetOwnerNoSee`) and `SetForceRefPose`;
  - `component_setters::plan_light_environment` (an attached `SetLightEnvironment`);
  - `render_hooks::owner_no_see` (hook 2): its unconditional `REATTACH_OWNER` fallback is gone. Past the owner rule
    it returns its plan. The body composition's write phase applies the plan (`bOwnerNoSee`, then `bNeedsReattach`,
    through the setter's apply side) on the made component, after the custom head and before hook 3, under #518's
    partial-write marking;
  - `particle_activation.rs` and `pickup_rb.rs` already applied this rule and are unchanged, and so is
    `particle_activation`'s per-component update refusal.
- **Tests:**
  - the nine tests the note lists as changing now expect the rule:
    - in `ow-inventory` `attach_mesh_tests.rs`, an ordinary owner passes, a static or tick-disabled one refuses, and
      a deleted one passes;
    - in `ow-world` `body_composition_tests.rs`, hook 2's plan is applied, and a tick-disabled pawn refuses;
    - in `ow-combat` `spawn_attach_tests.rs`, a spawned copy's ordinary owner passes and a static one refuses;
  - one new test covers 2.1.2 items 1, 4 and 5: only the component is touched, an owner not loaded is refused, and
    owner flags of another kind or missing are a data error.

  The fixtures' toy `Actor` classes gained the owner flags (synthetic).
- **The slice gate** (installed Off strict, under the machine lock, main f29f2c8e and this head, run the same way): 5
  ok / 9 NotYet / 0 errors on both.
  - Link 1 moves in `maya:8` from "the deferred reattach: the owner's component update" to hook 3's "the
    light-environment step's array check (not identified)" (NATIVE_BODYCOMP_RENDER_HOOKS.md). Maya's pawn instance is
    an ordinary owner, so hook 2's plan is applied.
  - Links 3, 4, 5, 13 and 14, behind link 1, report the same new stop.
  - Every other row is unchanged.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
