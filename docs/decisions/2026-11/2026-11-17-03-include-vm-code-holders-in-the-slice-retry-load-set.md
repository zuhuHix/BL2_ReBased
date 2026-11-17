## 2026-11-17: Include VM code holders in the slice retry load set

AI-assisted (Codex/GPT-6.1 Sol).

The CLI retries a cooked-copy refusal under `IdentityPolicy::Loaded`. Its old set named only Startup and the
source map. After the template reader change, Off mode needs the class-default template's holder too: the
installed slice stopped because WillowGame was outside that set. Loading a class through the separate code
index does not make its file visible to the definition identity policy.

Keep the repair in `crates/ow-cli/src/cmd/slice.rs`: append `ow_object::reflect::CODE_PACKAGES`, once per name,
after Startup and the map. This reuses the VM's existing bounded code-package dependency set and ordering
(`ow-vm/src/interp/load.rs`, `build_native_index`, and `ow-object/src/reflect.rs`). Explicit slice sources retain
precedence over code-package copies. The retry trigger is unchanged. The store, parser, bounds checks, stub
policy and other commands are unchanged. This is CLI setup; the actual game load order remains UNVERIFIED.

Synthetic fixtures in `slice_tests.rs` reproduce the absent code holder on the old caller. They check Off template
loading, Reference's existing template refusal, exclusion of an unrelated package, absent objects, and typed
missing, ambiguous and corrupt holder errors in both modes. The external path is the existing installed-package
reader and project CLI. No extraction, native analysis, new tool, host or real-game session is part of this change.

Installed comparisons and gate results are recorded in the lane handoff under ignored
`local/p2/fix-slice-loadset/`. Passing these checks establishes caller visibility, not gameplay parity; subsequent
native or gameplay stops remain outside this decision.
