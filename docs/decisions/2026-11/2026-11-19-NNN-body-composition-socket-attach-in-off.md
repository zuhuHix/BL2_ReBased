## 2026-11-19: The body composition's socket attach to a skeletal parent, in Off

AI-assisted (Claude Code)

Lane IMPL-BODYCOMP-SOCKET-BONE-TABLES, implementer, with Claude Opus 5.5. The file number is a placeholder until the
maintainer gives one. Written on #618 and rebased onto main 92c79308 after batch 8 landed #618 and #608.

- **Clean room.** The rule is coded from NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md section 4 alone, as c376's analyst
  lane wrote it. The note is copied in unchanged at its r1 head 05b96935. This lane did not read the executable.
- **Off.** At #529's site, the refusal "the socket and bone tables of <mesh>" (layer-absent) gives way to the note's
  rule, in its preflight order:
  - the remap through the parent's `GestaltData.SocketMappings`;
  - a None name or no mesh is false, with nothing of the mesh read;
  - the sockets, in order, up to the first match (new `skeletal::find_socket`, comparing name ids);
  - on a miss, the bones (new `skeletal::find_bone`, `NameTable::find` with no interning);
  - the gates, in the note's order: the detach, `ParentAnimComponent`, the exact decal class, all not-read; the
    parent's `bAttached`, layer-absent "a skeletal mesh component's bone transforms";
  - the destinations' shapes.

  A true attach writes the parent's `Attachments` entry (overwrite or append, unit scale) and, for a skeletal
  component, `AttachedToSkelComponent`, in place of the owner retry. Every refusal comes from the build-wide
  preflight, so a refused build writes nothing, with no partial-writes prefix.
- **Reference** is unchanged: the body composition is Off only.
- **This lane's additions to the note:**
  - an unloaded template of the new component refuses with the existing lazy-template text before the gates. Its
    `bAttached` cannot be read without a load;
  - the decal class is compared by path, case-blind.
- **The harness input** (hub ruling, 05:35Z): `crates/ow-cli/src/cmd/mesh_sockets.rs`, `preload_mesh_sockets`.
  - It loads a mesh's unloaded socket objects, Off only, logged before and after, labelled "UNVERIFIED stand-in";
  - a failed load keeps its error;
  - it has set-up tests.
  - It is wired right after #592's `preload_mesh` in the `maya` step's class inputs, which are now six. #592's
    harness test counts 10 lines.
- **Tests:**
  - `ow-world` `world/body_composition_socket_tests.rs`: the note's tests 1 to 13. Each refusal is checked against
    #518's three counters and the name table's length;
  - `ow-cli` `cmd/mesh_sockets_tests.rs`: 4 set-up tests;
  - #529's old "socket tables" case is removed from the named-refusals test.
- **UNVERIFIED** in the running game: every rule here, the fitted gate fields, and that attachment 0 is unattached
  when the socket attaches run (the note's open question 2).

## Edits: review r1 of #641

1. **The remap runs for every child of a skeletal parent,** a None name included, so a mapping from None to a socket
   attaches. The None check comes after it. Such a child now needs the parent template loaded (the lazy-template
   refusal), since the remap reads it. There is a regression for None remapped to a socket.
2. **The destination is checked against the heap's write rules in the preflight** (`Heap::check_value`, which is
   read-only). The check covers:
   - the parent's current `Attachments` list;
   - the planned entry, built by the same function the write uses, with the new component's template standing in
     for its class. Each field's declared kind is checked, the offsets' too, and `Component` against the class it
     takes.

   A malformed destination (a float `Yaw`) is a `VmError::Script` before any write. Its test asserts #518's three
   counters and the name table's length.
3. **The socket preload reports readable sockets and slotless stand-ins apart** ("N loaded, M already readable, K
   unresolved stand-ins"). A stand-in is left as it is. There is a harness test.
