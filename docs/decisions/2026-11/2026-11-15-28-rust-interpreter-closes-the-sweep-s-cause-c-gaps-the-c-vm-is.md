## 2026-11-15: Rust interpreter closes the sweep's cause C gaps; the C++ VM is frozen

AI-assisted (Claude Code). Lane FIX-VMGAPS of the Rust port closes the three interpreter gaps decision 2026-11-15-23
listed as cause C (five WillowGame functions), then freezes the C++ script VM on the sweep clause of the amended
`ow-vm` gate (architecture section 6), which decision 23 set as the freeze condition. The gate's test-file clauses are
not all met (`tests/vm_test.py`, below). It builds on decisions 2026-11-15-02 (the typed heap), 2026-11-15-20 (static
arrays as consecutive slots), 2026-11-15-23 (the sweep and its gate differences A and B) and 2026-11-15-27 (object
states). Every rule below is a port of the C++ reading; none was observed in the running game, so all stay UNVERIFIED
there.

- **The three gaps, the C++ rule each follows, and where the port puts it.**
  - **A static array used as a whole** (decision 20's open item).
    - *C++:* a static-array variable is one array value of `dim` elements, so reading it whole gives that array, and
      passing it copies the array into the callee's local.
    - *Rust:* reading a static-array local, property, state or default variable, or struct member as a value now gives
      the array of its elements, read in order, with the same steps as the C++ (`aggregate::whole_static`). A script
      callee's static-array parameter takes such an array one element a slot (`eval::bind_parameter`). An `out`
      static-array parameter with no alias (a host call, as the sweep makes, or a script temporary) is read back after
      the call as the array of its elements (`eval::read_parameter`).
    - *Still `NotYet`:* assigning a static array whole, an `out` static-array argument that aliases the caller's
      storage, and a static-array parameter given anything but an array of exactly its size. The C++ keeps such a value
      whole in its local, which slots cannot hold; no sweep of the nine code packages meets these.
    - *Met by:* `WillowPlayerController.ClientTravelToSession` (passes its own static-array parameter on) and
      `OnDestroyForTravelComplete` (passes a static-array property), both to `PlayerController.ClientTravelToSession`.
    - *Real-data cover for `read_parameter`:* five functions with an `out` static-array parameter, which the sweep
      calls from the host with a zero array: Engine `OnlineGameInterface.ReadPlatformSpecificSessionInfo` and
      `OnlineGameInterface.ReadPlatformSpecificSessionInfoBySessionName`, and IpDrv
      `OnlineGameInterfaceImpl.ReadPlatformSpecificSessionInfoBySessionName`,
      `MeshBeaconHost.OnReceivedClientCreateNewSessionResult` and `MeshBeaconClient.OnTravelRequestReceived`. A first
      version of this lane refused every `out` static-array parameter, and all five failed with its `NotYet`; with
      `read_parameter` they run to the end, and the Engine and IpDrv sweeps are again the results in the table below.
  - **A `None` given to a native's struct parameter.**
    - *C++:* an element read past the end of a dynamic array, and a struct member read through an object that lacks
      the property, give `None`. The Rust interpreter already gave the same `None`. The C++ vector and rotator natives
      read every field of a value that is not a struct as 0 (`comp` in `natives_core.cpp`), so they compute with a
      zero struct.
    - *Rust:* the Core natives take a struct and leave argument conversion to the interpreter (`natives_core.rs`,
      "Arguments"). So `bind_arguments` now turns a `None` given by value to a native's struct parameter into that
      struct's zero (`eval::native_struct_zero`). A script callee keeps the `None` in its untyped local, as the C++
      does, and a call from the host is not converted.
    - *Representation difference:* the Core natives that compare the whole value, `==(vector,vector)` and
      `!=(vector,vector)`, would now compare a zero vector where the C++ compares `None` (which is unequal to a zero
      vector there). A local, uncommitted count over the nine sweeps found the conversion made 4 times: twice for
      `Object.Add_VectorVector` (the cause C cases) and once each for `Actor.FastTrace` and
      `BehaviorHelpers.GetRotationFromAttachmentLocationData`, both natives with no implementation (logged stubs, which
      read no argument). The comparison natives were never reached by it.
    - *Hub decision (on this lane's report):* the conversion stays in the interpreter. Core `==(vector,vector)` and
      `!=(vector,vector)` given such an argument see a zero vector where the C++ sees `None`, and the nine sweeps
      never hit this. Having `comp` in `natives_core.rs` read a non-struct as zeros, as the C++ does, is on the hub's
      follow-ups.
    - *Met by:* `WillowAutoTestManager.TravelTheWorld.DownTrace` (an element of an empty dynamic array) and
      `WillowWeapon.GetEffectLocation` (a struct member through a mesh component the sweep leaves unloaded).
  - **A Byte stored into an Int property.**
    - *C++:* `IntConstByte` makes a byte value, and a byte argument bound to an int parameter stays a byte in the
      untyped local. `AddItem` stores it into an `array<int>` as it is, with no assignment rule, and since the C++ value
      keeps bytes and ints in one integer field it reads as that int everywhere.
    - *Rust:* the heap's type check is unchanged (AGENTS.md rule 5). The interpreter's representation rules at a heap
      write (`eval::fit_for_heap`, which every property write and every heap-array item goes through) now turn a Byte
      for an Int slot into the Int of the same number. That conversion loses nothing. A Byte for a Float slot, and
      every other mismatch, is still refused.
    - *Why not Float:* the C++ value keeps a float in its `f` field and an int, byte or bool in its `i` field. A byte
      in a float slot therefore reads as 0 wherever the C++ reads the float field directly, but as its number through
      `number()`, so no single lossless conversion gives both. An Int shares the byte's `i` field, so for Int there is
      one.
    - *Met by:* `FrontEndPlayerListGFxObject.OnPlayerDetailsLoaded`, through `AddPlayerDetailsButton`.
    - *Not reproduced:* a later C++ assignment to such an element keeps the byte kind and wraps to 8 bits. The Rust
      element is an Int, as declared, so it does not wrap. The sweeps do not meet this.
- **`states::forget`.** The sweep's `release_unreferenced` now calls `states::forget` for each object it frees, once
  the free has succeeded, as decision 27 asks of every path that frees objects. The sweep output is unchanged by it.
  The call has no test of its own; the sweep's test package has no state code, so a state frame cannot be observed
  there.
- **Unit tests** (invented `Core` package, AGENTS.md rule 9; `interp::aggregate::tests`):
  - `a_static_array_passes_whole_into_a_static_array_parameter`: a property and a local static array passed whole; a
    host call with an `out` static-array parameter reads it back whole; a scalar or a wrong-sized array is `NotYet` and
    leaves no frame behind.
  - `a_none_given_to_a_natives_struct_parameter_is_the_zero_struct`: an element past the end of an empty dynamic
    array reaches a native as the zero struct and a script callee as `None`, and the out-of-bounds line is logged.
  - `a_byte_stored_into_an_int_slot_is_that_int`: a byte added to an `array<int>` is stored as an int; one added to
    an `array<float>` is still a heap error.
  - `static_array_elements_are_their_own_slots` now expects a whole static array to read as its elements, where it
    expected `NotYet`.
- **Evidence.** Local runs on pc-hummer, release builds, outputs under ignored `local/`. Rust 1.96.0 on `origin/main`
  9cfbe10 plus this lane (the nine sweeps repeated on 89a2523 with the same result; the test-file counts on dc388cc,
  which changes only documents); C++ with MSVC 14.50 (`build/Release/ow-package.exe` of this tree). "C++ with A" is that
  binary rebuilt with decision 23's A patch (both decode loops read an ObjectProperty-tagged value as a class when the
  declaration says ClassProperty), built from a local, uncommitted change that was restored before the commit. It
  reproduces decision 23's WillowGame C++-with-A list exactly.
  - **`--vm-sweep --top 100000` on the nine code packages under `OW_PARITY=reference`**, the `seconds` field left out:

    | Package | Attempted | Succeeded C++ / C++ with A / Rust | Total steps C++ / C++ with A / Rust | Failure groups | Unimplemented-native rows C++ / C++ with A / Rust |
    |---|---|---|---|---|---|
    | Core | 44 | identical in all three | 1,165 | 0 | 0 |
    | Engine | 3,574 | 3,570 / 3,570 / 3,570 | 291,269 / 291,559 / 291,659 | the same 4 | 309 / 308 / 306 |
    | GameFramework | 14 | identical in all three | 616 | 0 | 8 |
    | GearboxFramework | 508 | 508 / 508 / 508 | 12,845 / 12,872 / 12,872 | 0 | 89 (the same rows) |
    | WillowGame | 8,355 | 8,318 / 8,318 / 8,318 | 1,792,636 / 1,792,786 / 1,793,125 | the same 13 | 799 / 799 / 795 |
    | GFxUI | 42 | identical in all three | 359 | 0 | 5 |
    | IpDrv | 206 | identical in all three | 7,861 | 0 | 12 |
    | OnlineSubsystemSteamworks | 234 | identical in all three | 2,510 | 0 | 3 |
    | AkAudio | 0 | identical in all three | 0 | 0 | 0 |

    - Against the unpatched C++, 6 of 9 are byte-identical apart from `seconds` (Core, GameFramework, GFxUI, IpDrv,
      OnlineSubsystemSteamworks, AkAudio). Against the C++ with A, 7 of 9 (GearboxFramework too). Decision 07's
      string-unit differences did not show.
    - **Engine** is unchanged by this lane, its residue is decision 23's cause B. The steps are 291,659 against
      291,559 with A, the figure decision 23 records, and the 16 rows that differ from the C++ with A are the same 16 as
      before this lane: 9 C++ rows (5 `function ?.<Name>` rows of methods called on a classless stand-in, and natives
      reached through them) and 7 Rust rows naming the stand-in class's natives.
    - **WillowGame.** The failure groups are now the C++'s 13 in full, succeeded and failed are equal (8,318 / 37),
      and the cause C rows match: `Object.Localize(string,string,string)` is reached by 119 functions and
      `Actor.FastTrace` by 1, as in the C++. Every other row that differs from the C++ with A (20 Rust rows, 24 C++
      rows) is one of decision 23's cause B rows, unchanged by this lane: the set of differing rows was 47 before it
      and is 44 after, the three gone are the two `Localize` counts and `FastTrace`, and none is new.
    - **The WillowGame step residue is cause B as decision 23 attributes it, and this lane changes only the five
      functions.**
      - Against `origin/main` without this lane, the only package that changes is WillowGame. Its three cause C
        failure groups go, `Localize` goes from 118 to 119 functions, `FastTrace` appears, and the total rises by
        exactly 325 steps (1,792,800 to 1,793,125).
      - Each of the five functions was checked alone, as the difference of `--limit k` and `--limit k-1` sweeps at
        its attempt (2269, 2270, 4048, 6125 and 6333; the unfixed binary fails at exactly those attempts). The fixed
        Rust takes the C++ with A's steps at each, 40, 32, 560, 28 and 42, where it took 23, 15, 284, 17 and 38. Those
        17 + 17 + 276 + 11 + 4 = 325 steps are the whole change.
      - So the remaining +339 steps against the C++ with A were already in the residue decision 23 recorded (+14
        with the five functions stopping 325 steps short). That they are cause B rests on decision 23's attribution;
        it was not re-bisected here. A partial local bisection of `WillowPlayerController` alone found differences
        at single attempts of 7 or 14 steps, present before and after this lane, and did not check what they are.
  - **Times** of the Rust sweep loop are unchanged (WillowGame 11.78 s, Engine 5.01 s, GearboxFramework 4.91 s,
    against 0.64, 0.21 and 0.12 s in the C++).
  - **The gate's test-file clauses, on the Rust binary.** Each file was run whole, and also through a local harness
    that runs every statement of the file on its own. A failed statement is recorded and the run goes on; an assert
    site passes when it ran and every run of it passed. The C++ binary passes all 103 assert sites of `vm_test.py`
    under it (decision 23 counted 101 sites with its own harness, which counted differently).
    - `tests/natives_test.py`: passes; 7 of 7 sites.
    - `tests/vm_states_test.py` under `OW_PARITY=reference` (the `GotoState` stand-in cases of section 6.1): passes;
      15 of 15 sites.
    - `tests/runtime_test.py`: passes; 15 of 15 sites.
    - `tests/vm_test.py`: fails as a file; 91 of 103 sites pass. The 12 that fail are other lanes': `MakePair` 1
      (W3-DEFS), `--inventory-move` 10 (W5-EQUIP) and `--mover-probe` 1 (W5-DIALOG), the three groups decision 23
      counted as 1, 11 and 1. The binary without this lane gives the same 91 of 103 under the same harness.
  - Nothing was checked in the running game; this lane had no session.
- **The freeze.** The sweep clause of the amended `ow-vm` gate (architecture section 6, "`--vm-sweep` JSON identical
  to C++ on the 9 packages under `OW_PARITY=reference` ... apart from decision 2026-11-15-07's string-unit differences
  and decision 2026-11-15-23's causes A and B"), which decision 23 set as the freeze condition, is met. The gate's
  test-file clause is not: `tests/vm_test.py` fails on the Rust binary until W3-DEFS, W5-EQUIP and W5-DIALOG land
  their parts. So `src/vm.cpp`, `src/interp.cpp`, `src/natives.cpp`, `src/natives_core.cpp`, `src/census.cpp`,
  `src/census.hpp`, `src/vm_sweep.cpp`, `src/vm.hpp` and `src/natives.hpp` open with a `// FROZEN (...)` header
  like the one W3-SWITCH put on `src/package.*`. It names this lane, decisions 23 and 28 and the sweep clause, and
  says the test-file clauses are not all met. Only the
  comments change; the code is as it was, and the C++ stays the differential reference until W7-RETIRE deletes it.
- **Open items.**
  - The `ow-vm` gate's `tests/vm_test.py` clause: 91 of 103 sites on the Rust binary, the rest W3-DEFS', W5-EQUIP's
    and W5-DIALOG's.
  - The three whole-static-array uses that stay `NotYet` (assignment, an aliasing `out` argument, a parameter given
    another shape), until script that needs them is found.
  - `natives_core.rs` reading a non-struct argument as zeros itself, as the C++ `comp` does (the hub's follow-up);
    the interpreter conversion would then go.
  - Cause B's step residue in WillowGame (+339) has not been bisected to its functions; it rests on decision 23's
    attribution.
