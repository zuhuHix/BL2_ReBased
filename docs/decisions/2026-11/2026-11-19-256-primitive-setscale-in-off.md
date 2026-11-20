## 2026-11-19: PrimitiveComponent.SetScale in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SETSCALE, slice link 8.

**The read.** The hub's brief let this lane both read the stock engine native and implement it if it turned out
plain. The read was local and read-only, through the project's Ghidra tooling. The raw output stays under ignored
`local/analysis/setscale/`. The rule, in our own words, is `docs/verification/NATIVE_PRIMITIVE_SETSCALE.md`.

**The rule.** The same scale does nothing. A change writes `Scale`, then runs the component's deferred transform
update. That update sets `bNeedsUpdateTransform` and, in two cases, asks one of two virtuals whose bodies were not read:
- with no owner and `bAttached`, the component's own transform update;
- with an owner that is static or tick-disabled and not being deleted, the owner's component update.

**What changes.** `ow-world` gains `world::primitive_scale`, bound by the world's pack in Off only; Reference keeps the
stub policy. `PrimitiveComponent.SetScale(NewScale)` follows the note's section 3, in order:
1. no receiver, or a non-float argument: `VmError::Script`;
2. an unloaded receiver: `NotYet { NotRead }`;
3. the declarations, checked as the native reads them, or `VmError::Script`:
   - the component's `Scale` (float), `bNeedsUpdateTransform` and `bAttached` (bools) and `Owner` (an object);
   - with a loaded owner, its `bStatic`, `bTickIsDisabled` and `bDeleteMe` (bools);
4. the same scale (a plain float comparison, so a NaN counts as a change): nothing;
5. before any write, `NotYet { NotRead }` for:
   - either unread virtual;
   - an owner the store has not loaded, whose flags cannot be read;
6. otherwise, `Scale` := `NewScale`, then `bNeedsUpdateTransform` := true. Both slots were read in the preflight.

The preflight writes, loads, draws and interns nothing.

**Not modelled.** The owner's two object-flag tests are not modelled. They can only skip the update, so where they
would skip it, the port refuses and the game does nothing. That is conservative, not a different answer.

**Tests** (`crates/ow-world/src/world/primitive_scale_tests.rs`, on classes defined by hand and a small invented
package). Every refusal and no-change call asserts #518's three counters and the name table's length unchanged. A
call that writes asserts exactly two committed writes. The cases:
- the same scale;
- a change with no update to ask for;
- each refusal;
- the declaration shapes;
- the Reference binding (`world::tests`).

**Not verified:**
- anything in the running game;
- the two virtuals' bodies and the owner's two object flags;
- the field identity of `Scale`, by fit (the note's section 1).

The test counts and the slice-gate reading are in the GATES row.
