# Native bytecode opcodes: the Gearbox typed temporaries, the attribute reference and the attribute assignment (2026-10-06)

AI-assisted (Claude), analyst lane G20. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Opcode numbers are written as "opcode 4C (decimal 76)" style
(hexadecimal, no prefix) to match `src/script.hpp`.

The question this note answers: what the VM does, from the opcode handlers, for the opcodes the disassembler shows as
`Op4C` ... `Op50`, `Op5E` and `Op5F` (`docs/verification/SCRIPT_BYTECODE_DISASM.md`, "What is not done"), and how that
interacts with the "Gearbox local-variable array" that every script function carries in its header.

## Summary

| Opcode | Project name | Meaning read from the handler | Slice relevance | Confidence | Status |
|---|---|---|---|---|---|
| opcode 4C (decimal 76) | `Op4C` | hidden typed temporary, 32-bit integer; operand = index into the function's temporary array | every function with compiler-introduced int temporaries (8,747 uses) | high | UNVERIFIED |
| opcode 4D (decimal 77) | `Op4D` | same, 32-bit float | 2,173 uses | high | UNVERIFIED |
| opcode 4E (decimal 78) | `Op4E` | same, 8-bit byte | 284 uses | high | UNVERIFIED |
| opcode 4F (decimal 79) | `Op4F` | same, full 32-bit boolean word | 2,903 uses | high | UNVERIFIED |
| opcode 50 (decimal 80) | `Op50` | same, 32-bit object reference | 18,823 uses, the most common one | high | UNVERIFIED |
| opcode 5E (decimal 94) | `Op5E` | reference to an instance property that is the value side of an attribute; runs exactly the same handler as the plain instance-variable opcode 01 | 349 uses; every read of an attribute value in script | high | UNVERIFIED |
| opcode 5F (decimal 95) | `Op5F` | "let attribute": store into the attribute's base companion, recompute the attribute value from base and modifier stack, mark replication-dirty; no change notification | 116 uses (Skill.UpdateGrade, WillowWeapon.CalculateWeaponBaseValues, ...) | high for what it does, medium for the dirty hook | UNVERIFIED |
| opcodes 57 and 59 (decimal 87 and 89) | `DynArrayInsertItem`, `DynArraySort` | read a 16-bit skip operand after the array expression, which the project's table does not list | none (never occur in the nine packages) | medium | UNVERIFIED |

## How the VM dispatches (context for everything below)

The VM keeps one table of 4096 handlers indexed by the opcode byte; every slot starts as an "unknown code token" error
handler and the real handlers are written into it by start-up registration code. Opcodes 4C-50, 5E and 5F **do** have
handlers, so none of them is an error token in this build. Handlers are shared when the machine code is identical:
opcode 5E runs the same code as opcode 01; opcodes 4F and 50 run the same code as 4C (all three copy 32 bits); 4D is a
separate but equal-looking handler that copies through the floating point unit; 4E copies one byte. Opcodes 5B, 5C, 5D
have no handler that was found (and neither did 04, 0C, 15, 18, 2B, 31 by the method used; those are stock tokens the
loop most probably handles itself; not resolved, see Open).

Every handler for a "variable" opcode publishes three pieces of VM state that an enclosing Let, Context or call reads
immediately afterwards: the **address** of the storage, the **property** being accessed (or none) and the **container
object** (or none). It also copies the current value to the caller's result buffer when the caller supplied one. This is
what makes the same expression usable as an rvalue, an assignment target and an out argument.

## The Gearbox local-variable array (header, in memory, and what Link does with it)

- **File layout** (as the project already decodes it): a 16-bit count N, then N 16-bit values, at the start of every
  script function. Read as N/2 **pairs**: (frame offset, tag). N was even in all 12,968 functions of the nine packages;
  4,061 functions have N above zero, 7,839 pairs in total, at most 16 pairs in one function.
- **What the pairs are:** hidden temporaries that Gearbox's script compiler added to the function's frame. In every one
  of the 4,061 functions every pair is referenced by at least one typed-temporary opcode, and no typed-temporary
  operand points outside the array (checked over the whole package set with `research/script_disasm.py`).
