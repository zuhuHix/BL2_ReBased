# The body composition's socket and bone tables (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BODYCOMP-SOCKET-BONE-TABLES (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented #618 and the copy pass
(#608), which come before this step; those are different rules. It implements nothing here.

**Sources, in order:**
- the records:
  - NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529), section 4.1 step 6 (the attach) and step 7 (the `Parent` link), and
    its section 7 refusal "the socket and bone tables of <mesh>";
  - NATIVE_BODYCOMP_RENDER_HOOKS.md (#622);
  - the skeletal mesh data layer, `ow-world` `world/skeletal.rs` (#569, decision 2026-11-19-184): `sockets` and
    `reference_skeleton`;
- the installed packages:
  - the class layouts of `SkeletalMeshComponent` (with its `Attachment` and `GestaltDataContainer` structs),
    `SkeletalMesh`, `SkeletalMeshSocket`, `ActorComponent`, `PrimitiveComponent`, `CustomSkeletalMeshComponent`,
    `GearLikenessMeshComponent` and `DecalComponent`, through `tools/ghidra/class_layout.py`;
  - Maya's mesh's socket objects, through `ow-package --object-dump` (names only are recorded here);
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1 -ReadOnly` and a local call-site listing, under the
  machine lock:
  - the component's attach is the `IWorldBody` slot #529 already named on `SkeletalMeshComponent`;
  - its callees: the socket-name remap, the socket search, the bone search and the component attach;
  - the class tests, identified from the class names their lazy class lookups register.

  Its field reads land on the computed layouts, a structural check of the fit. The fields are `SkeletalMesh`,
  `GestaltData.SocketMappings`, `bAcceptsDynamicDecals`, `bAttached`, `Attachments`, `AttachedToSkelComponent`,
  `SpaceBases` and `bOverrideAttachmentOwnerVisibility`, and on the mesh `Sockets` and its bone-name map.
  - `SkeletalMeshComponent`'s own fields need a 16-byte shift;
  - `PrimitiveComponent`'s flags need the known 8-byte shift;
  - `ActorComponent`'s fields and the mesh's fields need none.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/sock/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. Where the step sits

The body composition makes a component for an attachment and appends its entry. Then (#529 section 4.1 step 6) it
asks the parent to attach it, with a zero location, a zero rotation and the attachment's `ComponentData.MeshSocketName`.
On Maya's first build the parent of attachments 1 to 6, 9 and 12 is attachment 0's new `SkeletalMeshComponent`. Its
answer to that call is what "the socket and bone tables" means. Each of those eight attachments carries a socket name,
so each one asks.

**Template or made object.** Every component read is the **made** one:
- the parent: attachment 0's new component, a copy of its template made earlier in the same build;
- the new component: the attachment's own new component.

The made copies hold their templates' values except what earlier steps of the build wrote. The mesh asset, its socket
objects and its bones are **shared**: the parent's `SkeletalMesh` names the same asset as its template.

## 2. What the skeletal component's attach does

In order. The answer is true or false, and false sends #529's owner retry.

1. **The socket-name remap.** The name is looked up in the parent's `GestaltData.SocketMappings`, in order. The first
   entry whose first name equals it replaces it with that entry's second name. With no match, the name stays.
2. **The socket search.** With a `SkeletalMesh` set, its `Sockets` are searched in order, and the first socket whose
   `SocketName` equals the name wins.
   - **On a hit:**
     - the socket's `RelativeLocation` is added to the passed location;
     - its `RelativeRotation` is added to the passed rotation, component by component;
     - the bone name becomes the socket's `BoneName`;
     - the socket's `RelativeScale` is **not** used.
   - **On a miss**, or with no mesh: the answer is **false**. That happens when the name is None, when there is no
     mesh, or when the mesh's bone-name map finds no bone of that name. Otherwise the name is used as the bone name,
     with the offsets unchanged.

   The bone-name map is the mesh's map from bone name to index, built from its reference skeleton's names (by fit;
   the same names the port's `reference_skeleton` reads). Name equality is the engine's name equality: case-blind,
   with the number part compared.
3. **The decal case.** It applies only when the parent's `bAcceptsDynamicDecals` is clear and the attached object's
   class is **exactly** Engine's `DecalComponent`. Then the parent's `bAcceptsDynamicDecals` is set, and its
   deferred reattach runs: `bNeedsReattach`, and the owner's component update.
4. **The component attach**, with the bone name, the location, the rotation and a **unit scale** (1, 1, 1). Then the
   answer is **true**, whatever the attach did.

### 2.1 The component attach

1. A virtual test of the parent, not identified. By its place, an object-state test (being destroyed). When it
   answers true, the attach does nothing.
2. **The detach first.** When the attached object's `bAttached` is set, it is detached from where it is.
3. A field of the parent is tested, by fit its `ParentAnimComponent`. When it is set, the attach stops here with
   nothing written. **UNVERIFIED:** the object the test reads is by fit.
4. **The entry.** The parent's `Attachments` list (`Component`, `BoneName`, `RelativeLocation`, `RelativeRotation`,
   `RelativeScale`):
   - an entry whose `Component` is the attached object gets its other four fields overwritten;
   - otherwise a new entry is appended with all five.
5. **The back link.** When the attached object's class chain holds `SkeletalMeshComponent`, its
   `AttachedToSkelComponent` is set to the parent. That is true of `CustomSkeletalMeshComponent` and
   `GearLikenessMeshComponent`, so all eight of Maya's.
6. **The placement**, only when the parent's own `bAttached` is set, the bone name is not None, the parent has a mesh
   and the bone is found with an index below the length of the parent's `SpaceBases`:
   - the attached object's world transform is composed from the entry's offsets, that bone's current transform (from
     `SpaceBases`) and the parent's `LocalToWorld`;
   - when the parent's `bOverrideAttachmentOwnerVisibility` is set and the attached object is a primitive, its
     `bOwnerNoSee` and `bOnlyOwnerSee` are copied from the parent through their setters;
   - the attached object is attached to the parent's scene with the parent's `Owner` and that transform. This sets
     its scene and `Owner` fields, clears its `bNeedsReattach` and `bNeedsUpdateTransform`, sets its transform, and
     attaches it when it is valid. A primitive is then announced to a render-side manager.

   This whole branch is the **bone-transform layer**: the current bone transforms and the world placement. **9554's
   note on `SetSkeletalMesh` covers it** (link 8's stop). This note does not read it further.
7. **The streaming notice.** When the attached object is a `PrimitiveComponent`, an engine-wide manager is told that
   it was attached. That is render or streaming state, not read further. Nothing script-visible is known to change.

## 3. Maya's eight socket attaches (installed data, local probe)

The parent is attachment 0's component, of mesh `Char_Siren.Mesh.Skel_SirenBody`.
- **The parent's template:** `SocketMappings` is empty, `Attachments` is empty, `bAcceptsDynamicDecals` is set,
  `bOverrideAttachmentOwnerVisibility` is clear and `ParentAnimComponent` is None.
- **The mesh:** it holds **13 sockets**, and the port's skeleton source gives **27 bones**.
- **The sockets:**
  - by name: `LilithMeleeFX`, `StorageDeck`, `FX_Mark`, `BlightPhoenixFX`, `ClassMod`, `Grenade`, `FX_RFoot`,
    `FX_LFoot`, `Shield`, `SnapZoom`, `Weapon`, `ShieldParticle` and `Launcher`;
  - in the port they are 13 handles the store has **not loaded**.

| Attachment | Socket name | Matched as | Bone |
|---|---|---|---|
| 1 | `Root` | bone (no socket of that name) | `Root` |
| 2, 3 | `Spine3` | bone | `Spine3` |
| 4 | `Shield` | socket | `Hips` |
| 5 | `ClassMod` | socket | `Spine1` |
| 6 | `R_Thigh` | bone | `R_Thigh` |
| 9 | `Hips` | bone | `Hips` |
| 12 | `Launcher` | socket | `Spine3` |

So every attach answers true. In order, the parent's `Attachments` gets eight entries (1, 2, 3, 4, 5, 6, 9, 12):
- for the three socket hits, the socket's location and rotation;
- for the five bone hits, zero offsets;
- unit scale for all eight.

Each new component's `AttachedToSkelComponent` becomes the parent, and none is a decal. A bone match needs every socket
read first, because no socket of that name may exist. So each of the eight reads all 13 socket objects or stops at its
own hit.

**The placement branch (2.1 step 6) in the port.** The port's actor attach of attachment 0 does not set `bAttached`,
and the template's is clear, so in the port the branch is skipped. In the running game the pawn is spawned in a world,
so attachment 0 is most likely attached by then and the branch runs (**UNVERIFIED**). That is the shared layer with
9554's transforms.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces #529's refusal "the socket and bone tables of <mesh>" (`LayerAbsent`,
the skeletal mesh data layer) at the same site in the body composition's preflight. It reads the port's layer
(`skeletal::sockets` and `skeletal::reference_skeleton`, #569) and nothing else. **Reference** keeps the stub policy.

**Preflight** (write-free, load-free, RNG-free; it runs where #529's refusal is). The first failure is the answer:
1. **The remap:** the parent's `GestaltData.SocketMappings`, read from the settled template values the preflight
   already uses (#622 r1, item 3), applied as section 2 step 1. A missing field or a wrong kind is a
   `VmError::Script`.
2. **The sockets:** `skeletal::sockets(parent mesh)`, in order.
   - The layer's own refusals pass through unchanged. An unloaded socket handle is `NotYet { LayerAbsent }`, "the
     skeletal mesh data layer: the unloaded socket <name>", the store's load. **That is Maya's next stop in the
     port.** The layer loads nothing, and neither does this.
   - A first name match is the hit (section 2 step 2).
3. **The bones**, on a miss: `skeletal::reference_skeleton(parent mesh).bone_index(name)`. No source, or no skeleton
   for the mesh, is the layer's `NotYet { LayerAbsent }`. No bone means the answer is false: the existing owner retry
   with no socket name.
4. **The attach's gates,** on a hit or a bone:
   - the parent's unidentified state test (2.1 step 1) is taken as passing for a made component (UNVERIFIED);
   - the new component's `bAttached` set: `NotYet { NotRead }`, "InitializeBodyComposition: the detach before a
     socket attach";
   - the parent's `ParentAnimComponent` set (2.1 step 3, by fit): `NotYet { NotRead }`, "InitializeBodyComposition: a
     socket attach to a component with a parent animation component";
   - the decal case (section 2 step 3, the class **exactly** `Engine.DecalComponent`, and the parent's
     `bAcceptsDynamicDecals` clear): `NotYet { NotRead }`, "InitializeBodyComposition: a decal attached to a skeletal
     component" (the deferred reattach, as in #622);
   - the parent's `bAttached` set: `NotYet { LayerAbsent }`, "InitializeBodyComposition: the placement of a socket
     attach", with the bone-transform layer as 9554's note names it.
5. **The planned entry:** for the parent, against its planned `Attachments`, that is the template's list plus the
   entries this build planned before it. An existing entry for the same component is an overwrite; otherwise an
   append.

**The writes,** in the build's write phase, after every preflight, in the attachments' order:
- the parent's `Attachments` entry: an append or an overwrite, with `Component`, `BoneName`, location, rotation and
  scale (1, 1, 1);
- the new component's `AttachedToSkelComponent`, when its class is a `SkeletalMeshComponent`.

Nothing else is written: the streaming notice has no port state. A refusal after earlier writes of the build is
marked "stopped after partial writes: " by the build's existing write tracker.

**The `Parent` link** (#529 section 4.1 step 7, hub ruling 3) comes right after attachment 1's attach and stays
`NotRead`. When it is revisited, its parent-bone map reads both meshes' bone names through the same
`reference_skeleton` layer. That is the one table this note shares with it.

**Not in scope:** the placement branch (9554's transforms), and the harness question below.

**A harness choice for the hub, not a rule:** with this answer in place, the six links stop at the first unloaded
socket of `Skel_SirenBody` (`SkeletalMeshSocket_20`, the store's load). The mesh's 13 socket objects are in the same
package as the mesh. A harness preload of them, beside #592's mesh preload, is the obvious stand-in for the streaming
load (UNVERIFIED), and that is the hub's ruling. The rule does not load them itself.

### 4.1 Test list

On synthetic classes (`package()` / `tag()`, every value invented):
1. A socket hit: the entry gets the socket's location and rotation added to zero, its `BoneName`, unit scale; the
   socket's own scale is ignored. The new component's `AttachedToSkelComponent` is the parent.
2. A bone hit (no socket of that name): zero offsets, the name as the bone. A socket and a bone of the same name: the
   socket wins.
3. No socket and no bone: false, and the owner retry runs as today. A None name, and a parent without a mesh: false.
4. The remap: a `SocketMappings` entry rewrites the name before the search, and the first matching entry wins.
5. Two attaches of the same component: the second overwrites the entry, and the list does not grow. Two different
   components: two entries in order.
6. A non-skeletal new component (an invented primitive): the entry is written and the back link is not.
7. An unloaded socket handle before the hit: the layer's `LayerAbsent`, nothing loaded or written, #518's three
   counters unchanged. An unloaded handle after the hit is never read: the answer stands.
8. No skeleton source on a bone search: `LayerAbsent`, the counters unchanged.
9. Each gate refusal (the new component's `bAttached`, the parent's `ParentAnimComponent`, the exact decal class with
   the flag clear, the parent's `bAttached`): its own refusal with the counters unchanged. A decal **child** class,
   and a decal with the flag set, are not refused.
10. A refusal on the second of two socket attaches, after the build wrote the first: marked "stopped after partial
    writes: " by the build's tracker.
11. Under `ParityMode::Reference`: unchanged.
12. Maya's shape (invented names): three socket hits and five bone hits in her order give eight entries in that order.

## 5. Local probe (never pushed)

The probe tree:
- `origin/main` 4c4522d7 with #618 (3630cd59, which carries #608 193ef94f);
- #592's own diff (the mesh preload and the skeleton source), applied with a three-way merge;
- the `.psk` export of Maya's body.

**Local stand-ins, behind one environment variable:**
- 0cde's owner event (#612) and #622's rendering hooks, answered;
- for the measurement only: this note's step logged at the refusal site, and the `Parent` link passed.

**Installed Off `--slice-gate`, strict:** ok 4, notyet 9, error 1 of 14 on every run.
1. With only #618 and #608, the six links (1, 3, 4, 5, 13, 14) stop at "the skeletal mesh data layer: the unloaded
   skeletal mesh Skel_SirenBody". That is the mesh preload #592 adds.
2. With #592's preload: "the owner's event after the tree initialisation" (#612).
3. With the event and the hooks stood in: **"InitializeBodyComposition: the socket and bone tables of
   Char_Siren.Mesh.Skel_SirenBody"** (`LayerAbsent`, "a skeletal mesh data reader (sockets and the reference
   skeleton)"). That is this note's stop, reached in step `maya:8` of link 1; links 3, 4, 5, 13 and 14 are blocked
   behind it with no step of their own.
4. Logging section 3's matches and passing the `Parent` link: the next stop is "stopped after partial writes:
   AnimNodeSpecialMoveBlend: an owner that does not implement SpecialMoveInterface" (NATIVE_ANIM_NODE_INIT.md). That
   rule is outside this note.

## Open questions

1. **Not read:**
   - the parent's first state test (2.1 step 1);
   - the object 2.1 step 3 tests (by fit, the parent's `ParentAnimComponent`);
   - the render-side manager calls (2.1 steps 6 and 7).
2. **UNVERIFIED:**
   - whether attachment 0 is attached (`bAttached`) when the socket attaches run in the running game. If so, the
     placement branch runs there and the port skips it;
   - whether the placement's `Owner` write matters to later script. The port's actor attach writes `Owner` only for
     attachments 0 and 13.
3. **UNVERIFIED:**
   - the field fits under the 16-byte and 8-byte shifts;
   - that the bone-name map holds exactly the reference skeleton's names.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her `Mesh.Attachments`: expect entries for the head and the seven gear-likeness components,
   in section 3's order, with the bone names of its table. The three socket hits should carry non-zero offsets, and the
   others zero; every scale should be 1.
2. Read `HeadMesh.AttachedToSkelComponent` and one gear-likeness component's: expect `Mesh`.
3. Read `Mesh.bAttached` at that point, and each attached component's `Owner`: this settles open question 2.
