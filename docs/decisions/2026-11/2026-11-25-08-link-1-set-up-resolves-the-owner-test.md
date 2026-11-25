## 2026-11-25: Link 1's set-up resolves the special-move owner test's interface tables (route (a))

AI-assisted (Claude Code, Opus 5.5). Lane ROUTE-A, slice link 1, stacked on #667.
Harness only: no game rule changes. It implements the set-up that
docs/verification/NATIVE_SPECIALMOVE_OWNER.md section 4.1 calls route (a), with the strict cross-check #665's review
asked for. Synthetic fixtures only.

- **What changes:** `ow-cli`, the `maya` step's class inputs (`combat.rs` `class_inputs`). After the pawn archetype's
  preload and before the tree preload and the class select, a new module `combat/owner_setup.rs` adds two labelled
  harness lines:
  1. `preload_class(GearboxFramework.SpecialMoveInterface)`, the existing set-up class loader, an UNVERIFIED
     stand-in for the game having the interface loaded;
  2. `owner_tables`: `Vm::resolve_interface_tables` on the archetype's class chain, then a cross-check of the
     resolved answer (`implements_resolved_chain`, what the owner test reads) against the strict
     `CodeIndex::implements_declared`.
- **Where each answer comes from (the line says it):**
  - "read from the packages' interface lists": both agree; the entries resolved at set-up are a load, an UNVERIFIED
    stand-in for when the game resolves them;
  - "unresolved": the strict reading has no answer (an entry or a class chain did not resolve), fails (an entry's
    lookup failed, for example an entry naming something that is not a class), or disagrees. The set-up then answers
    `NotYet` (`NotRead`, record the owner note). The plain resolve keeps a failed lookup as "resolved to nothing", and
    the raw resolved-chain read (`implements_resolved_chain`) skips such an entry, so it alone would give a false no.
    The owner test does not take that no: since #667's review r1 it refuses such an entry itself. The strict read keeps
    the failure's typed error and checks the whole chain before `maya` goes on;
  - interface not loaded, no code key, or undecoded bodies: nothing is resolved, and the owner test answers that case
    itself (O1, O3/O3', O4). No structural fallback is taken anywhere, as the owner test takes none.
  - No pawn archetype loaded: the tables' line says it skipped.
- **Review r1 of #670:**
  - **Every entry:** `resolve_interface_tables` resolves every interface entry on the pawn class's chain, not only the
    one naming `SpecialMoveInterface`. The line says "the pass covers every entry on the chain", then how many
    previously unresolved entries this pass resolved (an earlier query may have resolved others). So other read-only
    interface questions about Maya's pawn class now answer from resolved entries too. `Vm::plain_interface_read_only` has no
    guard like the owner test's for an entry resolved to nothing on its no.
  - **An earlier stop:** a set-up refusal stops the `maya` step before the tree, mesh and socket preloads and
    `ServerPlayerSelectClass`, which is earlier than the owner test's O5 stopped it.
  - **Loads:** the store's handle counter does not move when a class or a package is loaded for an entry's lookup.
    So the line's load note is now measured on the code index's class count and the store's loaded-package count. A
    resolve that registers a class the index did not hold, or loads another package, is reported as a write. The
    entries count is on the refusal line too.
  - **Tests added:** an entry naming a second, unloaded package (the note appears, and not on a second run); an entry
    naming a Function export (the strict lookup fails: "an entry's lookup failed", `NotRead`, the owner note's
    record); a hand-defined pawn class ("no code key, nothing resolved"). A failing `preload_class` is not tested here;
    it is the existing loader's own line.
- **Unchanged:** the owner rule (`ow_world::world::anim_node_init::owner_implements`) still loads and resolves
  nothing; Reference does none of this.
- **Verified (host only):** on an invented package with decoded bodies, the owner test refuses O5 before the set-up
  and answers yes after it with no write or load. A class whose only entry names another interface reads as a no
  (O7). An entry importing a class from a package no store holds gives the refusal. An undecoded chain resolves
  nothing and the owner test gives O4. The combat harness test lists the two new lines (the toy packages hold no
  interface). Swapping the cross-check for the plain answer fails the refusal test.
- **Not verified:** nothing in the running game; the installed slice gate was not run (no install yet), so which
  answer Maya's pawn class gives, and where link 1 stops next, is not measured. **UNVERIFIED.**
