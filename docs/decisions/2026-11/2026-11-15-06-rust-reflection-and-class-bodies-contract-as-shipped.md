## 2026-11-15: Rust reflection and class bodies contract as shipped

AI-assisted (Claude Code). Lane W2-REFLECT of the Rust port fills `crates/ow-object/src/{reflect,class_body}.rs`
(with the tests in `reflect/tests.rs`), `crates/ow-package/src/testkit/reflect.rs` and
`crates/ow-cli/src/cmd/classes.rs`. It ports `decodeClassBody` and the class loader of `src/vm.cpp` (`classAt`,
`loadClass`, `resolveRef`, `indexOf`, `findExport`, `implements`, `implementsStructurally`), `checkClassBodies` and
`compareInterfaces` (`src/vm_sweep.cpp`), and the `--class-check` / `--interface-compare` branch of `src/cli.cpp`. The
class body layout, its checks and their texts are the C++ ones, in the C++ order. It also adds what the C++ VM never
had: Class, ScriptStruct, Enum and property exports loaded into `ClassTable` layouts (decision D14). This file records
where the shipped interface differs from revision 3 of `docs/rust-port/ARCHITECTURE.md` section 3.3, and the reading
rules the layouts use.

- **`decode_class_body` returns `Result<ClassBody, ClassBodyError>`, not `ReflectError`.** `ClassBodyError` has one
  variant per C++ text and a transparent `Read(ReadError)`, so `--class-check` prints the C++ failure lines.
  `ClassBody` keeps the C++ fields (flags, within, default object, config name, interface entries).
- **`CodeIndex` is the C++ loader as a value.** It loads classes lazily by package-relative reference
  (`class_at`, `find_class`), caches reference answers (`resolve_ref`: an all-import chain is looked up in its root's
  package with no global fallback; one nested in a local export goes through `PackageStore::resolve`) and keeps per
  class what the interface rules need: super class, body or its error, declared function names. `ClassKey` names a
  loaded class. `implements` resolves interface entries lazily, one by one, as the C++ does.
- **`load_code_packages(store, names, classes) -> Result<CodeIndex, ReflectError>`** fails only when a code package is
  missing or a super chain holds a non-class (the C++ throws there). A class whose layout cannot be defined stays
  declared; `CodeIndex::layout_of` and `layout_failures` report it with its cause chain.
- **The structural interface rule is C++ behaviour, not a project stand-in.** `implements` falls back to
  `implements_structurally` (an `I<Upper>` name, at least one function, every one defined along the class chain)
  whenever a class chain's bodies did not decode, in both parity modes. That is what the C++ `Runtime::implements`
  does, and it is ported as such (hub decision for this lane). It is not behind `ParityMode::Reference` and not in
  architecture section 6.1.
- **Layout reading rules** (the C++ `readProperty` / `childProperties` / `structAt` reading, now typed):
  - own declarations are the child exports whose class name ends in `Property`, in descending export order. A child
    under the 40-byte header is skipped, as the C++ skips it. A read past the package data is an error here, where the
    C++ skips that child too;
  - the references from byte 40 (42 with flag `0x20`): object, component and interface properties name their class
    first; a class property its metaclass second; struct and array properties their struct and inner declaration; a
    byte property its enum or 0;
  - an attribute value property (flag bit 63) names its stack, then its base. A byte attribute names its enum first,
    so the stack is its second reference and the base its third. They are linked with `ClassTable::link_attribute`;
  - a referenced class is only declared: `<stem>.<name>` when it resolves, its import path when no installed package
    holds it (`Core.Class` and the other engine-native classes). A struct is defined on first use, its super struct and
    by-value fields first. A struct inside an array that is still being defined (an array of itself) is only declared;
    a by-value cycle is refused;
  - an Enum export is `[net index][None][next][count][names]` with a count of at most 1024 (`enumNames`). It must also
    end exactly at its serial size. The C++ does not check that; it holds for all 792 Enum exports of the nine code
    packages and is the structural check `PHASELOCK_STOCK_DATA.md` records;
  - a super class reference that resolves to nothing is refused. The C++ loader leaves that class with no super.
    A class whose super chain held a non-class keeps that error for every later call. The C++ keeps such a class
    registered without its super.
