## 2026-11-19: Controller's QueryInterface rule stops at a native class whose answer was not read

AI-assisted (Claude Code, Opus 5.5). Lane FOLLOWUP-602-CLASS-POLICY, stacked on #602's accepted head 5e1ca57a. The
decision number is pending from the hub (three digits); NNN is a placeholder. **Disclosure:** this lane's author also
implemented #602 (from #589's note; it did not read the natives). So this is a fix round on its own code against the
three review points the hub kept from #637's review, not an independent review.

- **Point 1, registration: checked, holds because** the ordinary and the read-only rules are kept in per-class maps
  keyed by the exact class. Both dispatch paths take the nearest registered class on the asked object's chain, so
  `WillowMind`'s own rule (reading `MyWillowPawn`) shadows `Controller`'s for a `WillowMind` and its script children,
  and `Controller`'s answers for `AIController` and `GearboxMind`. Registration keeps an existing binding (both forms
  are checked). No code change. Test added: every production binding present, each class's exact binding, and the
  same answer on both paths (a `WillowMind` whose `Pawn` names another pawn still answers through `MyWillowPawn`).
- **Point 2, class policy: fixed.** The rule registered for `Engine.Controller` covered every class under it,
  including native classes whose own answer #589 did not read. `ClassTable::is_native` marks engine stand-ins only,
  so the class bodies' flags are used:
  - **A fitted native-class bit**, `CLASS_NATIVE` in `ow-object`'s `class_body.rs` (bit 7, value 128), **UNVERIFIED**.
    The structural oracle, over the installed code packages: it is set on 2,547 of 3,339 decoded classes, and on all
    101 loaded classes that declare a native function. That supports the fit, but does not prove that a class without
    the bit has no native code.
  - **The rule:** from the asked object's class upward, the first class that is native (the bit, an engine stand-in,
    or a body that did not decode) and outside the note's read list stops the rule with `NotYet { NotRead }`. The read
    list is `Controller`, `AIController`, `GearboxAIController`, `GearboxMind` and `WillowMind`. A read class, or the
    top of the chain, ends the walk; script-only classes inherit.
  - **Two lists and two refusals:** "the interface answer of <class>" is checked before any step; "the pawn accessor
    of <class>" is checked only when a step reads the pawn. The two read lists are the same five classes today
    (#589 sections 1 and 3).
  - **Tests:** an unread native class that lists the interface, and its script child, each `NotRead` on both paths;
    under a policy that reads the answer but not the accessor, a plain hit answers and each pawn step is the
    accessor's `NotRead`.
- **Point 3, preflight order: partly fixed.**
  - **Checked, holds because** the receiver's residency is checked by the dispatch before any rule runs, on both
    paths. The controller guard can only match a controller already answering further up the same chain, which
    passed that check, and a read-only chain loads nothing in between. So checking the guard after residency changes
    no outcome. Fields are already read, and their shape and residency checked, only when a step reaches them.
  - **Fixed:** a reserved interface class id with no defined layout (`ClassTable::by_path` accepts one) is
    `NotYet { NotRead }`, "the interface class <path> is reserved but not defined", after the guard and the class
    policy and before any step. Before this, its name was read from the undefined layout.
  - **Tests:** a cold pawn skipped by a plain hit (answered, still cold); the same cold pawn on a reached branch
    (`NotRead`); a malformed pawn slot skipped by a plain hit and reported on a reached branch; the reserved interface.
- **Every case** checks #518's three counters, the class table's length and the name table's length before and after.
  The production binding stays keep-existing and Off only; Reference is unchanged.
- **Verified:** nothing in the running game. Every rule is UNVERIFIED.
