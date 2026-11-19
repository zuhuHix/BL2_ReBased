## 2026-11-19: AttributeDefinition.GetValue as a script native in Off

AI-assisted (Claude Code, Opus 5.5). Lane FIX-ATTR-GETVALUE-NATIVE. The script native
`AttributeDefinition.GetValue(object,object,object)` is bound to `ow-attr`'s existing library rule,
`AttributeSystem::value_of` (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"). No second copy of the rule was
written. No analysis output, Ghidra or REA was opened. There was no real-game session: the rule stays **UNVERIFIED**
in the running game. No dependency was added, no bounds check was loosened and no existing `NotYet` guard was relaxed.

**What changed.**

- `crates/ow-combat/src/hit/attribute.rs` (new), registered by the combat pack (`hit::Pack`), `ParityMode::Off` only.
  The binding adapts the call:
  - `this` is the definition. `ContextSource` and, when passed, `OptionalOverrideContextSource` go to `value_of`.
  - With a resolved context, the value chain's result is returned and the context is written to `ResolvedContext`.
    With none, the result is 0.0 and the context is None. The override is tried first and the source is the silent
    fallback, all in the library.
  - `ResolvedContext` is written only when the caller passed it.
  - An argument that is not an object reference is a script error, never read as None.
  - Whatever the library leaves unread comes back as its own `NotYet`, unchanged: a resolver class it does not
    implement, or a multi-context definition.
- **Reference binds nothing.** The C++ reference has no script native for `GetValue` (its attribute reads are library
  calls in `src/progression.cpp`), so there is no stand-in to label, and the call stays the unbound native it was.
  The library's own Reference stand-ins (the designer attribute) are unchanged.

**Verified (automated, invented classes).** `ow-combat`: 2 new tests on an invented `Engine` package (a `Pawn` with a
plain float attribute, the two attribute classes with the game's `GetValue` parameter shape, the pawn context resolver
and the object-property value resolver).

- In Off, six cases: a pawn source, a source that does not resolve, no source, an override that resolves, an override
  that does not (the source is the fallback), and both resolving (the override wins). Each gives the right value and
  out context, and equals `value_of` on the same arguments.
- In Off, an unimplemented resolver class passes the library's `NotYet { NotRead }` through, and a non-object argument
  is an error.
- Under Reference the key `AttributeDefinition.GetValue(object,object,object)` is not bound.
- Gate numbers are in `docs/verification/GATES.md`.

**Installed (Off; outputs under ignored `local/p2/fix-attr-getvalue/`).**

- On this branch alone (main 78cd48d6 plus this change; main has since moved to b985dcbf, docs only), `--slice-gate` strict is 0 ok / 14 `NotYet` / 0 errors. The
  six combat links stop before the native, at `FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren`, because #431
  is not on main yet. No before binary of this main was run, so "unchanged" is not claimed.
- On a local, unpushed merge with `fix/rust-class-starting-values` (#431, #434 and decision 2026-11-19-57), links 1, 3,
  4, 5, 13 and 14 move from the error on the unbound `AttributeDefinition.GetValue` to a `NotYet` in the library: the
  value resolver class `WillowGame.BlackMarketUpgradeAttributeValueResolver` is not read (NATIVE_ATTRIBUTES.md section
  6). They still stop inside `maya:8`; they do not pass it. Rows 10 to 12 stop at `UIInteraction.GetDataStoreClient`
  (`accept`). Totals: 0 ok / 14 `NotYet` / 0 errors.

**Open.** `BlackMarketUpgradeAttributeValueResolver`, now the six links' stop: it needs an analyst note before an Off
rule. `GetBaseValue` and `StaticGetAttributeValueOrDefault` are not bound as script natives by this lane.
