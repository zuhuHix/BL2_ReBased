## 2026-11-25: The slice set-up resolves hook 1's gear-likeness check

AI-assisted (Claude Code, Claude Opus 5.5). Lane HARNESS-GEARLIKENESS-SETUP (the hub's call on #657's two local
probes).

- **Base:** first stacked on #657 (fix/rust-bodycomp-render-hooks at a53c54a; parked 2026-11-20). #657 and
  IMPL-TREE-INIT-OWNER-EVENT have since landed, so on 2026-11-25 the lane's own commits were brought onto main
  d3c7a13 as fix/rust-gearlikeness-setup-2, then rebased onto main faa3bbc (#661, #662), the current base. The branch
  also carries #657's four LOW review findings as their own
  commit. The merge kept both set-up lines next to the mesh-sockets preload from main (sockets first, then the
  gear-likeness lines), so the combat harness test now counts 12 lines.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The body composition's rules
  (`ow_world::world::render_hooks`) are not changed, and still load nothing.

**What changed (harness only).** The combat driver's Maya set-up (`combat.rs`, inside the `maya` step, after her
mesh preload) does two things:
1. It loads `WillowGame.IGearLikenessConsumer` with link 9's `preload_class`. Its line is "harness input: set-up class
   ... (a write; UNVERIFIED stand-in for the game having it loaded)".
2. It resolves the interface entries `implements(Engine.SkeletalMeshComponent, WillowGame.IGearLikenessConsumer)`
   reads, with a new `slice_gate::resolve_interface`. Its line says where the answer came from:
   - **read** when the chain's class bodies decoded: the answer is the packages' own interface lists. On the
     installed game it was "no, read from the packages' interface lists" in the 2026-11-20 pc-hummer run on the #657
     stack (below); not re-measured on the current base.
     Since review r1 of #665 (MED 1) the answer is cross-checked with the strict `CodeIndex::implements_declared`:
     `implements` caches an entry whose lookup failed as "resolved to nothing", which would read as a settled no.
     When the strict reading has no answer or disagrees, the line says "unresolved, an entry did not resolve to a
     class" (or names the lookup error) and the set-up answers `NotYet` (`NotRead`). `CodeIndex::implements` itself
     is not changed here; Resolving the entries at set-up, rather than
     when the game first asks, is labelled the harness input ("a write; UNVERIFIED stand-in for when the game
     resolves them");
   - **UNVERIFIED stand-in** when the bodies did not all decode (synthetic packages): the structural fallback, which
     is not the game's interface list;
   - a class the packages do not hold resolves nothing;
   - when the class lookup itself loaded code (by the store's load counter), the line says so (a write).
3. **Only attachment 0.** The set-up resolves the entries for `Engine.SkeletalMeshComponent` only (Maya's mesh,
   attachment 0), but hook 1 runs for every made component. A later attachment of another class may stop at
   `GEAR_UNRESOLVED` (`NotRead`), which is an honest `NotYet`, not a fault of this set-up.

**Measured (2026-11-20, pc-hummer, on the #657 stack, installed Off `--slice-gate`; not re-measured on the new
base, because this Linux desktop has no install yet):** links 1, 3, 4, 5, 13 and 14 now stop at hook 2's pawn test
("the owner-no-see gate's pawn test (not
identified)", `NotRead`). Maya's `Mesh` template has `bOwnerNoSee` clear, so the unidentified pawn test decides. Its
analyst note is another lane's; the rule is unchanged here.

**#657's LOW findings, folded in:**
1. the two stand-in answers of hook 1 (a class the code index does not hold, and the structural fallback) are
   labelled UNVERIFIED stand-ins in one sentence of the module documentation, and at the code;
2. the not-settled check's failed-template branch is not reached from the walk (the walk refuses a failed template
   first); a comment says so, and a test calls hooks 2 and 3 directly with a failed handle;
3. note test 12's other half: a pawn `LightEnvironment` not declared as an object, and a `bOwnerNoSee` or
   `bAcceptsLights` that is not a bool, are script errors before any write;
4. `refused` in `body_composition_tests.rs` also asserts the name table's length, so every refusal there is checked
   intern-free.

**Review r1 of #665, also folded in:** the not-settled check drops its dead `is_failed` clause (`DefStore::is_unloaded`
already covers a failed handle; the comment says so, and the direct-call test stays), and test 12
(`a_malformed_controller_field_is_a_script_error`) also checks the name table's length, as the other refusals do.
