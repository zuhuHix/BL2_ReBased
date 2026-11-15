## 2026-11-15: Object model: slot layouts in an arena heap, not Send

AI-assisted (Claude Code). Lane W1-OBJ of the Rust port fills `crates/ow-object/src/{ids,value,names,heap,layout}.rs`:
the object handles, `Value`, the name table, the class / struct / enum tables with their slot layouts, and the object
heap. It implements the maintainer's decisions D14 (slot layouts in an arena heap) and D18 (the simulation is
deliberately single-threaded), both approved on 2026-11-14 and recorded in the Rust-port plan
(`docs/rust-port/ARCHITECTURE.md`, on main since 3c1c47f; section 3.3 now lists the shipped signatures). No package is
read here and no parsing behaviour changes; this file records how the Rust object model differs from the C++ VM's
(`src/vm.hpp`, `src/vm.cpp`, `src/interp.cpp`), as the plan asks.

- **Slots, not a name-keyed bag (D14).** The C++ VM keeps an object's properties in a map from lower-cased name to
  value. In Rust a class's layout is its super class's slots followed by its own declarations, so a property's slot is
  fixed when the class is defined, and a `PropId` from the declaring class is the same slot in every subclass. These
  slot numbers are the project's own layout, not the engine's memory offsets; no native offset is used (rule 4).
- **Static arrays.** A property with `array_dim > 1` takes `array_dim` consecutive slots, one element each, addressed
  by `PropAddr { prop, index }`; an index at or past `array_dim` is refused on every read and write. The C++ held a
  static array as one array value. Struct values lay out static-array fields the same way.
- **Arena semantics.** The C++ keeps an object alive while anything holds its `shared_ptr`. In Rust an object lives in
  a heap slot until it is freed. `destroy` marks it destroyed (pending kill): it stays readable and writable, and the
  live-object iteration skips it, as the C++ iterators skip `destroyed`. `free` is the garbage-collection step: it
  removes the object, clears every reference to it held in the heap in the same call (object and interface references
  become null, a delegate bound to it becomes unbound, an outer becomes none, inside arrays and structs too), and bumps
  the slot's generation, so a handle kept outside the heap (a VM frame: clearing those is the VM's job) is refused from
  then on, also after the slot is reused; a slot whose generation would wrap is retired. UE3 clears references when its
  collector removes an object; when that happens after `Destroy()` in BL2, and whether a delegate keeps its function
  name with a null object, is UNVERIFIED. Nothing calls `free` yet: who frees what and when is for the world and VM
  lanes.
- **Live objects** are the instances (made at run time or instantiated from an export) in creation order. Class default
  objects and resource stand-ins are not live, as in the C++ (`Runtime::liveObjects` tracks only what `instantiate`
  made).
- **Not `Send` (D18).** `Value` holds `Rc` (strings, structs and arrays, copied on write), so `Value`, `Heap`,
  `ClassTable` and `ObjectSpace` cannot cross threads. A compile-time assertion in `value.rs`, `layout.rs` and
  `heap.rs` stops the build if one of them becomes `Send` (checked by hand: asserting it on `u32` fails with E0283).
  Changing this is a breaking contract change under the plan's section 7 bar.
- **Equality.** `same_value` follows the C++ `sameValue` (UnrealScript `==`) for values of one kind. `sameValue` answers
  by the left side's kind, so mixed kinds are lopsided there: `Name == Str` compares lower-cased texts and `Str == Name`
  bytes; `Class(null) == None` is true and `None == Class(null)` false; an int against a string, an object or `None`
  compares integer fields both leave at 0. In Rust a name and a string are unequal in both orders, a null class equals
  `None` in both orders (as the interpreter's `None` literal is a null object reference, `EX_NoObject`), and other
  unrelated kinds are unequal. `sameValue` returns false for every delegate pair; the interpreter's delegate operators
  (`EX_EqualEqual_DelDel` and its siblings) compare object and function name byte for byte. Rust compares object and
  function too, but the function names case-insensitively, because names are interned (UNVERIFIED in the running
  game). No C++ test pins any of these cases.
- **Names.** Names are interned once, case-insensitively with ASCII folding, and print in the first spelling interned.
  The C++ keeps each value's own spelling, so where one name is spelt two ways an output port that must stay
  byte-identical to the C++ has to check that case. A name carries UE3's FName instance number: a trailing `_<digits>`
  (no leading zero, below `i32::MAX`) is split off, so `Projectile_1234` is the entry `Projectile` with number 1235 and
  numbered spawns do not grow the table. The split rule is the public UE3 convention, UNVERIFIED for this executable.
- **Struct defaults.** `zero_value` is the plain zero. The C++ `Runtime::zeroValue` applies struct defaults
  (`applyStructDefaults`, stream layout FITTED); its Rust counterpart is `default_value` / `struct_defaults`, computed
  on each call from the zero, the defaults of by-value inner structs and the struct's own default entries, so defaults
  set later reach every struct that holds it. New objects start from these defaults. Reading the default stream out of
  a package stays W3-DECODE's.
- **Writes are checked.** A heap write checks the value against the declaration: the variant fits the kind, an object
  reference and a delegate's receiver name objects that are not freed, an object reference one of the declared class or
  a subclass, a class reference a class of the table that is the declared metaclass or a subclass, a struct its
  declared type and entry count, arrays and struct fields element by element. `write_in` changes one array element or
  struct field and checks only the new value. Interface references are checked only for not being freed until the
  class bodies (W2-REFLECT) say which classes implement what.
- **Strings** are UTF-8; the game's `FString` is UTF-16, so which unit `Len`, `Mid` and their siblings count in is a
  decision for the VM lanes (UNVERIFIED: no live check of a non-ASCII string yet).
- **Attribute companions** are checked as the C++ `companions()` (`src/interp.cpp`) checks them: the value property
  is an attribute kind with high flag bit 31, the stack an array, the base has high flag bit 30, all in one class.

Verified: 51 unit tests on synthetic class tables (`cargo test -p ow-object`). Nothing here has been compared with the
running game; the C++ VM is the reference for every rule above except the differences listed under Equality.
