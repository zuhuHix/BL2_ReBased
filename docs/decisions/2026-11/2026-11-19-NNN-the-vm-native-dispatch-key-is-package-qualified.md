## 2026-11-19: The VM's native dispatch key is package-qualified

AI-assisted (Claude Code, Opus 5.5). Lane VM-DISPATCH-KEY. The number NNN is a placeholder; the hub gives it.

- **Why.** Review flagged it twice (r1 of #509, and #500's TELEMETRY table). A native's registry key is
  `Class.Name(type,type)`, built by the loader as the C++ `nativeKey` is, and it leaves out the package. The registry
  was VM-wide, and the packs' binding helpers bind by path with first-wins (`ow_combat::hit::bind` skips a key that is
  already bound). So two packages that declare the same `Class.Function` shared one key. Binding one bound the other
  too, and a pack binding the second skipped it silently.
- **What was found.** The key was not package-qualified anywhere: `Vm::register_native` and `Vm::has_native` took
  the bare key, and the call, `prepare_call`, the Off call-entry check and the census looked the bare key up.
- **Changed (crates/ow-vm/src/api.rs):**
  - **`Vm::register_native_for(f, imp)`** binds native `f` for its package: the first component of its path,
    compared case-insensitively. Every function of that package with the same key uses the binding.
  - **What it refuses:** a function that is not native, and a key already bound in that package or bound VM-wide.
    Either gives "duplicate native registration".
  - **Lookup.** `Vm::is_bound(f)`, and the call itself, try the function's package binding first, then the VM-wide
    key.
  - **`Vm::register_native(key)` and `Vm::has_native(key)` keep their meaning:**
    - the key form binds every package's function with that key, as before;
    - `has_native` answers for a key bound either way;
    - `register_native` is refused when a package binding of the key exists. That keeps the old refusal for the
      order "a pack binds by path, then another registers the key".
- **Callers moved to the package form:**
  - the path-based helpers: `ow_combat::hit::bind` (first-wins, now per function), `ow_combat::loot::enable`,
    the AI actions and awareness packs, the behavior kernel's `bind`, `ow_inventory` definition and globals, the
    mission bridge's `bind` and `bind_optional`, the mover, and the data store's `bind`;
  - the "is this function bound" checks: the behavior kernel's read-only query, `slice_pickup::bound_native`, and
    the two starting-values checks in `ow_combat::hit::pools`.

  Literal-key registrations (`"Actor.Sleep(float)"` and the like) and the iterator registry are unchanged and stay
  VM-wide.
- **Behaviour.** The stub log line and `UnboundNative` still name the bare key. With the real packages, class names
  are unique across packages, so no two packages share a key, and every lookup resolves as before. The installed Off
  `--slice-gate` is unchanged (gate output in the hand-back). Reference mode is unchanged on the real packages for the
  same reason. The C++ registry stays keyed by the bare key, so it would differ only on a synthetic set where two
  packages share a class name.
- **Tests (crates/ow-vm/src/api/tests.rs, synthetic packages `PackA` and `PackB`):**
  - `two_packages_with_one_class_function_do_not_shadow_each_other`;
  - `first_wins_is_kept_within_one_package_and_key`;
  - `the_vm_wide_key_still_binds_every_package_and_refuses_an_overlap`.

  The existing `bind_skips_a_missing_function_and_leaves_a_lower_packs_key_to_it` (ow-combat) still holds first-wins
  against a key-form binding.
- **Not verified:** nothing here is about the running game. This is the port's own dispatch rule.
