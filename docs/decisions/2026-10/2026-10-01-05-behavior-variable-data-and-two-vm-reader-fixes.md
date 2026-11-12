## 2026-10-01: behavior variable data and two VM reader fixes

AI-assisted. `BehaviorProviderDefinition` variable values are an untagged block after the tagged properties:
one entry per `VariableData` element, in sequence then variable order, with a size per `EBehaviorVariableType`
name (Bool/Int/Float/Object 4, Vector 12, DirectionVector 48, InstanceData 12, Attribute 20, UnaryMath 8,
BinaryMath 12, AttachmentLocation 32, Flag 8, Named*/AllPlayers 0). The table is FITTED: checked by exact
consumption on 32,185 provider exports (0 mismatches), reference/class checks and cross-copy consistency
(0 inconsistent), and trusted only when consumed exactly. `Behavior_CompareObject` now runs from data (its
script: equal objects follow link 0, otherwise link 1); the Fire dummy's sequence is chosen by its
`BehaviorSequenceEnableByMission` condition, whose evaluation is native, so the rule used is UNVERIFIED.

Parsing behaviour change (`src/vm.cpp`): enum bytes declared in another package now resolve through the
declaring `ByteProperty` (previously read as 0); struct values start from the `ScriptStruct`'s own default
tags (header 52 bytes, fitted; 1,275 of 1,275 structs in the code packages end exactly at the export end) and
only when that stream is consumed exactly. No bounds check was loosened. `CMakeLists.txt` gained the
`behavior-synthetic` test. Verified: CTest 10/10, nine package comparisons, unchanged mission/Kismet/door
runs apart from the new `SetSequence` action field. Not verified against the game: link-id semantics, the
once-per-event rule (duplicate links always differ by id byte; the Fire mission's `Default` event has two such
pairs, so this rule may drop real activations), and the enable-condition rule. Record:
`docs/verification/BEHAVIOR_DATA_DECODE.md`.

