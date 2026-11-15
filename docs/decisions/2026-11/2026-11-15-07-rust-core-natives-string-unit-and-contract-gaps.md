## 2026-11-15: Rust core natives string unit and contract gaps

AI-assisted (Claude Code). Lane W3-CORENAT of the Rust port fills `crates/ow-vm/src/natives_core.rs`, a port of
`src/natives_core.cpp` onto the W2-VMAPI registry: 155 natives and the two `Actor` iterators, under their C++ keys.
This file records the three places where the port does not follow the C++ one to one, and why, and which lanes finish
the parts that wait. The hub accepted the string unit and the lane assignments for this lane.

- **String natives count UTF-16 code units.** This covers `Len`, `Left`, `Right`, `Mid`, `InStr`, `Asc` and `Chr`.
  The UTF-16 code unit is the unit of the game's `FString`. The C++ counts the bytes of its UTF-8 `std::string`.
  - For ASCII text the two counts are equal, so these natives give the C++ results.
  - For other text, the C++ would cut through a UTF-8 sequence. A Rust `str` cannot hold that, so the Rust port does
    not reproduce it.
  - `Chr` keeps the low 16 bits of its argument; the C++ keeps the low 8.
  - These natives work on the UTF-8 bytes and match the C++ byte for byte: concatenation, search, replace, split and
    comparison. `Caps` and `Locs` map ASCII only, as C++ `toupper`/`tolower` do in the C locale.
  - **A known limit of `Value::Str` (ow-object, W1-OBJ's contract), not of this lane.** A string value is a Rust
    `str`, so it cannot hold a lone surrogate. A lone surrogate, or a cut through a surrogate pair, becomes U+FFFD and
    is lost: `Chr(0xD83D) $ Chr(0xDE00)` gives two U+FFFD, not the pair. The game's `FString` would keep them.
    Changing this is a change of W1-OBJ's `Value` contract.
  - **An explained difference for W4-VMTOOLS, listed in advance.** The `--vm-sweep` JSON output of the Rust binary
    will differ from the C++ one wherever non-ASCII text reaches `Len`, `Left`, `Right`, `Mid`, `InStr`, `Asc` or
    `Chr`: the counts and positions are UTF-16 units there, not UTF-8 bytes. That is this decision, not a parity bug.
  - Read from the C++ source, and checked for ASCII by unit tests in `natives_core.rs`.
  - UNVERIFIED: anything about non-ASCII text in the running game. No non-ASCII string has been checked live.
- **Two contract gaps. Both return `NotYet { LayerAbsent }` until the lane named below fills them (hub decision).**
  1. **GotoState under `ParityMode::Reference`.** It needs the states a class declares and a method lookup by class,
     name and state (C++ `Class::states` and `Runtime::findMethod`: from the object's class upward, the state's own
     function first, then the class's).
     - Neither `ClassTable` nor `Vm` provides these on the shipped contract.
     - The port keeps both behind a private `StateLookup` trait. Its shipped implementation answers `NotYet`.
     - **W3-INTERP** adds `Vm::find_method(class, name, state)` and `Vm::class_declares_state(class, state)` (it
       needs method lookup for dispatch anyway). It may edit `register()` in `natives_core.rs` to wire them in place
       of `ContractStates`.
     - Already running: the no-op paths (no object, no argument, the same state without bForceEvents, the label log).
     - Tested: the full C++ algorithm, over a stand-in lookup on the `tests/vm_states_test.py` scenarios.
     - Outside Reference, GotoState is `NotYet { NotPorted }` until W5-STATES (architecture section 6.1).
  2. **`QueryInterface` and the `InterfaceClass` filter of `AllActors` / `DynamicActors`.** They need C++
     `Runtime::implements`.
     - W2-REFLECT's `CodeIndex::implements` exists, but the `Vm` holds no `CodeIndex` and `ClassLayout` carries no
       interface list.
     - What works now: `QueryInterface` returns the object when its class derives from the asked class, and None for
       a null object or a null class.
     - Where the interface table would decide, the result is `NotYet`. The iterators return `NotYet` when a filter is
       given and at least one candidate exists.
     - **W4-INTERP2**, which owns interface context, adds the interface query and makes the same kind of `register()`
       edit.
  - **The manifest lines.** The `tests/natives_test.py` line and the `tests/vm_states_test.py` `mode=reference` line
    of `tests/rust_parity.d/corenat.txt` land with **W4-VMTOOLS**, which owns `--native-census` and
    `--native-selftest`.
- **The GotoState nesting counter lives per VM, inside the native.** C++ keeps it as the `Runtime::stateChangeDepth`
  field. The Rust port keeps it in the registered closure, as one `Rc<Cell<u32>>` per VM, because the lane may not
  change `Vm`. The bound is the C++ one: 8 nested state changes, and the 9th is abandoned with the C++ log line. The
  counter unwinds on every path, errors included. W5-STATES may move it onto `Vm`.
- **How the float expectations of the tests were confirmed (review r1).** Every result that depends on rounding or on
  the C runtime is asserted as an f32 bit pattern produced by the C++ build. That covers the choke-point functions,
  `**`, `Lerp`, float `+`, the vector reciprocal and all three `GetAxes` rows. The C++ build is Release with the MSVC
  14.50 toolset, on pc-hummer. Each value was confirmed by `ow-package --run` on a synthetic package. Its script
  functions return `Native(constants) == FloatConst(bits)`; each returned true, and one ulp off returned false. The
  first `Rand(10)`, `Rand(1000)` and `FRand()` under Reference (0, 626, `0x3e939c7e`) came from the same build, and
  a hand computation of the C++ LCG gives the same values. The generator stays under ignored `local/`. This confirms
  the C++ build, not the game.
