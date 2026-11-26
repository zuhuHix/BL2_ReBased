## 2026-11-26: Maya's anim sets preloaded at the combat run's set-up

AI-assisted (Claude Code, Opus 5.5). Lane PRELOAD-MAYA-ANIMSETS, on #751's head 405315a9 (itself on main c8d70e3f), with
#751's review LOWs as a separate commit. No session was run.

Edits: docs/decisions/2026-11/2026-11-26-10-updateanimations-per-node-step-on-the-animation-tree.md

That decision's "Not verified" line now labels the note's other fits UNVERIFIED (fitted) as well: the fields past
`CachedNumDesiredBones` (`bIsIssuingNotifies`, `bReverseAnimSeqSearchOrder`, `bAutoGenerateAdditiveSeqName`,
`AddAnimSeqName`) and the list names `AnimSets` and `Sequences` (review of #751, LOW 1).

- **What was decided** (the hub's ruling):
  - in the game, the anim sets on Maya's mesh are resident when she spawns, because they load with her character's
    packages. So the combat run's `maya` step loads them as a harness set-up input, as link 8's Startup material
    preload does (#712);
  - it loads **exactly** the anim sets the archetype's `Mesh` component lists in `AnimSets`, then each set's
    `Sequences`, through the store (`ow-cli` `combat::preload_mesh_anim_sets`);
  - each load is logged as a harness input: "set-up preload: <path> (UNVERIFIED: the game streams it with the
    character)". Nothing else is loaded.
  - On the installed packages the list is one set, `Anim_Siren.Base_Siren`, with 58 sequences.
- **Off and Reference:** the preload itself checks no parity mode. It sits in the `maya` step's harness inputs
  (`apply_class_inputs`), and those run in Off only: Reference applies none of them, with no class select. So wherever
  the step's inputs run, Off and Reference would see the same preload; today only Off runs them.
- **The slice gate** (installed, Off, strict, under the machine lock, on a local merge of #751 + #752 + this branch,
  not pushed): 6 ok / 8 NotYet, 42.9%, as without this branch.
  - Without it, links 1, 3, 4, 5, 13 and 14 stop at #752's initialisation refusal for `Anim_Siren.Base_Siren`.
  - With it, they stop one native later, in `UpdateAnimations`: "an object the store has not loaded (a load is a
    write): the anim set Anim_Siren.Unarmed_Siren".
  - That set is not on the component's list: `WillowPawn.UpdateAnimSets` appends it from
    `BodyClassDefinition.GetWeaponHoldAnimSets` (the script read). Preloading the weapon holds' sets as well is the
    hub's call. The ruling limits this preload to the component's list.
- **#751's review LOWs:**
  - `tree_ticks.rs`' module doc and decision -10 label the note's other fitted names;
  - test 7's search-order case now puts a lazy set in the slot searched second and the match in the slot searched
    first, so one order gives the link-up and the other the not-loaded refusal;
  - a new test covers `AUTO_NAME_ISSUING`, and a flag-clear additive node under the guard (re-set and lookup still
    run).
- **Verified:** the tests on invented data (exactly the listed sets and their sequences load, five load transitions;
  a second run loads nothing), and the installed gate rows above.
- **Not verified:** that the game's streaming load makes these sets resident (the hub's premise); everything in the
  running game.
- **Sensitive files:** none.