- **The tag:** its high four bits give the temporary's size in bytes and its alignment, with 0 meaning 4. The census of
  the tags matches the opcodes exactly: tag high bits 4 (size 4) for 3,206 distinct temporaries = 1,726 integer + 866
  boolean + 614 float slots; high bits 0 (4 bytes, objects) for 4,547 = the object slots; high bits 1 (size 1) for 86 =
  the byte slots. The low twelve bits give the temporary's **position in the frame sequence** (see Link below).
- **In memory:** when a function is loaded the array is stored as `[N, file value 0, file value 1, ...]`, that is shifted
  by one: index 0 holds the count and the file's first value sits at index 1. This is why every typed-temporary operand
  is **odd**: operand 2k+1 means "file pair k", and the value the handler reads at that index is the pair's frame offset
  (file value 2k), with the tag (file value 2k+1) at the next index. The operand is odd in all 32,930 uses checked.
- **Link (class/function linking)** resets all the offset entries to "unassigned" and then recomputes them while walking
  the function's declared properties in chain order: a counter counts items placed so far, declared properties **and
  temporaries both**; whenever the counter equals a pair's id (tag low twelve bits) the temporary is placed at the current
  frame size aligned up to its size, the frame grows by its size and the counter advances; leftover temporaries are placed
  after the last declared property. The temporaries therefore live **inside the function's frame**, interleaved with
  declared locals, and the frame size includes them. The offsets stored in the file are what this procedure yields
  (checked by hand on three functions: `Object.ClampRotAxis` has four int parameters and one temporary with id 4 at
  offset 16; `Object.GetRandomOptionSumFrequency` has a 12-byte array parameter and a 4-byte return value then ids 2, 3, 4 at
  offsets 16, 20, 24; `Object.LerpColor` three float temporaries at 52, 56, 60 after eight declared properties).
  A clean-room VM may therefore use the file's offsets as they are, or allocate its own storage per index.
- **Frame source:** the handlers read the function's array through the current function node and add the offset to the
  frame's locals base. Typed temporaries exist only in function frames (they test that the node is a function; a node that
  is not one would fault, which compiled state code never triggers as far as the package census shows).

## opcodes 4C, 4D, 4E, 4F, 50 (decimal 76 to 80): typed temporaries

