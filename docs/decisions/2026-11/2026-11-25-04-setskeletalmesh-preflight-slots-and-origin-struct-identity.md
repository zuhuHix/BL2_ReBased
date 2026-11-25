## 2026-11-25: SetSkeletalMesh's preflight: every receiver slot before any write, and Origin's struct identity

AI-assisted (Claude Code, Opus 5.5). Lane FIX-645-PREFLIGHT. It fixes two MEDIUM findings from a late review of #645
(SetSkeletalMesh's mesh change in Off, decision 2026-11-19-252). Each was checked on main d3c7a13 before the fix.
Clean room: the change follows docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md and the port's own heap rules. The
executable and the analysis output were not opened. Synthetic fixtures only. Off only; Reference still binds nothing.

**Finding 1, confirmed: a missing receiver slot was found mid-write.**
- `Heap::write` resolves the object's slot as well as the declaration and the value. Row D checked only the
  declaration and the value.
- A receiver with fewer slots than its class's layout (its public `slots` cut, or a stand-in) therefore passed
  preflight when the cut slot was one the rows never read.
- Probe on main, a toy class whose last slot is `bHasValidBodies`, cut there: three writes committed (`SkeletalMesh`
  changed), then an untyped `VmError::Heap(SlotMissing)`. It carried no "stopped after partial writes" mark, because
  that mark is only added to a `NotYet`.
- **Fix:** row 1 (the receiver resident) now also refuses a receiver with fewer slots than its class's layout.
  - The refusal is `NotYet { NotRead }` with the text `CUT`, on NATIVE_SET_SKELETAL_MESH.md's record.
  - It comes before any read, so every slot the rule reads or writes is settled first.
  - After row 1 and D, a heap write has nothing left to fail on.
- Rows 5 and 6 use the same test for the meshes (it was "no slots at all").
- One behaviour moves: a slotless receiver was a `VmError::Heap` at row 3 and is now row 1's `NotRead`. Both were
  write-free.

**Finding 2, partly wrong.**
- What is wrong: `Origin` and `RotOrigin` are never written. Row 11 only reads them, and the note proposes no
  transform write.
- What is right: row 11 checked their declarations by shape only (a struct with `X`, `Y`, `Z` floats, or `Pitch`,
  `Yaw`, `Roll` ints). A lookalike struct passed.
- **Fix:** the struct type must be `Core.Object.Vector` or `Core.Object.Rotator` (`struct_by_path`), as
  `actor_trace` already does for its vectors. Another struct is a `VmError::Script` naming the field, raised in
  preflight with nothing written, like row 11's other declaration errors.
- The note's "a `Vector` struct" is a fit on `Engine.SkeletalMesh` and stays UNVERIFIED.

**Tests** (`skeletal_mesh_change_tests.rs`). The fixture's `Vector` and `Rotator` move to `Core.Object`. `Engine`
gains same-named lookalikes inside `SkeletalMesh`, two mesh classes that use them, and `ToyLateBodiesComponent`. Each
new test asserts #518's counters, the name table and the component's slots unchanged.
1. `a_receiver_missing_a_slot_writes_nothing`: the component cut at each of its twelve fields, and the late class cut
   at `bHasValidBodies` (the probe's case). Each gives row 1's `NotRead` with no prefix. The late class whole still
   makes the change.
2. `a_cut_mesh_is_not_read`: a new mesh with cut slots is row 6's `NotRead`.
3. `a_lookalike_origin_struct_is_refused`: a lookalike `Origin` and a lookalike `RotOrigin`, on the new mesh and on the
   old, each give the script error.

All three fail on main's module and pass with the fix.

**Not verified:** anything in the running game. Whether the native can ever meet a cut receiver is not known: the
port's heap can hold one, and the rule now refuses it.

**Review r1** (Sol 6.2 xhigh, on #664):
1. **The stored value, not only the declaration.** Row 11 now also requires the slot's struct value to have the
   declared type before it reads members by index. A lookalike value with zero members put in a correctly declared
   `Origin` or `RotOrigin` used to let all four writes through. It is now the same script error, tested for both
   fields with the counters unchanged.
2. **No split between preflight and writes.** `Plan`, `Plan::apply` and `answer` are private. A caller could
   otherwise plan, cut the receiver, then apply, which gives three writes and then `SlotMissing`. Nothing outside the
   module used them.
3. **A cut resident mesh has its own text.** `OLD_CUT` and `NEW_CUT`, still `NotYet { NotRead }` on the change's
   record, instead of the unloaded-handle text, which pointed to a load. The test covers both meshes.
