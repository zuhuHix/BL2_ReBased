## 2026-11-19: The body composition's rendering hooks in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-BODYCOMP-RENDER-HOOKS, implementer. The decision number is
pending from the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md` (#622, on main), section 4. Clean room: this lane
  implemented from the note and opened no native code or analysis output.
- **Base:** #631 (fix/rust-tree-init-owner-event at 36334758, accepted). This branch is stacked on #631, not on #618
  (#618 is on main). It is rebased onto main once #631 lands.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only (the body composition is Off-only);
  Reference is unchanged.

**What changed.**
1. The setters' Off rules (`SetHidden` / `SetOwnerNoSee`, `SetLightEnvironment`) moved from `ow-inventory`'s
   `attach_mesh.rs` into `ow-world` (`world/component_setters.rs`), split into read-only planners and apply
   functions. The checks, their order and the error texts are unchanged, and `ow-inventory`'s natives call them. The
   move is the note's answer to the crate cycle (`ow-inventory` depends on `ow-world`).
2. `world::is_local_player`, the Off `IsLocalPlayerController` answer as a read-only function. The native now calls
   it, with the same answers and refusals.
3. `world/render_hooks.rs` holds the three hooks, run in the body composition's preflight for each made component:
   hook 1 before the custom-head step, hooks 2 and 3 after it. None of them writes in Off: each does nothing or refuses
   before any write.
   - **Hook 1:** `WillowGame.IGearLikenessConsumer` must be defined at set-up, else `NotRead`. The made class is asked
     whether it implements it, read only and in `implements`' own order. An undecoded chain (synthetic packages)
     answers by the structural rule. Otherwise only entries already resolved answer, and an unresolved entry is
     `NotRead`. A hand-declared class answers through its nearest package parent, or no. Yes is the gear-likeness
     refresh's `LayerAbsent` ("a rendering layer").
   - **Hook 2:** the gate's known parts come first: a `PrimitiveComponent`, then the pawn's `Controller` (declared as
     an object), an `Engine.PlayerController`, local by the Off rule. A failing part is nothing. Then the template must
     be settled, and `SetOwnerNoSee(true)`'s planner runs on the slots the attach produces (`Owner` the pawn, the two
     update flags clear). A flag already true is nothing. Otherwise the pawn test is `NotRead`.
   - **Hook 3:** the pawn's `LightEnvironment` (declared as an object) None, or a made class that is not a
     `MeshComponent`, is nothing. Then the template must be settled, and its `bAcceptsLights` clear is nothing; set,
     the array check is `NotRead`.
4. The body composition's `RENDER_HOOKS` refusal of every made component (ruling 4) and its test switch are gone.

**Not done (the note's conditional items):** the lighting-channel branch and the one `LightEnvironment` write (note
test 13) stay out until the array check is read or the hub grants a harness input. Mode 3 never reaches the hooks,
because the walk refuses every mode other than 0 first. The interface name, the controller test and
`bAcceptsLights` are by fit, as the note says.

**Tests:** the note's tests 1 to 12 are in `body_composition_tests.rs`. The five tests that used the old refusal as
their stop now stop at hook 3's array check. `ow-inventory`'s setter tests are unchanged. The numbers and the slice
gate are in `docs/verification/GATES.md`, row IMPL-BODYCOMP-RENDER-HOOKS.

Landing amend (hub ruling, 2026-11-20): #641's socket tests no longer call the removed stub (`answer_render_hooks_for_tests`, 14 calls in `ow-world/src/world/body_composition_socket_tests.rs`; this lane's real hooks answer there); #625's `Want::Float` lines carried into `component_setters.rs`; #643's `record_ready` and `publish_attach` kept in `attach_mesh.rs`; no rule change.
