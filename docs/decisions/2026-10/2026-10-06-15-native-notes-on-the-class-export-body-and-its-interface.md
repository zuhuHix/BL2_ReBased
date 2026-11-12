## 2026-10-06: native notes on the Class export body and its interface table (UNVERIFIED)

AI-assisted (Claude), analyst lane G19, Ghidra; own-words note `docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md`, no listing text. The
Class export body for version 832/46: struct part (with script bytes), state part (probe mask, label offset, state flags, function map),
class part (class flags, where 16384 marks an interface; within; config name; component map), then the interface table (count, then
interface class reference and `VfTable_<Interface>` property reference, or null), several name lists, a few fixed-size fields of unknown
meaning, and the default object reference. An oracle script outside the repository fits all 3,339 Class exports of the nine code packages
exactly; it has not been run in-tree, so the layout stays UNVERIFIED until an in-tree oracle passes. Compared with the structural stand-in
`Runtime::implements`: no disagreement on IMission, IUsable, IMissionObjective or IMissionDirector, but 975 missed pairs (interfaces with no
functions of their own), 66 false positives (mostly IGFxMenuScreenTickable and IInstanceData), and 19 real interfaces whose names do not
start with I. Nothing confirmed in the running game.

