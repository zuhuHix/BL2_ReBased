## 2026-10-05: VM: a class constant evaluates to its class

AI-assisted (Claude), implementer lane I1. `EX_ObjectConst` in `src/interp.cpp` decided "is a class" by the name of the
export's class reference; class exports have class reference 0, so class constants became stand-in objects and static
calls through them (e.g. `GetWillowGlobals`) did nothing. Class reference 0 now means a UClass (the rule `vm.cpp`'s
`classNameOf` already used). Verified: new synthetic case in `tests/vm_test.py` (fails before, passes after); the
real-data `--slice-run` and `--inventory-move` outputs are byte-identical before and after; the mover scripts' only class
constant sits in a `foreach` header the interpreter skips. Door suite not rerun for this step.

