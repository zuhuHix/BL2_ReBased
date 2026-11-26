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
    resident whenever its attribute is. That covers a resolver in the attribute's own package; an import from another
    package is UNVERIFIED;
  - the port's store is lazy (a load never loads what it references) and loads never nest, which is why step 0 is
    met today.
- **Proposed Off answer:**
  - the evaluator's chain walks (`resolve_context`, the value walk and `preflight_read`) load each non-None entry
    before they dispatch it, through the existing `load_free_refusal` check;
  - load-free scopes are unchanged;
  - a failed resolver load leaves the handle, and the walk's residency check reports `BadData` where the entry is
    reached;
  - step 0 stays as a guard. No harness preload is needed;
  - a `get_value` now loads 1+N, and #518's `load_transitions` rises to match.
- **Review r1 (#787):** the trigger moved from the attribute's load to the walks, since the VM, the context resolvers,
  direct store calls and harness preloads also make an attribute resident; the store's laziness is stated, and its
  rule kept; a failed load and the 1+N count are written; the residency claim is limited to the attribute's own
  package; a live check and two tests are added.
- **Clean room:** a different lane implements this note.
