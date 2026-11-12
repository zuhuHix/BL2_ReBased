## 2026-10-06: native notes on the Gearbox bytecode opcodes 4C-50, 5E and 5F (UNVERIFIED)

AI-assisted (Claude), analyst lane G20, Ghidra; own-words note `docs/verification/NATIVE_BYTECODE_OPCODES.md`, no listing text. Opcodes
4C-50 are typed hidden temporaries (int, float, byte, bool, object): an lvalue into a frame slot named by the function header's
local-variable array (frame offset and tag pairs; the operand indexes the in-memory array, which is the file array shifted by one).
Opcode 5E is the attribute value-property reference and runs the same handler as instance-variable opcode 01. Opcode 5F is "let
attribute": it writes the base companion, recomputes the value from base and modifier stack (NATIVE_ATTRIBUTES formula), calls a
replication hook and does not fire the attribute-changed notification. Package checks over the nine code packages: 4,061 functions,
7,839 pairs, every pair referenced, no operand out of range. Corrections: NATIVE_ATTRIBUTES assumed 5F notifies; `src/interp.cpp` treats 5F
as a plain Let (stale base) and leaves unset typed temporaries as None. Opcodes 57 and 59 carry a 16-bit operand after the array (neither
occurs in the packages). All UNVERIFIED in game.

