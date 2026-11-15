## 2026-11-15: Rust interpreter aggregates, foreach, delegates and interfaces as shipped

AI-assisted (Claude Code). Lane W4-INTERP2 of the Rust port fills `crates/ow-vm/src/interp/{aggregate,iter,delegate}.rs`
(the struct, array, iterator, delegate and interface cases of `src/interp.cpp`). It uses its two grants (the `Vm`
interface query, the C++ `Runtime::implements`, and the edit of `natives_core::register` that wires it into
`QueryInterface` and the iterators' InterfaceClass filter) and two the hub granted after the lane's first report (the
dispatch seam in W3-INTERP's files, and the `tests/vm_test.py` fixture gaps). It builds on decision 2026-11-15-14
(frames, live out aliases, `MAX_NESTING`, `STACK_SIZE`). This file records the shipped interface, the rules ported,
where the Rust output differs from the C++ and why, and what stays open.

- **Interface.**
  - `Vm::implements(&mut self, class, iface) -> bool` (`api.rs`, granted): `CodeIndex::implements` over the classes'
    package exports, which falls back to the structural rule when a chain's bodies do not all decode, as the C++ does.
    A class defined by hand (no package export) implements nothing.
  - `natives_core::register` (granted): `QueryInterface` answers child-of or implements; `AllActors` and
    `DynamicActors` keep the objects whose class implements the InterfaceClass. Their two unit tests that pinned the old
    `NotYet` now assert the answer for classes defined by hand (None, and an empty list); the hub accepted the flip.
  - **Granted edit: the dispatch seam in W3-INTERP's files** (approved by the hub after the fact; the stubs W3-INTERP
    left took only an opcode, so they could not be filled without changing their call sites), one line per file:
    - `crates/ow-vm/src/interp/eval.rs`: the `Place::In(Box<Place>, Vec<ow_object::heap::Step>)` variant with its read
      and write arms, the `foreach` stack local to `run`, the dispatch arms of `lval`, `eval` and `Let`,
      `InterfaceContext` passed through and `InterfaceCast` sharing `DynamicCast`, `DynamicCast` through
      `Vm::implements` (the interface `NotYet` and its `I<Upper>` guess removed), `pub(crate)` on `Frame::at` and eight
      helpers, and a new `frame_value_mut`.
    - `crates/ow-vm/src/interp/convert.rs`: the primitive casts 0x36, 0x37 and 0x46 return the value as it is (the C++
      "views share a reference"; UNVERIFIED in the game).
    - `crates/ow-vm/src/interp/mod.rs`: its documentation paragraph on what runs and what waits.
  - **Granted edit: `tests/vm_test.py`'s fixture** (granted by the hub, in its own commit). `make_function`'s array
    children (the `Items` locals) name an inner `IntProperty`, made before the children and moved into its array; the
    `VfTable_` table properties of `make_class` name the fixture's invented `Pair` struct. Invented values only (rule
    9); no assertion changed. The C++ reads neither reference. The Rust reflection refused both layouts (`Items names no
    inner property`, `VfTable_IThing names no struct`), the kind of gap PR #315 closed for object and class
    properties; no bound is loosened.
  - Public constants: `aggregate::MAX_ARRAY_LEN`, `aggregate::MAX_VALUE_DEPTH`, `iter::MAX_ITERATORS` (crate-visible
    modules).
