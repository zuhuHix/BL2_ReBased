## 2026-11-25: ParticleSystemComponent.SetColorParameter in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PSC-SETCOLORPARAMETER, stacked on `fix/rust-setmaterial-glow`
(#718, a3ac4c13). It implements docs/verification/NATIVE_PSC_SET_COLOR_PARAMETER.md section 3 (#719 at its review r1
head db29bf0f, lane F's note). Clean room: this lane did not read the executable. No real-game session was run.
**Every rule is UNVERIFIED in the running game.** Reference binds nothing.

**Where.** `ow-inventory` `attach_mesh::psc_color`, bound first-wins with attach_mesh's other component natives.

**The rule**, in the note's order:
1. no receiver: a data error;
2. a None name: nothing, and nothing else is read. This check comes before the receiver's load check (review r1 of
   #719);
3. a receiver the store has not loaded: `NotYet { NotRead }`, "SetColorParameter on a component not loaded at set-up
   (a load is a write)";
4. the declaration: `InstanceParameters`, an array of a struct with `Name`, `ParamType` (a byte) and `Color` (four
   bytes); each element is checked too. Any mismatch is a `VmError::Script`;
5. the plan: the first element with the name and `ParamType` 5 (`PSPT_Color`) gets its `Color` replaced; else the
   struct's zero value with the three fields set is appended. An element with the same name and another type is not
   matched. The array is built whole and checked against the declaration.

Then one write, `InstanceParameters`. No other field is written: no reattach, no transform update, no render-side
state.

**Tests** (`psc_color_tests.rs`, an invented package): section 4's 1 to 11, plus the native key the package's
declaration gives. Test 10 uses two plain components standing for the weapon's own copy and the class default's.

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 8 NotYet / 1 error, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`. Link 8 is 1 of 5 at `init`: `SetColorParameter` answers, and the stop is
past it, an **error at the unbound `WillowWeapon.OnCreate(array)`**. A throwaway probe (an `eprintln!` in the native,
reverted, never committed) on a separate run checked the note's harness question. The receiver,
`TracerParameterTemplate`, is a runtime object (`Origin::Runtime`), not the class default's subobject
(`Origin::Default`). So the weapon holds its own copy, and the class default's component is not written.

**Gates.** `cargo test --workspace -j 8`: 3,625 passed / 0 failed / 57 ignored. `crate_edges_test.py`: 19 OK.
