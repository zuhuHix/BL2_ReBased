## 2026-11-15: Rust native census and VM sweep as shipped

AI-assisted (Claude Code). Lane W4-VMTOOLS of the Rust port fills `crates/ow-vm/src/{census,sweep}.rs` and
`crates/ow-cli/src/cmd/census.rs`: the ports of `src/census.*`, `sweepPackage` (`src/vm_sweep.cpp`) and `nativeSelfTest`
(`src/natives.cpp`), behind `--native-census`, `--vm-sweep` and `--native-selftest`. It builds on decisions 2026-11-15-14
and 2026-11-15-20 (the interpreter) and adds the manifest lines decision 2026-11-15-07 left to it. This file records the
shipped interface, the places the port does not follow the C++ one to one and why, the evidence, and what stays open.

- **Interface.**
  - `ow_vm::sweep::sweep_package(&mut Vm, PackageId, &SweepOptions) -> SweepReport`. The report holds the C++ JSON's
    fields; the CLI prints them with the `jsonString` escapes. The wall-clock `seconds` is printed as `std::ostream`
    prints a double (6 significant digits). It is the one field two runs never share, and every comparison below
    leaves it out.
  - `ow_vm::census::native_census(&mut Vm, &str, &CensusOptions) -> Result<CensusReport, CensusError>`. The report
    holds the entries, the script-function and native tables and the static summary; the CLI prints them with the
    `quote` escapes. `CensusError` (`#[non_exhaustive]`) is `Entry` (the C++ `RuntimeError` text, `entry file line
    <n>: ...` where the C++ says so) or `Vm` (the `VmError` of a lookup or value, its causes through `source()`).
    `CensusError::not_yet` names the `NotYet` behind one; the command then prints it and exits with code 3
    (architecture section 4), and every other error prints as `ow-package: <text>` with exit code 1.
  - `ow_vm::census::native_self_test()`. It exercises the Rust VM's registry: a key is registered, found and called,
    an unknown key is not found, an unbound native runs as the logged stub, and a second registration of a key is
    refused. It prints the C++ summary line unchanged; the C++ tested its older `NativeRegistry` class instead.
  - Both run commands build the VM as `--run` does (`StubPolicy::LogAndZero`, the Core natives) on a thread of
    `interp::STACK_SIZE` bytes. Every report type is `#[non_exhaustive]`.
- **Followed as the C++ does.** Option parsing and its order (the source package loads before the options are read).
  The entry-file grammar, values and paths. The per-entry counters and log categories (digits as `#`, 90 bytes; head
  lines 200 bytes). The static walk: final, virtual, global, native, delegate and Context calls, the typed
  temporaries, receiver typing, the name-only rule (at most 8 candidates), interface receivers, closures A and B, and
  the report's orderings. The sweep's skip rules, failure categories and rankings.
