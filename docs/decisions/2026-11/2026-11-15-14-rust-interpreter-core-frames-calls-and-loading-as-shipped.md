## 2026-11-15: Rust interpreter core: frames, calls and loading as shipped

AI-assisted (Claude Code). Lane W3-INTERP of the Rust port fills `crates/ow-vm/src/interp/` (a port of `src/interp.cpp` and
of the run half of `src/vm.cpp`), `crates/ow-cli/src/cmd/vm.rs` (`--run`, `--run-batch`, `--native`) and
`tests/rust_parity.d/vm.txt`. It uses the grants of its lane entry and the hub's two additions (W3-DEFS's
`DefStore::code_mut`, and `CodeIndex::child_declarations` in W2-REFLECT's `reflect.rs`). This file records the shipped
interface, the rules ported, where the Rust output differs from the C++ and why, and what waits for other lanes.

- **Interface.**
  - `ow_vm::api` (W2-VMAPI's file, within the grant):
    - the body of `Vm::run_frame` (`interp::run_frame`);
    - `Vm::find_function` and `Vm::find_class` load from the code packages (C++ `findFunction`, `findClass`, with
      their error texts);
    - the `<- caller` trace of the depth-limit message (the four innermost active script functions, `interp.cpp:711-713`);
    - one private field `code: interp::CodeState`.
    - Two small seams, flagged for the hub: `pub(crate) Vm::call_bound` (a call with arguments the interpreter bound
      itself, since an omitted argument may sit before a supplied one) and `pub fn Vm::steps()` (the C++ `Runtime::steps`
      that `--run` prints).
  - `Vm::find_method(class, name, state)` and `Vm::class_declares_state(class, state)` (C++ `findMethod`, `Class::states`)
    read the classes whose code the VM loaded. A class defined by hand (no package behind it) declares nothing. A
    package class whose code is not loaded is an error, not a guess. `natives_core::register` wires both in place of
    `ContractStates`, which is removed; the one test that pinned the missing lookup now checks the wired one.
  - `ow_object::reflect` (additive): `CodeIndex::child_declarations(store, names, classes, package, outer)` types any
    export's child properties exactly as `define_layout` types a class's own (no attribute companions), and
    `ClassInfo::states()` / `StateInfo` list a class's states and their functions as the C++ `loadClass` collects them.
  - `ow_vm::interp` (public): `describe` and `describe_for_string` (the C++ `Value::describe` and `describeForString`),
    `instantiate_from_defaults` (a new object that starts from the class default object, as the C++ `instantiate`
    copies `defaults->props`; `Vm::instantiate` starts from the layout's defaults only), `strtod_prefix` and
    `STACK_SIZE`.
  - The W4-INTERP2 files `interp/aggregate.rs`, `iter.rs` and `delegate.rs` are `NotYet { NotPorted }` stubs.
- **Rules ported.**
  - **Steps.** One per statement, per expression, per place taken (`lval`) and per call, where the C++ ticks: the
    `--run` step counts equal the C++ ones.
  - **Places.** Where the C++ holds a `Value*`, the interpreter keeps a place (a frame slot, a typed temporary, an
    object's property element, or a temporary).
  - **Out arguments.** An out argument's place is taken before the call and written back after it. The C++ writes
    through an alias during the call; the two differ only if the callee also reaches the caller's variable another way.
  - **Loading.** Loading a class declares every function it and its states declare, with typed parameters and the
    native key built as `vm.cpp:600-606` builds it.
  - **Decoding** happens on a function's first call (as the C++), and a failure is kept and reported on every call
    as `cannot run <path>: <why>`.
  - **Native index.** Built on first use over the nine code packages in the C++ order, first index wins.
  - **Class default objects.** Used for a call with no object, `Class.static`, `default.` reads and `New`; they come
    from `DefStore::class_default`.
  - **IsA through stand-ins.** A cast through an engine-native stand-in class (decision 2026-11-15-08) that is not
    already a yes is `NotYet`, not `false`.
- **Representation rules** (the typed slots of decision 2026-11-15-02). A heap write gets four rules after the C++
  assignment rule (`interp.cpp:511-522`):
  1. `None`, or the `None` literal, stored into a class, interface or delegate property becomes that kind's null.
  2. An object reference stored into an interface property becomes an interface reference, and the reverse.
  3. Any other `None` stored into a property becomes the property's zero. The C++ stores the `None` kind there,
     which reads as 0 wherever a number is read.
  4. Locals stay untyped, as in the C++.

  String constants:
  - An ANSI constant is one character per byte (Latin-1, the UE3 widening of an ANSI character).
  - A Unicode constant is decoded from its UTF-16 units: a surrogate pair is its character, a lone surrogate U+FFFD
    (decision 2026-11-15-07).
  - For ASCII both equal the C++ text.
- **Stack depth (the brief's question).** The VM runs on a thread with a larger stack. `ow-cli` runs every VM command
  on a 256 MiB thread (`interp::STACK_SIZE`), and the deep unit test (200 nested frames, the depth limit) passes on a
  16 MiB thread in a debug build. Decoding stays lazy inside a call, as the C++ does.
- **Where the output differs from the C++, and why.**
  - **`vm_test.py`'s invented package declares reference properties with no class.**
    - The places: the `<Name>ModifierStack_Element` object properties; `make_function`'s object and class
      parameters, locals and return values; and the `Items` arrays with no inner property.
    - Every property of the installed packages carries those references. The Rust reflection refuses the layout
      (`names no class`), where the untyped C++ bag never reads them. No bound is loosened.
    - The effect: with the fixture as tracked, no `Core.Foo` function runs on the Rust binary.
    - The local comparison below patched two of those spots in its own copy. **`tests/vm_test.py` needs the same fix
      before W4-INTERP2's whole-file gate can pass on the Rust binary; that file is not this lane's.**
  - **A float given for an int parameter of a Core native is refused** (W3-CORENAT's `BadArgument`). The C++ reads
    the zero integer field. Only a host call can do this (`--run --arg f:1.5` for an int); compiled script casts first.
  - **Off mode only** (architecture 6.1):
    - an unnamed object is `<Class>_<n>` (W2-VMAPI);
    - `FRand` draws from the CrtRand stream;
    - `GotoState` is `NotYet` until W5-STATES.

    Under `OW_PARITY=reference` these three equal the C++.
  - **A struct local or return value starts from its layout's zero and the struct defaults the class table holds.**
    The C++ `zeroOfDef` reads the struct's default stream every time. A struct that no class default object reached
    (`MakePair`'s `Pair` in `vm_test.py`) has no defaults loaded, so it prints `Slots=[0,0]` where the C++ prints
    `[5,9]`. This needs a definition-store call that loads a struct's defaults (open item).
  - **Hexadecimal float text** is not parsed: the `0x4C` cast is `NotYet { NotRead }`, and the batch's `stod` refuses
    it. A batch `s:`/`t:` value that is not UTF-8 becomes U+FFFD where the C++ keeps the raw bytes. A
    `ComponentProperty` parameter is reported as `ObjectProperty` in the batch's type checks.
- **Verified (automated, local, pc-hummer; C++ `build/Release/ow-package.exe` of this branch, MSVC 14.50).**
  - **Unit tests.** 27 interpreter tests on an invented package, re-expressing the `vm_test.py` cases that use only
    this lane's features (constants, natives and the 32-bit wrap, `SumTo`, `Pick`, virtual dispatch, instance
    variables, optional and out parameters, unbound natives and native indices, the typed temporaries and their errors,
    `NoneCtx` and the `SumTo` census counters, the 0x5F None context, casts, steps, the step and depth limits, the
    `GoA` state events under Reference, find and error texts). Also 3 CLI tests and 3 reflection tests.
    `tests/runtime_test.py` runs no VM command, so none of its cases is this lane's.
  - **`--native`.** `tests/rust_parity.d/vm.txt`: 8 diff lines (the `natives_test.py` command lines among them), 8 pass.
  - **`vm_test.py`'s package, with the two fixture patches above.** 64 `--run`, `--run-batch` and `--native` command
    lines (the batch has 34 cases, every error path of the C++ among them), compared byte for byte with the C++.
    - As committed, 27 of 64 are identical. The other 37 need a class default object and print
      `DefStore::class_default`'s `NotYet`.
    - With a local `class_default` stand-in (`ow_object::cdo::class_default`, as `--object-dump` uses it; in the
      working tree only, never committed), 59 of 64 are identical in Off and 59 of 64 under Reference.
    - The 5 left: `Arr` and `Append` (W4-INTERP2 arrays), `MakePair` (struct defaults), `SetSpeed`
      (`ow_attr::registry::let_base` is W3-ATTREFFECTS's stub) and `Add2 --arg f:1.5` (above). Step counts equal.
  - **The installed game, the same stand-in, through `--run-batch`.** Every function of `Core.Object` with no self
    class, and every function of `Engine.Actor`, `WillowGame.WillowPawn` and `WillowGame.WillowPlayerController` with
    the class as self: 2,747 cases per mode.
    - Under Reference: 326/326, 309/312, 494/513 and 1,465/1,596 identical.
    - Every difference is a case the C++ ran and the Rust port stops with `NotYet` for another lane's feature:
      `foreach`, aggregates, delegates and interfaces (W4-INTERP2), `let_base` (W3-ATTREFFECTS), `QueryInterface`
      (W4-INTERP2).
    - Off adds only the Off rows above.
  - **Reflection after the addition.** `tools/verify_packages.py` on both readers: 9 packages, 3,339/3,339 class bodies
    exact. The Rust `--interface-compare --layouts`: 3,339 classes defined, 0 failed, 257 linked attributes.
- **Open items.**
  - The W3-DEFS body: `class_default` (and `resolve_ref` / `load` for object constants and unloaded handles), then
    rerun the comparisons above without the stand-in.
  - A definition-store call that loads a struct's defaults, for frame zero values (W3-DEFS).
  - `let_base` (W3-ATTREFFECTS).
  - The `vm_test.py` fixture (hub).
  - Typed error sources on `VmError` for reflection and bytecode decoding, in place of the C++-text `Script`
    strings this lane uses for them (W2-VMAPI's contract). Definition store errors already travel as W3-DEFS's
    `VmError::Def`.
  - `IsA` / `ClassIsChildOf` in `natives_core.rs` still answer `false` through an engine-native stand-in class
    (W3-CORENAT).
  - Architecture section 3.8 text for the seams above.
- **UNVERIFIED.** Every rule here is the C++ VM's reading, none checked in the running game: the evaluation order of
  arguments in the caller's context, the out-argument write-back, the typed temporaries 0x4C-0x50, the 0x5E/0x5F
  attribute opcodes, the primitive casts and their string forms, the rotator-vector conversions, and the Latin-1
  reading of ANSI constants.
