// FROZEN (lane FIX-VMGAPS, 2026-11-15; docs/rust-port/ARCHITECTURE.md section 6; decisions 2026-11-15-23 and
// 2026-11-15-28). The C++ script VM stops being developed: no new feature, no new consumer, edits only to keep it
// building. It stays the differential reference of the Rust port (crates/ow-vm; --vm-sweep, --native-census, --run)
// until W7-RETIRE deletes it. It was frozen at the ow-vm gate: --vm-sweep on the nine code packages under
// OW_PARITY=reference gives the Rust result apart from decision 2026-11-15-07's string-unit differences and decision
// 2026-11-15-23's causes A and B, where the Rust port keeps its own reading of the package.
#pragma once

#include "vm.hpp"

#include <string>

namespace vm {

// Native census (Phase 2): which natives does a set of script entry points reach?
//
// Two measurements over the same entry list, both estimates and neither a measurement of the real game:
//  - dynamic: each entry runs on the VM (objects built from class defaults, every native without an implementation
//    a logged zero-result stub) and Runtime's call counters say which functions were entered and how often;
//  - static: the script functions and natives reachable through the decoded bytecode's Final/Virtual/Global/native
//    calls. A virtual call is resolved over the receiver's static class and every subclass that overrides the name,
//    so that part is an upper bound; receivers whose class cannot be inferred are listed as unresolved, not guessed.
//
// Entry file (text; '#' starts a comment; whitespace-separated tokens):
//   new <label> <Package.Class>            an object built from the class defaults
//   export <label> <Package> <ObjectPath>  an installed export instantiated through the VM (class defaults + its data)
//   set <path> <value>                     assign a property (links between the objects above)
//   add <path> <value>                     append to an array property
//   (a path is $label.Property followed by any .Field and [index] steps into structs and arrays that already exist)
//   run <Package.Class.Function> <self> [<value>...]   one entry; self is $label or - (the owner's default object)
//   runclass <Package.Class>               one entry per script function the class declares, no arguments
// A value is $label, none, class:<Package.Class>, struct:<Package.Class.Struct> (zero value), i:<int>, f:<float>,
// b:<0|1>, y:<byte>, s:<text>, n:<name>.
// Objects and their state persist from one entry to the next, in file order.
struct CensusOptions {
    size_t stepLimit = 2'000'000;       // executed expressions per entry
    bool staticClosure = true;
};

// Runs the entry file and returns the JSON report. Throws RuntimeError for a malformed file or an unresolvable
// object or function (a script that throws while running is reported as a stopped entry, not thrown).
std::string nativeCensus(Runtime& runtime, const std::string& entryFile, const CensusOptions& options);

} // namespace vm
