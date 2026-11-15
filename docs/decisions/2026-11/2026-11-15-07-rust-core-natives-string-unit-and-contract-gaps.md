## 2026-11-15: Rust core natives string unit and contract gaps

AI-assisted (Claude Code). Lane W3-CORENAT of the Rust port fills `crates/ow-vm/src/natives_core.rs`, a port of
`src/natives_core.cpp` onto the W2-VMAPI registry: 155 natives and the two `Actor` iterators, under their C++ keys.
This file records the three places where the port does not follow the C++ one to one, and why. The hub accepted the
string unit for this lane.

- **String natives count UTF-16 code units.** This covers `Len`, `Left`, `Right`, `Mid`, `InStr`, `Asc` and `Chr`.
  The UTF-16 code unit is the unit of the game's `FString`. The C++ counts the bytes of its UTF-8 `std::string`.
  - For ASCII text the two counts are equal, so these natives give the C++ results.
  - For other text, the C++ would cut through a UTF-8 sequence. A Rust `str` cannot hold that, so the Rust port does
    not reproduce it.
  - A cut through a surrogate pair gives U+FFFD.
  - `Chr` keeps the low 16 bits of its argument; the C++ keeps the low 8.
  - These natives work on the UTF-8 bytes and match the C++ byte for byte: concatenation, search, replace, split and
    comparison. `Caps` and `Locs` map ASCII only, as C++ `toupper`/`tolower` do in the C locale.
  - Verified: the ASCII behaviour against the C++ source, by unit tests in `natives_core.rs`.
  - UNVERIFIED: anything about non-ASCII text in the running game. No non-ASCII string has been checked live.
- **Two contract gaps. Both return `NotYet { LayerAbsent }` until W3-INTERP fills them.**
  1. **GotoState under `ParityMode::Reference`.** It needs the states a class declares and a method lookup by class,
     name and state (C++ `Class::states` and `Runtime::findMethod`: from the object's class upward, the state's own
     function first, then the class's).
     - Neither `ClassTable` nor `Vm` provides these on the shipped contract.
     - The port keeps both behind a private `StateLookup` trait. Its shipped implementation answers `NotYet`.
     - Proposed for W3-INTERP: `Vm::find_method(class, name, state)` and `Vm::class_declares_state(class, state)`.
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
- **The GotoState nesting counter lives per VM, inside the native.** C++ keeps it as the `Runtime::stateChangeDepth`
  field. The Rust port keeps it in the registered closure, as one `Rc<Cell<u32>>` per VM, because the lane may not
  change `Vm`. The bound is the C++ one: 8 nested state changes, and the 9th is abandoned with the C++ log line. The
  counter unwinds on every path, errors included. W5-STATES may move it onto `Vm`.
