## 2026-11-19: Rust pawn QueryInterface rule for IAIInterface in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PAWN-AI-INTERFACE, implementer. Number 62 was given by the hub.
The branch is on `origin/main` bb46e11, which holds #441. The source is `docs/verification/NATIVE_PAWN_AI_INTERFACE.md`
as on main (decision 2026-11-19-51 and #441's two review rounds). Clean room: this lane did not read the natives and
opened no `local/analysis/`, Ghidra or REA output. There was no real-game session, so every rule is **UNVERIFIED** in
the running game. No dependency, no CLI flag and no parsing change. Reference is unchanged (R3).

- **Why.** `Behavior_AIHold`'s script asks its context with `Object.QueryInterface(IAIInterface)`. In Off a pawn
  context was a flat `NotYet` ("Pawn IAIInterface routing is unread"), and the native gave only the plain answer, the
  class's own interface table.
- **`ow-vm`: a class's own answer.** `Vm::register_interface_rule(class, rule)` binds a class's `QueryInterface`
  answer, which its subclasses use unless they have their own (the nearest registration wins); a second registration
  of one class is refused.
  - `Vm::query_interface`, which the native now calls, takes that answer, else the plain one, which is unchanged and is
    now `Vm::plain_interface` (the table, parents included, with the stand-in-class check of decision 2026-11-15-08).
  - **A question asked twice** (the note's section 2, UNVERIFIED): while an object is answering, a question that comes
    back to that same object and interface further down the chain gets none at once, and the asker goes on.
  - No rule is registered in `ow-vm` itself, so with no registration every class keeps the plain answer.
- **`ow-combat`: the new module `pawn_interface`**, registered in Off by `holds::register`, which both the combat pack
  and `ow-ai`'s hold pack call. Nothing is registered under Reference.
  - **`Engine.Pawn`, the note's section 2**, first answer that is not none:
    1. the pawn itself when its class or a parent lists the interface;
    2. its `Controller` when set, whatever that controller answers. Only when `Controller` is not set, the `Controller`
       of its `DrivenVehicle`. A declining controller does not lead to the vehicle;
    3. its `Mesh`, when set;
    4. otherwise none.

    The three fields are read by name and must be declared: a class without one is an error, never "not set".
  - **What stays `NotYet` in Off** (the note's "Open" list). Steps 2 and 3 hand the question to another object, which
    "answers by its own rule". That object is asked only when its class has a registered answer. Otherwise the step is
    `NotYet { NotRead }` naming the open answer: `GearboxMind`'s own (the Fire dummy's mind is a `WillowMind`),
    `WillowPlayerController`'s, a skeletal mesh component's, or for any other class, that class's own answer. No such
    answer is registered yet, so in Off a pawn answers itself, none (nothing to ask), or a named `NotYet`.
  - **`WillowGame.WillowAIPawn`** (section 3): `Pawn`'s rule, then, only when it declines and the asked class is
    `IPlayerMaster` (or derives from it), the player master step. Its field is identified by meaning only, so the step
    is `NotYet`. For `IAIInterface` it answers exactly as `Pawn`.
  - **`WillowGame.WillowVehicle`**: its own rule is in the note for completeness only and is not implemented, so a
    vehicle's question is `NotYet`, not `Pawn`'s rule.
- **The hold behavior in Off** (`holds::apply_hold_behavior`, which `--combat-run` and `ow-ai`'s hold run use).
  - Every context is judged before any script runs (the hub's rule: refusals before any write). A pawn's question is
    asked once ahead by the rule. Its `NotYet` stops the run with every hold list as it was, the earlier contexts
    included: before this lane they had already run.
  - A pawn the rule answers with none now runs the script, which then holds nothing, as the note says.
  - A pawn class with no rule registered is an error, so a world that forgot `holds::register` is not answered quietly
    by the plain rule.
  - Under Reference the pawn `NotYet` stays as it was, and other contexts with a `Controller` or `MyWillowMind` keep
    their `NotYet`.
  - Nothing in the rule writes: a `NotYet` from it leaves the world unchanged. No `NotYet` is possible after a write
    on this path, so the hub's "stopped after partial writes" case does not arise here.
- **The mission slice** (`ow-mission` `FireMissionSlice`, which `--slice-run`, `--mission-run` and `--slice-gate` build)
  registers `Pawn`'s rule itself (`pawn_interface::register`, Off only, keeping a rule already there), since its
  worlds bind no hold pack and its `Behavior_AIHold` handler is the one that asks. Without it a pawn context would be
  the "no rule registered" error there.
- **`ai_interface_for`** (`ow-combat` `hit`, Reference only) is kept as the C++ has it. Its documentation now lists
  where the note differs from it (the `Controller` field, the pawn's class first, a declining controller leading to the
  mesh, the vehicle then the mesh then none, every pawn class).
- **For the Fire dummy** (slice link 11): its controller is its mind, a `WillowMind` (a `GearboxMind`), whose own
  answer the note leaves open. So the hold stops at the named `NotYet` for `GearboxMind`'s answer, one step before the
  mind's `GetAIComponent`, which is itself still `NotYet` (NATIVE_AI_HOLDS.md). Reaching `GetAIComponent` needs a
  reading of `GearboxMind`'s own `QueryInterface` answer.
- **Verified (automated, local, desktop).**
  - **Synthetic tests**, invented classes and objects throughout:
    - `ow-combat` `pawn_interface::tests`, 8 tests on an invented `Engine.upk` built with `ow_package::testkit` (the
      Rust side of the `package()` / `tag()` helpers), whose class exports decode, so the interface tables are read.
      A stand-in for a read answer is a rule the test registers on an invented class. The branches:
      - a pawn whose class lists the interface, and one that only inherits it, answer themselves before their
        controller is asked;
      - a set controller answers; a declining one leads to the mesh and never to the driven vehicle's controller;
        with no mesh, none;
      - with no controller: the driven vehicle's controller; a vehicle with no controller leads to the mesh; nothing
        set answers none;
      - the open answers as named `NotYet`s: a `WillowMind` controller (`GearboxMind`'s own answer),
        `WillowPlayerController`'s, a plain controller class's, a skeletal mesh component's, and a vehicle's
        controller;
      - a controller that asks the pawn back gets none, and the pawn goes on to its mesh;
      - `WillowAIPawn`: `IPlayerMaster` after a decline is `NotYet`, and `Pawn`'s rule answers first;
      - `WillowVehicle` is `NotYet`;
      - registering again keeps the rule, and Reference registers none.
    - `ow-vm` `query_interface_takes_a_registered_class_answer`, through the `Object.QueryInterface` native: a
      subclass takes its parent's answer, the nearest registration wins, a class without one keeps the plain answer, a
      question asked twice gets none, and a second registration is refused.
    - `ow-ai` hold run, both modes: in Off a pawn with nothing to ask runs the script; a pawn whose controller is a
      `WillowMind` is `NotYet` naming its controller, with nothing run; Reference as before. The context-order test
      now expects nothing run when a later context refuses. The test world's `Engine.Pawn` gains a `Mesh` property.
    - `ow-mission` slice, both modes: the dummy's pawn with nothing to ask now runs the hold script in Off (its context
      is seen, `Idle_Hold` follows), where it was `NotYet` before; a pawn whose `Controller` is another object is
      `NotYet` naming its controller, with nothing run; Reference keeps the no-op. The toy `Engine.Pawn` gains
      `DrivenVehicle` and `Mesh`, and the toy `GearboxFramework` package an `IAIInterface` class.
  - **Gates.** `cargo build --workspace` clean; `cargo test --workspace` 2,099 passed / 0 failed / 45 ignored; clippy
    on the four touched crates adds no warning in a touched file; ctest at `-j 2` 173/173, 0 failed, 538.78 s (C++
    unchanged); verify_packages 9 packages, all class bodies exact; `tests/crate_edges_test.py` 19 OK.
  - **Installed, Off, before (main bb46e11) and after (this branch).**
    - `--slice-gate --cooked <CookedPCConsole>` (strict): byte-identical, 0 pass / 14 `notyet` / 0 fail. No row reaches
      `Behavior_AIHold`. Link 11, the Fire dummy's objective, stops first at `accept`, on
      `UIInteraction.GetDataStoreClient`. The combat links stop at `maya:8`, on
      `WillowPlayerController.ApplyCharacterClassStartingValues`.
    - `--mission-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire ... accept obj:RockPaper_GoToRange obj:Fire
      turnin`: the same before and after.
    - `--combat-run ... player:8 spawn:dummy:1 ...` does not spawn the dummy in Off: it stops at
      `TargetableAttributeValueResolver`, before any hit. The hold path is therefore covered only by the synthetic tests
      today.
- **Open.**
  - For an analyst: `GearboxMind`'s own `QueryInterface` answer (the next step for the Fire dummy),
    `WillowPlayerController`'s, a skeletal mesh component's, and the `PlayerMaster` field.
  - The note's other open items stay as they are: whether `Controller` and `MyWillowMind` can differ, and what
    `Controller` holds on a dead or unpossessed pawn. The rule reads only the field.
  - The prepared bl-sdk session in the note confirms each branch.