- **Operand stream:** one signed 32-bit integer (4 bytes in the file and in memory, unlike object references which are 4
  in the file and 8 in memory): the index into the in-memory array, always odd and below N. Nothing else is read.
  (Matches the project's table: one plain integer each; it also explains the "+4 bytes" the least-squares fit found.)
- **Reads:** the function's array (see above) and 4 bytes (opcodes 4C, 4F, 50), 4 bytes through the floating point unit
  (opcode 4D) or 1 byte (opcode 4E) at the temporary's address.
- **Does (in order):** clears the "current property" and "container object" state (so an enclosing Let sees no property
  and no container); publishes the temporary's address; if the caller gave a result buffer, copies the current value
  there (4, 4, 1, 4, 4 bytes respectively). **It never writes the temporary.** Writing is done by the enclosing Let, which
  evaluates its right-hand side with the published address as the destination.
- **Result type by opcode:** 4C int (signed 32), 4D float (32), 4E byte, 4F bool (a whole 32-bit word, not a bit of a
  bitfield; the script uses it directly as a condition and as an operand of the boolean natives, never inside the
  bit-extracting boolean wrapper), 50 object reference. The opcode is the only place the type is recorded besides the
  tag's size nibble: opcodes 4C, 4F, 50 do the same thing at run time; the type only matters to the compiler and to a
  typed implementation. Which is which was read from usage: first assignments are `IntZero`/`IntOne` or integer struct
  members for 4C, `FloatConst` or float arithmetic for 4D, `ByteConst` and byte primitive casts for 4E, `True`/`False`
  and boolean function results for 4F, and casts, context chains and function results that return objects for 50.
- **Are they lvalues?** Yes, exactly like a local: the opcode is the left operand of Let (3,707 first assignments are to
  opcode 50, 1,287 to 4C, 564 to 4D, 1,309 to 4F, 49 to 4E in WillowGame) and, since the handler publishes the address
  even when no result buffer is given, they also work as out arguments.
- **Lifetime and initial value:** one slot per pair per call; the slot's index is never shared between two different
  opcodes in the same function (checked on all 7,839 slots). Temporaries are assigned before use in nearly all
  functions; 841 slots are textually read before their first assignment (typically inside loops or branch-dependent code),
  so a zero initial value matters. The frame zero-fill was not read (see Open); assume zero: int 0, float 0.0, byte 0, bool
  false, object None.
- **Calls other natives / script:** none.
- **Constants / formulas:** none.
- **Edge cases:** an operand outside the array is not checked by the handler (it would read past the array); the package
  census never produces one. A function with no pair array never contains these opcodes.
- **Call sites in the packages (usage fits):** `WillowActionSequencePawn.TargetTooFar` holds a cast of its `Target` in an
  object temporary and two float temporaries (distances), `BodyClassDefinition.GetWeaponHoldAnimSets` caches the object
  returned by `GetWeaponHoldDef` in an object temporary, `Action_BikeMove.CalcDests` and
  `Action_BunkerBoss_Flight.GetFlightPathToPerch` use integer temporaries seeded with `IntZero`/`IntOne`,
  `WillowDamageTypeDefinition.CalcRadiusDamageScale` uses float temporaries seeded with `FloatConst` and native float
  arithmetic, `WillowTradeManager.ResolveTrade` keeps a byte temporary seeded with `ByteConst` and combined with the byte
  bit operations, `Action_BunkerBoss_Flight.GotoPerch.Update` and `Action_Burrow.CheckCloaked` keep boolean temporaries
  assigned `True`/`False` and boolean function results.
- **How `src/interp.cpp` treats them today:** one `Slot` per (opcode, index) in a per-frame map, read and written through
  the generic location code, default value `None`. Consistent with the above except the default: a read of a never-written
  slot should yield the typed zero, not `None`. Keying by (opcode, index) is safe (the index is never reused with another
  opcode); keying by index alone would also be correct.
- **Implementer checklist:**
  1. A typed-temporary operand is an index, not a property reference; do not resolve it as an object reference.
  2. Reading clears the current property and container (a Let to it triggers no replication or attribute logic).
  3. Typed zero as the initial value (UNVERIFIED zero-fill), per call, never shared between calls.
  4. The slot is writable and usable as an out argument.
  5. Do not rely on the type nibble beyond size; the opcode gives the type.
- **Open:** whether the frame is zeroed on entry; whether the tag's low bits or the opcode matter to anything at run time
  (garbage collection of the object temporaries was not read).

## opcode 5E (decimal 94): attribute property reference

- **Operand stream:** one object reference to a property (4 bytes in the file, 8 in memory), exactly like the plain
  instance-variable opcode 01. Same layout as the project's table (`r`).
- **Reads / Does:** the handler is the *same machine code* as opcode 01. It records the property, the container object (the
  object the surrounding Context selected, or the executing object) and the storage address (container plus the
  property's own offset), and, if a result buffer was given, asks the property to copy its value there.
- **What kind of property:** across the nine packages all 349 uses refer to **attribute value properties** (the
  `FloatAttributeProperty`, `IntAttributeProperty` or `ByteAttributeProperty` flagged with bit 31 of the high flag word, 514
  attribute properties in total: 257 value side, 257 base side). No use refers to anything else. The same packages never
  read an attribute value with the plain opcode 01 (0 of the checked reads), and read one default through the default-variable
  opcode in 14 places. So opcode 5E is "the compiler's marker for an instance property of attribute type"; at run time it
  reads the attribute's **stored value** (the computed one, the same number the property-level natives return).
- **Interaction with 5F:** it is what 5F uses as its left side. 5F depends on the property state the handler leaves behind
  (see below), which is why the compiler cannot use opcode 01 there.
- **Call sites:** reads such as `WillowPawn.GoFromInjuredToHealthy` (revival health multiplier), `StatusEffectsComponent.
  ApplyStatusEffect` (the damage causer's spread interval modifier on a weapon or projectile), reads of a controller's
  status damage modifiers and of `ResourcePool.MaxValue` through a Context; left sides of 5F in every function listed under
  opcode 5F.
- **Implementer checklist:**
  1. Decode as a property reference exactly like opcode 01 and run the same logic.
  2. Reading returns the attribute's stored value (not the base, not a recomputation).
  3. Publish the property so an enclosing 5F can find the attribute's companions.
- **Open:** whether the property-copy step does anything special for attribute classes (it is the property's normal copy
  step: the float and integer attribute classes override the single-element copy with a plain 4-byte copy or a block copy
  for arrays; nothing attribute-specific was seen).

## opcode 5F (decimal 95): let attribute (typed Let)

- **Operand stream:** two expressions, left then right, nothing else (the project's table says `EE`; confirmed). It acts as
  a statement: it writes no result.
- **The attribute property model it relies on (from the packages, checked on `Controller.InstigatedAmpDamageModifier`):**
  each attribute is three properties in the owning class: the **value** property (name `X`, high flag bit 31), a **base
  value** property (`XBaseValue`, high flag bit 30) and an array property of modifiers (`XModifierStack`). The value
  property's serialized tail lists, in order, the stack property and the base property; the base property's tail lists
  nothing and then the value property. The in-memory property object exposes these as "stack companion" and "paired
  property" accessors; for the value property the paired property is the base, for the base property it is the value.
  (Byte attributes keep the same two links one slot further in the object because byte properties carry an enum first.
  The left side of the 116 uses is a float attribute in 98 cases and an integer attribute in 18, never a byte attribute.)
- **Does (in order):**
  1. Clears the "current address" state and evaluates the left expression (always a value-side attribute reference, either a
     bare opcode 5E or a Context wrapping one: 87 and 29 of the 116 uses). This yields the container object, the value
     property and the address.
  2. If no address or no property came back (a None context): logs the error "Attempt to assign attribute through None" at
     the script's error level, uses a shared zeroed 8-byte scratch as the destination and carries on with step 3 only to run
     the right-hand side (nothing is stored in a real object and steps 4 to 6 are skipped).
  3. Otherwise the destination is the storage of the **base** property (the value property's paired property) inside the same
     container, computed **before** the right-hand side runs.
  4. If the left side was the special "length of a dynamic array" target: logs "Illegal call to execLetAttribute() for
     Array <property name>" and stops; the right-hand side is not evaluated. (The script never does this.)
  5. Evaluates the right-hand side with the base storage as its destination: the new number is stored **as the base value**.
  6. **Recomputes the value property** from the base just stored and the property's modifier stack, into the value storage.
     The formula is the stack formula of NATIVE_ATTRIBUTES section 3 (shared by the property-level add/remove natives):
     single precision, with all stack entries summed by kind: kind 1 adds to a pre-add sum, kind 2 to a post-add sum, kind 0 to a
     positive scale sum when its number is above zero or to a non-positive scale sum when it is zero or below, other kinds
     ignored; result = (base + pre-add sum) * ((1 + positive scale sum) / (1 - non-positive scale sum)) + post-add sum.
     The float attribute stores it as a float; the integer attribute and the byte attribute truncate toward zero (integer
     keeps 32 bits, byte keeps the low 8). A modifier entry is read as: kind byte, then a float amount. With an empty stack
     the value simply equals the base.
  7. Calls the executing object's "replication dirty" hook with the value property as argument. This is a virtual on the
     base object that does nothing in the base class, `ResourcePool` and `Skill`; what the classes that override it do was
     not read (it is the same hook the plain Let calls on the container for properties carrying the replicated flag, and the
     property-level add/remove natives call after changing a value). Two details: the call is **unconditional** (the plain
     Let only calls it when the property has the replicated flag) and it is made on **the executing object**, not on the
     Context target, so for a left side with a Context the hook receives an object different from the one that changed. This
     is read from the code as written and may well be an engine quirk.
- **What it deliberately does not do:** it does **not** run the attribute change notification (the owner's "attribute value
  changed" virtual with the property name, run only if the property has the notify flag, bit 29). That notification is what
  clamps a resource pool's `CurrentValue` and what raises a skill's refresh flag in the property-level natives
  (NATIVE_ATTRIBUTES section 4). Evidence that scripts know this: `Skill.UpdateGrade` assigns `Skill.Grade` with 5F and then
  sets `bForceRefreshModifiersNextTick` to true by hand on the next line, which would be redundant if 5F notified.
- **Differences from the plain Let (opcode 0F):** the destination is the *base* property, not the property named on the left;
  the value property is recomputed afterwards; the replication hook is unconditional, on the executing object, with the value
  property; the None error text differs ("assign attribute" versus "assign variable"); dynamic array length targets are an
  error instead of a resize. Common to both: the left side is evaluated first and the right side is evaluated straight into
  the destination memory (so conversions must be explicit in the bytecode, which the compiler does).
- **Result type:** none (a statement). The handler does not write the result buffer.
- **Calls into script:** none. **Calls other natives:** the property's own operations (copy, recompute from base and stack),
  the hook above.
- **Occurrences and call sites:** 116 uses in 55 functions (Engine 26 in 10 functions such as `Controller.ApplyCharacterClassDefaults`,
  `GameInfo.SetPlayerDefaults`, `PlayerController.SetFOV`, `Pawn.SetSprinting`; WillowGame 90 in 45 functions). In WillowGame:
  `WillowWeapon.CalculateWeaponBaseValues` (26, assigning `ShotCost`, `ClipSize`, `Weapon.InstantHitDamage`,
  `InstantHitMomentum` ... from the weapon type definition or from `AttributeInitializationDefinition.EvaluateInitializationData`),
  `WillowWeapon.CalculatePartDependentWeaponBaseValues` (8), `InstancedDesignerAttribute.SetBaseValue` (3: `Value`, then `IntValue`
  from the cast of `Value`, then `BoolValue` from `IntValue`), `Skill.UpdateGrade` (assign `Grade` from `NewGrade` plus one, then
  set the refresh flag), `WillowPawn.GoFromInjuredToHealthy` (assigns the pawn's `GroundSpeed` the property's **default value**),
  `Action_Drive_Pursuit.Start` (left side is a Context on `MyVehicle` selecting `SpeedMultiplierAIOnly`),
  `WillowInteractiveObject.InitializeFromDefinition` (`MaxHealth`, a notify-flagged int attribute) and
  `WillowPlayerController.OnExpLevelChange`. Every use fits "initialise or reset an attribute's base number"; none assigns
  to the base companion directly, and the plain Let is never used with a value-side attribute reference on the left.
- **How `src/interp.cpp` treats it today:** the same as a plain Let (one location, one assignment). That writes the value
  property only: the base property stays stale, the stack is not consulted and nothing recomputes, so a later add or remove
  of a modifier (which recomputes from the base) would use the old base.
- **Implementer checklist:**
  1. On an attribute value reference, find the base and stack companions from the property's tail references in the package,
     not by name guessing (the name pattern `XBaseValue` and `XModifierStack` fits all checked examples).
  2. Assign into the **base**, then recompute the **value** from base and modifier stack with the formula above (float: float
     arithmetic; int and byte: truncate toward zero). Stack kind numbering: 0 scale, 1 pre-add, 2 post-add.
  3. Do not run the attribute-changed notification and do not set any refresh flag implicitly.
  4. Evaluate the left side before the right side; remember the destination across the right side's evaluation.
  5. A None context logs the error, still evaluates the right side, and changes nothing.
  6. Array-length lvalue: log the error, do not evaluate the right side.
  7. Treat the replication hook as a no-op unless a class override is implemented.
- **Open:** the classes overriding the replication hook; whether a base-side or non-attribute property on the left would
  fault (the code dereferences the companion unconditionally; the compiler never emits it); byte attribute behaviour in
  practice.

## Other opcodes: what was checked

All handlers for the opcodes 00-6F were located except the ones named in the dispatch paragraph. The table in `src/script.hpp`
needs these additions from the handlers (none of the three opcodes occurs in the nine packages, so the structural decode
cannot disagree with them):

- **opcode 57 (decimal 87), DynArrayInsertItem:** after the array expression a 16-bit skip offset, then the index expression,
  then the item expression, then the end-of-parameters token. The project table lists it as array then parameters (`EP`);
  the handler says it should be array, 16-bit, parameters (`EwP`), as for opcodes 46, 47, 55 and 56. UNVERIFIED because it
  never occurs.
- **opcode 59 (decimal 89), DynArraySort:** array expression, 16-bit skip offset, one parameter, end token (same remark;
  table says `EP`).
- **opcode 5A (decimal 90), FilterEditorOnly:** one 16-bit value read, after which the code position jumps to the start of
  the script plus that value, unconditionally in this build. The table's `w` is right. One occurrence in the packages.
- Confirmed as laid out in the table: opcode 35 (decimal 53) StructMember (two references, two bytes, then an expression),
  opcode 58 (decimal 88) DynArrayIterator (two expressions, a byte, an expression, a 16-bit value), opcode 54 (decimal 84)
  DynArrayAdd and opcodes 39 and 40 (decimal 57 and 64) DynArrayInsert and DynArrayRemove (array then two parameters, no
  16-bit value), opcodes 55 and 56 (decimal 85 and 86) (array, 16-bit, one parameter), opcode 47 (decimal 71)
  DynArrayFindStruct (array, 16-bit, a name expression for the member, the value expression, end token; fits `EwP`).
  The project's failing functions are therefore unlikely to be caused by these.
- Handler sharing worth knowing (stock UE3 does the same): 25, 28 and 2A share one handler; 26 and 27 another; 3B with 3D and
  3C with 3E; 0B with 2F; the Gearbox ones are 5E with 01 and 4F, 50 with 4C.

## Corrections to earlier notes

- **NATIVE_ATTRIBUTES (G14), section 4 and "Confirmed: `Skill.UpdateGrade` -> `SetAttributeBaseValue` -> notification forces the skill
  refresh":** `Skill.UpdateGrade` in script assigns `Grade` with opcode 5F, which runs **no notification**, and then sets
  `bForceRefreshModifiersNextTick` itself. The refresh therefore does not depend on the notification in that path. The
  notification (clamp of `CurrentValue`, the `Grade` refresh) still applies to the property-level natives; a script that assigns a
  notify-flagged attribute with 5F skips it (the notify-flagged value properties assigned this way in the packages are
  `Skill.Grade` and `WillowInteractiveObject.MaxHealth`).
- **NATIVE_ATTRIBUTES open item "typed assignment ... read as set base, recompute, notify":** answered above: set the base,
  recompute the value from base and stack, replication hook; **no** notification.
- **`docs/verification/SCRIPT_BYTECODE_DISASM.md` and `src/script.hpp` comments:** the opcodes 4C-50 are not "behaving like typed
  local-variable slots" by resemblance, they are hidden typed temporaries selected through the function's pair array; 5E
  is the attribute-property reference; 5F is "let attribute".

**Confirmed in game 2026-10-07 (lane L1), in part:** `Skill.UpdateGrade(N)` on a live skill (Mind's Eye, empty modifier stack) set both `Grade` and `GradeBaseValue` to N for N = 2, 5, 3 and
set `bForceRefreshModifiersNextTick`: base and value are both written. **Correction (observed):** the stored grade is `max(N, 1)` (N = 0, -3, 1 all gave 1), so the two-argument native
call whose second operand is the constant 1 (`native_250`) is a maximum, not "N plus one" as written in the call-site list above. Not shown in game: "no change notification" (a native
virtual that script hooks cannot see) and the recompute from a non-empty modifier stack ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md), "lane L1").

## Not read yet

- Which classes override the replication-dirty hook and what they do; the attribute-changed virtual's other overrides
  (NATIVE_ATTRIBUTES open items).
- Whether the call machinery zeroes the whole frame including the temporaries.
- The unlocated handlers (04, 0C, 15, 18, 2B, 31, 5B, 5C, 5D) and what the loop does for them.
- Byte attribute behaviour in practice; the use of the tag's low bits other than in Link; garbage collection of object
  temporaries.

## Open

- Zero-fill of the frame (assumed).
- Whether anything besides Link and the typed handlers reads the pair array (the other readers found by a search for the field
  were not examined).
- The exact effect of the executing-object versus Context-target difference in the replication hook for the 29 Context uses.
