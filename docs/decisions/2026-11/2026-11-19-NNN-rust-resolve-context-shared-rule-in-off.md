## 2026-11-19: `AttributeDefinitionBase.ResolveContext` is one shared rule in Off, bound by the action-skill and equip packs

AI-assisted (Claude Code). Lane IMPL-RESOLVE-CONTEXT, coded from NATIVE_RESOLVE_CONTEXT.md section 5 at 871f3ccc
(#604) alone. The analyst was another lane, so the clean room holds. **Every rule is UNVERIFIED in the running game.**
Off only. Reference is unchanged: the equip pack binds nothing there, as before.

- **The shared rule.** `ow_attr::AttributeSystem::resolve_context_native` (`ow-attr` `resolve_native.rs`) runs the
  following checks in order. The first failure is the answer:
  - R1: the definition is an unloaded handle. `NotYet { NotRead }`;
  - the chain's shape. A malformed one is a `VmError::Script` naming `ContextResolverChain`;
  - R2: any entry of the chain is an unloaded handle. `NotYet { NotRead }`. R2 is checked over every entry before R3;
  - R3: any entry's class is not registered. The registry's own `NotYet { NotRead }`;
  - then the walk (`eval::resolve_context`). Every check before it reads only, with no load, write or draw. Once the
    walk starts, any `NotYet` is terminal, through `NotYet::after_partial_writes`, as `GetValue` does (decision
    2026-11-19-58).
- **The wrapper.** `ow_inventory::equip::natives::resolve_context_native` checks two things first:
  - the receiver (an `Engine.AttributeDefinitionBase` or a child, by `by_path` only);
  - the argument (an object or None).
  It then calls the shared rule.
- **Two bindings, either order.**
  - The equip pack binds the wrapper through its own keep-existing check (`bind_resolve_context`). The old equip
    native is removed.
  - The action-skill pack binds the same wrapper through `ow_combat::hit::bind_fn`, next to
    `register_cast_get_value`, so link 9's world has it.
  - Whichever registers first binds it, and the other does not fail.
- **Tests on invented packages:**
  - in `ow-inventory`, the note's tests 1 to 11 and 13 to 16;
  - in `ow-skills`, test 12, both orders.
  - The stand-ins in test 11: the "scripted" step is a test-registered resolver that writes, and the refusing step is
    a test-registered resolver that answers `NotYet`. Both are invented.
  - Every refusal before the walk checks #518's counters unchanged.
- **Test 13 and the installed class tree (for the hub).** In the installed packages `FlagDefinition` extends
  `GBXDefinition`, not `AttributeDefinitionBase`. Under the note's own receiver check (step 1, and test 15), a
  `FlagDefinition` handed to this base native is a `VmError::Script`, not the base walk. The fixture follows the
  installed tree, so the test pins the `VmError::Script`. If the hub wants test 13 as the note words it, the receiver
  check must change; it is in one place, the wrapper.
- **Multi-context receivers** run the same base walk (test 14), as the note says.
- **Gates:**
  - `cargo test --workspace -j 2`: 2,750 passed / 0 failed / 49 ignored (12 new). `crate_edges_test.py` OK. Clippy has
    no hits in the touched files.
  - **The installed equip run** (`--inventory-move --equip-run`, the 50-weapon list), Off, main c687c135 against this
    branch: **identical output**. No phase reaches a `ResolveContext` refusal: init stops before it on every weapon
    (the globals object on 49, `BulletSpeed` on 1).
  - **The installed Off strict `--slice-gate`**, under the machine lock, main against this branch: **identical**, 3 ok
    / 10 NotYet / 1 error. Link 8 is an error at `init` (steps `make`, `init`), at the unbound
    `WillowWeapon.RecomputeAttributeBaseValues()`. Link 9 is NotYet at `cast` (steps `spawn`, `cast`), at the noun
    resolver's strategy-2 refusal, which #523 settles. On main, neither link reaches `ResolveContext` yet; the earlier
    probes reached it only with #523 and the link 9 stack merged.
- **Open items:**
  - the note's test 13 assumed the wrong parent class for `FlagDefinition` (it is a `GBXDefinition` in the installed
    packages, not an `AttributeDefinitionBase`). The port follows the note's receiver check, so that receiver is a
    `VmError::Script`, as the hub ruled (2026-11-20).
