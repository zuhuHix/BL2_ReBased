## 2026-11-25: WillowWeapon.OnCreate in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-WEAPON-ONCREATE, stacked on `fix/rust-psc-setcolorparameter`
(d3c22f85). It implements docs/verification/NATIVE_WEAPON_ON_CREATE.md section 4 (#723, with its fix 137f5537). Clean
room: this lane did not read the executable. No real-game session was run. **Every rule is UNVERIFIED in the running
game.** Reference binds nothing (the stub policy).

**Where.** `ow-behavior` `kernel::natives`, bound in Off with the kernel pack. `BroadcastBehaviorEventFromScript`'s
per-provider loop is now a shared step, `broadcast_every_provider(vm, pid, name)`; the broadcast and `OnCreate` both
call it, and the broadcast's behaviour is unchanged.

**The rule**, in the note's order:
1. a supplied `Providers` array: `NotYet { NotRead }`, "WillowWeapon.OnCreate with a Providers argument (how a
   non-variable argument is passed is not read)", before anything fires;
2. the handle is the weapon's own `ConsumerHandle`, read as a plain field (not argument 0, so not through `routed`).
   Missing, or not a handle struct with a `PID`, is a `VmError::Script`;
3. no kernel service in the port: `NotYet { LayerAbsent }`, "the behavior kernel natives", never a silent nothing;
4. one rule per owner of the handle: `Kernel` fires `OnCreate` (filter -1, no payload) on every provider in
   registration order through the shared step; `Crate` runs that crate's route body for `OnCreate`, else the stub
   policy's answer; no owner, or `Released`, does nothing.

The native itself writes nothing. **Non-atomic (UNVERIFIED):** the firing inherits the broadcast's behaviour.
Providers run one by one, so when a later provider refuses, an earlier one's writes stay, and the refusal is marked
"stopped after partial writes" (#518's counters), with its kind and record kept.

**Tests** (`kernel/natives/tests.rs`, invented providers in the toy package): the note's section 5, 1 to 12, in 9
tests, including the two-provider test where the second refuses after the first wrote. Two toy behavior classes were
added to the fixture package (`Behavior_ToyWrite`, `Behavior_ToyRefuse`).

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 8 NotYet / 1 error, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`, the same totals as before. Link 8 is 1 of 5 at `init`: `OnCreate` answers
(the weapon's providers have no `OnCreate` entry), and the stop is past it, an **error at the unbound
`WillowWeapon.GetFiringModeDefinition()`**.

**Gates.** `cargo test --workspace -j 8`: 3,634 passed / 0 failed / 57 ignored. `crate_edges_test.py`: 19 OK.
