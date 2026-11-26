## 2026-11-26: A skipped flag array is LayerAbsent

AI-assisted (Claude Code, Claude Opus 5.5). Queue item 0am, lane MAYA9-SKIPPED-FLAG-LAYERABSENT, stacked on #781
(origin/fix/rust-maya9-natives 9c41966, not on main yet). Clean room: the rule is docs/verification/NATIVE_MAYA9_AND_AFTER.md
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
- **Verified:** the tests on invented data. **Not verified:** anything in the running game.
- **Sensitive files:** none.
