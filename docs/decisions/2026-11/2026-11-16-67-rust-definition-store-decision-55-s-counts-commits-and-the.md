## 2026-11-16: Rust definition store: decision 55's counts, commits and the archetype depth boundary

AI-assisted (Claude Code). Lane FIX-DEFS-FOLLOWUPS closes the three low findings of review r2 of PR #359
(FIX-DEFS-COLLECTION, landed as 1075dc2). No behaviour changes; one test assertion is added.

Edits: docs/decisions/2026-11/2026-11-16-55-rust-definition-store-collection-actors-engine-native.md

- **Why decision 55 is edited.** Its test count (five) and gate numbers (1,598 tests) were the first round's; review
  r1 added two tests and brought the workspace to 1,600. Its measurement sections did not name the commit each one
  used. The edit corrects the count (seven), gives both gate numbers, and names the commits: the level-object and
  scene runs used f481008 with the `ow-defs` files of 79a5e1f, and the consumer runs of review r1's M2 used ce6f681
  with those of 2f5ed03. No finding or number of decision 55 changes otherwise; three over-long lines are rewrapped.
- **The level-object runs repeated with the landed code.** Review r1 changed two rules after those runs: code classes
  are left to the decoder (M1), and a class default object is recognised as an archetype by its name (L1). The local
  driver (outputs under ignored `local/p2/fix-defs-collection/` only, pc-hummer) was rebuilt on f481008 with the
  `ow-defs` files of 2f5ed03 and run again over the 32,556 level objects of the 26 `Sanctuary*` files in both modes,
  and over H2's scene. Every object's values, every collection read, every `StaticMesh` slot, both modes' logs and the
  scene's placement list are byte-identical to the 79a5e1f run; only the elapsed times differ. So decision 55's counts
  (154 collection actors, 10,223 and 2,197 components, 10,905 typed `StaticMesh` slots, 11,444 objects with newly typed
  references, 12 `WorldInfo` arrays, 18 objects with archetype values) stand for the landed code.
- **The depth boundary.** `an_archetype_that_cannot_be_followed_is_not_yet_in_off`
  (`crates/ow-defs/src/store/collection/tests.rs`) now also loads `Deep_1`, whose chain is exactly 16 archetypes, in
  both modes, beside `Deep_0`'s 17, which is a `NotYet` in Off. Changing the limit check to one less makes the test
  fail (checked).
- **Verified.** The gates of the lane's handoff. **UNVERIFIED.** Everything decision 55 says is, unchanged.
