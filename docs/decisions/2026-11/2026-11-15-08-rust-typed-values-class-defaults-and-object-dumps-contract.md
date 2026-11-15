## 2026-11-15: Rust typed values, class defaults and object dumps contract as shipped

AI-assisted (Claude Code). Lane W3-DECODE of the Rust port fills `crates/ow-object/src/{decode,cdo,prefix}.rs` (tests
in `decode/tests.rs` and `cdo/tests.rs`) and `crates/ow-cli/src/cmd/objects.rs`. It ports the loading half of the C++
VM that the reflection lane left: the tag reader (`TagReader`, `applyTaggedDefaults`, `applyStructDefaults`,
`Runtime::resource`), class default objects (`defaultsOf`, `buildDefaults`), `instantiateExport` and the
`--object-dump` branch of `src/cli.cpp` (`valueJson`). Values are typed through the reflection layouts (decision
2026-11-15-06) and stored in heap slots (decision 2026-11-15-02), not in the C++ name-keyed bag. This file records the
shipped interface, where it differs from revision 3 of `docs/rust-port/ARCHITECTURE.md` section 3.3, and where the
output differs from the C++.

- **Interface.** `decode::ValueCx` borrows the store, the `CodeIndex`, the `ObjectSpace` and a `DecodeState` (the
  C++ `Runtime`'s caches: one stand-in per referenced object, the structs whose defaults are read, the class default
  objects, an export-name index, the log) with the parity mode. `ValueCx::apply_tags` is `applyTaggedDefaults`;
  `cdo::class_default` and `cdo::instantiate_export(cx, package, index, PropertyPrefix)` are the C++ functions of the
  same meaning. `PropertyPrefix` is the contract's enum (4, 8, 26). `to_value` takes the walker tree's package as an
  argument (`to_value(tree, decl, package, cx)`), since its references are package-relative. `ReflectionTyper::new`
  answers from the cache only, so `decode::prepare_typer` loads the classes of a package's exports first.
- **Reading rules (the C++ ones).** A tag's header is checked outside the per-value guard, so a bad header ends the
  stream (and is logged as `defaults: Default__X: <error>` for a class default); a value read inside its tag's size that
  fails or does not end there is skipped and logged as `defaults: skipped <Class>.<Property> (<Type>)`. Attribute
  tags are read only at the top level; delegate, map and byte-attribute tags are skipped. The eight native structs
  are read untagged in the C++ field order and placed by name into the declared layout; Box, Matrix and other
  fixed-layout structs the C++ VM reads as tagged streams fail and are skipped, as in the C++. Arrays are typed by the
  declaration, with the C++ element widths. A struct starts from its defaults; the default stream (52-byte header,
  word at +44 zero, FITTED) applies only when consumed exactly. Static arrays take one tag per element and ignore an
  index past them.
- **Differences kept only under `ParityMode::Reference`** (three C++ behaviours, so that `--object-dump` can be
  compared byte for byte; the hub may list them in architecture section 6.1):
  - arrays with elements inside a struct's default stream stay empty (the C++ reads those streams with no array
    typing). Off types and decodes them;
  - an enum value name the declared enum lacks gives 0. Off refuses it and skips the property, so no value is guessed;
  - a numbered enum value (stored as a base name and a number) is compared to the enum's entries without their
    numbers, as `enumPosition` does, so it matches nothing and gives 0. Off gives its position.
- **Differences in both modes** (the typed slots hold what the bag did not; R7 exceptions for the hub to accept or
  change):
  - **class properties**: these packages tag a class property `ObjectProperty`, its reference naming the class. The
    C++ VM decodes by the tag and keeps an object stand-in in the class property, printed as the class's object path;
    here the value is the class, printed `class:<Package>.<Class>`. A class property explicitly tagged with reference 0
    prints `null` there and `class:None` here;
  - **undeclared properties**: a tag naming a property the cooked class does not declare (editor-only properties
    stripped from the class, still tagged) has no slot. The C++ bag prints it; here it is dropped (recorded in
    `DecodeState::dropped`, and logged only where the C++ decode would also have failed).
- **Engine-native stand-in classes.** 38 classes are referenced by the nine code packages and exported by no
  installed package (Core's reflection classes, `Engine.StaticMesh`, `Engine.Level`, `Engine.Model`, the net classes).
  A heap object needs a defined class, so a stand-in of such an object gets its class defined at its import path with
  no properties of its own under `Core.Object` (once, if nothing defined it before; listed in
  `DecodeState::native_classes`). No native property is invented; the classes between it and `Core.Object`, if any,
  are UNVERIFIED.
- **A Rust-only bound.** Struct defaults that wait on other structs' defaults (through arrays of structs in a default
  stream, which the C++ never reads) nest at most 64 deep, and a struct needing its own defaults while they are read is
  a typed error that skips that one array; the C++ has neither case.
