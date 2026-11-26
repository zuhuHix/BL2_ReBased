## 2026-11-26: An attribute's resolvers load with the attribute

AI-assisted (Claude Code)

Lane ANALYST-EVALUATOR-LOADS-RESOLVER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** on the installed data, the `TargetableAttributeValueResolver` that links 3 and 4's spawn reaches is an
  unloaded store handle, a subobject of `Targetable_Is_Champion`. #786 refuses it at step 0 of
  NATIVE_TARGETABLE_RESOLVER.md section 5, under the hub's ruling on #783. The hub asked whether the game's evaluator
  loads it, or whether it is resident with its outer.
- **What:** an addendum to `docs/verification/NATIVE_TARGETABLE_RESOLVER.md`, section 5.1, and a pointer at step 0.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock: the evaluator's two chain walks on the attribute
    definition;
  - the layout tool for the chain fields.

  Raw output stays under ignored `local/analysis/resid/`.
- **Findings** (UNVERIFIED in game):
  - the walks call each non-None resolver directly, with no load and no residency check;
  - in the game the resolver is loaded with its attribute's package, before the attribute can be read, so it is
    resident whenever its attribute is.
- **Proposed Off answer:**
  - `ow-attr`'s load of an attribute definition also loads the resolvers of its two chains, in the same step;
  - load-free scopes are unchanged;
  - step 0 stays as a guard. No harness preload is needed.
- **Clean room:** a different lane implements this note.
