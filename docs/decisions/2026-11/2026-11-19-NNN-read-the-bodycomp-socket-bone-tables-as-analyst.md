## 2026-11-19: The body composition's socket and bone tables are a socket search, a bone search and an Attachments entry

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BODYCOMP-SOCKET-BONE-TABLES, analyst only (clean room: a different
lane implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md` reads what #529's refusal "the
  socket and bone tables of <mesh>" stands for: the skeletal component's `IWorldBody` attach, which Maya's attachments
  1 to 6, 9 and 12 ask of attachment 0's new component. In order:
  1. the socket name is remapped through the parent's `GestaltData.SocketMappings`;
  2. it is searched among the mesh's `Sockets` (a hit adds the socket's location and rotation and takes its bone), and
     otherwise among the mesh's bone names (no bone: false, so the owner retry runs);
  3. a decal case (an exact `DecalComponent` with `bAcceptsDynamicDecals` clear) sets that flag and reattaches;
  4. the component attach runs with unit scale and answers true. It writes or overwrites the parent's `Attachments`
     entry and sets a skeletal child's `AttachedToSkelComponent`.

  Only when the parent is itself attached does it also place and attach the child in the scene from the current bone
  transforms. That is the bone-transform layer 9554's note on `SetSkeletalMesh` covers, and it is shared with this
  note, not read here.
- **Proposed Off answer:** replace #529's refusal at the same preflight site. Through #569's layer, it reads the
  remap, the sockets and the bones. It refuses `NotRead` for the detach, the by-fit `ParentAnimComponent` stop and the
  decal case, and `LayerAbsent` for the placement when the parent is attached. Then it writes the `Attachments`
  entries and the back links in the build's write phase. The `Parent` link stays under ruling 3; when revisited, it
  reads both meshes' bone names through the same layer. Reference is unchanged. The note has a 12-item test list.
- **Maya (installed data):**
  - three socket hits (`Shield`, `ClassMod`, `Launcher`) and five bone hits (`Root`, `Spine3` twice, `R_Thigh`,
    `Hips`): eight entries in order;
  - the mesh's 13 socket objects are unloaded in the port, so with this answer the next stop is the first of them (the
    store's load);
  - a harness preload beside #592's mesh preload is left to the hub.
- **Sources:**
  - the records (#529, #622, #569);
  - the packages: class layouts, and the socket objects' names and bones;
  - `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` and a local call-site listing under the machine lock.
    Raw output stays under ignored `local/analysis/sock/`.

  No addresses, decompiler output or listing structure is in tracked files. No bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**.
- **Probe:** main 4c4522d7 + #618 (3630cd59, with #608) + #592's diff, with 0cde's owner event and #622's hooks stood
  in locally. The six links (1, 3, 4, 5, 13, 14) stop at "InitializeBodyComposition: the socket and bone tables of
  Char_Siren.Mesh.Skel_SirenBody" in `maya:8`, ok 4 / notyet 9 / error 1. Past it (measurement only), the next stop is
  `AnimNodeSpecialMoveBlend`'s owner interface (NATIVE_ANIM_NODE_INIT.md).
- **Index:** a NATIVE_INDEX row was added.
