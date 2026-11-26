## 2026-11-26: A skipped flag array is LayerAbsent

AI-assisted (Claude Code, Claude Opus 5.5). Queue items 0am and 0an, lane MAYA9-SKIPPED-FLAG-LAYERABSENT, on main d3be3e5
(first written on #781's branch). Clean room: the rule is docs/verification/NATIVE_MAYA9_AND_AFTER.md
section 3.3 ("an array the store marked skipped: `NotYet { LayerAbsent }`"); this lane did not read the executable. It
read the script call site with `research/script_disasm.py` and the parameter's flags from the installed
`GearboxFramework.upk` (output under ignored `local/analysis/maya9_skip/`). No session was run. UNVERIFIED in the
running game.

- **What was found:** `FlagDefinition.ApplyFlagInitializationData`'s first parameter, `FlagsToSet`, is declared
  `const out` (property flags 0x400182: parameter, out, const). The slice's call, in
  `WillowPawn.GetAttributeStartingValuesFromCharacterClass`, passes the character class definition's instance variable
  `DesignerFlagStartingValues`. So from script the native's argument aliases that property, and the store's skip mark
  for it can be asked. Decision 39 recorded this case as not decidable; it is.
- **What was decided** (Off only; Reference binds nothing):
  - `ow-vm`: `NativeCall::arg_property(i)`, read only: the object property an out argument from script aliases,
    `(owner, element)`, or `None` for a local, a temporary, a call from the host or an index past the parameters;
  - `ow-combat` `maya9`: after the declaration check and before the empty test, an argument aliasing a property that
    `DefStore::is_skipped` marks is `NotYet { LayerAbsent }`, "flag initialization: the flags array's property was
    skipped when the store decoded it (its struct layout)". Nothing is loaded or written. An argument that aliases no
    property is answered by its value, as before.
- **Tests** (`maya9_tests.rs`, an invented package): a skipped array is `LayerAbsent`; a decoded empty array is
  nothing; a decoded one-entry array is the entry's `NotRead`; an argument by value answers as before. Each call
  leaves #518's counters, `class_code_loads` and the name table unchanged. With the skip check disabled, the skipped
  test fails.
- **Maya:** `CharClass_Siren`'s `DesignerFlagStartingValues` decodes (empty), so the slice is unchanged.
- **The review follow-ups (queue 0an) of #781 r2 and #786:**
  - #781 r2: the events' test counts the hand-off to the kernel, not a registered provider's `fire_event` (the hub's
    ruling, now in its doc); a pawn class that declares no `ConsumerHandle` is `VmError::Script` with nothing written
    (new test); **UNVERIFIED order:** the enter event's route is fixed in the preflight, before `OnExitStance` runs, so
    a behavior the exit event runs cannot change where the enter event goes (in the module doc);
  - #786: the `ITargetable` cast's walk answers `NotRead`, not 0.0, for every chain it cannot walk to its end: an
    ancestry short of `Core.Object` (now tested), a class with no layout, and a chain past 256 classes (the same reason
    as the import-only rule: a "no" there is not proven). The no-layout case is tested on the walk directly, since the
    class table gives every class it declares a layout.
- **Verified:** the tests on invented data. **Not verified:** anything in the running game.
- **Sensitive files:** none.