- **Where the port differs, and why.**
  - **Object lifetime in the sweep.** The C++ VM frees an object when its last `shared_ptr` goes, and its `live_` list
    is weak, so the engine iterators of later sweep attempts no longer see an earlier attempt's objects. A reference
    cycle leaks and stays alive: a camera and the modifiers that point back at it are the case met in Engine. The Rust
    heap keeps every object until it is freed.
    - So after each attempt the sweep releases the runtime objects (`Origin::Runtime`) that nothing it can see
      references, and frees them (`ObjectSpace::free_many`, one heap walk that also clears struct defaults), so the
      heap does not grow with the sweep.
    - The holders it counts are every instance `Heap::live` lists, of any origin: runtime objects and the definitions
      the store loads during an attempt (`Origin::Export`). A reference is a property value (inside structs and arrays
      too), an interface, a delegate's object, or the `Outer`. A cycle keeps itself.
    - Three kinds of holder are not counted, because the heap lists no other objects to a caller outside
      `ow-object`: class default objects, resource stand-ins (which now carry slots), and objects that script has
      `Destroy()`ed but that still hold strong references. An object only they reference is released and freed here,
      where the C++ keeps it (open, below).
    - That case was not met on the nine code packages: the sweep output is the same with and without the wider
      holder set and the frees. Before the rule, Engine's sweep differed by 114,529 steps; after it, the residue is
      cause B below.
  - **Finding a function by its export.** `CodeIndex::find_export` drops a leading `<package>.` from a path, as the
    C++ `findExport` does. So a function of a class named like its package (`OnlineSubsystemSteamworks.
    OnlineSubsystemSteamworks`, `Engine.Engine`) cannot be found by its plain path; `--run` fails on such a path in
    both binaries.
    - The C++ sweep and census reach functions by export index. The port loads the owner class, which declares every
      function under its own path, and then finds the declared one. It looks an export up by its full,
      package-prefixed path.
    - Before this, the Rust sweep attempted 10 of OnlineSubsystemSteamworks' 234 functions.
  - **"Decode failed earlier".** The C++ sweep skips, as undecodable, a function whose script failed to decode on an
    earlier call. The interpreter keeps that state inside `interp`.
    - So the sweep keeps the VM's call census on. A script function the census has seen entered was decoded (`invoke`
      counts before it decodes, in both VMs). Such a function is skipped when its script does not decode; the sweep
      decodes it again with the same `ow_script::decode::decode`.
    - A function whose declarations did not type is attempted, and fails with the loader's `cannot run` text. The C++
      `functionAt` throws there and the sweep passes over it uncounted. This case was not met.
  - **The census's `set`, `add`, `export` and values** use the typed heap (decision 2026-11-15-02), so these are
    representation rules, not game rules:
    - a value of the wrong kind is a heap error. The C++ bag stores anything, and its `add` replaces a non-array with
      an array.
    - `none` becomes the typed null of a class, interface or delegate property, and an object stored into an
      interface property becomes an interface reference, as the interpreter's own stores do.
    - a static-array property is consecutive slots, so `$o.Arr[i]` addresses element `i`.
    - `struct:` of a struct that is not loaded is `NotYet` (a struct is defined when a class or function that uses
      it loads; the C++ `structAt` loads it).
    - hexadecimal `f:` text is `NotYet` (no parser for it is written).
    - `export` is `NotYet` after the package and export lookups, which give the C++ errors. The definition store holds
      the export's object under its path, so a copy into a new object, as the C++ `instantiateExport` makes, would
      leave two live objects where the C++ has one, and the engine iterators would visit both.
    - a counted function the VM does not find again by its path is an error, as the C++ `countedFunctions.at()`
      throws.
  - **Names.** The VM's name table folds case and keeps the first spelling it meets, as UE3 FNames do. A census label
    such as `d`, used as an object name before the package's state name `D` was ever interned, would respell `D`.
    `vm_states_test.py` failed on exactly that. Before naming an object the census interns every name of the
    packages that hold its class chain: in UE3 a loaded package's name table is in the FName table before any script
    runs. The C++ census holds names as strings, so it is unaffected.
