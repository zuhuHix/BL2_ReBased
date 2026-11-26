## 2026-11-26: New takes five operands in the decoders and the interpreter

AI-assisted (Claude Code, Opus 5.5). Lane NEW-FIVE-OPERANDS (queue 0p), on main 1709b8a8. Clean room: implemented from
docs/verification/NATIVE_TOUCH_COMPONENT_TEMPLATE.md alone (its census and section 3, decision 2026-11-26-02); this lane
did not read the executable. A parsing behaviour change (rule 4). No session was run.

- **What was decided:** bytecode `New` (0x11) has **five operands**: outer, name, flags, class, template.
  - `research/script_disasm.py` and `ow-script` `ops.rs` decode it as `EEEEE`; both decoded `EEEE` before.
  - `src/script.cpp`, the frozen C++ reference, still decodes four. That is why Reference keeps the C++ evaluation
    below.
- **The census check** (`python research/script_disasm.py --arg-check`, installed packages, counts only, no listing):
  - it compares every final call (by object reference) and every virtual call (by a name whose declarations all have
    one parameter count) with the callee's declared `Parm` properties, without `ReturnParm`;
  - five operands: 12,978 script functions, 12,968 of them decoding exactly, as before; 30,324 of 32,495 calls
    resolve; **0** pass more arguments than declared and 0 fewer;
  - a scratch copy with four operands: the same decode totals, and **5** calls pass one argument more, each with a
    `New` among its arguments. That is the note's count;
  - the check exits non-zero unless the extra-argument count is 0 and the decode totals are those two numbers.
- **The interpreter** (`ow-vm` `interp/eval.rs`, `new_object`):
  - **Off** evaluates all five operands first, in that order, so outer, name, flags and template are evaluated even
    with no class (the handler's order, section 3.1). Then:
    - no class: None, as before;
    - a class whose chain reaches `Engine.Actor` (by name, no load): None, with the script error logged and no
      object made. This is new: before, the port made the object;
    - **a template:** a typed refusal before anything is made, from the first of section 3.3's checks that fails. In
      order: a template the store has not loaded, a class default not built, a template of another class, class
      flag bit 23 (instancing the template's components), bit 10 (a per-object configuration read). A class whose
      body did not decode gets the bit-23 refusal, because its flags cannot rule the step out;
    - **a template that passes every check** is `NotYet { NotPorted }`, "copying a template's values". Option B's
      copy (values, transient reset, archetype) is a separate lane. The port's objects have no archetype field yet,
      and the transient flag is not modelled;
    - name and flags are evaluated and not applied, as before: the VM's generated name, and no flags.
  - **Reference** keeps what the C++ does with its four-operand reading: the class first, then (for a class) the
    object and the outer. The template is evaluated last, where the C++ evaluates it as its next statement or the
    call's next argument, and its value is dropped. Name and flags are not evaluated, as before.
    - Remaining difference: Rust evaluates the template inside the enclosing expression, the C++ after it. In the
      call case the C++ also passes one more argument. Neither is observable for the 225 `New` expressions in the
      packages, whose templates are `Nothing` or plain values.
- **The slice gate** (installed, Off, strict; base 1709b8a8 and this change):
  - links 6 and 7, which misread the `New`, move. They stopped in `touch` at the bounds' template `NotRead`
    (NATIVE_COMPONENT_BOUNDS.md, option B). Now they stop one step earlier, in `make` (`AttachMeshToPickup`), at "new:
    instancing a template's components": `SkeletalMeshComponent` has class flag bit 23. That is the note's predicted
    next stop (section 3.4, item 1);
  - their `make` step no longer passes, so the step share goes from 47.0% to 42.9%, with 6 of 14 links ok on both;
  - every other row is unchanged.
- **NATIVE_PICKUP_SAVE_ROTATION.md section 1** gets a dated addendum: the decoder half is done, and the copy is
  still open.
  - Decision 2026-11-25-64 is not edited: it records review r1's four-operand reading as it stood, and this decision
    resolves it.
- **Verified:** the handler read (lane F, Ghidra) and the argument-count oracle above (this lane, on the installed
  packages). The tests on invented data cover the decoder (Rust and Python), the operand order in both modes, the
  Actor refusal and every template refusal.
- **Not verified:** everything in the running game, including what the game does with a template's components (bit
  23) and per-object config (bit 10), and the name and flags handling.
- **Sensitive files:** none of the listed ones. The opcode table in `ow-script` is a bytecode-layout change under rule 4.
