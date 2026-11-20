# What `AnimNodeSpecialMoveBlend` asks its owner (draft, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-SPECIALMOVE-OWNER (decision
`docs/decisions/2026-11/2026-11-19-259-special-move-blend-owner.md`, number pending). Analyst only: a different lane
implements it (clean room). This lane's author implemented #575 (the per-node initialisation, from
NATIVE_ANIM_NODE_INIT.md), whose step H is the starting point here, so a third lane should implement this note.

**Sources, in the brief's order:**
- **bl-sdk:** no live reads are recorded for this node, and no session was granted. Nothing here is observed.
- **The script** (`research/script_disasm.py` over the nine code packages):
  - `AnimNodeSpecialMoveBlend` has no script functions: its eight functions are native;
  - `SpecialMoveInterface` declares five native getters (`GetActor`, `GetSMComponent`, `GetSkelMesh`,
    `GetDefaultRootMotionMode`, `GetDefaultRootMotionRotationMode`), a native `SetReplicatedSMData` and four script
    server calls (`ServerSpecialMove_Play`, `_Queue`, `_Stop`, `_StopAny`);
  - the animation definitions' script events (`GearboxAnimDefinition.AnimFinished` and others) take the interface as
    their `SMI` argument.
- **The native code:** `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries and
  table reads, under the machine lock). The steps:
  - the node's exec steps;
  - the node's primary table (from its class registration's constructor);
  - the virtuals the exec steps call.
- **Field names:** `tools/ghidra/class_layout.py`. The node's own fields sit 4 bytes after the tool's placement in
  memory: a fit, checked by the field reads landing on `SMI`'s two halves and `AnimParameters`' data and count.
- **The records:** NATIVE_ANIM_NODE_INIT.md section 2, step H (the initialisation's owner test), and its section 3,
  rule 7 (the port's refusal).

Raw output stays under ignored `local/analysis/smo/`. This note is in our own words: no decompiler output,
pseudo-code, addresses, absolute offsets or listing structure. **Every rule here is UNVERIFIED** in the running
game.

## 1. During the tree initialisation

NATIVE_ANIM_NODE_INIT.md, step H, already holds the node's whole owner part. It is the last step of its per-node
initialisation:
- **the one question:** whether the skeletal component's `Owner` is set and implements
  `GearboxFramework.SpecialMoveInterface`;
- **yes:** the node's `SMI` becomes that owner, as the interface;
- **no owner, or an owner that does not implement it:** `SMI` is left as it is. Nothing else is read or written,
  and nothing is called on the owner.

So the initialisation **asks the owner nothing beyond its class**: no interface function is called, and neither the
pawn's special-move component nor any of its state is read. The packages list `SpecialMoveInterface` for
`GearboxPawn`, so Maya's `WillowPlayerPawn` implements it, and her three special-move nodes take her pawn as `SMI`
(NATIVE_ANIM_NODE_INIT.md).

## 2. Why the port refuses at this step today

The port's rule (#575, `ow-world` `anim_node_init.rs`, `owner_implements`) answers the owner test only from the code
index's **already resolved** interface entries (`CodeIndex::implements_resolved`). It refuses `NotRead`, "an owner
that does not implement SpecialMoveInterface", whenever that answer is not a plain yes. That one text covers the
cases below; they are numbered here as in section 4.1's table, and only O6 and O7 are real answers:
- **O1.** `GearboxFramework.SpecialMoveInterface` is not in the class table (not loaded).
- **O2.** The world's code layer holds no class at all.
- **O3.** The owner's class has no code key (a class defined by hand, with no package export behind it).
- **O3'.** The interface has no code key.
- **O4.** A class body on the owner's chain, or the interface's, did not decode.
- **O5.** An interface entry on the chain is not resolved yet, and no resolved entry answers yes.
- **O6.** A resolved entry is the interface or derives from it: a yes. `implements_resolved` gives no answer here
  when an unresolved entry comes first in its order, although `implements_resolved_chain` answers yes.
- **O7.** Every entry on the chain is resolved and none is the interface: a real no.

The refusal's text therefore overstates it. On link 1 the pawn is expected to answer yes (section 1), so the stop
means one of O1 to O5 (most likely O5, "not resolved yet"), or O6 hidden behind an earlier unresolved entry. It does
not mean "does not implement". This reading of the port is from its code. Which case link 1 meets has not been
probed.

## 3. After the initialisation: what the node's natives ask of `SMI`

The node's natives run when a special move plays (NATIVE_DOWN_TIMERS.md and NATIVE_SPECIAL_MOVE_PLAY.md reach them
through `SpecialMoveComponent`). Each exec step reads its arguments and calls one virtual of the node. "The active
entry" is the `AnimParameters` entry at the slot's active child index. Every native that reads it first checks that
the index is inside the array (an engine assertion, not a script-visible answer). What those virtuals do:

| Native | `SMI` unset | `SMI` set |
|---|---|---|
| `Play` | builds a zeroed play data and runs `PlayData`: as `PlayData` | as `PlayData` |
| `PlayData` | returns 0, writing nothing | `SkelComponent` None, or its mesh None: returns 0, writing nothing. Otherwise it asks one `SpecialMoveInterface` function (**not identified by name**), which must answer non-zero, then plays: that writes the active entry, `AnimState` and the root-motion settings (not this note's subject) |
| `Finish` | **with a `CustomAnimDef` on the active entry:** `AnimState` := 2, and no event. **Without one:** nothing at all (not even `AnimState`) | with a `CustomAnimDef`: `AnimState` := 2; then, when the same interface function answers non-zero, a script event by name to that definition, with `SMI`, the node, the entry's play data and an interrupted flag. By its parameters this is the definitions' `AnimFinished` (UNVERIFIED: the name was not resolved). Without one: nothing |
| `Stop` | always `AnimState` := 2. With a playing flag set it also stops the active child's sequence, with `SavedBlendOutTime`. It does not use `SMI` | the same |
| `SetRootMotion` | nothing | `SkelComponent` None: nothing. Otherwise it copies the root-motion modes onto the component and onto an object it resolves (**not identified**) |
| `IsPlaying(def)` | false (also with `def` None) | `def` None: false; `def` is the active entry's definition: true; otherwise `def`'s own virtual decides, given `SMI` (**not read**) |
| `GetCurrentAnimName`, `GetCurrentAnimDef` | not read | not read |

**So an owner that lacks the interface never raises an error.** The node only declines: no play, no finish event,
not playing, no root motion. The writes that remain are `Finish`'s `AnimState`, and `Stop`'s, which does not use
`SMI` at all. The interface is reached through `SMI` alone: the node never asks the owner for it again after the
initialisation, and never reads the special-move component directly.

## 4. Proposed Off answers (for a separate implementer)

**Every rule below is UNVERIFIED.**

### 4.1 Link 1's owner test (section 2)

**The set-up: route (a), the existing practice.** Resolve the question at Maya's set-up, before her pawn's tree copy,
as a labelled harness input. The hooks already exist:
- `preload_class(vm, "GearboxFramework.SpecialMoveInterface")` (`ow-cli` `slice_gate.rs`, link 9's set-up class
  loader, with its "harness input: set-up class ... (a write; UNVERIFIED stand-in for the game having it loaded)"
  line);
- `Vm::resolve_interface_tables` on the class chain of Maya's pawn class, as
  `ow_combat::pawn_interface::read_only::prepare` does for link 9's pawns. Its line is "harness input: interface
  tables of <class>: <n> entries resolved at set-up (a load; UNVERIFIED stand-in for the game having them
  resolved)".

Where: the `maya` step's class inputs (`ow-cli` `combat.rs`, `class_inputs`), after the pawn archetype's preload and
before the class select that spawns the pawn. Extend that step's list, or add a link 1 set-up list beside link 9's
`SETUP_CLASSES`. After it, the read-only answer is the yes section 1 expects. **Not proposed:** reading the packages'
interface lists inside the rule (route (b)), because `CodeIndex::class_at` may load and fills `interface_targets`, so
it is not read-only, and a name-only compare cannot settle derivation.

**The rule: one outcome per case of section 2,** in this order. All of it is read-only (no write, load, draw or
intern):
1. O1, then O2, from the class table and the code layer.
2. O3 and O3', from the code keys.
3. O4, testing decoding directly: `CodeIndex::class(k).body_decoded()` over the interface's key and every key of
   `CodeIndex::super_chain(owner key)` (both public, neither loads).
4. Only then `CodeIndex::implements_resolved_chain(owner key, interface key)` alone decides O5, O6 and O7. #575's
   `implements_resolved` is not used, since it gives no answer in part of O6.

| Case | Kind | Answer or text |
|---|---|---|
| O1. the interface class is not loaded | `NotRead` | "AnimNodeSpecialMoveBlend: the interface class GearboxFramework.SpecialMoveInterface is not loaded (a load is a write)" |
| O2. no code layer | `LayerAbsent` | as `Vm::plain_interface_read_only`: "the interface table of <class>: the world has no code layer (no class export is loaded)", layer "the code layer (package class exports)" |
| O3. the owner's class has no code key | answer, or the stand-in `NotYet` | a hand-defined class implements nothing, as `Vm::implements` answers it. A yes when the owner's class is a child of the interface; otherwise a no (`SMI` left as it is), unless the class chain meets an engine-native stand-in, which gives the stand-in `NotYet` |
| O3'. only the interface has no code key | `NotRead` | "AnimNodeSpecialMoveBlend: the interface GearboxFramework.SpecialMoveInterface has no code key (its table cannot be read)". `plain_interface_read_only` answers a quiet no here; this rule does not take it, because a loaded interface with no package export is not a case the note read |
| O4. a body did not decode | `NotRead` | "AnimNodeSpecialMoveBlend: the class chain of <class> did not decode (the structural fallback is not taken)" |
| O5. an unresolved entry, and no resolved yes | `NotRead` | "AnimNodeSpecialMoveBlend: the interface table of <class> has an entry not resolved at set-up (resolving it is a load)", where <class> is `implements_resolved_chain`'s `Err(class)` |
| O6. a resolved yes | answer | `SMI` := the owner, as the interface |
| O7. a resolved no | answer, or the stand-in `NotYet` | `SMI` left as it is, nothing written, when no engine-native stand-in is on the chain; otherwise the stand-in `NotYet` |

**The ruling on O4:** `implements_resolved_chain` answers an undecoded chain with `implements_structurally`, a
fallback. It is not taken here: the native's own test is the class's interface table, and the structural rule was not
read for this node.

**The stand-in check (O3, O7):** the engine-native stand-in test is `interp::is_child`, which is `pub(crate)` in
`ow-vm`. From `ow-world`, reach it through the public `Vm::plain_interface_read_only`, which runs it for its own no
answers, or add a public read-only wrapper for it in `ow-vm`.

**A "settled no"** is O3 or O7 answered without the stand-in `NotYet`. Only then is `SMI` left as it is. Route (a)
makes O5 answerable for Maya. A settled no is reached the same way for any owner whose entries are all resolved and
which does not implement the interface.

### 4.2 The natives of section 3, when a lane binds them

- **`SMI` unset:** answer as the table's middle column:
  - `Play` and `PlayData` return 0;
  - `IsPlaying` answers false;
  - `SetRootMotion` does nothing;
  - `Finish` writes `AnimState` := 2 only when the active entry has a `CustomAnimDef`, and sends no event.

  These answers rest on the field fits of the source list (the node's fields 4 bytes after the layout tool's
  placement), which are UNVERIFIED.
- **`Stop`,** whether `SMI` is set or not: `AnimState` := 2. The sequence stop with a playing flag set is a not-read
  refusal, "AnimNodeSpecialMoveBlend.Stop: the active child's sequence stop (not read)". It is planned before the
  `AnimState` write, so the refusal writes nothing.
- **`SMI` set:**
  - `PlayData` with `SkelComponent` or its mesh None returns 0, writing nothing. Otherwise a not-read refusal,
    "AnimNodeSpecialMoveBlend.PlayData: the owner's special-move gate (an unidentified interface function)";
  - `Finish`: the same gate refusal ("AnimNodeSpecialMoveBlend.Finish: ..."). Its `AnimState` write is planned, and
    made only after the gate answers, so the refusal writes nothing;
  - `SetRootMotion` with `SkelComponent` None does nothing. Otherwise a not-read refusal,
    "AnimNodeSpecialMoveBlend.SetRootMotion: the root-motion target (an unidentified object)";
  - `IsPlaying(def)`: `def` None gives false, and the active entry's definition gives true. Any other definition is a
    not-read refusal, "AnimNodeSpecialMoveBlend.IsPlaying: another definition's own answer (a virtual of the
    definition, not read)".
- **An active child index outside `AnimParameters`:** a `VmError::Script` naming the node (the engine asserts there).
- **Reference** is unchanged.

### 4.3 Tests

On synthetic classes built with `package()` / `tag()`, every value invented:
1. **The owner test**, on the package-backed fixture of `anim_tree_copy_tests.rs`
   (`h_takes_a_package_backed_owner_that_implements_the_interface`), one test per row of 4.1's table:
   - O6, a resolved yes, writes `SMI`;
   - O6 with an unresolved entry before the resolved yes, in `implements_resolved`'s order: still `SMI` := the owner,
     not O4 or O5;
   - O1, O2, O3', O4 and O5 give their kinds and texts;
   - O3 and O7 settled leave `SMI` and write nothing;
   - a no through a stand-in gives its `NotYet`.

   Each refusal goes through `refused_clean` (the same file: heap writes, handles, RNG draws, loads unchanged) and
   leaves the name table's length unchanged. After an unanswered test, assert that the chain's interface entries are
   still unresolved, as `implements_resolved_chain_reads_only_resolved_entries` (`ow-object` `reflect/tests.rs`) does.
   The test-only override `OWNER_ANSWER` (`anim_node_init.rs`) answers only yes or no. Extend it to return one of the
   outcomes, or replace it with package-backed fixtures.
2. **Route (a):** the set-up's lines, labelled, Off only. After the set-up, the read-only answer for the fixture's
   pawn class is yes. Before it, O5.
3. **`SMI` unset:**
   - `Play`, `PlayData`, `IsPlaying` and `SetRootMotion` write nothing;
   - `Finish` writes `AnimState` with a `CustomAnimDef` and nothing without one;
   - `Stop` writes `AnimState`, and its sequence stop refuses before any write.
4. **`SMI` set:**
   - each refusal of 4.2 comes before any write, `Finish`'s `AnimState` included;
   - `PlayData` and `SetRootMotion` with `SkelComponent` None decline;
   - `IsPlaying`'s two answers.
5. **An out-of-range active child index:** the `VmError::Script`.

## Open questions

1. **Not read:** the interface virtual the play and finish paths ask, and the name of `Finish`'s script event (by
   fit, `AnimFinished`). Also the object `SetRootMotion` resolves, and the two getters.
2. **Not probed:** whether link 1's stop is the "not resolved" case (expected) or a real no.
3. **UNVERIFIED:** the 4-byte shift of the node's fields against the layout tool.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read `SMI` on each of her three `AnimNodeSpecialMoveBlend` nodes. Expect her pawn.
2. Hook the definitions' `AnimFinished` during a special move that ends. Expect it once, with the node and her pawn
   as `SMI`.
