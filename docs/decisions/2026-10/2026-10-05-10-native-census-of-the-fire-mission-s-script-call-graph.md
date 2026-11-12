## 2026-10-05: native census of the Fire mission's script call graph

AI-assisted (Claude). Phase 2 step A. Added opt-in call counters to `vm::Runtime` (filled in `Interp::invoke`; off by
default, existing callers unaffected), `ow-package --native-census` (`src/census.*`; entry file
`tools/slice_native_census_entries.txt`, 319 entries) and `tools/slice_native_census.py`; record in
`docs/verification/NATIVE_SLICE_CENSUS.md` (names and counts only).

- Result: 126 natives reached dynamically, 577 in closure A (primary static edges), 1,934 in closure B (upper bound with
  subclass overrides and interface implementers); 24 of the 514 non-operator natives in A are implemented. All 319 entries
  complete on default objects with stubbed natives, but only 90 enter a second script function: the dynamic numbers are a
  floor.
- Verified (automated): counters and closures on a synthetic package (`tests/vm_test.py`), every dynamically entered function
  inside the static closure B, CTest 10/10, packages 9/9.
- UNVERIFIED: every number as a description of the real game (no trace was taken); the static resolution of virtual and
  interface calls; calls made by native code back into script (the entry list stands in for them).
- Finding, left unchanged here: `ObjectConst` of a class export evaluates to a stand-in object because class exports have
  class reference 0 (`src/interp.cpp`), so static calls through class constants (e.g. `GetWillowGlobals`) do nothing in the VM.
- Sensitive file: `CMakeLists.txt`, one source (`src/census.cpp`) added to `ow-core`; nothing else changed.

