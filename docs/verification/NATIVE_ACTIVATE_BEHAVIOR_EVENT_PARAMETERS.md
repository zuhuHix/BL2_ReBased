# BehaviorKernel.ActivateBehaviorEventFromScript with Parameters (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-ACTIVATE-BEHAVIOR-EVENT-PARAMETERS (decision 2026-11-19-NNN,
a placeholder), for slice link 9. **Analyst only:** a different lane implements it (clean room). This lane's author
implemented other behavior-kernel and spawn rules earlier, but not this native's script route, and will not implement
this note.

**Sources:**
- the installed script, read with `research/script_disasm.py` (via NATIVE_SET_BEHAVIOR_VARIABLE.md, #538, for the
  callers);
- the installed packages: the enum `EBehaviorVariableLinkType` (read in its installed order) and Phaselock's behavior
  provider, read through the port's own provider loader (`ow-package --behavior-dump`, plus a local, uncommitted
  print of the event entries' variable links);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, one headless run at a time
  under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/aep/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** Once the behavior-variable setters are bound (#538), link 9's `cast` reaches
`LiftActionSkill.OnSelectedTarget`, which fires its event with a `Parameters` array. The port refuses that call:
`ow_behavior::kernel::natives` answers a call with `Parameters` as `NotYet { NotRead }`, "ActivateBehaviorEventFromScript
with Parameters (how they reach the event is not read)".

## 1. The callers on link 9's path

- `LiftActionSkill.OnSelectedTarget(TheConsumeHandle, SkillInstigator, SkillTarget)` (NATIVE_SET_BEHAVIOR_VARIABLE.md
  section 1): a local `Parameters` array of `BehaviorVariableValue`, length 2; entry 0 is set to the caster and entry 1
  to the target, both by `SetObjectBehaviorVariable` (so both are `BVAR_Object`); then
  `ActivateBehaviorEventFromScript(ConsumerHandle, GetBehaviorProviderDefinition(), 'OnSelectedTarget', <filter
  omitted>, Parameters)`.
- The other `LiftActionSkill` events, the `ExecuteActionSkill` events and two `ShieldDefinition` events have the same
  shape (fill a local array, then fire). They are not on link 9's path.
- **Phaselock's provider** (`GD_Siren_Skills.Phaselock.ActionSkill_Phaselock`'s `BehaviorProviderDefinition_0`, one
  sequence): its event entry `OnSelectedTarget` has two output-variable links:

  | Property name | Link type | Connection index | Target variable |
  |---|---|---|---|
  | `SkillInstigator` | `BVARLINK_Output` | 0 | the object variable `Siren` (one of the sequence's two `BVAR_Object` variables) |
  | `SkillTarget` | `BVARLINK_Output` | 1 | the object variable `Target` (the other one) |

  The sequence's many `BVAR_NamedVariable` references named `Siren` and `Target` resolve to those two object
  variables (the named-variable rule of NATIVE_MARCUS_USE_CHAIN.md). The port's loader already resolves the links this
  way.

## 2. What the native does with Parameters

**It is the same event activation as the native event raises, with the array as the event's payload.**
NATIVE_MISSION_DISPATCH.md A1 and NATIVE_MARCUS_USE_CHAIN.md ("Behavior_RemoteCustomEvent", "Event payload to named
variable") already describe the activation: the provider is required (None fires nothing), an omitted filter is -1,
every enabled sequence of that provider on the consumer with an open event entry of that name runs it, depth first.
This note adds what the array does.

1. **The array is passed by reference, not copied.** The native hands the kernel the caller's array itself, its
   elements and its length. The kernel reads it while the event fires; nothing keeps it afterwards.
   - With the argument omitted, there is **no payload**.
   - An argument that is not a variable reaches the kernel as no payload too (no installed caller does this).
   - An **empty** array (length 0) also binds nothing: the kernel tests for a payload before it binds.
2. **When the binding happens:** per matching event entry, after the entry's trigger count and time are updated and
   before any of the entry's output links is followed. So the variables hold the values before the first behavior
   downstream runs. Each sequence that fires the event binds into its own variables.
3. **What is matched: the position, then the type.** For each of the event entry's variable links:
   - only links of type `BVARLINK_Output` (value 3 of `EBehaviorVariableLinkType`, whose installed order is
     `BVARLINK_Unknown`, `BVARLINK_Context`, `BVARLINK_Input`, `BVARLINK_Output`) take part;
   - the link's **connection index** picks the element: it must be below the array's length, else the link is skipped;
   - the link's **property name is not used** (as for the native raises);
   - each of the link's target variables, after the named-variable resolution, receives the element **only if the
     element's `VariableType` matches the variable's type**:

     | Target variable type | Element `VariableType` that binds | What is written |
     |---|---|---|
     | `BVAR_Bool` | `BVAR_Bool` | the element's `IntValue` |
     | `BVAR_Int` | `BVAR_Int` | `IntValue` |
     | `BVAR_Float` | `BVAR_Float` | `FloatValue` |
     | `BVAR_Vector` | `BVAR_Vector` | `VectorValue` (X, Y, Z) |
     | `BVAR_Object` | `BVAR_Object` | `ObjectValue` (None included), into the consumer process's object slot for that variable; a slot index out of range writes nothing |
     | `BVAR_Attribute` | `BVAR_Float` | the float: into the variable when it has no context objects, else set as the attribute's value on each context object (not read further) |
     | `BVAR_DirectionVector` | `BVAR_Vector` | a direction-dependent write on the context objects, by the variable's sub-mode (rotation from the direction, its negation, a location, a scaled speed; not read further) |
     | any other type | none | nothing |

     A type that does not match writes nothing and raises nothing: the variable keeps its value.
   - A target that is a **named Kismet variable** also has the element handed to the Kismet variable. That hand-off
     follows the type rule above where the resolved target has one of the types in the table: a match writes and then
     hands over, a mismatch skips both. A resolved target of any other type is handed the element whatever its type.
     What the Kismet variable does with it was not read (review r1 of #549; UNVERIFIED, read coarsely).
4. **After the activation**, the kernel notifies its registered listeners of the event (name and filter). This step is
   the same with or without Parameters, it is not read, and the port does not model it today.
5. **No filter callback** comes from script. The event entry's own filter object, when it has one, is consulted only
   with a callback, so from script it does not gate (as A1 says).

## 3. Empty, None, extra and missing entries

- **An element still `BVAR_None`** (a `BehaviorVariableValue` no setter wrote): matches no variable type, so it binds
  nothing; the variable keeps its earlier value. In the port's VM an element of a grown local array that no setter
  wrote is the value None (#538 section 5, item 1), which the implementer reads as the zero struct: `BVAR_None`.
- **An object element whose `ObjectValue` is None** (`SetObjectBehaviorVariable(…, None)`): `VariableType` is
  `BVAR_Object`, so it binds, and the object variable becomes None. This is not the same as `BVAR_None`.
- **Extra elements** (no output link has their index): ignored.
- **Missing elements** (an output link's index at or past the length): that link is skipped; its variables keep their
  values.
- **An empty array:** nothing bound, as with no array.

## 4. Proposed ParityMode Off answer (for a separate implementer)

**Scope:** the script route `ActivateBehaviorEventFromScript` with its fifth argument given. Reference keeps its stub
policy. Everything below is **UNVERIFIED**.

**The payload model.** The port's `Provider::fire_event` takes a payload of objects (`&[Option<ObjRef>]`) and treats an
empty payload as "no payload, use the named outputs". That cannot say "an element of another type" or "a payload that
is present but empty". The implementer needs a payload that keeps, per element, its type (at least `BVAR_Object` with
its object, `BVAR_None`, and "another type"), and a separate "no payload" case. The empty-array case then binds
nothing.

**Write-free preflight** (before the first event fires; the kernel's own state, such as trigger counts and variable
values, lives outside the heap in the port, so #518's counters do not see it, and nothing may be refused after it
changes):
1. Argument 4 must be an array (`VmError::Script` otherwise). Each element must be a `BehaviorVariableValue` struct or
   None (None read as `BVAR_None`); anything else is `VmError::Script`, naming the index. **That refusal is the
   port's own**: in the game the array is always typed.
2. `VariableType` is read **by name** from the loaded `EBehaviorVariableType` (the port's fixtures order their toy enum
   differently on purpose).
3. For **every sequence of the provider that has an event entry of that name, enabled or not, open or not** (review
   r1 of #549, the hub's ruling: `fire_event` re-checks each sequence's enabled state as it goes, so a behavior of an
   earlier sequence can enable a later one before it fires; checking only the sequences enabled at preflight time
   would let a later one fire with unchecked bindings), for every `BVARLINK_Output` link of those entries whose
   connection index is below the length, and every target variable:
   - element `BVAR_Object`, variable `BVAR_Object`: the plain binding (answered);
   - element and variable types that do not match, or element `BVAR_None`: nothing (answered);
   - element `BVAR_Bool`, `BVAR_Int`, `BVAR_Float` or `BVAR_Vector` meeting a variable of the same type:
     `NotYet { NotPorted }`, "ActivateBehaviorEventFromScript: binding a <type> Parameters entry" (the port's
     variables keep only object values today; the rule itself is read);
   - an `BVAR_Attribute` variable with a float element, or a `BVAR_DirectionVector` variable with a vector element:
     `NotYet { NotRead }`, "ActivateBehaviorEventFromScript: a Parameters entry into an attribute or direction
     variable";
   - a named Kismet variable target, whatever the element's type (the hand-off is not read past the type rule of
     section 2, item 3): `NotYet { NotRead }`, "ActivateBehaviorEventFromScript: a Parameters entry into a Kismet
     variable".
4. Nothing is loaded in the preflight.

**The writes:** the existing activation, with the binding of section 2 in place of the current empty-payload outputs:
per firing entry, the trigger count, then the object bindings, then the links. No new refusal after the first firing.

**The counter rule:** the native writes nothing of its own on the heap before the activation. Any refusal it returns
comes from the preflight, unmarked. A refusal from a behavior downstream is that behavior's (and its caller's) to mark.

**For link 9 that answer is enough:** both of `OnSelectedTarget`'s elements are `BVAR_Object` and both of its links
target `BVAR_Object` variables, so `Siren` becomes the caster and `Target` the target before the dialog, cooldown,
provoke and hold behaviors run.

**A related port divergence (recorded; out of scope by the hub's ruling).** With no payload and an empty outputs map,
the port's `fire_event` clears every object variable an output link targets: it sets the variable's stock path to the
empty string and its live object to None, so the variable becomes None. The game binds nothing without a payload
(section 2, item 1): those variables keep their values. The same empty-payload, empty-outputs path runs for:
- `ActivateBehaviorEventFromScript` without Parameters (`Behavior_RemoteCustomEvent` and the like);
- `BroadcastBehaviorEventFromScript`;
- the sequence enabled and disabled events the kernel raises itself.

The hub ruled all of them out of scope for this note (review r1 of #549); they stay a recorded divergence.

## 5. Link 9 after this native (a local probe)

A local, uncommitted probe stacked these branches on `fix/rust-flag-expression-evaluate` (#535):
- `fix/rust-link9-extent`, `fix/rust-encroachment-check`, `fix/rust-link9-trace-filter`, `fix/rust-link9-next-stop`
  and `fix/rust-link9-attr-get-value`;
- an `IsSplitScreen` false binding, the object setter from #538 section 5, and an object-only form of section 4.

Under the lock, installed Off strict, link 9 reached `cast` and stopped earlier than this native. The stop was
NounAttributeValueResolver with replication strategy 2 on a provider, the `NotYet` #523 settles. #523
(`fix/rust-link9-noun-replication`) does not merge onto that base, because its counters carry the earlier names. So
**the stop after `OnSelectedTarget` was not reached** and is open (open question 4).

## Open questions

1. **UNVERIFIED:** the attribute and direction-vector bindings' details (section 2, item 3), and the Kismet hand-off.
   None is on link 9's path.
2. **UNVERIFIED:** the kernel's listener notification after the activation (section 2, item 4): who listens on the
   slice.
3. The port divergence for a script route without Parameters (section 4): a hub ruling.
4. Link 9's stop after this native: needs #523 folded onto the #535 stack.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `LiftActionSkill.OnSelectedTarget` during a Phaselock on an enemy. After the call, find the skill's behavior
   consumer and read the Phaselock sequence's object variables `Siren` and `Target`. Expect Maya's pawn and the target.
2. On the same hook, before the native, set `Parameters[1].VariableType` to `BVAR_None`. Expect `Target` to keep its
   previous value (for example the last target, or None on a first cast).
3. Set `Parameters[1]` with `SetObjectBehaviorVariable(…, None)`. Expect `Target` to become None.
4. Shorten `Parameters` to length 1. Expect `Target` to keep its value and `Siren` to be set.
