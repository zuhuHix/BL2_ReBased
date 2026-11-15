## 2026-11-15: Object model: slot layouts in an arena heap, not Send

AI-assisted (Claude Code). Lane W1-OBJ of the Rust port fills `crates/ow-object/src/{ids,value,names,heap,layout}.rs`:
the object handles, `Value`, the name table, the class / struct / enum tables with their slot layouts, and the object
heap. It implements the maintainer's decisions D14 (slot layouts in an arena heap) and D18 (the simulation is
deliberately single-threaded), both approved on 2026-11-14 in the Rust-port plan (ARCHITECTURE.md revision 3,
section 3.3, on the branch `docs/rust-port-plan`). No package is read here and no parsing behaviour changes; this file
records how the Rust object model differs from the C++ VM's (`src/vm.hpp`, `src/vm.cpp`), as the plan asks.

- **Slots, not a name-keyed bag (D14).** The C++ VM keeps an object's properties in a map from lower-cased name to
  value. In Rust a class's layout is its super class's slots followed by its own declarations, so a property's slot is
  fixed when the class is defined, and a `PropId` from the declaring class is the same slot in every subclass. These
  slot numbers are the project's own layout, not the engine's memory offsets; no native offset is used (rule 4).
- **Static arrays.** A property with `array_dim > 1` takes `array_dim` consecutive slots, one element each, addressed
  by `PropAddr { prop, index }`; an index at or past `array_dim` is refused on every read and write. The C++ held a
  static array as one array value. Struct values lay out static-array fields the same way.
- **Arena semantics.** The C++ keeps an object alive while anything holds its `shared_ptr`. In Rust an object lives in
  a heap slot until it is freed. `destroy` marks it destroyed (pending kill): it stays readable and writable, and the
  live-object iteration skips it, as the C++ iterators skip `destroyed`. `free` removes it and bumps the slot's
  generation, so every handle to it is refused from then on, also after the slot is reused; a slot whose generation
  would wrap is retired. Live objects are visited in creation order. Nothing frees objects automatically yet: who frees
  what and when (the world after a tick, a collector for non-actor objects) is for later lanes, behind the same API.
- **Not `Send` (D18).** `Value` holds `Rc` (strings, structs and arrays, copied on write), so `Value`, `Heap`,
  `ClassTable` and `ObjectSpace` cannot cross threads. A compile-time assertion in `value.rs`, `layout.rs` and
  `heap.rs` stops the build if one of them becomes `Send` (checked by hand: asserting it on `u32` fails with E0283).
  Changing this is a breaking contract change under the plan's section 7 bar.
- **Equality.** `same_value` is the C++ `sameValue` (UnrealScript `==`) with two deliberate differences, neither
  pinned by a C++ test: values of unrelated kinds (a string and an int, a float and an object) are unequal, where the
  C++ compared fields both left at zero; and two delegates are equal when they name the same object and function,
  where the C++ returned false for every pair (UNVERIFIED in the running game).
- **Names.** Names are interned once, case-insensitively with ASCII folding, and print in the first spelling
  interned. The C++ keeps each value's own spelling, so where one name is spelt two ways an output port that must stay
  byte-identical to the C++ has to check that case.
- **Writes are checked.** A heap write checks the value against the declaration: the variant fits the kind, an object
  reference names a live object of the declared class or a subclass, a class reference the declared metaclass or a
  subclass, a struct its declared type and entry count, arrays and struct fields element by element. Interface
  references are checked only for being live until the class bodies (W2-REFLECT) say which classes implement what.
- **Attribute companions** are checked as the C++ `companions()` (`src/interp.cpp`) checks them: the value property
  is an attribute kind with high flag bit 31, the stack an array, the base has high flag bit 30, all in one class.

Verified: 35 unit tests on synthetic class tables (`cargo test -p ow-object`). Nothing here has been compared with the
running game; the C++ VM is the reference for every rule above except the two equality differences.