- **Evidence.** Local runs on pc-hummer, release builds: Rust 1.96.0, and the C++ with MSVC 14.50. Outputs stay
  under ignored `local/`.
  - **Rust unit tests on an invented package** (`census.rs` `toy`, AGENTS.md rule 9):
    - `vm_test.py`'s census block re-expressed (counters, virtual dispatch, the stub row, None contexts, closures A
      and B, sites, entries, step limit, `--no-static`);
    - entry-file errors, `set` and too-many-arguments, the undecodable listing, the name rule, the self-test and the
      number parsers;
    - the sweep's counts, the skip of a function an earlier call failed to decode, `--limit`, `--class` and `--top`,
      and the release rule (a self-referencing object stays, the rest are released);
    - the census's `NotYet` cases (`struct:` not loaded, hexadecimal `f:`, `export`) and the CLI's exit-3 failure.
  - **The same package through both binaries** (`--vm-sweep` with and without `--limit` and `--class`;
    `--native-census` with, without and with a lowered step limit, on an entry file using every command but
    `export`): 12 of 12 command lines identical, in Off and under Reference.
  - **`tests/vm_test.py`'s package through both binaries** (every entry file of `vm_test.py` and `vm_states_test.py`,
    three bad files, `--vm-sweep` variants and `--native-selftest`): 32 of 32 identical under Reference. Off gives
    26 of 32; the six that differ are `GotoState`'s Off-mode `NotYet` (W5-STATES, architecture section 6.1).
  - **Tests on the Rust binary.**
    - `tests/natives_test.py` passes, and `tests/vm_states_test.py` passes under `OW_PARITY=reference`.
    - `tests/vm_test.py`, counted per assertion site with a local harness that runs every statement separately: 88 of
      101 pass (51 before this lane). That includes all 43 asserts of the census block (lines 733-858), which holds
      decision 20's 37 census sites.
    - The 13 sites that fail are other lanes': `MakePair` 1 (W3-DEFS), `--inventory-move` 11 (W5-EQUIP), and
      `--mover-probe` 1 (W5-DIALOG).
  - **The installed game.**
    - `--vm-sweep` on the nine code packages under `OW_PARITY=reference`: 6 of 9 are identical to the C++ (Core,
      GameFramework, GFxUI, IpDrv, OnlineSubsystemSteamworks, AkAudio). Engine, GearboxFramework and WillowGame differ.
      Every difference traces to cause A, B or C below. Decision 07's string-unit differences did not show in any
      package.
    - **Against the unpatched C++** (`build/Release/ow-package.exe` of this tree), the three packages that differ.
      Attempted, the skipped counts and the slowest function are equal in all three.

      | Package | Attempted | Succeeded C++ / Rust | Failed C++ / Rust | Total steps C++ / Rust | Failure groups | Unimplemented-native rows |
      |---|---|---|---|---|---|---|
      | Engine | 3,574 | 3,570 / 3,570 | 4 / 4 | 291,269 / 291,659 | the same 4 | the same 25 |
      | GearboxFramework | 508 | 508 / 508 | 0 / 0 | 12,845 / 12,872 | none | the same 25 |
      | WillowGame | 8,355 | 8,318 / 8,313 | 37 / 42 | 1,792,636 / 1,792,800 | the C++'s 13, plus 3 Rust-only groups of 2, 2 and 1 functions (cause C) | 25 each; `Object.Localize(string,string,string)` is reached by 119 in C++ and 118 in Rust, the rest equal |

    - **The A patch, to repeat the comparison.** In `src/vm.cpp`, the loop that applies a class default object's
      tags picks the value reader by the tag's type name. Add one case before the ObjectProperty case: when the tag
      says ObjectProperty and the declaration the loop already looked up says ClassProperty, read the reference as a
      class (the reader the ClassProperty case uses). Rebuild `ow-package` only, run the sweep, and restore the file.
      With it, GearboxFramework is identical. Engine differs only by B: 100 steps over 3,574 functions, 94 of them in
      `SVehicle.TakeRadiusDamage`. WillowGame differs only by C's five functions, plus the one `Localize` count that one
      of them no longer reaches.
    - **Off mode** adds only the stand-ins of architecture section 6.1 and decision 08: `GotoState` `NotYet`, and
      engine-native stand-in classes.
    - **`--native-census` with the static closure**, on a local entry file over the installed packages (`new`, `run`,
      `runclass`; 14 entries, 3,679 script functions walked, 1,395 natives): byte-identical to the C++.
  - **Times** (seconds, sweep loop only; C++ / Rust):

    | Package | C++ | Rust |
    |---|---|---|
    | Core | 0.09 | 0.05 |
    | Engine | 0.21 | 5.05 |
    | GameFramework | 0.09 | 4.17 |
    | GearboxFramework | 0.12 | 4.98 |
    | WillowGame | 0.63 | 11.68 |
    | GFxUI | 0.09 | 0.06 |
    | IpDrv | 0.10 | 0.06 |
    | OnlineSubsystemSteamworks | 0.10 | 0.06 |
    | AkAudio | 0.05 | 0.02 |

    Rust figures are the mean of two runs (each within 0.04 s of the other). Freeing the released objects after
    every attempt costs about 2.5 s on WillowGame (9.17 s when they were only marked destroyed), and up to 0.26 s on
    the others, since `free_many` walks the heap and the struct defaults each time.

    The roughly 4 s is paid once: a census file holding only `new p GameFramework.GamePlayerController` takes 4.1 s
    in Rust and 0.07 s in C++ (class lookup and class default object). The census over the installed packages takes
    4.8 s against the C++'s 0.46 s.
  - Nothing was checked in the running game; this lane had no session. The census and the sweep assert no game
    behaviour: they measure the interpreter.
