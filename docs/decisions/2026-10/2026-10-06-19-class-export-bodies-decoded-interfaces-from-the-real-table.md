## 2026-10-06: Class export bodies decoded; interfaces from the real table; typed temporaries and let-attribute (UNVERIFIED in game)

AI-assisted (Claude), implementer lane I2, from `NATIVE_CLASS_SERIAL_LAYOUT.md` and `NATIVE_BYTECODE_OPCODES.md`. Parsing change: the VM now
decodes every Class export body (`decodeClassBody` in `src/vm.cpp`) and keeps the class flags, within class, config name, interface table and
default object; the rest is consumed for its size only. The structural oracle `ow-package --class-check` (run by `tools/verify_packages.py`)
decodes 3,339 of 3,339 Class exports of the nine code packages exactly; every default object is `Default__<Class>` of its class, and all 367
interface entries name interface-flagged classes. The first body word is not resolved (AkAudio's WwiseSoundVolume points one past the export
table); no bounds check was loosened. `Runtime::implements` now answers from the interface tables of the class chain; the old structural rule
stays as `implementsStructurally`, used only for synthetic classes without a decoded body. On real data the slice's interfaces (IMission,
IUsable, IMissionObjective, IMissionDirector) give the same answers as before; across all classes 975 pairs become yes and 66 become no.
Interpreter: hidden typed temporaries (opcodes 4C-50) read as the typed zero when unset, with operands checked against the function header's
local-variable array; opcode 5F writes the attribute's base companion and recomputes the value from the modifier stack (truncating int and
byte) without the change notification; 5E was already the instance-variable read. Not yet: the attribute add/remove/set-base natives. Checks:
ctest 11/11, verify_packages 9/9 with all class bodies exact, UE build succeeded, quest suite 90/90 + resume 11/11 (rerun by the
orchestrator), door suite 16/16, no new stubs. Verified: the byte layout by the in-tree oracle; the meaning of the decoded fields and the
interpreter rules are UNVERIFIED in game.

