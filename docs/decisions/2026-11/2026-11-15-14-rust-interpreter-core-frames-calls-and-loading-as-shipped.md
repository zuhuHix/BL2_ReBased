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
    - Review r1 of PR #314 (granted): `Arg` carries a crate-private alias of the caller's storage, and
      `NativeCall::set_out` writes through it at once; `run_bytecode` lets the interpreter decode before the depth
      check; `Vm::call` documents the stack a caller needs.
    - Review r2 (granted): `NativeCall::arg` reads an aliased out argument from the caller's storage;
      `NativeCall::args` is crate-private, so `set_out` is the only way to write an out value (`arg_count()` added);
      the outs of a native call are read the same way; the docs of `Arg::value`, `Arg::out` and `set_out` say so.
  - `Vm::find_method(&mut self, class, name, state)` and `Vm::class_declares_state(&mut self, class, state)` (C++
    `findMethod`, `Class::states`) load the class's code first if it is not loaded, so `GotoState` called from the host
    on a class nobody loaded still finds its states. A class defined by hand (no package behind it) declares nothing.
    `natives_core::register` wires both in place of `ContractStates`, which is removed; `StateLookup` takes `&mut Vm`.
    `Object.IsA` and `Object.ClassIsChildOf` (granted in review r1) answer through the interpreter's class checks.
  - `ow_object::reflect` (additive): `CodeIndex::child_declarations(store, names, classes, package, outer)` types any
    export's child properties exactly as `define_layout` types a class's own (no attribute companions), and
    `ClassInfo::states()` / `StateInfo` list a class's states and their functions as the C++ `loadClass` collects them.
  - `ow_vm::interp` (public): `describe` and `describe_for_string` (the C++ `Value::describe` and `describeForString`),
    `instantiate_from_defaults` (a new object that starts from the class default object, as the C++ `instantiate`
    copies `defaults->props`; `Vm::instantiate` starts from the layout's defaults only), `STACK_SIZE`, `MAX_NESTING`,
    `MAX_FRAME_SLOTS`, `MAX_LIVE_SLOTS`, `on_vm_stack` (runs a closure on a `STACK_SIZE` thread) and
    `VmThreadError` (a panic on that thread is handed back as `Panicked`, not re-raised).
  - `ow_core::num::strtod_prefix` and `StrtodPrefix` (additive, review r2): the C `strtod` prefix scanner, shared by
    the VM's string-to-float cast and the CLI's `std::stod`.
  - `ow_object::heap` (review r2, granted): `check_value` accepts a class value for an object property declared as a
    class's ancestor (`CLASS_ANCESTORS`).
  - The W4-INTERP2 files `interp/aggregate.rs`, `iter.rs` and `delegate.rs` are `NotYet { NotPorted }` stubs.
- **Rules ported.**
  - **Steps.** One per statement, per expression, per place taken (`lval`) and per call, where the C++ ticks: the
    `--run` step counts equal the C++ ones.
  - **Places.** Where the C++ holds a `Value*`, the interpreter keeps a place (a frame slot, a typed temporary, an
    object's property element, or a temporary).
  - **Out arguments alias the caller's storage live**, as the C++ `Slot::alias`. A frame's storage lives on the VM's
    frame stack, so a place can name any active frame's slot. An out argument's place travels with the argument: a
    script callee reads and writes it, and a native reads it (`NativeCall::arg`) and writes it (`set_out`, the only way)
    at once. With `x = 0`, `F(out A, out B) { A = 1; return B; }` called as `F(x, x)` returns 1 and leaves `x` 1, as the
    C++; a native's out write is seen by script it calls back into; with `x = 1`, `x += ++x` gives 4, as the C++; a
    native given one variable for two out parameters sees a write through the other. An out argument that is not a
    storage location (the C++ `scratch`) keeps a copy in the callee.
  - **Stale frames** (Rust-only). A frame slot is named by its index and the serial number the VM gave the frame. An
    `Arg` copy kept past its call still names the caller's frame; once that frame has returned, a read or write through
    it is a typed stale-frame `VmError::Script`, never a panic and never a read of or a write into the frame that took
    the index since (the C++ alias would dangle).
  - **Error order** as the C++ `invoke`: a function that cannot run, or does not decode, says so before the depth
    check. The 0x5F let finds the companions (from the property's declaring class) before it evaluates the right side,
    then finds the value, its base and its stack on the container by name, as `interp.cpp:588-592` does.
  - **`GotoLabel` and state variables** as the C++: a `GotoLabel` is stepped over (latent state code is not run), and
    a state variable reads as an instance variable. Both UNVERIFIED in the game; state code is W5-STATES'.
  - **Loading.** Loading a class declares every function it and its states declare, with typed parameters and the
    native key built as `vm.cpp:600-606` builds it.
  - **Decoding** happens on a function's first call (as the C++), and a failure is kept and reported on every call
    as `cannot run <path>: <why>`.
  - **Native index.** Built on first use over the nine code packages in the C++ order, first index wins.
  - **Class default objects.** Used for a call with no object, `Class.static`, `default.` reads and `New`; they come
    from `DefStore::class_default`.
  - **IsA through stand-ins.** A cast, `IsA` or `ClassIsChildOf` through an engine-native stand-in class (decision
    2026-11-15-08) that is not already a yes is `NotYet`, not `false`.
- **Representation rules** (the typed slots of decision 2026-11-15-02). A heap write gets four rules after the C++
  assignment rule (`interp.cpp:511-522`):
  1. `None`, or the `None` literal, stored into a class, interface or delegate property becomes that kind's null.
  2. An object reference stored into an interface property becomes an interface reference, and the reverse.
  3. Any other `None` stored into a property becomes the property's zero. The C++ stores the `None` kind there,
     which reads as 0 wherever a number is read.
  4. A class stored into an object property declared as a class's ancestor (`Core.Object`, `Field`, `Struct`, `State`,
     `Class`, the public UE3 chain, UNVERIFIED for this executable) is kept: a class is an object in UnrealScript. This
     rule is the heap's own (`ow_object::heap::check_value`, review r2).
  5. An object of an engine-native stand-in class that the heap refuses for a typed object property is `NotYet`, not
     the heap's `WrongClass`: its real chain is unknown.
  6. Locals stay untyped, as in the C++.

  String constants:
  - An ANSI constant is one character per byte (Latin-1, the UE3 widening of an ANSI character).
  - A Unicode constant is decoded from its UTF-16 units: a surrogate pair is its character, a lone surrogate U+FFFD
    (decision 2026-11-15-07).
  - For ASCII both equal the C++ text.
- **Stack depth (the brief's question; review r1).** The VM runs on a thread with a larger stack. Expressions nest at
  most `MAX_NESTING` (51,400 = 200 x 257) levels across all active frames; past it a call is a typed `VmError::Script`
  (a Rust-only bound), never a stack overflow.
  - Measured locally on one shape: 200 frames, each a statement nesting 252 levels of eval -> native -> call (the unit
    test `the_worst_case_nesting_fits_the_vm_stack`): 278 MB against 512 MiB in a debug build (about 5.5 KB a level),
    65 MB in a release build (1.3 KB a level). A second shape adds a function decoded first at the 200th frame and
    nesting 254 levels (`a_decode_at_depth_fits_the_vm_stack`). UNVERIFIED as a bound for every dispatch path: another
    path through the interpreter may cost more stack a level. The interpreter's dispatch functions are split so their
    frames stay small unoptimised (21 KB a level before).
  - A caller needs `interp::STACK_SIZE` (512 MiB, reserved address space, committed as it is used): `ow-cli` runs
    every VM command through `interp::on_vm_stack`, and `Vm::call` says so; a panic on that thread is reported ("the VM
    thread panicked"), not re-raised. Decoding stays lazy inside a call, as the C++.
- **Frame bounds (Rust-only refusals, review r1).** Before a frame is allocated, its storage is checked: at most
  `MAX_FRAME_SLOTS` (4,096) value slots a frame and `MAX_LIVE_SLOTS` (1,048,576) for all active frames together. The
  storage is then reserved fallibly (`try_reserve_exact`); a refusal or a failed reservation is a typed
  `VmError::Script` naming the function and the sizes. No dimension is clamped. The installed code packages need at
  most 257 slots a frame (16,323 functions with children, measured locally), so no installed function meets it.
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
  - **Unit tests.** 38 interpreter tests on an invented package, re-expressing the `vm_test.py` cases that use only
    this lane's features (constants, natives and the 32-bit wrap, `SumTo`, `Pick`, virtual dispatch, instance
    variables, optional and out parameters, unbound natives and native indices, the typed temporaries and their errors,
    `NoneCtx` and the `SumTo` census counters, casts, steps, the step and depth limits, the `GoA` state events under
    Reference, find and error texts). The 0x5F let runs `vm_test.py`'s attribute block with its expected values: base
    and value both set (10, 10), adding modifiers recomputes nothing, then 40.5 for base 20, 8.25 for base 2 through
    `Context(Self)`, the int attribute truncated toward zero (13 for 10, -11 for -10), an int right side converted to
    the float base (7, giving 12), no change notification, and the None context logged. A string right side, which the
    C++ stores silently, is refused by `let_base` with nothing written. Review r1 added: `F(x, x)` with two out
    parameters (1 and `x` = 1, the C++ values) through a local and through an instance variable, a native's out write
    seen by the script it calls back into (5), a 65,536-slot frame and a live-slot overflow refused, the worst-case
    nesting on a `STACK_SIZE` thread (the depth limit first, the nesting bound with the depth raised), `GotoLabel` and
    a state variable, a class kept in an object property and a stand-in object refused with `NotYet`, `find_method` and
    `GotoState` on a class nobody loaded, the decode error before the depth error, and 0x5F on an object without the
    property. Review r2 added: `x += ++x` (4, the C++ value), a native `F(x, x)` seeing a write through its other
    argument (7), a kept `Arg` refused as a stale frame after its frame returned and when a new frame holds its index
    (the new frame's variable not written), a decode at depth on a `STACK_SIZE` thread, and a VM-thread panic handed
    back. A heap test of the class-ancestor rule and an `ow_core::num` test of `strtod_prefix`. Also 3 CLI tests, 3 reflection tests, and a Core natives test of `IsA` / `ClassIsChildOf` through a
    stand-in class.
    `tests/runtime_test.py` runs no VM command, so none of its cases is this lane's.
  - **`--native`.** `tests/rust_parity.d/vm.txt`: 8 diff lines (the `natives_test.py` command lines among them), 8 pass.
  - **`vm_test.py`'s package, with the two fixture patches above.** 64 `--run`, `--run-batch` and `--native` command
    lines (the batch has 34 cases, every error path of the C++ among them), compared byte for byte with the C++.
    - As committed, 27 of 64 are identical. The other 37 need a class default object and print
      `DefStore::class_default`'s `NotYet`.
    - With a local `class_default` stand-in (`ow_object::cdo::class_default`, as `--object-dump` uses it; in the
      working tree only, never committed), 60 of 64 are identical in Off and 60 of 64 under Reference (with
      W3-ATTREFFECTS' `let_base`; before it landed, `SetSpeed` was a fifth difference).
    - The 4 left: `Arr` and `Append` (W4-INTERP2 arrays), `MakePair` (struct defaults) and `Add2 --arg f:1.5` (above).
      Step counts equal.
  - **The installed game, the same stand-in, through `--run-batch`.** Every function of `Core.Object` with no self
    class, and every function of `Engine.Actor`, `WillowGame.WillowPawn` and `WillowGame.WillowPlayerController` with
    the class as self: 2,747 cases per mode.
    - Under Reference: 326/326, 309/312, 498/513 and 1,465/1,596 identical.
    - Every difference is a case the C++ ran and the Rust port stops with `NotYet` for another lane's feature:
      `foreach`, aggregates, delegates, interfaces and `QueryInterface` (W4-INTERP2). With W3-ATTREFFECTS'
      `let_base` no 0x5F case differs any more.
    - Off adds only the Off rows above.
  - **Reflection after the addition.** `tools/verify_packages.py` on both readers: 9 packages, 3,339/3,339 class bodies
    exact. The Rust `--interface-compare --layouts`: 3,339 classes defined, 0 failed, 257 linked attributes.
- **Open items.**
  - The W3-DEFS body: `class_default` (and `resolve_ref` / `load` for object constants and unloaded handles), then
    rerun the comparisons above without the stand-in.
  - A definition-store call that loads a struct's defaults, for frame zero values (W3-DEFS).
  - The `vm_test.py` fixture (hub).
  - Typed error sources on `VmError` for reflection and bytecode decoding, in place of the C++-text `Script`
    strings this lane uses for them (W2-VMAPI's contract). Definition store errors already travel as W3-DEFS's
    `VmError::Def`.
  - Architecture section 3.8 text for the seams above.
- **UNVERIFIED.** Every rule here is the C++ VM's reading, none checked in the running game: the evaluation order of
  arguments in the caller's context, the live out-argument aliasing, `GotoLabel` stepped over and state variables read
  as instance variables, the class-into-object-property rule, the typed temporaries 0x4C-0x50, the 0x5E/0x5F
  attribute opcodes, the primitive casts and their string forms, the rotator-vector conversions, and the Latin-1
  reading of ANSI constants.
