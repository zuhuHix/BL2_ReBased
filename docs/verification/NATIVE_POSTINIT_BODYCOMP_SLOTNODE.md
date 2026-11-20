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
| 1.1 | identifier `Mesh`: the **old `Pawn.Mesh` released** | **this note, section 2** | `NotYet { NotRead }`, "InitializeBodyComposition: releasing the old Mesh" (#542), raised when the pawn holds a `Mesh` at the build or this build already assigned one |
| 1.2 | `Pawn.Mesh` = the made object; `MeshBodyCompositionIndex`; the mesh flag | #529, NATIVE_MESH_REST.md | written by the mesh-rest step |
| 1.3 | the five lookups (`ArmBoneBlend`, `AimOffset`, `LeftHandIK`, `HeadLookAt`, `bUsingFootIK`) | NATIVE_ANIM_NODE_LOOKUP.md section 1 | `anim_node_lookup` (#557, #618), over the planned tree copy |
| 1.4 | **`SlotNodes` emptied and rebuilt** from the component's tick list: each `AnimNodeSlot` or subclass, in tick order | NATIVE_ANIM_NODE_LOOKUP.md section 2 | the same module |
| 1.5 | `RepMeshScale` on authority; `CollisionComponent` with `bUseMeshForCollision`; the `ReattachWeapons` event | #529, NATIVE_MESH_REST.md | the mesh-rest step |
| 2 | identifier `CylinderComponent`: the **old `Pawn.CylinderComponent` released**, then set, and `CollisionComponent` | **this note, section 2** (the release) and #529 | `NotYet { NotRead }`, "InitializeBodyComposition: releasing the old CylinderComponent", raised when the pawn holds one at the build or this build already assigned one |
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
3. **When at least one copy was removed:**
   - when the old component is a `PrimitiveComponent`, an engine-wide manager is told it was detached. This is
     render-side or streaming-side, the counterpart of the attach's notice in NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md
     section 2.1 step 7, and is not read further;
   - **the component's own detach:** when its `bAttached` is set, its scene detach runs (a virtual, not read). Then its
     scene pointer and its **`Owner`** are cleared.
4. **When no copy was in `Components`:** nothing else happens. The old component keeps its `Owner` and stays as it was.

Then the hook sets the new value. The old component is not destroyed here.

**Two corrections to #529** (NATIVE_INITIALIZE_BODY_COMPOSITION.md):
- **the list:** it is the pawn's own `Components`, not "a global list". #529 says "a global list" in section 4.5
  (step 1, the release) and in section 7 (the release item). The port's code repeats it in `ow-world`
  `world/body_composition.rs`: `releasing_old`'s documentation and the comment above the two release checks in the
  preflight;
- **the gate:** what decides whether anything happens is the old component's **membership in `Components`**, not
  whether it was attached. #529 sections 4.5 and 7 say that a component that is set but not attached releases
  nothing. Rather, a component in `Components` is removed whether or not it is attached, and only its own detach
  (step 3) depends on `bAttached`.

**Maya.**
- **`Mesh`:** measured. In the port the spawned pawn holds no `Mesh` at the build (decisions 2026-11-19-140 and
  2026-11-19-153), so the `Mesh` release is not reached.
- **`CylinderComponent`:** **not measured, UNVERIFIED.** No record measures `Pawn.CylinderComponent` at the build.
  #529 says stock `Pawn` defaults give the pawn a cylinder and Maya's archetype sets one, and decision 2026-11-19-44
  gives each spawned actor its own copies of its template components. So once attachments 0 to 12 pass, attachment
  13 most likely meets Maya's own `CylinderComponent`: in `Components` and, in the running game, attached at spawn.
  **The attached case is the expected one for the cylinder**, and section 4 refuses it (its scene detach was not
  read). That refusal is the likely next stop of the six links after attachment 12 (section 3).

## 3. What links 1, 3, 4, 5, 13 and 14 need

1. **From this hook:**
   - the lookups and `SlotNodes` are ported, and so is the mesh-rest step; the `Mesh` release is not reached
     (measured). The rendering hooks are #622's, being implemented by 0cde;
   - **attachment 13's `CylinderComponent` release** is the expected later stop (section 2, UNVERIFIED, not measured).
     In the game the old cylinder is attached, so section 4 refuses it: "PostInitBodyComposition: releasing an
     attached old CylinderComponent (its scene detach)". Lifting it needs the component's scene detach read, which
     this note does not read (open question 1).
2. **The stop after the hooks is not in this hook.** With local stand-ins for #612's owner event, #622's hooks, the
   socket attach and the `Parent` link (the probe recorded in NATIVE_BONE_TRANSFORM_LAYER.md section 5, #646,
   accepted, landing batch 12; on main 241a6c47), the six links next stop at "stopped after partial writes: AnimNodeSpecialMoveBlend: an owner that does
   not implement SpecialMoveInterface". That is the **slot nodes' own initialisation** (NATIVE_ANIM_NODE_INIT.md
   section H, step 9), not the hook's `SlotNodes` rebuild.
3. **Its cause is the port's, not an unread native (by fit).** NATIVE_ANIM_NODE_INIT.md reads step 9 in full, and
   Maya's pawn class does implement `GearboxFramework.SpecialMoveInterface` (through `GearboxPawn`). The port's owner
   test (`anim_node_init::owner_implements`) answers "unknown" (None) in three cases:
   - the interface class is not in the port's class table;
   - the pawn's class or the interface has no key in the code index;
   - the interface entries the answer needs are not resolved yet (resolving may load class code, which is a write).

   The refusal text is the same for "unknown" and for a real "no". Which case Maya meets is **by fit** (most likely
   the third). A probe would settle it by logging, at the refusal, which of the three holds (and whether the answer
   was None or a real no). NATIVE_SPECIALMOVE_OWNER.md (#647 @ 2de44efb) splits these cases further as O1 to O7 and
   gives their rules. **Lifting it is #647's**, not this note's; it needs no further reading of the game.

## 4. Proposed Off answer for the release (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces the two refusals of steps 1.1 and 2 (`body_composition::releasing_old`, "InitializeBodyComposition:
releasing the old <field>") in the body composition's preflight. **Reference is unchanged.**

**Two cases keep a refusal:**
- **an old value assigned earlier in this build** (a second `Mesh`, or a second `CylinderComponent` attachment, the case
  decision 2026-11-19-153 tests): it keeps today's refusal, "InitializeBodyComposition: releasing the old <field>". In
  the game that component's own attach ran earlier in the build, so it is the attached case, which is refused anyway;
- the attached case below.

**Preflight** (write-free, load-free, RNG-free, intern-free), when the pawn's old value of the field was set before
the build:
1. the pawn's `Components` declared as an array of objects, else `VmError::Script` naming it;
2. **membership, against the planned `Components`:** the pawn's `Components` as the build will hold it when this
   attachment's hook runs. That is the stored list plus this build's earlier appends, including this attachment's own
   append by its attach (#529 section 4.1, step 6), which comes first;
3. **the old component not in it:** the release writes nothing (rule 4 of section 2);
4. **in it, an unloaded handle or a slotless stand-in:** `NotYet { NotRead }`, "PostInitBodyComposition: the old
   <field> is an unloaded handle (a load is a write)";
5. **in it, resident:**
   - its class declares `bAttached` (a bool) and `Owner` (an object), else `VmError::Script`;
   - `bAttached` set: `NotYet { NotRead }`, "PostInitBodyComposition: releasing an attached old <field> (its scene
     detach)". This is the expected case for Maya's cylinder (section 2).

**The writes**, in the build's write phase, before the new value is set:
- the pawn's planned `Components` without any copy of the old component. The removal is composed after this
  attachment's own append, so the list written is the planned list with every copy of the old one removed;
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
6. an old component in `Components` that is an unloaded handle or a slotless stand-in: the `NotRead`, nothing loaded,
   and raised before `bAttached` or `Owner` is read (a stand-in whose class lacks `bAttached` still gets the `NotRead`,
   not the `VmError::Script`);
6a. **planned `Components`:** an old component appended earlier in this build (by another attachment's attach) counts
    as a member and is removed; the written list keeps this attachment's own append, after which the removal is
    composed;
6b. **assigned in this build:** a second `Mesh` attachment, and a second `CylinderComponent` attachment, keep today's
    "releasing the old <field>" refusal, before any write;
7. `Components` of another kind, and an old component class without `bAttached` or `Owner`: `VmError::Script`;
8. every refusal leaves #518's three counters and the name table's length unchanged;
9. under `ParityMode::Reference`: unchanged.

## Open questions

1. **Not read:**
   - the component's scene detach (a virtual);
   - the manager's detach notice;
   - whether anything else holds the released component.
2. **UNVERIFIED, not measured:** that Maya's spawned pawn holds her own `CylinderComponent`, in `Components` and
   attached, at the build, in the port and in the running game.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Before and after Maya's first body build, read `Pawn.Mesh`, `Pawn.CylinderComponent` and `Components`: expect the
   archetype's two components gone from `Components` and their `Owner` None, and the new ones present.
2. Read `Pawn.SlotNodes`: expect three `AnimNodeSpecialMoveBlend` nodes, in NATIVE_ANIM_NODE_LOOKUP.md's order.
