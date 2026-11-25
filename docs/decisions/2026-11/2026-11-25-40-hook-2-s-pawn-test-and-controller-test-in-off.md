## 2026-11-25: Hook 2's pawn test and controller test in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-BODYCOMP-HOOK2-PAWN-TEST, on integration/2026-11-25-c15
(25425bb9). It implements docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md section 6 (lane F's amendment, #699, at
44532835 after review r1), from the note only. No real-game session was run. **Every rule is UNVERIFIED in the
running game.** Off only: the body composition's preflight is Off's, and Reference keeps its stand-ins.

**What changes.** `ow-world` `render_hooks::owner_no_see` (the body composition's hook 2) drops the `PAWN_TEST`
refusal ("not identified") and the `IsLocalPlayerController` check, with that check's two refusals (no local-player
list, and a controller outside it whose `Player` is set). In their place, read only and load-free:
- **the pawn test** (`Pawn.IsPlayerPawn`, by the pawn's first native class, the fitted native-bit walk that
  `set_physics` uses). `WillowPlayerPawn` passes. `Pawn`, `GearboxPawn`, `WillowPawn` and `WillowAIPawn` pass when
  `Controller` is set and its `bIsPlayer` is set. Any other class is undecided;
- **the `PlayerController` cast**, judged by the class the `Controller` handle carries, so a lazy handle is judged too;
- **the controller test** (`PlayerController.UsingFirstPersonCamera`, by its first native class).
  `WillowPlayerController` passes when `bBehindView` and `bViewingThirdPersonMenu` are both clear. `PlayerController`
  and `GearboxPlayerController` are undecided (the engine's camera test, not read), and so is any other class;
- a `Controller` the store has not loaded is undecided where a test must read it (a `Pawn`-body pawn test, or a
  `WillowPlayerController`'s controller test).

**The order** is the note's exact sequence:
1. primitive;
2. the pawn test;
3. the cast;
4. the controller test;
5. the template settled (`NOT_SETTLED`);
6. `plan_flag(bOwnerNoSee, true)`.

A decided failing test is nothing. An undecided part's `NotYet { NotRead }` is raised only when every decided part
passed, the template is settled and the planner would write; the first one in gate order wins. Where everything
passes, the setter's deferred reattach refuses first (`REATTACH_OWNER`, since the attach sets `Owner`), so hook 2
still writes nothing. A missing field, or one not declared as a bool (`bIsPlayer`, `bBehindView`,
`bViewingThirdPersonMenu`), is a `VmError::Script` naming it.

**The refusal texts:**
- "the owner-no-see gate's pawn test (IsPlayerPawn) on <class>";
- "the owner-no-see gate's camera test (UsingFirstPersonCamera of PlayerController)", or of `<class>`;
- "the rendering hooks: an object the store has not loaded (a load is a write)".

All have the note as record.

**Tests** (`world/body_composition_tests.rs`, synthetic). Section 6.2's eleven items, each refusal write-free,
intern-free, load-free and free of class-code loads. Two existing tests changed, as the note says:
- section 4's test 6 is now the setter's `REATTACH_OWNER`;
- test 7's non-local case is gone (test 3 covers it: a controller with a `Player` set and in no local list passes).

The fixture marks the hand-declared `WillowPawn` native, adds `WillowPlayerPawn` (hand-declared, native) and a package
`WillowPlayerController`, gives `Engine.Controller` a `bIsPlayer`, and adds two lazy controller handles.

**What link 1 would meet** (the note's prediction, not a run): Maya's pawn and controller pass, so her `Mesh` template's
`bOwnerNoSee` decides between nothing (then hook 3's array check) and `REATTACH_OWNER`. On pc-hummer link 1 stops
earlier, at `maya:8`'s missing reference skeleton (no UModel export here), so the slice gate does not reach hook 2.

**Gates.** `cargo test --workspace -j 8`: 3,500 passed / 0 failed / 55 ignored. `crate_edges_test.py`: 19 OK.
Installed Off strict `--slice-gate`: 5 ok / 8 NotYet / 1 error, X 41.3%, every row as c15's own run. This is what the
host did, not parity.