- **Not deleted: the schemas and `SchemaTyper`.** The architecture's gate deletes `tools/*-arrays.schema` and
  `SchemaTyper` once the typers agree. They do not agree (below), and the schemas are still used by 21 tools, the
  ctest entries `properties-synthetic` (`tests/properties_test.py`) and `tool-schema-lines`
  (`tests/schema_lines_test.py`), the C++ CLI (`--array-schema`, until W7) and the Rust CLI (`cmd/properties.rs`,
  `cmd/assets.rs`) with `ow-assets`. The deletion is left to the hub.

- **Verified (automated, local, installed game, C++ `build/Release/ow-package.exe` of this branch).**
  - `--object-dump <index> 4` with and without `--all` on all 3,339 class default objects of the nine code packages
    (6,678 runs), both binaries exit 0 everywhere. Under `OW_PARITY=reference`: without `--all`, 3,339 of 3,339
    byte-identical (line ends normalised); with `--all`, 2,268 identical and 1,071 differing, every difference in the two
    both-modes classes: 1,674 undeclared keys printed by the C++ (39 distinct names, in 914 objects) and 945 class
    values (934 object paths and 11 explicit nulls, in 500 objects); the logs are equal in every run. In Off mode, 823
    runs differ from the reference mode: 822 by log lines only (four enum properties whose tag value is the name
    `None`; the inherited value is 0 in each, so the values are the same) and one property (a numbered enum value, 2
    where the C++ gives 0). The struct-default array case does not occur in these objects: of the 1,330 ScriptStruct
    default streams of the nine packages the package walker reads (1,338 exports of at least 52 bytes), one holds an
    array with elements, and no class default reaches it by value.
  - The same comparison on the 619 attribute definitions of `Startup.upk` (1,238 runs): no difference in either mode
    besides 120 class values in 40 runs. `ContextResolverChain` decodes in all 619, equal to the C++ VM's: 612
    non-empty chains, 503 of one resolver and 109 of two, in their stored order (report F3 Open, below).
  - 20 command lines with bad options, numbers, prefixes, indices and paths: stdout, stderr and exit code equal.
  - The reflection typer against the five schemas over every export of the 914 packages under `CookedPCConsole`
    (2,058,664 exports, 2,026,612 tag streams walked at prefix 4, 8 or 26): of 36 schema entries, 18 agree wherever the
    walk meets them and 18 differ (301,124 array tags agree, 109,669 differ). The differences: a schema's name-only key
    meets properties of other declarations (`Expressions` holding `AttributeExpressionData` structs, `Materials` in
    terrain layers and interpolation tracks, the time-varying, particle and behavior parameter arrays, the terrain
    schema's `Materials` on mesh components), where reflection types each declaration; a schema struct label that is
    not the declared struct's name (five gestalt and weapon-part entries; the same decode, another label);
    `ReferencedTextureGuids` (38,421 tags on materials), which the cooked classes do not declare, so reflection gives
    nothing where the schema gives `Guid`; and 18 tags met by walks at the wrong prefix in three BrushComponent exports.
  - Rust unit tests on invented packages for each rule above, including `tests/behavior_test.py` case 4 (struct
    defaults, and a default stream not consumed exactly), the parity-mode differences, the stand-ins, the native
    classes and the typer's root and path; the CLI test texts were checked byte for byte against the C++ binary on the
    same package.
- **Report F3 Open.** The context-resolver chain decodes: the C++ VM's `--object-dump` already typed object arrays
  through its declarations, and so does this port; the package walker gets the same through `ReflectionTyper`. The
  chain order is the stored element order. That this order is the game's evaluation order stays UNVERIFIED.
- **Seen, not changed (the formats stay the C++ ones; for the hub).** Enum-byte array elements are 8 bytes (an FName)
  in all 19 such array tags of the class defaults, where the C++ VM reads 1 byte, so it drops those arrays (logged
  skips such as `AllowedRotationFormats`); this port does the same. The one bool array with elements in a struct
  default stream has 3 elements in 7 bytes, 1 byte each, where the C++ VM reads 4, so it fails in both.
- **Decision 06's open items.** Struct field order: values are kept in the declared layout order and the eight
  native structs are read in the C++ order and placed by name, so no value depends on the two orders agreeing;
  printing uses the C++ order for those eight and the declared order (super struct first, own fields in descending
  export order, the C++ `structAt` order) for the rest, which the comparison above matches. The descending-export
  declaration order stays unchecked against the game, as there. A map's value is `Value::None`, printed `null` as the
  C++ prints its `Value()`.
- **UNVERIFIED.** Every tag format here is the C++ VM's reading of 832/46 packages, none checked in the running game;
  the property prefixes 4, 8 and 26; that an enum value named `None` means no value; the declared field order of the
  native structs against their stored order (the value is placed by name, so only printing depends on it).
