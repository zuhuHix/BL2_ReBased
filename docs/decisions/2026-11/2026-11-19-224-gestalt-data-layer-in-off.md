## 2026-11-19: The gestalt component data layer in Off: the fill and the whole-slot copy

AI-assisted (Claude Code, Claude Opus 5.5), lane IMPL-GESTALT-DATA-LAYER. Coded from
`docs/verification/NATIVE_GESTALT_DATA_LAYER.md` alone (#603, `research/rust-gestalt-data-layer` 9e5113c0, with the
hub's ruling on #603), stacked on #596 (`fix/rust-gestalt-mesh-layer` 491a1ba). This thread did not write the note
and read neither native (clean room). **Every rule is UNVERIFIED** in the running game: no bl-sdk session was run.

**What changed (Off only; Reference keeps the stub policy and binds neither native).**
- `crates/ow-inventory/src/gestalt_data.rs` (new) holds the layer: the component's `GestaltData` slot in the heap,
  one `GestaltDataContainer` value.
  - **The fill** (`plan_fill`, the note's section 2): `PartMeshNames`, `SkeletalMesh`, the two emptied arrays, the
    early stop without a shared mesh, one `GestaltLODModel` per gestalt info with the matching parts' fragments in
    the parts' order, the start bounds, then per list name the socket mappings and the bounds (copied whole while the
    radius is not above 0, else the union).
  - **The union** in the note's operation order, every step in f32, the roots through `ow_core::num::sqrt_f64` and
    back to f32 (rule N3). Min and max are compare-and-select. Bit parity with the game is UNVERIFIED.
  - **The start constant** is the zero vector, a port input (UNVERIFIED: the engine sets it at start-up).
  - **The copy** (`plan_copy`): F1 on the receiver, then the argument; the argument's value checked against the
    receiver's declaration; one write.
- `crates/ow-inventory/src/gestalt.rs` (#596's): `plan_build` and `plan_init` are the write-free preflights (`&Vm`);
  `build_composite_mesh` and `init_gestalt_mesh_data` now take `&mut Vm` and make each preflight's single
  whole-slot write. The four `LayerAbsent(GESTALT_LAYER)` sites the note names are gone, with the constants
  `GESTALT_LAYER`, `GESTALT_DATA` and `INIT_GESTALT_DATA`. `MERGE_LAYER` stays.

**The preflight, in the note's order** (each refusal before the single write, so #518's counters do not move and
none carries the "stopped after partial writes: " prefix):
1. the component's `GestaltData` declares the seven members at their declared types, nested structs included (a
   data error naming the member). Members are matched by name, and struct names are not checked;
2. F1: (i) a store definition with undecoded slots, `NotRead` ("BuildCompositeMesh: a component whose properties the
   store has not decoded (a decode is a write)"); (ii) a slotless stand-in, `NotRead` with #596's `STAND_IN` text and
   the note's record (the hub's ruling); (iii) a slot that does not match its declaration, a data error;
3. the definition's four members (a data error naming the first that does not fit), and their slots complete;
4. F2: the definition's three arrays re-walked from its export with the package walker
   (`ow_package::props::read_tagged`), typed from reflection as the store types them
   (`ow_object::decode::ReflectionTyper`), with no `Raw` or delegate value anywhere in their trees. The walk reads
   the package's bytes only. **Port answers, UNVERIFIED:** the walk starts 4 bytes into the payload (an ordinary
   object); a failed walk is F2; a definition with no export origin (one built in the heap) has no tags to walk, so
   its heap values stand;
5. the new value checked with `Heap::check_value` before the write.

For the copy, check 4 runs F1 on both components with the copy's texts:
- F1(i): #596's `NOT_DECODED`;
- F1(ii): "InitGestaltMeshDataFromOther: a component that is a stand-in with no properties (an object the store cannot
  load)";
- F1(iii): "InitGestaltMeshDataFromOther: a GestaltData slot that does not match its declaration".

The self-copy still answers first, with nothing written.

**Verified (host only):** 10 new synthetic tests in `gestalt_data_tests.rs` (rule 9), on #596's invented package,
extended with the container's members, two serialized gestalt definitions and one without the arrays. They cover the
note's tests 1 to 9, plus the native's single write (one committed write, no draw, no load). Test 2 pins the union's
three cases exactly in f32. Test 5's F2 case is a part element holding a `DelegateProperty` tag, which the walker
leaves raw while the store still loads the definition. It is not the note's "nested `Parts` left raw by the typer"
case, which this fixture could not produce. #596's three tests that expected the layer's refusal now expect the fill,
the copy, or F1's texts. Test 10 (Reference binds nothing) is #596's.

**Review r1 of #613, folded in.** Tests now reach every refusal of the declaration check: a container member and a
definition array of the wrong kind, an incomplete definition slot, and a planned value the heap's write rules refuse
(the fixture's container `SkeletalMesh` is now declared as the mesh class). One pins the component's declaration
before F1. F2's walk names its prefix by `PropertyPrefix::Ordinary`, the one the store's `derive_prefix` picks. The
note's own F2 case (a nested `Parts` the typer leaves raw) is still not pinned; the delegate-tag case stands in.

**Open item, the first check when link 8 reaches this native: F2's coverage on real data.**
- A possible false pass: an array the definition does not tag itself, inherited from its archetype or class default,
  is absent from the walk, so the filter is empty and passes.
- Possible false refusals: an unrelated tag that makes the whole walk fail, and a `Raw` value in a field of the three
  arrays that the fill does not read.

**Not verified:** every rule in the running game; the start constant's value; the union's bit parity; that the
installed gestalt definitions' arrays walk whole (F2 on real data). The installed slice gate does not reach these
natives yet: link 8 stops earlier, at `WillowWeapon.CacheWeaponCard`.