- **Gate differences A and B (hub decision on this lane's report).** The `ow-vm` row of architecture section 6 now
  reads "identical to C++ under Reference apart from decision 07's string-unit differences and decision 23's causes A
  and B". Rust does not reproduce either one under Reference. Both are representation quirks of the C++ VM, and copying
  them would only add stand-in code that W7-RETIRE then deletes.
  - **A. A class-typed default cooked with an ObjectProperty tag.**
    - *Example:* `MoveRequestClass` of `GearboxFramework.Action_PawnMovementBase`, a `ClassProperty` whose cooked
      default tag says ObjectProperty and whose value names a class.
    - *C++:* the default decode switches on the tag's type (`src/vm.cpp`, `applyTaggedDefaults`), so the property
      holds an object stand-in. `New` returns None for a value that is not a class (`interp.cpp:469-471`), and every
      Context on the result is a None context.
    - *Rust:* the value follows the declared `ClassProperty` (the definition store's typed defaults), so it is a
      class and `New` makes the object.
    - *Why Rust keeps it:* the declaration says the property holds a class, and `New` with a class is what the script
      asks for. The C++ result comes from reading the tag's type in place of the declaration.
    - *Shown by:* a local, uncommitted change to the C++ decode that followed the declared type. GearboxFramework then
      matched, and so did the Engine cameras.
    - *Settles decision 20's open item:* in `WillowPlayerController.InitInputSystem`, the C++ path that gives None is
      this one, an ObjectProperty-tagged cooked default.
    - *UNVERIFIED:* what the game's `New` does with this default. No session observed it.
  - **B. A resource stand-in keeps its class.**
    - *C++:* the `Runtime::resource` stand-in has no class. A method called on it is
      `UNIMPLEMENTED function ?.<Name>`, and `DynamicCast` of it gives None.
    - *Rust:* the stand-ins of decision 2026-11-15-08 carry their class. A method finds that class's function, and a
      cast to a class of its chain succeeds. Example: `SVehicle.TakeRadiusDamage` casts its mesh component to
      `SkeletalMeshComponent` and goes on.
    - *Why Rust keeps it:* the reference names an object of a known class. A classless object is the C++ stand-in's
      limit, not something the package says.
    - *UNVERIFIED:* the game's behaviour on these calls and casts; in the game the objects are loaded, not stand-ins.
- **Open items.**
  - **C. Three interpreter gaps, five WillowGame functions.** These are real gaps; the hub starts a follow-up lane
    after this one lands.
    - A static array used as a whole is `NotYet` (decision 20): `WillowGame.WillowPlayerController.
      ClientTravelToSession` and `WillowGame.WillowPlayerController.OnDestroyForTravelComplete`.
    - `Core.Object.Add_VectorVector` refuses a non-struct argument ("argument 0: expected a struct"). The C++ native
      reads such a value as a zero vector. Two functions hit it, by two input paths:
      - `WillowGame.WillowAutoTestManager.TravelTheWorld.DownTrace`: an element read past the end of an empty
        dynamic array;
      - `WillowGame.WillowWeapon.GetEffectLocation`: a struct member read through a Context on a mesh component.
    - The heap refuses a Byte value written to an Int property, where the C++ stores it:
      `WillowGame.FrontEndPlayerListGFxObject.OnPlayerDetailsLoaded`.
  - **The freeze waits.** Section 6 freezes the C++ VM files at the `ow-vm` gate. Under the amended gate that means
    9 of 9 packages identical apart from the listed differences, so not before C lands. No `FROZEN` header is added
    yet. The precedent, when it comes, is the header W3-SWITCH put on `src/package.*`.
  - **Performance.**
    - The one-time 4 s class-default build is probably the O(heap) free in the definition store's loads, which is
      already on the hub's list.
    - A sweep runs 19 to 46 times slower than the C++ on the four large packages. That is over the 2x of the risk
      table (architecture section 13). The per-attempt `free_many` heap walk is part of it (WillowGame, above); a
      free that touches only the referencing objects would need an `ow-object` index of references.
    - The census's static walk adds 0.35 s.
  - **Contract follow-ups** (on the hub's list):
    - `CodeIndex::find_export` (as the C++ `findExport`) drops a leading `<package>.` once. So `Vm::find_function`,
      and `--run`, cannot find a function of a class named like its package by its plain path. This lane goes
      through the owner class and the full path instead.
    - The sweep's release rule cannot see three kinds of holder: class default objects, resource stand-ins and
      objects script has destroyed. Each needs an `ow-object` accessor that lists those objects (or a reachability
      helper on `Heap`); `ObjRef::new` is crate-private, so a caller cannot walk the heap by index.
    - The decode-failure state is private to `interp`. The sweep infers "decoded before" from the call census and
      decodes again.
  - The census `export` line is `NotYet` until a census export makes one object: either the definition store's own
    object for the export (as the iterators should see it once), or a fresh object the store does not also list.
  - The census's name workaround (interning the names of the class chain's packages before naming a labelled
    object) belongs in the package load path: in UE3 a package's name table enters the FName table when the package
    loads. It is on the hub's list; the census keeps the workaround until the loader does it.
