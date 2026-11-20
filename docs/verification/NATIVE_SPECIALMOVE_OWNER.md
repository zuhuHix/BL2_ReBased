# What `AnimNodeSpecialMoveBlend` asks its owner (draft, 2026-11-20)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-SPECIALMOVE-OWNER (decision
`docs/decisions/2026-11/2026-11-19-NNN-special-move-blend-owner.md`, number pending). Analyst only: a different lane
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
pseudo-code, addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

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
that does not implement SpecialMoveInterface", whenever that answer is not a plain yes. That covers both of these:
- a real no;
- **"not known yet":** the owner class's interface entries are not resolved, or `SpecialMoveInterface` has no code
  key.

The refusal's text therefore overstates it: on link 1 the pawn is expected to answer yes (section 1), so the stop
means "not resolved yet", not "does not implement". This reading of the port is from its code. Which of the two
cases link 1 meets has not been probed.

## 3. After the initialisation: what the node's natives ask of `SMI`

The node's natives run when a special move plays (NATIVE_DOWN_TIMERS.md and NATIVE_SPECIAL_MOVE_PLAY.md reach them
through `SpecialMoveComponent`). Each exec step reads its arguments and calls one virtual of the node. What those
virtuals do with the owner:

| Native | Owner use | With `SMI` unset |
|---|---|---|
| `Play` | builds a zeroed play data, then runs `PlayData` | as `PlayData` |
| `PlayData` | needs the node's `SkelComponent`, `SMI` (both halves) and the component's mesh. Then it calls one `SpecialMoveInterface` virtual (**not identified by name**), which must answer non-zero before anything plays. After that, the play itself writes the active entry of `AnimParameters`, `AnimState` and the root-motion settings. That part is not this note's subject | returns 0: nothing plays, nothing is written |
| `Finish` | for the active entry with a `CustomAnimDef`: `AnimState` := 2. Then, when `SMI` is set and the same interface virtual answers non-zero, it sends the entry's animation definition a script event by name, with `SMI`, the node, the entry's play data and an interrupted flag. By its parameters this is the definitions' `AnimFinished` (UNVERIFIED: the name was not resolved) | `AnimState` := 2 only; no event |
| `Stop` | `AnimState` := 2; with a playing flag set it stops the active child's sequence. It does not use `SMI` | the same |
| `SetRootMotion` | with `SkelComponent` and `SMI` set, copies the root-motion modes onto the component and onto an object it resolves (not identified) | nothing |
| `IsPlaying(def)` | false without a definition or without `SMI`; true when `def` is the active entry's definition; otherwise it asks `def`'s own virtual, passing `SMI` | false |
| `GetCurrentAnimName`, `GetCurrentAnimDef` | not read | not read |

**So an owner that lacks the interface never raises an error.** The node only declines: no play, no finish event,
not playing, no root motion. The interface is reached through `SMI` alone. The node never asks the owner for the
interface again after the initialisation, and it never reads the special-move component directly.

## 4. Proposed Off answers (for a separate implementer)

**Every rule below is UNVERIFIED.**

1. **Link 1's owner test (section 2).** Keep the rule. Make "not known yet" answerable without a write, along one of
   these routes, the hub's choice:
   - **(a) the harness:** resolve `GearboxFramework.SpecialMoveInterface` and the pawn class's interface entries at
     Maya's set-up, as a labelled "UNVERIFIED stand-in", like link 9's set-up classes (`IFlagProvider`,
     `IRuleEngineOwner`). Then the read-only answer is the yes section 1 expects;
   - **(b) the rule:** answer from the packages' declared interface lists along the owner's class chain, read-only.
     A chain through an engine stand-in class stays unanswered, as NATIVE_ANIM_NODE_INIT.md section 3, rule 7, says.

   Either way, split the refusal. "Not known yet" gets a not-read refusal naming the unresolved class ("the interface
   entries of <class> are not resolved"). #602's `CodeIndex::implements_resolved_chain` already answers with that class
   (its `Err(class)`), read-only. A real no keeps rule 7's text. Neither may load, intern or write.
2. **A real no (owner set, interface not implemented):** the native leaves `SMI` as it is. The port may answer
   that, writing nothing, only when the no is settled (route (b) over a chain with no stand-in). Otherwise it keeps
   rule 7's not-read refusal.
3. **The natives of section 3, when a lane binds them:**
   - **`SMI` unset:** answer as the table's last column (decline; `Play` and `PlayData` return 0, `IsPlaying` false),
     with no write;
   - **`SMI` set:** the unidentified interface virtual decides the rest. Until it is read, a not-read refusal,
     "AnimNodeSpecialMoveBlend.<native>: the owner's special-move gate (an unidentified interface function)";
   - **`Finish`:** its script event is the same not-read refusal until its name is resolved. Its `AnimState` write
     must not come before a later refusal: plan it, then write.
4. **Reference** is unchanged.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented):
1. the owner test, three answers:
   - a resolved yes writes `SMI`;
   - an unresolved owner gives the split "not resolved" refusal;
   - a settled no (no stand-in on the chain) leaves `SMI`;
   - each refusal with #518's three counters and the name table's length unchanged;
2. route (a): the set-up's preload lines, labelled, Off only, and that the read-only answer becomes yes after it;
3. `Play`, `PlayData`, `Finish`, `IsPlaying` and `SetRootMotion` with `SMI` unset: the decline answers, nothing
   written;
4. with `SMI` set: the gate's not-read refusal, before any write, `Finish`'s `AnimState` included.

## Open questions

1. **Not read:** the interface virtual the play and finish paths ask, and the name of `Finish`'s script event (by
   fit, `AnimFinished`). Also the object `SetRootMotion` resolves, and the two getters.
2. **Not probed:** whether link 1's stop is the "not resolved" case (expected) or a real no.
3. **UNVERIFIED:** the 4-byte shift of the node's fields against the layout tool.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read `SMI` on each of her three `AnimNodeSpecialMoveBlend` nodes. Expect her pawn.
2. Hook the definitions' `AnimFinished` during a special move that ends. Expect it once, with the node and her pawn
   as `SMI`.
