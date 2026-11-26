## 2026-11-26: the aim-offset rebuild's test gaps from #745's review

AI-assisted (Claude Code, Claude Opus 5.5). Lane REBUILD-TEST-GAPS (queue 0t), on main 64f070fa: tests and docs for
#745's code (decision 2026-11-26-04), from its review by Opus high (ACCEPT). No rule changed: every new test passed on
the code as landed, so no rule bug was found.

Edits: docs/decisions/2026-11/2026-11-26-04-aimoffset-required-bones-rebuild.md

That entry's Tests paragraph now says what the tests cover after these gaps, and that they leave nothing of the note's
section 5 out.

**What changed (tests and one comment):**
- `aim_offset_tests.rs`: the toy mesh class has a property, so a slotless stand-in of it reads as not loaded. Test 6's
  unloaded-mesh case installs a skeleton source and asserts the layer's "the unloaded skeletal mesh" text; before, it
  stopped at "no skeleton source is installed" and still passed;
- `aim_offset_tests.rs`: a new test runs NATIVE_AIMOFFSET_REBUILD.md test 8 through the profiles layer: the toy source
  gives bone names (a numbered `Spine_2` as the reader's text), a mesh export of the package gives the skeleton, and the
  lists equal the decoded form's (0, 1, 2, 3 and 255, 0, 255, 1). Nothing is interned;
- `aim_offset_rebuild_tests.rs`: test 7 adds the literal case, a 257-bone skeleton with a 256-component profile, which
  is the bone limit's refusal;
- `blend_setters.rs`: `register`'s comment says four keys (the feather blend's was added in #745).

**Gates.** `cargo test --workspace -j 8` and `crate_edges_test.py` (counts in the GATES row). No slice gate: no rule
changed.
