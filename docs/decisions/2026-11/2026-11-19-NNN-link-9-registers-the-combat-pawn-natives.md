## 2026-11-19: Link 9's world registers the combat pack's pawn natives (IsStaggered, DropCombatPerch)

AI-assisted (Claude Code)

Lane LINK9-NEXT-STOP, with Claude Opus 5.5. The file number is a placeholder until the maintainer gives one. Stacked
on #650 (`fix/rust-link9-resolver-preload`), which sits on #644. No new rule, no dependency, and no bounds check
changed.

- **The stop:** on a local throwaway merge, never pushed, of main 241a6c47 with #644, #650 and #616's own commits
  (cherry-picked, since its resolve-context base landed in another form), link 9 errors at the unbound native
  `WillowAIPawn.IsStaggered()`. #611's Off binding lives in `ow_combat::hit::pools::register`, which only the combat
  pack calls. Link 9's world never registers that pack.
- **What:** a public `ow_combat::hit::register_pawn_natives` (a wrapper of the crate-private `pools::register`).
  `phaselock_setup` calls it last, after `ow_combat::holds::register`. It binds the pawn and pool natives the combat
  worlds bind: the class starting values, `SetDefaultPhysics`, `GetExpLevel`, `IsStaggered`, `DropCombatPerch` and
  the pool manager. It is Off only and keep-existing.
  - It is the port's own Off registration, not a harness stand-in, so it carries no harness label, like #644.
- **Test:** `link_9_s_set_up_binds_the_pawn_natives` (synthetic). The toy `WillowGame` package gains a `WillowPawn`
  declaring `IsStaggered`. Before the set-up the native is unbound and `Vm::prepare_call` refuses it; after, it is
  bound.
- **Measured** (installed Off `--slice-gate`, strict, release builds, under the machine lock):
  - **this branch:** ok 4, notyet 9, error 1. Link 9 is still at `AddAttributeModifier`, #616's native, which is not
    in this base;
  - **the throwaway merge with this change:** ok 4, notyet 10, error 0. Link 9 passes 1 of 2 steps. `IsStaggered`
    and `DropCombatPerch` answer, and `cast` stops with a NotYet at `Actor.Trace`'s T5, "the colliding components'
    bounds" (NATIVE_ACTOR_TRACE.md: the headless world holds no component bounds). That is a world layer the port
    lacks, not a harness preload or a binding.
- **UNVERIFIED** in game.
