## 2026-11-15: Rust typed values, class defaults and object dumps contract as shipped

AI-assisted (Claude Code). Lane W3-DECODE of the Rust port fills `crates/ow-object/src/{decode,cdo,prefix}.rs` (tests
in `decode/tests.rs` and `cdo/tests.rs`), `crates/ow-cli/src/cmd/objects.rs` and `tests/object_dump_test.py`. It ports
the loading half of the C++ VM that the reflection lane left: the tag reader (`TagReader`, `applyTaggedDefaults`,
`applyStructDefaults`, `Runtime::resource`), class default objects (`defaultsOf`, `buildDefaults`),
`instantiateExport` and the `--object-dump` branch of `src/cli.cpp` (`valueJson`). Values are typed through the
reflection layouts (decision 2026-11-15-06) and stored in heap slots (decision 2026-11-15-02), not in the C++
name-keyed bag. This file records the shipped interface, where it differs from revision 3 of
`docs/rust-port/ARCHITECTURE.md` section 3.3, and where the output differs from the C++. Reviews r1 and r2 of PR #304
shaped the contract before its freeze (the hub's decisions are marked "hub").

- **Interface.**
  - `decode::ValueCx` borrows the store, the `CodeIndex`, the `ObjectSpace` and a `DecodeState` (the C++ `Runtime`'s
    caches: stand-ins, struct defaults, class default objects, an export-name index, the log) with the parity mode and a
    `ReferenceResolver`.
  - `ReferenceResolver::resolve(&self, cx, package, reference) -> Result<Option<ObjRef>, DecodeError>` decides what
    every decoded object, component and interface reference points at. It is held as `&dyn` and called re-entrantly, so
    a resolver that decodes (a definition store loading the target lazily, W3-DEFS) gets the nested references too;
    state goes behind a `RefCell` not held across a call into `cx`. The default, `StandIns`, is the C++ policy
    (`ValueCx::resource`); `ValueCx::with_resolver` sets another. Resolver calls nest at most 64 deep, whichever resolver
    runs (`MAX_RESOLVE_DEPTH`; past it the reference is the typed `ResolveTooDeep` and the counter is restored on every
    path). An export reached again while it is being instantiated is the typed `InstantiationCycle` for the reference
    that reached it (skipped, as any failing value; the object in progress is not reused, which is the definition
    store's policy to choose), so an export that names itself, once or many times, is instantiated once. One top-level
    load (`ValueCx::load`: a `class_default` or `instantiate_export` call with everything a loading resolver decodes
    inside it) makes at most 65,536 resolver calls (`MAX_RESOLVES_PER_LOAD`, `DecodeState::resolve_budget`); past it
    the load aborts with `ResolveBudget`, which no stream skips (`DecodeError::aborts`), and every object the load
    allocated through the context is freed and dropped from the caches (references to them in struct defaults are
    cleared; a struct default whose read aborted is not cached). All three bounds are Rust-only.
  - `ValueCx::apply_tags` is `applyTaggedDefaults` and returns where the stream ended (past its `None` tag, from the
    payload start: the start of any untagged tail, for `DefStore::raw_tail`). `cdo::class_default` and
    `cdo::instantiate_export(cx, package, index, PropertyPrefix) -> Result<Instance, _>` (`Instance { object,
    tags_end }`, non-exhaustive) are the C++ functions of the same meaning. A failed `instantiate_export` frees the object
    it allocated, as the C++ drops its local object; a class default whose own stream breaks keeps its earlier writes,
    as in the C++. A class default object is registered before its tags are applied: a resolver that instantiates an
    export of the same class while those tags are read gets the object being built, with the tags applied so far,
    instead of starting another one. This is a Rust-only choice: the C++ `buildDefaults` sets `defaultsBuilt` first but
    assigns `cls->defaults` only at its end, so its re-entrant `defaultsOf` returns null and `instantiate` would
    dereference it.
  - `PropertyPrefix` is the contract's enum (4, 8, 26). `to_value` takes the walker tree's package
    (`to_value(tree, decl, package, cx)`), since its references are package-relative. `ReflectionTyper::new(store, code,
    space, package, mode)` answers from the cache only; `decode::prepare_typer` loads a package's classes first.
    `ClassTable::is_native(ClassId)` tells the engine-native stand-in classes (below), marked by
    `ClassTable::mark_native` (both additive in W1-OBJ's `layout.rs`, granted after review r2), so every reader of one
    object space agrees, whichever `DecodeState` defined the class.
  - `heap::Heap::alloc_stand_in` / `ObjectSpace::alloc_stand_in` (additive in W1-OBJ's `heap.rs`, granted in review r2)
    allocate an object of a defined class with no property slots and no outer; an instance origin (`Runtime`,
    `Export`) is refused (`HeapError::InstanceStandIn`), as `live()` would list a slotless object.
- **Reading rules (the C++ ones).** A tag's header is checked outside the per-value guard, so a bad header ends the
  stream (and is logged as `defaults: Default__X: <error>` for a class default); a value read inside its tag's size that
  fails or does not end there is skipped and logged as `defaults: skipped <Class>.<Property> (<Type>)`. Attribute
  tags are read only at the top level; delegate, map and byte-attribute tags are skipped. The eight native structs
  are read untagged in the C++ field order and placed by name into the declared layout; Box, Matrix and other
  fixed-layout structs the C++ VM reads as tagged streams fail and are skipped, as in the C++. A struct starts from its
  defaults; the default stream (52-byte header, word at +44 zero, FITTED) applies only when consumed exactly. Static
  arrays take one tag per element and ignore an index past them. `to_value` keeps the same struct nesting bound (16,
  counted as the VM reader counts, the next level checked only for a struct that enters a tagged stream, so a native
  struct at 16 is read; the package walker itself accepts 32) and skips, inside a struct, a field the struct
  does not declare, a field that does not type and a value the heap would refuse, as the C++ `tagged` does.
- **Stand-ins (hub).** A stand-in is a heap object of the target's class with `Origin::Resource` and no property slots
  (`alloc_stand_in`), as the C++ stand-in holds no properties; a stand-in costs one object, whatever its class. Under
  `ParityMode::Reference` a reference outside the package's tables gets the C++ stand-in named `?`, of class
  `Core.Object`, with `Origin::Resource { owner, index }`: the property keeps it (an array with one such element keeps
  the others), and `--object-dump` fails while printing its path with the C++ text `invalid object reference`, after
  the JSON printed so far, as the C++ streams it. In Off it is a typed error and the property is skipped.
- **Engine-native classes (hub).** 38 classes are referenced by the nine code packages and exported by no installed
  package (Core's reflection classes, `Engine.StaticMesh`, `Engine.Level`, `Engine.Model`, the net classes). A layout
  something else defined at that path (an overlay, a native pack) is used in either mode. Failing that, only under
  `ParityMode::Reference`, and only when the class is genuinely absent (no installed package holds the import, or the
  class path names a missing package or export; every other failure is returned), a stand-in of such an object gets
  the class defined at its import path with no properties of its own under `Core.Object` (once, if nothing defined that
  path first), listed in `DecodeState::native_classes` and marked in the class table (`ClassTable::is_native`), so
  W3-INTERP can answer
  `NotYet` for `IsA` through one. Outside the reference mode it is a `NotYet { NotRead }` and the property is skipped. A
  class an overlay or a native pack defines must be defined before any decode, or the stand-in class takes its path.
- **Array element widths (hub).** `ParityMode::Reference` keeps the C++ widths: an enum-byte element is one byte, a
  bool element a 4-byte word. Off uses the widths measured on the installed packages, FITTED and UNVERIFIED in the
  game: an enum-byte element is an 8-byte name placed in the declared enum (19 of 19 enum-byte array tags of the code
  packages' class defaults meet their size so), a bool element one byte (1 of 1: the only bool array with elements in
  their struct defaults, 3 elements in 7 bytes). With the C++ widths those arrays do not meet their sizes and are
  skipped. `ReflectionTyper` follows its mode: enum-byte arrays are `ByteProperty` under Reference and `None` in Off (no
  walker element type reads a name as an enum); bool arrays are `None` in both.
- **Behaviours kept only under `ParityMode::Reference`** (hub: rows of architecture section 6.1, pinned by no tracked
  test, the local comparison only):
  - arrays with elements inside a struct's default stream stay empty (the C++ reads those streams with no array
    typing). Off types and decodes them;
  - an enum value name the declared enum lacks gives 0. Off refuses it and skips the property, so no value is guessed;
  - an enum value is placed by the C++ `byteFromEnum`: the first Enum export of the package being read named as the
    tag's enum, else the enum a `ByteProperty` declaration's type reference names when its name matches, else 0; then
    the C++ `enumPosition`: that export's own entries in their package's spelling, without their FName numbers,
    against the value's whole text, the position kept to its low 8 bits as the C++ `uint8_t` (257 is 1). A numbered
    value (stored as a base name and a number) matches nothing and gives 0. Off gives the position in the declared
    enum, and a position past 255 is the typed `EnumPastByte`.
- **Struct defaults that need themselves (hub).** Only Off reads arrays in default streams, so only there can a
  struct's defaults wait on their own (`A { array<B> }`, `B { array<A> }`). They are refused the same way whichever
  struct of the ring is asked first: every struct whose stream met the cycle, swallowed or not, fails with
  `StructDefaultsCycle` and publishes nothing; a value of such a struct, and a class holding one by value, fail with it.
  Struct defaults waiting on others nest at most 64 deep (a Rust-only bound), and meeting that bound goes through the
  same barrier: in a ring longer than 64, every struct fails with `DefaultsTooDeep` and publishes nothing, whatever
  the starting struct. Each struct's and enum's export is the one its declaring property's type reference names,
  recorded when a class's layout is first used (the C++ `structAt(decl.package, decl.typeRef)`), so a second
  `PackageId` for the same file still finds it; a struct or enum no recorded class declares is still found by its
  path. One order dependence is left, a Rust-only Off-mode bound: a straight chain of 65 or more structs with no ring
  (each holding an array of the next) is refused when asked from its head, where it meets the bound, and read when
  asked from far enough down, where it fits.
- **Filed ids.** The stand-in cache and the stack of exports being instantiated are keyed by `(PackageId, index)` with
  the id `CodeIndex` files the package under (`resolve_ref`, `filed_id`: the first id met for its file, decision
  2026-11-15-10), so a reference through a second id of the same file (decision 2026-11-15-05) finds the same stand-in
  and the same instantiation in progress.
- **Two R7 exceptions, accepted as pinned (hub).** The typed slots hold what the bag did not, so `--object-dump`
  differs from the C++ in both modes, on class default objects and on every other export, with and without `--all`:
  - **class properties**: these packages tag a class property `ObjectProperty`, its reference naming the class. The
    C++ VM decodes by the tag and keeps an object stand-in in the class property, printed as the class's bare object
    path; here the value is the class, printed `class:<Package>.<Class>`. A class property explicitly tagged with
    reference 0 prints `null` there and `class:None` here. A class no installed package exports has no class to name,
    so it reads as null here (the C++ rule for class reads) where the C++ stand-in prints its import path. Without
    `--all` the C++ also hides such a property where the class default is null, because its `sameValue` counts a null
    class equal to an object stand-in; here it prints, as it differs;
  - **undeclared properties**: a tag naming a property the cooked class does not declare (editor-only properties
    stripped from the class, still tagged) has no slot. The C++ bag prints it; here it is dropped (recorded in
    `DecodeState::dropped`, and logged only where the C++ decode would also have failed). Without `--all` it shows
    only where it differs from the class default's.

  Tracked consumers of `--object-dump`: `tests/behavior_test.py`, `tools/prepare_mission_log.py`,
  `tools/creature_zone.py`, `tools/audio_slice_chain.py`, `tools/census_ambient_npcs.py`,
  `tools/prepare_ambient_world.py`, `tools/prepare_range.py`, `tools/slice_npc_assets.py`,
  `tools/ambient_npc_assets.py`. W3-SWITCH must rerun each of them on both binaries and compare before it switches
  their reader.
- **The schemas stay (hub).** The reflection typer and the schema typer agree on 18 of the 36 entries of
  `tools/*-arrays.schema` and differ on 18 (counts below). Most differences are the schemas' name-only keys meeting
  other declarations, where reflection is right; five are schema struct labels that are not the declared names (the
  same decode); the one real loss is `ReferencedTextureGuids`, which the cooked material classes do not declare, so
  reflection has no type where the schema has one. The schemas are used by 21 tools, the ctest entries
  `properties-synthetic` and `tool-schema-lines`, the C++ CLI (`--array-schema`, until W7) and the Rust CLI
  (`cmd/properties.rs`, `cmd/assets.rs`) with `ow-assets`. Proposed retirement, a later lane: (1) the Rust
  `--properties`, `--scene-records` and `--terrain-records` take the reflection typer as their default, keeping
  `--array-schema` as an override while the C++ binary exists; (2) decide `ReferencedTextureGuids` (an own-words
  declaration from the C++ census, or a kept schema line); (3) rerun each tool on both readers and drop its
  `--array-schema` argument; (4) delete the files, `SchemaTyper` and `tool-schema-lines` with the C++ in W7.

- **Verified (automated, local, installed game, C++ `build/Release/ow-package.exe` of this branch).**
  - Class default objects: `--object-dump <index> 4` with and without `--all` on all 3,339 of the nine code packages
    (6,678 runs), both binaries exit 0 everywhere. Under `OW_PARITY=reference`: without `--all`, 3,339 of 3,339
    byte-identical (line ends normalised); with `--all`, 2,268 identical and 1,071 differing, every difference in the
    two exception classes: 1,674 undeclared keys printed by the C++ (39 distinct names, in 914 objects) and 945 class
    values (934 object paths and 11 explicit nulls, in 500 objects); the logs are equal in every run. Off mode differs
    from the reference mode in 837 runs: log lines (enum properties whose tag value is the name `None`, kept at their
    inherited 0; the decoded enum arrays) and 14 values in 12 objects (11 enum arrays the C++ drops, 2 references to an
    engine-native class left as `NotYet`, one numbered enum value). Of the 1,330 ScriptStruct default streams the
    package walker reads, one holds an array with elements and no class default reaches it by value.
  - A Sanctuary sample, beyond class default objects (`OW_PARITY=reference`): the ten `Sanctuary_*` packages but the
    Air and Hole variants (43,540 exports); per package and export class, the first, middle and last export (3,374
    exports of 571 classes), each at prefixes 4, 8 and 26, with and without `--all`: 20,244 runs per binary. 11,862 runs
    fail on the C++ binary (mostly a prefix that is wrong for that export) and all 11,862 fail the same way on the Rust
    binary, stdout included: 6 of them are a reference outside the tables, where both print the JSON up to that
    property and then `invalid object reference`. Of the 8,382 runs both decode, 3,505 are byte-identical (1,592 with
    `--all`, 1,913 without), and the classifier puts every other difference in the two exceptions (17,241 undeclared
    keys of 277 names; 1,873 class values: 1,695 paths and 178 explicit nulls) except 55: 31 the class exception
    without `--all` (the C++ `sameValue` hiding, above), 20 class values naming a class no installed package exports
    (above), and 4 (2 values, 2 log lines) where the heap refuses a `GearboxFramework.WiringMesh` in a property
    declared `Engine.StaticMesh` (an engine-native stand-in class with no known subclasses; the C++ checks nothing).
    Before REFLECT-FIX (decision 2026-11-15-10), 20 more came from reflection failing to lay out
    `WillowGame.WillowImpactDefinition` when reached from `Sanctuary_Dynamic`; on its main they are gone. Off mode
    differed from the reference mode in 853 of the 20,244 runs (measured before REFLECT-FIX): log lines, references to engine-native classes (`Model`,
    `StaticMesh`) left as `NotYet`, the decoded enum arrays, numbered enum values, and the 6 references outside the
    tables, skipped there.
  - The 619 attribute definitions of `Startup.upk` (1,238 runs): no difference in either mode besides 120 class values
    in 40 runs. `ContextResolverChain` decodes in all 619, equal to the C++ VM's: 612 non-empty chains, 503 of one
    resolver and 109 of two, in their stored order.
  - 20 command lines with bad options, numbers, prefixes, indices and paths: stdout, stderr and exit code equal.
    `tests/object_dump_test.py` (class defaults, prefixes 4, 8 and 26, an object reference, an enum, arrays, the print
    order, a struct default, a reference outside the tables, the refusals) passes on the C++ binary (ctest
    `object-dump-synthetic`, with `--reference`) and on the Rust binary in both modes (`tests/rust_parity.d/decode.txt`).
  - The reflection typer against the five schemas over every export of the 914 packages under `CookedPCConsole`
    (2,058,664 exports, tag streams walked at prefix 4, 8 or 26: 2,026,612 under Reference, 2,027,459 in Off): of 36
    schema entries, 18 agree wherever the walk meets them and 18 differ (301,124 array tags agree, 109,669 differ, in
    either mode). The differences: a schema's name-only key meets properties of other declarations (`Expressions`
    holding `AttributeExpressionData` structs, `Materials` in terrain layers and interpolation tracks, the time-varying,
    particle and behavior parameter arrays, the terrain schema's `Materials` on mesh components), where reflection
    types each declaration; a schema struct label that is not the declared struct's name (five gestalt and weapon-part
    entries); `ReferencedTextureGuids` (38,421 tags on materials); and 18 tags met by walks at the wrong prefix in three
    BrushComponent exports.
  - Rust unit tests on invented packages for each rule above, including `tests/behavior_test.py` case 4, both cycle
    orders and a 65-struct ring from two starts, both name-table orders and the two `byteFromEnum` steps for the
    reference enum rule, a 258-value enum, the rollback of a failed instantiation, the resolver hook, a class default
    named by its own defaults through a loading resolver, the resolve bound, slotless stand-ins, the `?` stand-in, an
    overlay-defined native class in Off, a native struct at the depth bound and the element widths of each mode; the CLI test texts were checked byte for byte
    against the C++ binary on the same package.
- **What a byte-identical dump does not show** (review r1). Equal `--object-dump` text does not establish equal heap
  contents: without `--all` the values equal to the class default's are not printed; references print as paths, not
  identities; floats print with 6 significant digits; members past 12 levels print `"..."`.
- **Report F3 Open.** The context-resolver chain decodes: the C++ VM's `--object-dump` already typed object arrays
  through its declarations, and so does this port; the package walker gets the same through `ReflectionTyper`. The
  chain order is the stored element order. That this order is the game's evaluation order stays UNVERIFIED.
- **Decision 06's open items.** Struct field order: values are kept in the declared layout order and the eight
  native structs are read in the C++ order and placed by name, so no value depends on the two orders agreeing;
  printing uses the C++ order for those eight and the declared order (super struct first, own fields in descending
  export order, the C++ `structAt` order) for the rest, which the comparison above matches. The descending-export
  declaration order stays unchecked against the game, as there. A map's value is `Value::None`, printed `null` as the
  C++ prints its `Value()`.
- **UNVERIFIED.** Every tag format here is the C++ VM's reading of 832/46 packages, none checked in the running game;
  the Off element widths (FITTED); the property prefixes 4, 8 and 26; that an enum value named `None` means no value;
  the classes between an engine-native class and `Core.Object`.
