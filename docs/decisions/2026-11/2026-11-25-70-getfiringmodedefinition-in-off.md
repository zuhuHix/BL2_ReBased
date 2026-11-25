## 2026-11-25: WillowWeapon.GetFiringModeDefinition in Off, shared with the shot helper

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-GETFIRINGMODEDEFINITION, on main 72c6355e. It implements
docs/verification/NATIVE_WEAPON_GET_FIRING_MODE.md sections 2 and 3 (#731 with its review r1, on main). Clean room:
this lane did not read the executable. No real-game session was run. **Every rule is UNVERIFIED in the running
game**, except the barrel and type steps, which lane L7 observed live (PROJECTILE_FIRING_PATTERNS_LIVE.md section 3).
Reference keeps the stub policy.

**Where.** `ow-inventory` `equip::firing_mode`: the rule (`rule`), the native's load-free step (`resident`) and the
native, bound in Off from the pack's equip keys.

**The rule:** the barrel part's `CustomFiringModeDefinition` when the barrel is set and has one; else the elemental
part's; else the weapon type's `DefaultFiringModeDefinition` (None included); else None. Each object is reached only
when the rule reads it, through the caller's step. Each reached object must declare its mode field as an object
reference, else `VmError::Script`. The mode is returned as the field holds it. Nothing is written or cached.

**The native** reads the weapon's `DefinitionData` (declared as a
`WillowGame.WillowWeaponTypes.WeaponDefinitionData` holding the three objects, else `VmError::Script`). Its step is
load-free: an unloaded store handle or a slotless stand-in is `NotYet { NotRead }`, "...: the <part> <path>, which
the store has not loaded (a load is a write)". A failed handle is "..., whose load failed", with nothing retried.

**The shot helper** (`ow-combat` `hit::shot::firing_mode`, the note's settled switch) now calls the same rule. Its step
loads each object the rule reaches, so its loading stays on the rule's path. Its elemental case answers the elemental
part's mode, where it refused before. The `Part_ElementalMode` test now expects that mode, and a new test runs the
helper and the native's rule on the same resident parts for each step and asserts the same answer. **Off only**
(review of #735): the Off shot path now also takes the note's declaration check (a mode field not declared as an
object is a data error), while Reference keeps the earlier reading exactly, its lenient field reads and its refusal of
an elemental part with its own mode.

**Tests.** `firing_mode_tests.rs`, on invented classes: the note's section 5 in 7 tests. Every call asserts #518's
counters, `class_code_loads` and the name table unchanged. Lazy objects are slotless stand-ins. The failed-load text
is not exercised (it needs a store fixture with a failing export). Test 11 checks the native's part, the type
default's mode with no explosion override; `InitializeInternal`'s script tail runs on the installed slice gate
(link 8).

**The slice gate** (installed Off strict, release build, under the machine lock): **6 ok** / 2 NotYet / 6 errors,
`links ok 6/14`, `step share X = 6.583/14 = 47.0%` (main: 5/14, 41.3%). **Link 8 (`equip_gun`) passes, 5 of 5
steps** (`make`, `init`, `bases`, `equip`, `unequip`): past `GetFiringModeDefinition`, the type default's
`Bullet_Assault_Default` has no explosion override, so `InitializeInternal` returns, and the three script phases
after it run through. "Passes" means the host ran it under the strict grading, not parity with the game.

**Gates.** `cargo test --workspace -j 8`: 3,664 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK.