- **`PropKind::Map { key, value }` is added to `crates/ow-object/src/layout.rs`** (W1-OBJ's file, hub grant for this
  lane; additive on the `#[non_exhaustive]` enum, with its arms in `kind_name`, `check_kind`, `zero_value` and
  `fits`). `key` and `value` are `Option<Box<PropKind>>`, not boxed kinds: all 50 map properties of the nine code
  packages have both type references at 0, so there is no key or value kind to read, and none is invented. A map's
  value is `Value::None`, as the C++ VM gives a map property `Value()`. Where a package does name them, the first
  reference is read as the key and the second as the value.
- **Rust-only `--layouts` modifier** on `--interface-compare` prints the layout summary of the nine code packages.
  `args.rs` lists it in that command's options (hub grant); the C++ usage text, which `args.rs` keeps byte for byte,
  does not.
- **Output line ends are `\n`.** The C++ binary writes `\r\n` on Windows; the parity harness is to normalise them
  (hub decision, change pending).

- **Verified (automated, local).**
  - `python tools/verify_packages.py --reader target/release/ow-package.exe` passes: 9 packages, 3,339/3,339 class
    bodies exact.
  - `--class-check --failures` is byte-identical to C++ (line ends normalised) on each of the nine packages.
    `--interface-compare` is byte-identical too: 3,189 classes, 150 interfaces.
  - Rust and C++ give the same stdout, stderr and exit code on 20 synthetic command lines. The packages are
    `tests/vm_test.py`'s interface fixture, its four `mutate` variants and its plain package, generated under ignored
    `local/`. The command lines include the option and file errors.
  - Layouts: 3,339 of 3,339 classes of the nine code packages get one, `Engine.WorldInfo` and
    `WillowGame.WillowPlayerController` among them; none fails. 257 attribute value properties are linked to a base
    and a stack, the record's 257 (`src/interp.cpp`, NATIVE_ATTRIBUTES.md).
  - Rust unit tests re-express `vm_test.py`'s class-check, interface and `mutate` cases and cover each layout rule.
    The `testkit/reflect.rs` builders are pinned byte for byte against the Python helpers.
- **Open, outside this lane's files.** `Heap::check_value` (`crates/ow-object/src/heap.rs`) has no arm for a map, so
  `Heap::alloc` of a class with a map property is refused: a local probe gave
  `WrongKind { expected: "Map", found: "None" }`. Allocating `WorldInfo` or `WillowPlayerController` needs a
  `(PropKind::Map { .. }, Value::None) => Ok(())` arm there (W1-OBJ's file).
- **UNVERIFIED.**
  - In the running game: the meaning of the class body's fields and the interface-table rule
    (NATIVE_CLASS_SERIAL_LAYOUT.md).
  - The declaration order rule. The C++ checked it on 5,980 of 5,984 Core and Engine functions, not on classes or
    structs.
  - The enum end check, which is fitted on the packages.
  - For W3-DECODE: struct field order. The ClassTable has the declared order (a super struct's fields first, then
    the struct's own in descending export order), while the C++ VM's zero values of eight native structs use its own
    field order (`nativeStruct`: `Plane` as W, X, Y, Z). The tagless struct values W3-DECODE reads must follow the C++
    order, or say why not. The declared order of those eight structs in the installed Core was not looked at here.
  - For W3-DECODE: the descending-export declaration order is unchecked for classes and structs (the declaration
    order item above); the slot order of every layout rests on it.
  - The key-then-value reading of a map's two references. No C++ code reads them, and no installed code package has
    a non-zero one.
