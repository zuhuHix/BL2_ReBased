## 2026-11-19: ActivateBehaviorEventFromScript with Parameters, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-ACTIVATE-EVENT-PARAMETERS, slice link 9. Coded from
docs/verification/NATIVE_ACTIVATE_BEHAVIOR_EVENT_PARAMETERS.md alone (#549 at 39bc0800), with the hub's ruling on
#549's review. Clean room: lane ANALYST-ACTIVATE-BEHAVIOR-EVENT-PARAMETERS (another session) read the executable; this
lane did not open it or that lane's analysis output. Synthetic fixtures only.

- **What changes:**
  - **`ow-behavior` `kernel`:**
    - a typed payload, `PayloadElement`: each element keeps its `VariableType` name and its object;
    - `Provider::fire_event_with`, where `None` is no payload (the existing named-output path) and `Some` is a payload,
      empty or not;
    - `Provider::check_payload`, a write-free preflight;
    - `fire_event` keeps its signature and maps onto `fire_event_with` as before: an object payload, with an empty
      one meaning none.
  - **`ow-behavior` `kernel::natives`, the script route of `ActivateBehaviorEventFromScript`, Off only:**
    - `Parameters` is read as the event's payload: an array of `BehaviorProviderDefinition.BehaviorVariableValue`
      structs or None (None is `BVAR_None`);
    - `VariableType` is read by name from the loaded `EBehaviorVariableType`;
    - anything else is `VmError::Script`, the port's own check;
    - Reference keeps the old refusal text in the kernel rule and is otherwise unchanged.
- **The binding** (note sections 2 and 3). It happens per firing event entry, after its trigger-count update and
  before its links run. Each `BVARLINK_Output` link takes the element at its connection index; a link past the
  payload is skipped. It writes only when the types match:
  - an object element into an object variable is ported (None included);
  - a mismatch or a `BVAR_None` element writes nothing;
  - an empty payload binds nothing.

  On link 9, `OnSelectedTarget`'s index 0 binds `Siren` and index 1 binds `Target`.
- **Refused in the preflight,** before the first event fires, so never marked:
  - a same-typed bool, int, float or vector binding: `NotYet { NotPorted }`;
  - an attribute variable taking a float, or a direction-vector variable taking a vector: `NotYet { NotRead }`;
  - a named Kismet variable target: `NotYet { NotRead }`.
- **The hub's ruling on #549's review:**
  - the preflight checks every sequence with an entry of that event name, enabled or not, open or not, because the
    firing re-checks each sequence's state only when it reaches it, and an earlier behavior can enable a later one;
  - the no-Parameters clearing path (object outputs written empty) stays as it is, a recorded divergence out of
    scope. That covers the script route without Parameters, `BroadcastBehaviorEventFromScript` and the sequence
    enabled/disabled events.
- **A defensive refusal:** a refused binding reached during the firing anyway is `NotYet` marked "stopped after
  partial writes: ". With the all-sequences preflight it should not be reachable.
- **Not done:** the kernel's listener notification after the activation (note section 2, item 4; not read, not
  modelled), and the attribute, direction and Kismet bindings (refused).
- **Tests** (`ow-behavior`, invented classes and the fixtures' toy enum, which orders its values differently from the
  game):
  - object binding by index;
  - `BVAR_None`, short, empty and type-mismatched payloads binding nothing;
  - an object element holding None binding None;
  - the no-Parameters path unchanged;
  - preflight refusals: a same-typed Int (also in a disabled sequence), a Kismet target, and no firing;
  - the binding rule's table;
  - decoding by enum name, with its Script refusals;
  - the Off script route firing with `Parameters`, and Reference unchanged.

  `ow-behavior` 97 passed.
- **Checks:** `cargo test --workspace` 2,450 passed, 0 failed, 46 ignored; crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge with #523 (d2227301),
  #535 (7f8fc7d7) and #548 (22b29bfb), never pushed. The merge took #518's counter files and renamed #523's two
  counter calls locally. Result: 1 ok / 3 not yet / 10 error.
  - Link 9 passes `spawn` (1 of 2) and stops in `cast` at the unbound `ActionSkill.GetBehaviorProviderDefinition()`.
  - That is the argument `OnSelectedTarget` evaluates just before this native, so the new rule is not reached on
    link 9 yet. Another lane is analysing that native.
- **Review r1 of #555:**
  - **the existing object-payload route is unchanged** (the hub's ruling). `fire_event` (used by the
    `Behavior_RemoteCustomEvent`-style routes and Marcus's use chain) still binds an object element into the object
    variables its link targets and skips any other target silently, in both modes. Only the new script `Parameters`
    route applies the typed rule, and it refuses in its preflight. A private `Payload` route marker keeps the two
    apart;
  - the script route finds the provider's slot first and decodes `Parameters` only when something would fire, so a
    call that fires nothing reads no enum through the store;
  - the refusal inside the firing is labelled defensive, unreachable after `check_payload`;
  - **a further recorded divergence:** when the mission crate owns the consumer handle, `ow-mission`'s
    `n_event_from_script` (the bridge) still ignores `Parameters`;
  - tests: the old route still skips an Int and a Kismet target; through the script route, a refused `Parameters`
    (a same-typed Int, a Kismet target) runs no behavior, leaves the trigger count at 0 and the variables untouched;
    the binding comes before the links (each linked behavior sees the new `Siren`); two open entries of one sequence
    and two enabled sequences each bind. `ow-behavior` 100 passed.
- **UNVERIFIED.** Nothing here was checked in the running game. The note's live checks would confirm it.
