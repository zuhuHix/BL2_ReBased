## 2026-11-16: Host AnimTree evaluation, Maya's two trees headless (H7)

AI-assisted (Claude Code). Lane H7-ANIMTREE, host track, on top of H6-ANIM stage 1 (decision 2026-11-16-53).
Number 56 was reserved for this lane by the hub. There was no real-game session, and no native code was read
(no Ghidra, no REA). Every blending rule here is **UNVERIFIED** in the running game; the checks below are
structural, against the tree data, or against stage 1's sampler.

- **What changed.**
  - `crates/ow-host/src/anim/tree/` (new: `mod.rs`, `data.rs`, `load.rs`, `graph.rs`, `eval.rs`, `report.rs`,
    `tests.rs`), plus one `pub mod tree;` line in `crates/ow-host/src/anim/mod.rs`.
  - No dependency, no CLI flag (the installed-game check is an ignored test), no reader or bounds-check change.
- **Evidence, in the brief's order.**
  1. **C++:** `src/` has no AnimTree code.
  2. **Script:** the AnimNode classes of `Engine`, `GearboxFramework` and `WillowGame` own 78 functions; 74 have no
     script body (native). Of the four that do, three are `AnimNode`'s empty `OnInit`, `OnBecomeRelevant` and
     `OnCeaseRelevant`. The fourth, `WillowAnimNodeSequence.OnInit`, starts the node's clip at init when its
     `bAutoStart` is set, with the node's own `bLooping` and `Rate` (read with `research/script_disasm.py`). It is the
     one script rule used.
  3. **Records:** `NATIVE_PHASELOCK_PRESENTATION.md` section 2.3 (read from the native, UNVERIFIED) gives the
     special-move blend times: negative ones become 0; `EC_StopOnLastFrame` and `EC_OnAnimEnd` drop the blend-out;
     when the clip's length over `|rate|` is shorter than blend-in plus blend-out, both are 0 (unless it loops); only
     `EC_Loop` loops. The node plays the clip as a full-body blend.
  4. **Tree data:** each node's class, children, clip names, `Rate`, `bLooping`, `bPlaying`, `CurrentTime`, speed
     `Constraints`, blend times, `Child2Weight`, `BaseSpeed`, branch bones and flag objects. They are read through
     `ow-defs` with each class's defaults applied, so an untagged property is the class's own default. The super-class
     chains (read from the code packages' class exports) put each Willow or Gearbox class under its engine base.
- **Which classes are evaluated, and how** (`load.rs` `kind_of`; first match along the class chain).
  - **Sequence players:**
    - `AnimNodeSequence` and its subclass `WillowAnimNodeSequence` play one clip: advanced by
      `dt * Rate * play-rate scale`, looping or stopping at the end. A node starts at its stored `CurrentTime` and
      plays when `bPlaying` (or `bAutoStart`) is set.
    - `WillowAnimNodeSequenceAdditiveBlend` (a sequence subclass) also plays `AddAnimSeqName` as an additive delta.
  - **Blend lists:** subclasses of `AnimNodeBlendList` whose only addition is which child is active. Their selection
    is native and unread, so it is a named driver input:
    - `AnimNodeBlendByPosture`, `AnimNodeBlendByPhysics`;
    - `WillowAnimNode_AimState`, `WillowAnimNode_Falling`;
    - `WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu`;
    - and the `AnimNodeBlend` subclass `WillowAnimNode_Condition` (FALSE / TRUE by its flag object).
  - **`AnimNodeBlendBySpeed`:** the active child is the speed's band in `Constraints`, blended over `BlendUpTime` or
    `BlendDownTime`. `BlendDownPerc` is not applied.
  - **`AnimNodeBlendDirectional` and `WillowAnimNodeBlendDirectional`:** forward, backward, left and right weights
    from the move direction. The front half is **fitted to the tree data**: the four installed directional nodes'
    stored `DirAngle` and child weights fit `side = |a| / 90 deg`, `forward = 1 - side` (for example 8 degrees left:
    0.9111 / 0.0889). The back half mirrors it, UNVERIFIED. `DirDegreesPerSecond` and `BlendTime` are not applied.
  - **`AnimNodeAdditiveBlending`:** the base plus its `Add` child as a delta at `Child2Weight`.
  - **`AnimNodeBlendPerBone`:** `Target` replaces `Source` at `Child2Weight` on the bones at and below
    `BranchStartBoneName`.
  - **`AnimNodeScaleRateBySpeed`:** its subtree plays at `speed / BaseSpeed`.
  - **Slots:** `WillowAnimNodeSlot` (an `AnimNodeSlot`) plays a requested one-shot over its source.
    `AnimNodeSpecialMoveBlend` (also an `AnimNodeSlot`) plays a requested special move with the record's blend rules.
  - **The tree root:** `AnimTree` and `WillowAnimTree` pass their one child.
  - **`NotYet`, with the class name** (their own native part changes how poses combine and is not read):
    - `WillowAnimNode_AddCameraBone` (two inputs, Source and Add);
    - `WillowAnimNode_WeaponRecoil` (an `AnimNodeSpecialMoveBlend` that plays recoil);
    - `WillowAnimNodeFeatherBlend`;
    - `WillowAnimNode_ClimbLadder`;
    - `AnimNodeAimOffset` (so `WillowAnimNodeAimOffset`: its profiles are not decoded; `tools/prepare_skill_tree.py`'s
      tag reader also fails on this node).

    Evaluation refuses them with a typed `NotYet` (`Unsupported::Refuse`, the default). For reports, it passes them
    to their first child and counts them (`Unsupported::PassFirstChild`, a labelled stand-in).
  - **Skeletal controls** (`SkelControlLists`) are read, counted and not applied.
- **Stand-ins, UNVERIFIED** (no record, script or C++ settles them; `eval.rs` lists them):
  - weights move linearly to their target over the blend time;
  - a blend is the weight-normalised sum of translations and the sign-aligned, normalised sum of rotations;
  - an additive delta is scaled from identity by its weight (nlerp) and applied on the base. For rotations it comes
    after the base for a child bone and before it for the root, which is one order in the export's own convention.
    Translations are added;
  - a one-shot ramps in and out linearly;
  - a missing clip gives the bind pose, except below an additive input, where it gives no change;
  - every playing sequence ticks whatever its weight;
  - a `ScaleRate` node reached by two paths takes the scale of the first path the depth-first walk from the root
    takes;
  - a selection past a list's last child takes the last child;
  - a slot's or special move's source (the pose under the one-shot) is its first linked child;
  - `Driver::stop` ends a one-shot at once, with no blend-out;
  - a later AnimSet's clip wins over an earlier one of the same name;
  - sync groups (`SynchGroupName`, `AnimGroups`) and `bForceAlwaysSlave` are not applied;
  - the default blend time of a list with no `BlendTime` is 0.2 s.
- **The driver** (`eval.rs`).
  - Named inputs: speed, direction, selections by input name, flags by flag path, and a default blend time.
  - One-shot requests by node name.
  - A tick of `dt` in two passes, each in export-index order: pass 0 moves the blend weights, pass 1 advances clip
    times and slots. Notifies are reported only from nodes with weight after pass 0.
  - Deterministic: the same steps give the same bits, and ten ticks of 0.01 s match one of 0.1 s within 1e-9 for
    these linear rules.
  - Clip times are in the clip's own seconds (`SequenceLength` units) and advance by `dt * Rate * RateScale`; the
    export's folded `RateScale` is divided back out to sample. A one-shot runs at `|rate| * RateScale` clip seconds
    per real second; its blend times are real seconds, scaled into clip seconds when it starts, and the record's
    "shorter than blend-in plus blend-out" rule uses the play length in real seconds (`length / (|rate| *
    RateScale)`).
  - Requests and inputs are checked: a play rate must be finite and above 0, blend times finite, and speed,
    direction and the default blend time finite (`EvalError::BadPlay`, `EvalError::BadInput`).
  - Root motion (`RootBoneOption`, `RootRotationOption`) and notifies are read and reported; neither is applied.
- **Guards.**
  - Nodes are loaded from a work list, each once by export index, at most 4,096. A node or child in another file
    than the tree's is refused (`TreeError::OtherFile`): export indices are only unique within one file.
  - The finished graph is checked by one iterative depth-first walk: dangling links, cycles, and a depth budget of
    64.
  - Evaluation recursion stops at the same budget.
  - `effective_weights` walks at most 100,000 path steps; the steps it drops are counted in the log and reported
    (0 on the installed trees).
  - `NextControl` chains stop at 64.
- **Synthetic proof** (`tests.rs`, invented values):
  - blend weights summing to 1 and ramping linearly, including a switch mid-blend;
  - the additive delta on the identity base (the delta at weight 1, half the angle at 0.5, a missing additive clip
    adding nothing);
  - a slot that plays once and blends back;
  - the special-move blend rules;
  - a cycle, a dangling link and a too-deep chain refused, while a diamond passes;
  - a single-clip path equal to the sampler;
  - determinism over `dt`;
  - `NotYet` refused or passed through and counted;
  - the directional fit against the stored weights;
  - speed bands with a play-rate scale and notify reporting.
- **Installed game** (`local/p2/h7-animtree/report.json`; the ignored test `anim::tree::tests::installed_report`).
  - **The holder file.** Under `IdentityPolicy::FirstInScanOrder` both trees come from `GD_Assassin_Streaming_SF`,
    the first of the four `_SF` packages that hold `GD_PlayerShared.Character.*`. Their node classes and counts
    equal the `GD_Siren_Streaming_SF` copy's (local probe); the copies were not compared byte for byte.
  - **Hands tree:** 28 nodes, depth 11.
    - Evaluated (26):
      - `AnimNodeSequence` 14;
      - `AnimNodeBlendBySpeed` 2, `AnimNodeSpecialMoveBlend` 2, `WillowAnimNode_Falling` 2;
      - 1 each: `AnimNodeBlendByPosture`, `AnimTree`, `WillowAnimNodeBlendDirectional`, `WillowAnimNodeSequence`,
        `WillowAnimNodeSlot`, `WillowAnimNode_AimState`.
    - `NotYet` (2): `WillowAnimNode_AddCameraBone` 1 (the root's child), `WillowAnimNode_WeaponRecoil` 1.
    - No skeletal controls.
  - **Body tree:** 66 nodes, depth 18.
    - Evaluated (59):
      - `AnimNodeSequence` 28;
      - `AnimNodeAdditiveBlending` 5, `AnimNodeScaleRateBySpeed` 5;
      - `AnimNodeBlendBySpeed` 4, `AnimNodeBlendDirectional` 4;
      - `AnimNodeSpecialMoveBlend` 3, `WillowAnimNodeSequenceAdditiveBlend` 3;
      - 1 each: `AnimNodeBlendByPhysics`, `AnimNodeBlendByPosture`, `AnimNodeBlendPerBone`,
        `WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu`, `WillowAnimNode_Condition`,
        `WillowAnimTree`.
    - `NotYet` (7): `WillowAnimNodeFeatherBlend` 5, `WillowAnimNodeAimOffset` 1, `WillowAnimNode_ClimbLadder` 1.
    - Five skeletal controls not applied: `SkelControlLeftHandGripWeapon`, two `WillowSkelControlHandPlacement`,
      two `WillowSkelControl_FootPlacement`.
    - No sequence node sets a root-motion option.
  - **Clip sources.**
    - First person: the hands component's `AnimSets` (`1st_Person_Unarmed`, `Siren_1st`, `1st_Person_Generic`)
      plus `1st_Person_Pistol` as the weapon's set.
    - Third person: `Base_Siren` plus `RocketLauncher_Siren`. It is the one installed Siren set that holds the
      clips the tree's additive nodes name (`ADD_Idle`, `ADD_Run`); with the pistol's or rifle's set they are
      missing, and the additive nodes add nothing.
    - How the game adds a weapon's set is not read.
  - **Additive clips are deltas** (structural check): the `ADD_*` keys' translations are at most 0.006 uu (absolute
    clips: 83-125 uu); their rotations reach 37-81 degrees.
  - **First person**, at 30 Hz with the two `NotYet` nodes passed through:
    - idle for 1 s;
    - `Reload_Jakobs` once on `WillowAnimNodeSlot_9` (blend 0.2 s in and out, stand-ins): 61 ticks, 2.033 s, the
      clip's length;
    - idle for 1 s;
    - `Phase_Lock_Lift` on the `WeaponAction` special-move node (which node the game uses for it is not read): its
      `AnimNotify_UseBehavior` at 0.25 s reported.
    - Every pose finite. The farthest bone from the root is 169.4 uu in idle and 169.41-169.44 uu in the reload; the
      cast reaches 181.8 uu (the hands rise).
  - **Single-clip check:** with the path to the idle sequence at weight 1, the tree's pose equals stage 1's sampler
    of `Idle` at the node's time: worst bone difference 0 uu (the check fails the report above 1e-4).
  - **Third person:** standing for 1 s, then walking forward at 200 uu/s for 2 s, physics set to walking. Every pose
    finite; the farthest bone is 139-152 uu. The walk's footstep notifies are reported.
- **Review r1 of PR #354** (one commit on top of the first).
  - Fixed: an additive clip's untracked bones were sampled at their bind local and so added the bind pose again;
    they are now the identity delta.
  - Fixed: `RateScale` was missing from the advance of sequences and one-shots, so a clip with `RateScale` other
    than 1 played that many times too slowly. `Reload_Jakobs` has `RateScale` 1, so its 61 ticks are unchanged.
  - Added: the cross-file refusal, the request and input checks, `stop`, and the truncation count above.
  - The installed report gives the same numbers as before both fixes. The installed `ADD_*` clips track every
    bone (0 untracked), and the clips on the weighted paths have `RateScale` 1. The pistol's `ADD_Fire_Recoil`
    (`RateScale` 2) and `ADD_Fire_Revolver_Recoil` (2.5) would have been affected, but no evaluated node plays them
    yet.
  - Tests added: the additive rule on a rotated and moved base (for a child bone and the root), the sequence node's
    own `add_clip`, untracked bones, per-bone masking with a missing branch bone, the directional back half,
    `StopOnLastFrame`, `RateScale` 2 for sequences and one-shots, bad requests and inputs, and the file check.
- **Open items.**
  - The native rules of the five `NotYet` classes, the aim offset's profiles, and the skeletal controls.
  - Which child each Willow blend list picks (their selection natives).
  - Sync groups and the real blend curve shapes; `BlendDownPerc` and `DirDegreesPerSecond`.
  - Which node plays reload, fire and the Phaselock cast in the game, and with which blend times.
  - How weapon AnimSets join the component.
  - Applying root motion and notifies.
  - A CLI flag for the report (`args.rs` and `main.rs` are not this lane's).
  - A GATES entry (proposed in the handoff).
