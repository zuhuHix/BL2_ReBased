## 2026-10-07: VM fix, an assignment evaluated its target twice

AI-assisted (Claude), found by implementer lane I6 while matching the golden trace. `EX_Let` evaluated the left-hand side again to return
the assigned value, which repeated its side effects: `arr[arr.Length] = x` grew the array twice and stored two elements. In the slice this
put two users in Marcus's `PawnsUsingMe` and silenced his on-use dialog. The assignment now returns the target it already wrote. New
synthetic case `Foo.Append` in `tests/vm_test.py` fails without the fix. ctest 11/11, verify_packages 9/9.

