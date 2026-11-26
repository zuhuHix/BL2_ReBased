## 2026-11-26: UpdateAnimations' per-node step on the animation tree

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-UPDATEANIMATIONS-TREE-TICKS (queue 0u), on main afde0d73. Clean room:
implemented from docs/verification/NATIVE_UPDATEANIMATIONS_TREE_TICKS.md alone (#747 with its r1 fix, decision
2026-11-26-06); this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided:** `SkeletalMeshComponent.UpdateAnimations()` in Off runs the note's per-node step
  (`ow_world::world::tree_ticks`) in place of `bone_transform`'s `NotYet { NotPorted }` "UpdateAnimations: the tree
  nodes' ticks". Reference binds nothing, as before.
  - **The preflight** reads only: no store load, no class-code load, nothing interned. A name is matched by its text.
    - The tree root must be resident with an int `NodeInitTag`. The component declares `InitTag` (the tag field's
      name is a fit, UNVERIFIED) and `AnimTickArray`. The tick list is the component's `AnimTickArray`, which the
      tree copy fills like the root's.
    - Each entry, by its **exact** class:
      - the note's 28 empty-step classes: nothing;
      - `AnimNodeSequence` and `WillowAnimNodeSequence`: the plain step;
      - `WillowAnimNodeSequenceAdditiveBlend`: the plain step and the override;
      - any other class: a typed `NotRead`.
    - A lazy entry is its own `NotRead`.
    - **The guard:** `bIsIssuingNotifies` set **and** `AnimSeqName` not None skips the reset. When the reset runs, a
      set `AnimSeq` is the meta-data `NotRead`. With the guard holding, the reset does not run, so that call cannot
      come; this reading of section 3 is the port's.
    - Every additive node:
      - its declarations;
      - the auto-name `NotRead` while the guard holds;
      - the name looked up: `Add_None` with the auto-name flag, else its own `AddAnimSeqName`; None looks nothing up;
      - the lookup's gate, then the lookup: the component's `AnimSets` last to first, or first to last with
        `bReverseAnimSeqSearchOrder`; each set's `Sequences` matched by `SequenceName` text. The set's native map is
        replaced by this walk (UNVERIFIED);
      - a match is the link-up `NotRead`.
    - The not-loaded `NotRead` names the object's role and its store handle path.
  - **The writes** come after the whole preflight, so no refusal follows a write:
    1. `InitTag` := `NodeInitTag` + 1;
    2. per sequence node: the reset, unless the guard holds. For an additive node: with the auto-name flag,
       `Add_None` interned and written; then `AddAnimSeqName` re-set, `AddAnimSeq` None and `AddAnimLinkupIndex` -1;
    3. then `AnimSeqName` := the kept name and `CachedNumDesiredBones` := 0.
- **Not changed:** NATIVE_ANIM_NODE_INIT.md D's two disagreements (the additive override, and the guard's name test)
  are the hub's call, as the note says. The initialisation (`anim_node_init.rs` kind D) is as it was.
- **The slice gate** (installed, Off, strict; base afde0d73 and this change): 6 ok / 8 NotYet, 42.9% on both.
  - Links 1, 3, 4, 5, 13 and 14 move in `maya:8`. Before: "UpdateAnimations: the tree nodes' ticks" (NotPorted).
    Now: "UpdateAnimations: an object the store has not loaded (a load is a write): the anim set
    Anim_Siren.Unarmed_Siren". That is the note's section 7 second case: an anim set on her mesh's list is a store
    handle not loaded at this point.
  - Preloading it would be a harness input, which is the hub's call. The predicted `InitializeRBPhysics` stop is not
    reached yet.
- **Verified:** the tests on invented data (the note's 11) and the installed slice gate's rows above.
- **Not verified:** everything in the running game, including the lookup walk standing in for the set's map, and
  which anim sets her list holds in the game. **UNVERIFIED (fitted)** names: the `InitTag` field; the fields the note
  places past `CachedNumDesiredBones` by a 4-byte shift (`bIsIssuingNotifies`, `bReverseAnimSeqSearchOrder`,
  `bAutoGenerateAdditiveSeqName`, `AddAnimSeqName`); and the list names `AnimSets` and `Sequences` (review of #751,
  LOW 1, edited by decision 2026-11-26-13).
- **Sensitive files:** none.
