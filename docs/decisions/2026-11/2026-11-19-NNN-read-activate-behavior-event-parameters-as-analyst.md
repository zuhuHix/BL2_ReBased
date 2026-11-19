## 2026-11-19: Read how ActivateBehaviorEventFromScript passes Parameters to the event (analyst)

AI-assisted (Claude Code). Lane ANALYST-ACTIVATE-BEHAVIOR-EVENT-PARAMETERS, analyst only; a different lane implements
the note (clean room). Sources: the installed script (through NATIVE_SET_BEHAVIOR_VARIABLE.md, #538), the installed
packages (Phaselock's provider and `EBehaviorVariableLinkType`, read through the port's own loader), and
`Borderlands2.exe` read locally with the project's Ghidra tooling under the machine lock. Raw output stays under
ignored `local/analysis/aep/`. No code changed. There was no real-game session, so every rule stays **UNVERIFIED**.
The decision number is a placeholder.

- **New note:** `docs/verification/NATIVE_ACTIVATE_BEHAVIOR_EVENT_PARAMETERS.md`, with one row in `NATIVE_INDEX.md`.
- **What it settles:**
  - the array is the event's payload, by reference;
  - per firing event entry, after its trigger count and before its links, each `BVARLINK_Output` link takes the
    element at its connection index;
  - a target variable receives the element only when the element's `VariableType` matches the variable's type;
  - `BVAR_None` elements, mismatched types, missing and extra entries, and an empty array bind nothing.
- **Link 9:** Phaselock's `OnSelectedTarget` entry links index 0 to the object variable `Siren` and index 1 to
  `Target`. Both of the caller's elements are `BVAR_Object`.
- **Proposed Off answer** (section 4):
  - a typed payload;
  - a write-free preflight with named refusals: `NotPorted` for non-object bindings, `NotRead` for attribute,
    direction and Kismet targets;
  - every refusal before the first event fires, because the kernel's state is not on the heap and #518's counters do
    not see it.
- **For the hub:** the port's script route without Parameters writes an empty object into every output-linked object
  variable; the game binds nothing. Changing it alters current Off behavior.
- **The probe** (local, uncommitted): on the #535 stack, link 9 reached `cast` and stopped at #523's noun
  strategy-2 `NotYet` before this native. #523 does not merge onto that base (its counter names), so the stop after
  this native is open.
