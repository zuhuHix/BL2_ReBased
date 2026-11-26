## 2026-11-26: The node initialisation's sequence step runs the additive override and the two-condition guard

AI-assisted (Claude Code)

Lane ANIM-NODE-INIT-D-CORRECTIONS (queue 0v), with Claude Opus 5.5. Code in `ow-world` `anim_node_init.rs` and its
tests, plus a dated correction in NATIVE_ANIM_NODE_INIT.md section 2 D. No dependency, and no bounds check changed.

- **Why:** NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 3 (#747) found two disagreements with NATIVE_ANIM_NODE_INIT.md
  section 2 D and the port's kind D. The hub ruled: fix both, following the native read.
- **What:**
  - **(a) the additive override at init.** On `WillowAnimNodeSequenceAdditiveBlend`, after the base step and whatever
    the guard:
    - with `bAutoGenerateAdditiveSeqName` set, `AddAnimSeqName` := `Add_None`. The name is interned only at the write;
      the preflight matches by text;
    - then the re-set: `AddAnimSeqName` := the name, `AddAnimSeq` := None, `AddAnimLinkupIndex` := -1;
    - with a non-None name, a `SkeletalMesh` and a non-empty `AnimSets` on the component, the component's sequence
      lookup, read only in the preflight. A match is `NotYet { NotRead }` (the link-up is not read). A set or sequence
      that is not resident is `NotYet { NotRead }` too;
    - the auto-name while issuing notifies under a name is `NotYet { NotRead }`, as #747 refuses it.
  - **(b) the notifies guard.** The reset is skipped only when `bIsIssuingNotifies` is set **and** `AnimSeqName` is not
    None. The sequence-change refusal follows: it fires when `AnimSeq` is set and the reset runs.
- **Clean room:** implemented from #747's note (lane D's analysis) and the init note. This lane did not read the
  executable for this change.
- **Tests** (synthetic): the guard with no name, and with a set sequence; the override with the flag set (the intern
  at the write) and clear; a case-insensitive lookup match, with nothing written or interned; no mesh, so no lookup;
  the auto-name while issuing; Maya's three additive nodes by toy values. The old issuing test now gives its node a
  name.
- **Review r1 (#752):**
  - `AddAnimSeqName` is read as a name in the preflight whatever the auto-name flag, so a malformed declaration is
    refused before any write (test added);
  - tests added for the not-loaded refusal, the search order with a not-loaded and a matching set, and no lookup with
    the flag clear and no name;
  - the "case-insensitive" test is relabelled (the name table folds case at intern);
  - the init note says the same rules, with the init's own texts.
- **Verified:** host tests and the installed slice gate, as in the GATES row. Link 1 moved earlier, to the lookup's
  not-loaded refusal for Maya's anim set `Anim_Siren.Base_Siren` (the refusal names the object). **UNVERIFIED in the
  running game.**
