# The pawn's PostInitBodyComposition: what is left, and the slot nodes (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-POSTINITBODYCOMP-SLOTNODE (decision 2026-11-19-NNN,
number pending). Analyst only: a different lane implements it (clean room).

**Why.** NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529) section 4.5 and its open question 7 left the pawn's
`PostInitBodyComposition` partly unread. That includes the slot-node rebuild, which sits on links 1, 3, 4, 5, 13 and 14
right after the rendering hooks. Since then, three notes have read most of it. This note maps which note owns which
step, reads the one step still unread (the release of the old component), and says what the six links need.

**Sources:**
- the records: NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529); NATIVE_ANIM_NODE_LOOKUP.md (#543, the five lookups and
  `SlotNodes`); NATIVE_MESH_REST.md (the mesh flag and `ReattachWeapons`); NATIVE_BODYCOMP_RENDER_HOOKS.md (#622, the
  three rendering hooks); NATIVE_ANIM_NODE_INIT.md (the slot nodes' own initialisation);
- the port: `ow-world` `world/body_composition.rs`, `world/anim_node_lookup.rs` and `world/anim_node_init.rs` on main
  dba540a3;
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1 -ReadOnly` and a call-site listing under the machine
  lock. The hook's body was found as the routine that interns the name `CustomHead`, as for #622. Its release step was
  read through the routine it calls, and that routine's field reads land on `Actor`'s layout (`Components`) and
  `ActorComponent`'s (`bAttached`, the scene pointer, `Owner`), a structural check of the fit;
- no script side: the hook is native, and #543 already scanned the script for `SlotNodes` readers (none found).

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/pib/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. The hook, step by step, and who owns each step

Mode 0 (the pawn's build), per made attachment, in #529 section 4.5's order:

| Step | What it does | Read in | Port today (main) |
|---|---|---|---|
| 1.1 | identifier `Mesh`: the **old `Pawn.Mesh` released** | **this note, section 2** | `NotYet { NotRead }`, "InitializeBodyComposition: releasing the old Mesh" (#542), raised only when the pawn still holds a `Mesh` |
| 1.2 | `Pawn.Mesh` = the made object; `MeshBodyCompositionIndex`; the mesh flag | #529, NATIVE_MESH_REST.md | written by the mesh-rest step |
| 1.3 | the five lookups (`ArmBoneBlend`, `AimOffset`, `LeftHandIK`, `HeadLookAt`, `bUsingFootIK`) | NATIVE_ANIM_NODE_LOOKUP.md section 1 | `anim_node_lookup` (#557, #618), over the planned tree copy |
| 1.4 | **`SlotNodes` emptied and rebuilt** from the component's tick list: each `AnimNodeSlot` or subclass, in tick order | NATIVE_ANIM_NODE_LOOKUP.md section 2 | the same module |
| 1.5 | `RepMeshScale` on authority; `CollisionComponent` with `bUseMeshForCollision`; the `ReattachWeapons` event | #529, NATIVE_MESH_REST.md | the mesh-rest step |
| 2 | identifier `CylinderComponent`: the **old `Pawn.CylinderComponent` released**, then set, and `CollisionComponent` | **this note, section 2** (the release) and #529 | `NotYet { NotRead }`, "InitializeBodyComposition: releasing the old CylinderComponent", raised only when the pawn holds one |
| 3, 5, 6 | the three rendering hooks | NATIVE_BODYCOMP_RENDER_HOOKS.md (#622) | the hooks' refusal until #622's implementation (0cde) lands |
| 4 | `Data.Name` `CustomHead`: `HeadMesh` | #529 | written |

**Maya's slot nodes** (NATIVE_ANIM_NODE_LOOKUP.md section 3, from her installed tree): `SlotNodes` holds **three**
nodes, all `AnimNodeSpecialMoveBlend` (a subclass of `AnimNodeSlot`), in tick order `SpecialMoveBlend`, `TurnBlend`,
`WeaponAction`. The rebuild writes only `Pawn.SlotNodes`. It runs over the tree instance's nodes, so it needs the
instance; the port plans it over #618's planned copy.

## 2. The one step left: releasing the old component (steps 1.1 and 2)

Before the hook sets `Pawn.Mesh` (or `Pawn.CylinderComponent`), it hands the old value to **the pawn's own component
detach** (`Actor`'s, by fit its `DetachComponent`):
1. **None:** nothing.
2. **Every copy of the old component is removed from the pawn's `Components` array**, the others keeping their order.
   This corrects #529 section 4.5, which called it "a global list": it is the actor's own `Components`.
3. **When at least one copy was removed:**
   - when the old component is a `PrimitiveComponent`, an engine-wide manager is told it was detached. This is
     render-side or streaming-side, the counterpart of the attach's notice in NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md
     section 2.1 step 7, and is not read further;
   - **the component's own detach:** when its `bAttached` is set, its scene detach runs (a virtual, not read). Then its
     scene pointer and its **`Owner`** are cleared.
4. **When no copy was in `Components`:** nothing else happens. The old component keeps its `Owner` and stays as it was.

Then the hook sets the new value. The old component is not destroyed here.

**Maya.** Her archetype `Pawn_Siren` sets both `Mesh` and `CylinderComponent`. So in the running game both releases
most likely run on the first build, against components the spawn put in `Components` (**UNVERIFIED**). In the port as
measured (#542, and the probes since), the spawned pawn holds neither field at the build, so neither release is
reached. Both refusals stay in place for when it does.

## 3. What links 1, 3, 4, 5, 13 and 14 need

1. **From this hook:** nothing new on the port's current path. The lookups and `SlotNodes` are ported, the mesh-rest
   step is ported, and the releases are not reached. The rendering hooks are #622's, being implemented by 0cde.
2. **The stop after the hooks is not in this hook.** With local stand-ins for #612's owner event, #622's hooks, the
   socket attach and the `Parent` link (the probe recorded in NATIVE_BONE_TRANSFORM_LAYER.md section 5, on main
   241a6c47), the six links next stop at "stopped after partial writes: AnimNodeSpecialMoveBlend: an owner that does
   not implement SpecialMoveInterface". That is the **slot nodes' own initialisation** (NATIVE_ANIM_NODE_INIT.md
   section H, step 9), not the hook's `SlotNodes` rebuild.
3. **Its cause is the port's, not an unread native.** NATIVE_ANIM_NODE_INIT.md reads step 9 in full, and Maya's pawn
   class does implement `GearboxFramework.SpecialMoveInterface` (through `GearboxPawn`). The port's owner test
   (`anim_node_init::owner_implements`) answers only from interface entries the code index has **already resolved**,
   because resolving them may load class code, which is a write. At the build, the pawn class's entries are not
   resolved, so the answer is "unknown" and the rule refuses as for a "no". **Lifting it is an implementer or hub
   question:** resolve the pawn class's interfaces before the build (a set-up step, logged), or accept a resolution as
   a load-free read. It needs no further reading of the game.

## 4. Proposed Off answer for the release (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces the two refusals of steps 1.1 and 2 (`body_composition::releasing_old`, "InitializeBodyComposition:
releasing the old <field>") in the body composition's preflight. **Reference is unchanged.**

**Preflight** (write-free, load-free, RNG-free, intern-free), when the pawn's old value of the field is set:
1. the pawn's `Components` declared as an array of objects, else `VmError::Script` naming it;
2. **the old component not in `Components`:** the release writes nothing (rule 4 of section 2);
3. **in `Components`:**
   - its class declares `bAttached` (a bool) and `Owner` (an object), else `VmError::Script`;
   - `bAttached` set: `NotYet { NotRead }`, "PostInitBodyComposition: releasing an attached old <field> (its scene
     detach)";
   - an unloaded handle or a slotless stand-in: `NotYet { NotRead }`, "PostInitBodyComposition: the old <field> is an
     unloaded handle (a load is a write)".

**The writes**, in the build's write phase, before the new value is set:
- the pawn's `Components` without any copy of the old component;
- the old component's `Owner` = None.

The scene pointer is native-only in the port and is not written. The manager's notice has no port state.

**Not covered:** the attached case (refused above). Whether a released component is destroyed or reused later is not
this step's.

## 5. Test list

On synthetic classes (`package()` / `tag()`, every value invented), through the body composition's preflight and write
phase:
1. no old `Mesh`: no release, as today;
2. an old `Mesh` not in `Components`: nothing written for it; the new `Mesh` is set;
3. an old `Mesh` in `Components` twice, unattached: both copies removed, the others in order; its `Owner` None; then
   the new `Mesh` set;
4. the same for `CylinderComponent`, with `CollisionComponent` following #529's rule;
5. an attached old component in `Components`: the `NotRead`, before any write of the build;
6. an unloaded or stand-in old component in `Components`: the `NotRead`, nothing loaded;
7. `Components` of another kind, and an old component class without `bAttached` or `Owner`: `VmError::Script`;
8. every refusal leaves #518's three counters and the name table's length unchanged;
9. under `ParityMode::Reference`: unchanged.

## Open questions

1. **Not read:**
   - the component's scene detach (a virtual);
   - the manager's detach notice;
   - whether anything else holds the released component.
2. **UNVERIFIED:** that Maya's spawned pawn holds `Mesh` and `CylinderComponent` in `Components` at the build in the
   running game.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Before and after Maya's first body build, read `Pawn.Mesh`, `Pawn.CylinderComponent` and `Components`: expect the
   archetype's two components gone from `Components` and their `Owner` None, and the new ones present.
2. Read `Pawn.SlotNodes`: expect three `AnimNodeSpecialMoveBlend` nodes, in NATIVE_ANIM_NODE_LOOKUP.md's order.
