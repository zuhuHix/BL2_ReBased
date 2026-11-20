## 2026-11-19: The slice set-up resolves hook 1's gear-likeness check

AI-assisted (Claude Code, Claude Opus 5.5). Lane HARNESS-GEARLIKENESS-SETUP (the hub's call on #657's two local
probes). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **Base:** stacked on #657 (fix/rust-bodycomp-render-hooks at a53c54a). The branch also carries #657's four LOW
  review findings as their own commit; #657 itself is not changed.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The body composition's rules
  (`ow_world::world::render_hooks`) are not changed, and still load nothing.

**What changed (harness only).** The combat driver's Maya set-up (`combat.rs`, inside the `maya` step, after her
mesh preload) does two things:
1. It loads `WillowGame.IGearLikenessConsumer` with link 9's `preload_class`. Its line is "harness input: set-up class
   ... (a write; UNVERIFIED stand-in for the game having it loaded)".
2. It resolves the interface entries `implements(Engine.SkeletalMeshComponent, WillowGame.IGearLikenessConsumer)`
   reads, with a new `slice_gate::resolve_interface`. Its line says where the answer came from:
   - **read** when the chain's class bodies decoded: the answer is the packages' own interface lists. On the
     installed game it is "no, read from the packages' interface lists". Resolving the entries at set-up, rather than
     when the game first asks, is labelled the harness input ("a write; UNVERIFIED stand-in for when the game
     resolves them");
   - **UNVERIFIED stand-in** when the bodies did not all decode (synthetic packages): the structural fallback, which
     is not the game's interface list;
   - a class the packages do not hold resolves nothing.

**Measured:** links 1, 3, 4, 5, 13 and 14 now stop at hook 2's pawn test ("the owner-no-see gate's pawn test (not
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
