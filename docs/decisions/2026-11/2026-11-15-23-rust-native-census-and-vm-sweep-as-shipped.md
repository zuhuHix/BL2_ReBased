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
  - `ow_vm::census::native_census(&mut Vm, &str, &CensusOptions) -> Result<CensusReport, String>`. The report holds
    the entries, the script-function and native tables and the static summary; the CLI prints them with the `quote`
    escapes. An error is the C++ `RuntimeError` text (`entry file line <n>: ...` where the C++ says so).
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
    - So after each attempt the sweep destroys the runtime objects that no other live runtime object references. A
      reference is a property value (inside structs and arrays too), an interface, a delegate's object, or the
      `Outer`; a cycle keeps itself.
    - References held by class default objects are not counted. The heap lists only instances to a caller outside
      `ow-object`, so an object that script stored into a class default object is released here, where the C++
      keeps it.
    - That case was not met on the nine code packages. Before this rule Engine's sweep differed by 114,529 steps;
      after it, the residue is the classless stand-ins below.
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
    - `struct:` needs the struct loaded, as `--run`'s `st:` does.
    - hexadecimal `f:` text is refused, as `--run` does.
    - `export` copies the definition store's object for the export (read with the 4-byte prefix) into a new object
      named after the export.
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
      and the release rule (a self-referencing object stays, the rest are released).
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
      GameFramework, GFxUI, IpDrv, OnlineSubsystemSteamworks, AkAudio). The other three differ for three causes that
      are not this lane's. Decision 07's string-unit differences did not show in any package.
    - **A. Class-typed defaults cooked with an ObjectProperty tag.**
      - Example: `MoveRequestClass` of `GearboxFramework.Action_PawnMovementBase`, whose value names a class.
      - The C++ decode switches on the tag's type, so it holds an object stand-in, and `New` returns None for a value
        that is not a class (`interp.cpp:469-471`). The Rust value follows the declared `ClassProperty`, so `New`
        makes the object.
      - This settles the path decision 20 left open for `WillowPlayerController.InitInputSystem`.
      - Shown by a local, uncommitted change to the C++ decode that followed the declared type: GearboxFramework then
        matched, and so did the Engine cameras.
    - **B. Classless resource stand-ins.** The C++ `Runtime::resource` stand-in has no class, so a method on it is
      `UNIMPLEMENTED function ?.<Name>` and `DynamicCast` gives None. The Rust stand-ins (decision 2026-11-15-08)
      carry their class. With A patched out, this is all that is left in Engine: 100 steps over 3,574 functions, 94
      of them in `SVehicle.TakeRadiusDamage`.
    - **C. Three interpreter gaps, five WillowGame functions:**
      - a static array used as a whole is `NotYet` (decision 20), in 2 functions;
      - `Add_VectorVector` refuses a non-struct argument that came from reading past the end of an empty dynamic
        array, in 2 functions;
      - the heap refuses a Byte value written to an Int property, in 1 function.
      - With A patched out, WillowGame differs only by these five functions, and by one `Localize` count that one of
        them no longer reaches.
    - **Off mode** adds only the stand-ins of architecture section 6.1 and decision 08: `GotoState` `NotYet`, and
      engine-native stand-in classes.
    - **`--native-census` with the static closure**, on a local entry file over the installed packages (`new`, `run`,
      `runclass`; 14 entries, 3,679 script functions walked, 1,395 natives): byte-identical to the C++.
  - **Times** (seconds, sweep loop only; C++ / Rust):

    | Package | C++ | Rust |
    |---|---|---|
    | Core | 0.09 | 0.05 |
    | Engine | 0.21 | 4.79 |
    | GameFramework | 0.09 | 4.10 |
    | GearboxFramework | 0.12 | 4.89 |
    | WillowGame | 0.64 | 9.17 |
    | GFxUI | 0.09 | 0.06 |
    | IpDrv | 0.10 | 0.06 |
    | OnlineSubsystemSteamworks | 0.10 | 0.06 |
    | AkAudio | 0.05 | 0.02 |

    The roughly 4 s is paid once: a census file holding only `new p GameFramework.GamePlayerController` takes 4.1 s
    in Rust and 0.07 s in C++ (class lookup and class default object). The census over the installed packages takes
    4.8 s against the C++'s 0.46 s.
  - Nothing was checked in the running game; this lane had no session. The census and the sweep assert no game
    behaviour: they measure the interpreter.
- **Open items.**
  - The `ow-vm` gate's "sweep identical on the 9 packages" is not met while causes A, B and C stand. Each needs its
    owner, or a decision that accepts it as an explained difference:
    - A: W3-DEFS or W3-DECODE (the class-default decode), or a Reference stand-in in `New` (W3-INTERP's file);
    - B: W3-DECODE (decision 08's stand-ins);
    - C: W4-INTERP2 (static arrays, the past-the-end element) and W3-INTERP or W3-CORENAT (the Byte into Int store,
      the vector argument check).
  - Freezing the C++ VM files was not done. Section 6 freezes a layer at its gate, which is not met, and the files are
    outside this lane. The one precedent is the `FROZEN` header W3-SWITCH put on `src/package.*`. The question is in
    the handoff.
  - Performance: the one-time 4 s class-default build and a sweep 14 to 45 times slower than the C++ on the four large packages are over the 2x
    of the risk table (architecture section 13). The census's static walk adds 0.35 s.
  - The release rule does not count references held by class default objects (above). The census `export` command
    is tested only for its error path; no `export` line ran on the installed packages.