- **Rules ported** (each the C++ reading, UNVERIFIED in the running game).
  - **Places inside aggregates.** A struct member or an array element is its base place plus a path of struct entries
    and array elements. A read walks the path; a write goes through the heap's `write_in` for a property (after the
    representation rules of decision 2026-11-15-14) or through the frame's storage for a local (untyped, as in the C++).
    The path is followed again on every use. An element removed since the place was taken (`F(out A[1])` where `F`
    empties `A`) is a typed `VmError::Script`; the C++ pointer would dangle. An out argument naming an element aliases
    it live, as any out argument.
  - **Struct members.** The member is found by name in the value's struct layout. A value that is not a struct, or has
    no such member, is a temporary None (the C++ `scratch`).
  - **Static arrays** are consecutive slots (decision 2026-11-15-02), not one array value as in the C++:
    - `A[i]` on a static-array local, property, default property or struct field is that element's slot or entry;
    - an index outside `0 .. dim` is logged (`accessed array out of bounds: <i>`) and reads None;
    - a static array used as a whole (passed, assigned, returned) is `NotYet`. It is not seen in compiled script as far
      as read; the C++ handles it as an array value.
  - **Dynamic arrays**, each C++ case in its order (the array's place, then the operands, then the array is made an
    array if it was not one):
    - element read past the end: logged, None, no growth; element write past the end: the array grows;
    - `Length` read and set (a negative length empties; the result is the length asked for);
    - `Add`, `AddItem`, `Insert`, `InsertItem` (index clamped to `0 .. length`), `Remove` (only for `0 <= at <=
      length`), `RemoveItem` (every equal element), `Find` and `FindStruct` (by `same_value`, -1 when absent).
    - **What an array gains.** A heap array gains its element kind's zero. That is the same value the C++ adds (its
      `zeroLike` of a typed element is the kind's zero, and the C++ None it adds to an empty array is stored as the
      kind's zero by the heap rule). A local array gains `zeroLike` of its last element, or None when empty, as in the
      C++. An item added to a heap array is checked against the element kind (a float into an `array<int>` is the heap's
      `WrongKind`, where the C++ stores it untyped).
    - **Temporaries.** A temporary that is not an array is treated as the empty array the C++ makes of its scratch, and
      an element grown in a temporary is a temporary None. This was found by the installed-game comparison below: five
      `WillowPlayerController` functions write `X[n]` under a None context.
  - **`DynArraySort`** is the C++ stub site (`UNIMPLEMENTED DynArraySort`, `StubPolicy` key `DynArraySort`). Nothing is
    evaluated and nothing sorted, so no comparator runs and there is no sort order to match. A real sort is a rule
    still to read.
  - **Struct comparison** is `same_value`: field by field, the struct type not compared, as in the C++.
  - **`foreach` over a dynamic array.** The array, item and index places are taken once when the loop starts. The
    array is read through its place at every `IteratorNext`, so a body that changes the array is seen, as in the C++;
    the item and the index are written as they are. A state is pushed even for an empty array, so an empty inner loop
    does not pop the enclosing one.
  - **`foreach` over an engine iterator.**
    - The call sits under any `Context` wrappers and must be a numbered native with an iterator bound.
    - The wrappers are resolved from the frame's object (None skips the loop and counts as a None context), the
      arguments are bound as for a call, and the call counts in the census.
    - The out argument's place is the loop variable; it reads None once the list is exhausted.
    - An object destroyed since the list was made is skipped. Rust-only: so is a handle the heap no longer knows,
      since the C++ list keeps its objects alive.
    - A call that is not a bound iterator is the C++ stub site (`StubPolicy` key: the iterator native's key, or
      `foreach over an engine iterator` when the call names no function). It is jumped over with no state pushed, as
      in the C++, so its `IteratorPop` pops the enclosing loop's state there too. The call trace gets
      `iterator <path>`.
  - **Delegates.**
    - `DelegateProperty` and `InstanceDelegate` give the context object and the named function; `EmptyDelegate` gives
      an unbound delegate; `LetDelegate` is `Let`.
    - A call reads the delegate property of the context object. When it is bound to an object and a function name,
      the function is found on that object in its state and called on it; otherwise the delegate's declaration is
      called on the context object. No function found is logged (`delegate <name> is not bound`) and gives None.
    - The four comparisons compare object and function text as the C++ compares its value's fields. Names compare by
      their interned text; names are case-insensitive here, while the C++ compares the two spellings byte for byte.
    - **A delegate held in a local variable.** The C++ never reads it and calls the declaration (its own note: "not
      supported yet"). Under `OW_PARITY=reference` that is reproduced. In Off mode an unbound local calls the
      declaration (both readings agree) and a bound one is `NotYet`: which function the game calls then is not read
      (architecture R3).
  - **Interfaces.**
    - `QueryInterface`, `DynamicCast` and `InterfaceCast` answer child-of or implements.
    - A "no" through an engine-native stand-in class stays `NotYet` (decision 2026-11-15-08), in the casts, in
      `QueryInterface` and in the iterators' InterfaceClass filter (review r1 of PR #318).
    - **The caller rule.** `Vm::implements` stays a plain `bool`; its callers apply the stand-in check. After a "no" a
      caller asks `interp::is_child(vm, class, iface)?`, which answers `NotYet` through a stand-in class. The rule can
      be followed only inside `ow-vm`, because `interp::is_child` is crate-internal. A caller in another crate (a native
      pack) needs a public checked form, such as a `Vm` method returning `Result<bool, VmError>`. That is open (review
      r2 of PR #318); until it exists, such a caller must not treat a plain "no" from `implements` as settled.
    - `InterfaceContext` passes its operand through.
- **Recursion, bounds and order (the brief's questions).** Every expression level these files add goes through the
  interpreter's nesting budget (`MAX_NESTING`), including the static-array base walk. The only recursion on values is
  `zeroLike` over a struct's entries, held by the value-depth bound below; none runs on package data. Rust-only typed
  refusals:
  - a dynamic array holds at most `MAX_ARRAY_LEN` (1,048,576) elements, reserved fallibly;
  - a value nests at most `MAX_VALUE_DEPTH` (256) structs and arrays. Locals are untyped, so script could nest an array
    in itself (`Items.AddItem(Items)`), and comparing, printing and dropping such a value recurse once per level. The
    depth is measured without recursion, once per shared allocation, so it stays linear on values that hold one array
    many times.
  - a frame holds at most `MAX_ITERATORS` (4,096) loop states.

  No game value is known to meet any of them. There is no sort comparator (above), so no comparator order.

  **Known gap: the depth bound does not bound a value's size** (review r1 of PR #318). Shared allocations let a shallow
  value stand for very many leaves: `Items.AddItem(Items)` 60 times stays at depth 60 but holds about 2^60 leaf
  places. A later `==`, `Find`, `RemoveItem`, `Case` or a print then walks every one of them, with no step tick, so the
  call hangs instead of giving a typed error (the C++ would run out of memory). Size counting is a Rust follow-up (on
  the hub's list), not in this lane. Compiled script does not build such a value as far as read.
- **Where the output differs from the C++, and why.**
  - The heap typing above: an item of the wrong kind refused, a removed element refused, and a static array as a whole
    `NotYet`.
  - A local delegate in Off mode (above).
  - `FindStruct` and `Find` on a struct value: the C++ `elements()` of a struct is its fields, so it searches them;
    here a value that is not an array finds nothing. This is not seen in compiled script.
  - Delegate function names compare case-insensitively (names are interned without case), where the C++ compares the
    two spellings byte for byte: the delegate comparisons (`delegate.rs`, `parts`).
  - A delegate bound to an object that the heap has since freed: calling it is a `HeapError` (a stale handle) here,
    where the C++ keeps the object alive through its reference and calls the function on it.
- **Verified (automated, local, pc-hummer).**
  - **Unit tests.** 25 new `ow-vm` tests on invented packages (24 in the three files, and one in `natives_core.rs`
    from review r1: `QueryInterface` and the InterfaceClass filter answer `NotYet` through a stand-in class).
    - Aggregates: `vm_test.py`'s `Arr` (2) and `Append` (2, one element per assignment). The C++ results of `Length`,
      `Add`, `Insert`, `InsertItem`, `Remove`, `RemoveItem`, `Find` and `FindStruct`. Heap arrays growing with typed
      zeros and refusing a float item. Static-array elements of a property, a local and a struct field, with the
      out-of-bounds log. Struct copy and comparison. An out argument aliasing an element. A removed element refused. A
      temporary's write dropped. Both nesting refusals. `DynArraySort`'s stub under both stub policies, with no call.
    - Iterators: `vm_test.py`'s engine-iterator block with its C++ results (2, 1, 5, 0, 0; 3, 3, 0; 1, 0; 3; the first
      crate; None; 26, 20, 0), the None-context census count, the unbound iterator's log, trace and refusal by key, a
      destroyed object skipped, the dynamic-array loop (sum, last index, a body that shrinks the array) and the loop
      bound.
    - Delegates: bound, unbound, missing function and the four comparisons; a local delegate in both modes.
    - Interfaces: `vm_test.py`'s six `QueryInterface` cases on class bodies with interface tables, both casts, the
      object-interface view and the iterators' InterfaceClass filter.
    - **`tests/ui_trace_returns_test.py`, re-expressed.** That file tests the in-game bl-sdk tracer callback
      (`tools/sdk_trace/openwillow_uitrace`), which no Rust code replaces. The three cases are re-expressed as the VM
      side of the scalar trace replay (`tools/replay_trace.py` drives `--run-batch`):
      - a native's completed out struct is what the caller and `CallResult::outs` see, with the receiver kept;
      - a plain return needs no out arguments;
      - a failed call leaves nothing on the frame stack, and the next call runs as on a fresh VM.
  - **`tests/runtime_test.py`** passes on the Rust binary (`--resolve`, `--scene-records`, `--payload`; no VM command).
  - **`tests/vm_test.py`** with the fixture fix, counted by assertion site with a local harness that runs every
    assertion separately (`local/p2/W4-INTERP2/soft_run.py`). The file has 103 `assert` statements; two are the
    fixture builder's own checks inside `make()`, which the harness does not count, so 101 are counted.
    - **C++** (`build/Release/ow-package.exe` of this branch, MSVC 14.50): `python tests/vm_test.py` passes; by site,
      101 of 101 pass.
    - **Rust, this branch (`main` without PR #316):** 16 pass, 76 fail and 9 are not reached. Every failure and every
      site not reached is another lane's:
      - `DefStore::class_default`, which every `--run` and batch case with a self class needs (W3-DEFS, PR #316);
      - `--native-census` (W4-VMTOOLS), `--inventory-move` (W5-EQUIP) and `--mover-probe` (W5-DIALOG), each `not
        ported` with exit code 3.
    - **Rust on a throwaway local merge of `main` + PR #316 (45e1cd44) + this branch:** 51 pass, 43 fail and 7 are not
      reached:
      - `MakePair`'s struct defaults, 1 (W3-DEFS's open item, decision 2026-11-15-14);
      - `--native-census`, 37 (the 5F attributes, the iterator block and the census; W4-VMTOOLS);
      - `--inventory-move`, 4, and `--mover-probe`, 1, plus the 7 not reached behind them.

      No site fails on this lane's operations. `Arr`, `Append` and the six interface asks pass; the iterator block's
      cases pass as unit tests with the same C++ results.
  - **Byte-for-byte against the C++** (`build/Release/ow-package.exe` of W3-INTERP's worktree; `interp.cpp` is unchanged
    since). This lane's `diff_vm.py` covers 74 `--run`, `--run-batch` and `--native` command lines on `vm_test.py`'s
    package and its interface variant, patched as the fixture fix does (the inner properties then sat at the package
    root), and gives 72/74 in Off and under Reference. The two differences are W3-INTERP's
    known ones (`MakePair`, and `Add2 --arg f:1.5`, W3-CORENAT's `BadArgument`). `Arr`, `Append` and the ten interface
    asks are identical, step counts included.
  - **The installed game, through `--run-batch`, on `main` + PR #316** (no stand-in): W3-INTERP's 2,747 cases (every
    function of `Core.Object` with no self class, and of `Engine.Actor`, `WillowGame.WillowPawn` and
    `WillowGame.WillowPlayerController` with the class as self), against W3-INTERP's recorded C++ outputs.
    - **Under Reference:** 326/326, 312/312, 513/513 and 1,595/1,596 identical (W3-INTERP's stand-in baseline was
      326, 309, 498 and 1,465).
    - **The one left**, `WillowPlayerController.InitInputSystem`, is not this lane's. The function creates its
      PlayerInput object from the class its `InputClass` default names. In the C++ that creation gives None; here it
      gives an object (W3-DEFS's defaults give a class), and that object's native `InitInputSystem` stub then runs.
      W3-INTERP's real-package run on `main` points at the C++ reference: the C++ seems to hold the inherited
      `InputClass` default as an object stand-in, and its `New` returns None for a value that is not a class
      (`src/interp.cpp:469`). UNVERIFIED: no tracked record holds that trace, and `reference(true)` in `src/vm.cpp`
      reads a declared `ClassProperty` as a class, so which C++ path gives the stand-in is not settled. The Rust result
      follows the declared `ClassProperty`. The item stays open (below).
    - **Off adds only the Off rows of decision 2026-11-15-14:** `GotoState` `NotYet` (W5-STATES), `<Class>_<n>` names
      and `FRand`. One more case meets a decision 2026-11-15-08 stand-in class `NotYet`.
  - Nothing was checked in the running game; there was no session in this lane.
- **Open items.**
  - Rerun `vm_test.py` on the Rust binary once W3-DEFS (class defaults, struct defaults), W4-VMTOOLS
    (`--native-census`), W5-EQUIP and W5-DIALOG land. The inventory fixture's `CachedObjects` array also names no inner
    property; it is W5-EQUIP's `--inventory-move` block and was not part of this grant.
  - The installed-game rerun on `main` now that PR #316 has landed (the numbers above are from a local merge with its
    45e1cd44 head).
  - `WillowPlayerController.InitInputSystem`: find and record which C++ path holds the inherited `InputClass` default
    as an object stand-in (above), or show that the Rust reading is the game's.
  - A public checked form of `Vm::implements` for callers outside `ow-vm` (the caller rule, above).
  - A static array used as a whole, if script is found that needs it.
  - `DynArraySort` and a delegate called through a local in Off mode: both wait for the game's rule to be read.
- **UNVERIFIED.** Every rule above is the C++ VM's reading, none checked in the running game:
  - the element growth and the zero values;
  - the bounds logging;
  - the live view of an array under `foreach`;
  - the skipping of destroyed objects;
  - the delegate call and comparison rules;
  - the interface table rule (NATIVE_CLASS_SERIAL_LAYOUT.md);
  - the object-interface casts.
