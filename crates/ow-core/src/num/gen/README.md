AI-assisted (Claude Code).

# Oracle tables for `ow_core::num` and `ow_core::fmt`

`../tables.txt` and `../../fmt/tables.txt` hold invented inputs and what the C++ core's forms and Python give for
each (architecture section 5.2). The tests in `num.rs` and `fmt.rs` compare the Rust functions with them bit for bit
(and byte for byte for text). Nothing in them comes from the game.

- `oracle.py` writes the inputs, runs the Python side and merges the two sides. The merge stops without writing
  when the oracles disagree on any cell both of them produce.
- `oracle.cpp` is the C++ side: `std::pow` on doubles and floats, the Core natives' double transcendentals,
  `int64_t(double)` then `int32_t`, `weapon_gen.cpp`'s `HalfUp`, and the printers (`%.Ng`, `%.Nf`,
  `std::to_string(float)`, ostream default and `setprecision(9)`, `std::to_chars`). It is not part of the CMake
  build.

To regenerate, from the repository root in PowerShell (`local/` is ignored, so the intermediate files stay out of the
tree):

```powershell
$o = "local\p2\W1-NUM"; New-Item -ItemType Directory -Force $o | Out-Null
python crates/ow-core/src/num/gen/oracle.py inputs "$o\inputs.txt"
$vc = "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
cmd /c "`"$vc`" -vcvars_ver=14.50 >nul && cl /nologo /O2 /Ob2 /DNDEBUG /EHsc /std:c++20 /W4 /permissive- crates\ow-core\src\num\gen\oracle.cpp /Fo$o\oracle.obj /Fe$o\oracle.exe"
& "$o\oracle.exe" "$o\inputs.txt" | Out-File -Encoding utf8NoBOM "$o\cxx.txt"
python crates/ow-core/src/num/gen/oracle.py python "$o\inputs.txt" "$o\py.txt"
python crates/ow-core/src/num/gen/oracle.py merge "$o\cxx.txt" "$o\py.txt" crates/ow-core/src/num/tables.txt crates/ow-core/src/fmt/tables.txt
```

The flags are the C++ core's Release flags (CMake's `/O2 /Ob2 /DNDEBUG`, the project's `/W4 /permissive-`, the
default `/fp:precise`) with the toolset `ctest` uses (`-T version=14.50`). The 2026-11-15 tables were made on the
zuhu desktop (Windows 11, UCRT, Python 3.14.6).

Python cannot print a NaN's sign or payload, has no single-precision `pow` (it gives the double `pow` rounded once,
which agreed with `powf` on every row) and raises on domain errors where C returns NaN or an infinity, so those cells
come from C++ alone; the last column of each row says, per cell, which oracle produced it (`b` both, `c` C++, `p`
Python).
