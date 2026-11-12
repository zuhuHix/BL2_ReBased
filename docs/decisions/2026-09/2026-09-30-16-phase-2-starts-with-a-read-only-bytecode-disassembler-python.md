## 2026-09-30: Phase 2 starts with a read-only bytecode disassembler (Python prototype)

AI-assisted. Prompted by the inventory-parity work: the original menu's input
code could not be read, only observed. Added `research/script_disasm.py` (prototype, in the
manner of `native_count.py`) and `tests/script_disasm_test.py` (synthetic, 11 checks). It
reads the nine code packages with the existing Python reader and decodes every script
`UFunction` without executing anything.

Established from the data: the function header layout (a `u16` local-variable array, ten
`i32`, the in-memory size, the file size), `0x53` as end of script in this build, and that
jump/skip operands are measured in in-memory bytes where each object reference is 8 bytes
(a least-squares fit gave exactly 4 extra bytes per reference). Result: 12,968 of 12,978
script functions decode exactly under structural checks (header size meets the function tail,
grammar consumes exactly the script, in-memory size equals the header, jump targets are
statement starts); the 12,978 total matches the native census. Ten functions still fail
and some operand layouts are fitted, not proven: they are marked UNVERIFIED in the source.
Full record: `docs/verification/SCRIPT_BYTECODE_DISASM.md`.

Finding that changes the approach: the backpack sort logic (`extOnChangeSort`,
`ApplySortConfiguration`) is native C++, not bytecode, so its ordering still comes from
observing the game. Script-side menu navigation (`NormalMove`, `MoveDelta`, `StartEquip`,
`IsComparing`) is readable. Not done: C++ port (touches `CMakeLists.txt` and possibly the
`Reader`, both sensitive areas, awaiting confirmation), object model, interpreter.
No game bytes committed; listings stay under ignored `local/`.

