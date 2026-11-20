## 2026-11-19: InitGestaltMeshDataFromOther: one order on each operand

AI-assisted (Claude Code, Claude Opus 5.5). Lane FOLLOWUP-GESTALT-OPERAND-ORDER, an implementer follow-up to #613. It
follows the late Sol recheck of the landed code, and it is coded from NATIVE_INIT_GESTALT_MESH_DATA.md (#591) and the
hub's brief. This lane read no native. No real-game session was run. **Every rule is UNVERIFIED in the running
game.** Off only; Reference is unchanged and binds nothing.

**The order** (`ow-inventory` `gestalt.rs`, `operand`). The receiver is checked whole first, then the argument
`ComponentToCopyFrom`, each in the same order:
1. None is a data error (`VmError::Script`): "InitGestaltMeshDataFromOther: the receiver component is None"
   (`RECEIVER_NONE`, new), and "…: ComponentToCopyFrom is None" as before;
2. an unloaded handle is `NotYet { NotRead }` (`RECEIVER_NOT_LOADED`, `ARGUMENT_NOT_LOADED`). This is decided
   **before** any class or field is read;
3. a resident object whose class is not a `SkeletalMeshComponent` is a data error naming it, and so is one whose class
   declares no struct `GestaltData`.

Then come the self-copy (success, nothing written) and F1, the slot checks, as #613 landed them.

**What changed, and what is the port's choice.**
- **Residency now comes before the argument's class and field.** The note lists the argument's class check before its
  residency. The hub's brief orders residency first, so an unloaded handle is never classified by a class read. An
  unloaded handle of the wrong class is therefore `NotRead`, where it was a data error before. Port's order,
  **UNVERIFIED**.
- **The receiver gets the class check.** Before, a resident non-component whose class declared a same-named
  `GestaltData` passed as the receiver. The note names the class check for the argument only, so the receiver's is
  the port's choice, **UNVERIFIED**.
- **A None receiver is the rule's own data error.** The native's wrapper used to refuse it with "called without a
  component". The note does not read this case, since a script call always has a receiver. Port's choice,
  **UNVERIFIED**.
- Every check reads only the class table and the heap. Nothing loads, writes or draws, and no name is interned.

**No LayerAbsent on this path.** The brief asks for a declared slot whose storage is unavailable to reach "the shared
LayerAbsent". That `LayerAbsent` was the note's answer before the layer existed. #613 replaced it (decision
2026-11-19-224): F1 refuses a stand-in with `NotRead` (`INIT_STAND_IN`, the hub's ruling on #603), refuses an undecoded
store definition with `NotRead` (`NOT_DECODED`), and treats a malformed slot as a data error. This lane keeps those and
tests them exactly. **For the hub:** if a `LayerAbsent` is still wanted for some storage state, that state needs naming.

**Tests** (`gestalt_tests.rs`, on #596's invented package, rule 9):
- the fixture adds a `SkeletalMesh` field on the component, and `ToyLookalike`, a non-component class that declares
  `GestaltData`;
- `each_operand_refuses_in_one_order`: for each operand, None, unloaded, an unloaded handle of the wrong class (NotRead,
  so residency comes first), a non-component, and the look-alike. The receiver is checked before the argument. Every
  refusal leaves #518's counters, the loaded objects' slots and the name table's size unchanged, and the handles stay
  unloaded;
- `the_self_copy_the_layer_refusals_and_one_whole_slot_write`:
  - a self-copy changes nothing, with no name interned;
  - F1's stand-in and undecoded refusals are exact (kind, text, record) on either operand;
  - a copy between two whole components is exactly one heap write, with no draw and no load. The receiver keeps its
    identity and its own `SkeletalMesh`, its `GestaltData` equals the argument's value with the same objects inside,
    and the argument is unchanged;
- the old order test is replaced by these two; Reference binding nothing stays covered by
  `off_binds_both_natives_and_reference_binds_none`.

**Gates.** `cargo test --workspace -j 2`: 3,131 passed / 0 failed / 51 ignored; `ow-inventory`'s gestalt tests 28
passed. `crate_edges_test.py` OK. Clippy: no new hits; the one warning in `gestalt_data_tests.rs` was already on main.
Installed Off strict `--slice-gate` (release, under the machine lock), on main df0970ec plus this lane: 4 ok / 10
NotYet / 0 errors. Link 8 passes `make` and stops in `init` at `BuildCompositeMesh`'s cold gestalt definition, the
harness gap #632 closes. This native is not reached there, and the gate is the same as main's. ctest and
verify_packages were not run (no C++ change). This is what the host did, not parity.
