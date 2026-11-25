## 2026-11-25: Hook 3 through SetLightEnvironment; a made static mesh component is not read

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-HOOK3-ARRAY-CHECK, stacked on fix/rust-deferred-reattach-owner
(#714, at 1802cca6 after its review). Coded from NATIVE_BODYCOMP_RENDER_HOOKS.md section 7 alone (#715, analyst lane ANALYST-LINK1-NEXT).
Clean room: this lane is not its analyst, and no native code or analysis output was opened. Every rule is UNVERIFIED
in the running game.

- **What was decided:** hook 3 (`ow-world` `render_hooks::light_environment`) keeps its preflight gate: the pawn's
  `LightEnvironment` None, a made class that is not a `MeshComponent`, or `bAcceptsLights` clear is nothing, and the
  template must be settled. Past the gate, section 7.1 replaces the array-check refusal:
  1. a made `StaticMeshComponent` (or a subclass): `NotYet { NotRead }`, "the light-environment step on a static mesh
     component (its first LOD's lightmap)" (`STATIC_LIGHTMAP`). The port does not decode `LODData`. The
     lighting-channel step stays behind this refusal;
  2. any other mesh component: `component_setters::plan_light_environment` with the pawn's `LightEnvironment`, on the
     slots the attach produces (`Owner` the pawn, `bNeedsReattach` and `bNeedsUpdateTransform` cleared, `bAttached`
     as the template has it). The same value is nothing; not attached is the one `LightEnvironment` write; attached is
     the deferred reattach under the owner rule of decision 2026-11-25-49.

  The plan is held on the made component's `PostInit` and applied in the body composition's write phase after hook
  2's plan, through the setter's apply side, under #518's partial-write marking. `ARRAY_CHECK` stays defined and is no
  longer raised. Reference is unchanged.
- **Tests** (`body_composition_tests.rs`, section 7.2's ten): a skeletal component and a toy subclass take the pawn's
  light environment and nothing else (1, 3); the same value is nothing (2); a static mesh component is the new
  `NotRead`, write-free, and with `bAcceptsLights` clear it is nothing, as is a cylinder (4, 5); hook 2's writes and
  hook 3's on one component, after the walk's (6); a component `LightEnvironment` declared as an int, and an attached
  template on a class without the reattach fields, are the planner's script errors, write-free (7); an attached
  template takes `PreviousLightEnvironment`, `LightEnvironment` and `bNeedsReattach`, and a tick-disabled pawn is
  `REATTACH_OWNER` (8); a Maya-shaped two-attachment build passes attachment 0's hooks and stops at the `Parent` link's
  `NotRead`, write-free (9); Reference is covered by the existing suites (10). Section 4's old test 10 is replaced by
  tests 1 and 4. The fixture gained reattach-ready toy skeletal and static-mesh classes and `PreviousLightEnvironment`
  on its toy `PrimitiveComponent` (synthetic). The `stop_at_light` helper now stops at hook 3's `REATTACH_OWNER`.
- **The slice gate** (installed Off strict, under the machine lock, base 0bcf365e and this head, run the same way):
  5 ok / 9 NotYet / 0 errors on both.
  - Link 1 moves in `maya:8` from hook 3's "the light-environment step's array check (not identified)" to
    "InitializeBodyComposition: the Parent link's parent-bone map", unprefixed (write-free), as section 7.1 predicted:
    attachment 0 passes hook 3, and the build stops at attachment 1's `Parent` link (hub ruling 3).
  - Links 3, 4, 5, 13 and 14, behind link 1, report the same new stop.
  - Every other row is unchanged.

  The head was measured at 9ca895aa, on 0bcf365e. The branch was then rebased onto #714's review commit 1802cca6,
  which changes tests only.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
