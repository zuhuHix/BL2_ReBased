## 2026-11-26: Maya's anim sets preloaded at the combat run's set-up

AI-assisted (Claude Code, Opus 5.5). Lane PRELOAD-MAYA-ANIMSETS, on #751's head 405315a9 (itself on main c8d70e3f), with
#751's review LOWs as a separate commit. No session was run.

Edits: docs/decisions/2026-11/2026-11-26-10-updateanimations-per-node-step-on-the-animation-tree.md

That decision's "Not verified" line now labels the note's other fits UNVERIFIED (fitted) as well: the fields past
`CachedNumDesiredBones` (`bIsIssuingNotifies`, `bReverseAnimSeqSearchOrder`, `bAutoGenerateAdditiveSeqName`,
`AddAnimSeqName`) and the list names `AnimSets` and `Sequences` (review of #751, LOW 1).

- **What was decided** (the hub's two rulings):
  - in the game, the anim sets on Maya's mesh are resident when she spawns, because they stream with her character.
    So the combat run's `maya` step loads them as a harness set-up input, as link 8's Startup material preload does
    (#712);
  - it loads **exactly** these sets, each once, then each set's `Sequences`, through the store
    (`ow-cli` `combat::preload_mesh_anim_sets`):
    1. the sets the archetype's `Mesh` component lists in `AnimSets`;
    2. (the second ruling) the weapon holds' sets: what `BodyClassDefinition.GetWeaponHoldAnimSets` can return for
       her holds, which `WillowPawn.UpdateAnimSets` appends (`combat::hold_anim_sets`). By the script read of
       `GetWeaponHoldAnimSets` and `BodyWeaponHoldDefinition.GetAnimSets`: for the body class and up its
       `WeaponActionParentClass` chain, every hold in `WeaponHoldDefs` and its `DefaultWeaponHoldDef`, each hold's
       `AnimSetList` up its `ParentDefinition` chain. Holds and body classes are read, never loaded: one not resident
       is logged and skipped. The first-person holds (the arms) are not read;
  - each load is logged as a harness input: "set-up preload: <path> (UNVERIFIED: the game streams it with the
    character)". Nothing else is loaded.
  - On the installed packages: the component's `Anim_Siren.Base_Siren`, and the holds' `Anim_Siren.Rifle_Siren`,
    `Anim_Siren.RocketLauncher_Siren` and `Anim_Siren.Unarmed_Siren`, with 77 sequences in all. Nothing was skipped.
- **Off and Reference:** the preload itself checks no parity mode. It sits in the `maya` step's harness inputs
  (`apply_class_inputs`), and those run in Off only: Reference applies none of them, with no class select. So wherever
  the step's inputs run, Off and Reference would see the same preload; today only Off runs them.
- **The slice gate** (installed, Off, strict, under the machine lock, on local merges not pushed): 6 ok / 8 NotYet,
  42.9% in each run below.
  - #751 + #752: links 1, 3, 4, 5, 13 and 14 stop at #752's initialisation refusal for `Anim_Siren.Base_Siren`.
  - With the component's sets only: one native later, in `UpdateAnimations`, at the set `Anim_Siren.Unarmed_Siren`.
    `UpdateAnimSets` appends that set from the holds, hence the second ruling.
  - #751 + #752 at 9e4025f0 + this branch with the holds' sets: both `UpdateAnimations` and the init pass. The links
    stop at **`WillowPawn.InitializeRBPhysics`**, but at its entry check, "a component the store has not loaded (a load
    is a write)", not the physics `LayerAbsent` the note predicted. One of the pawn's `Components` entries is still a
    store handle.
  - No link-up refusal came up, so no additive lookup matched (`ADD_Idle` in particular) in the sets on her list at
    that point.
- **#751's review LOWs:**
  - `tree_ticks.rs`' module doc and decision -10 label the note's other fitted names;
  - test 7's search-order case now puts a lazy set in the slot searched second and the match in the slot searched
    first, so one order gives the link-up and the other the not-loaded refusal;
  - a new test covers `AUTO_NAME_ISSUING`, and a flag-clear additive node under the guard (re-set and lookup still
    run).
- **Verified:** the tests on invented data (exactly the listed sets and their sequences load, eleven load transitions (4 sets and 7 sequences);
  a second run loads nothing), and the installed gate rows above.
- **Not verified:** that the game's streaming load makes these sets resident (the hub's premise); everything in the
  running game.
- **Sensitive files:** none.
