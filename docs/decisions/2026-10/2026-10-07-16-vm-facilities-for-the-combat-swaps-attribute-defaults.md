## 2026-10-07: VM facilities for the combat swaps: attribute defaults, GotoState events, engine iterators

AI-assisted (Claude), implementer lane I9, working from the swap plan of scout lane S1. There are three general VM changes, one commit each, all
checked synthetically in `tests/vm_test.py`:
- **Attribute properties in class defaults load.** Tagged `FloatAttributeProperty` and `IntAttributeProperty` defaults used to be skipped (98
  on a `WillowPlayerController`, 42 on a pawn), so script saw 0. The layout was fitted from the packages: all 242 such tags in the 914 installed
  packages have size 4, and both the value and its `...BaseValue` companion are tagged. Value and base agree on all 1,513 pairs across 3,254
  default objects. The modifier stack stays empty. No `ByteAttributeProperty` tag exists.
- **`Object.GotoState` runs the state events.** When the state changes, or `bForceEvents` is set, `EndState(new)` runs on the old state, then
  the name is set, then `BeginState(old)` runs on the new state. This is the public UE3 order, UNVERIFIED against the game. Nesting is capped at
  8. A `GotoState` inside `EndState` wins over the outer one. An undeclared state only sets the name. Latent state code, labels and
  `bKeepStack` are not run, and are logged.
- **Engine `foreach` over `Actor.AllActors` and `DynamicActors`.** The layout was fitted and checked on all 790 iterator statements: the end
  offset lands on `IteratorPop` after `IteratorNext`. These natives visit the runtime's live objects of the class in creation order. UNVERIFIED:
  `DynamicActors` is treated as the same list as `AllActors`, and the list is a snapshot taken when the loop starts. Other iterators still stub.
  Also fixed: an empty dynamic-array `foreach` popped the enclosing loop's state.

The Fire `--slice-run` is unchanged apart from 42 removed "skipped" log notes. `--vm-sweep` success and failure counts are unchanged.

Checks (rerun by the orchestrator on the lane's build): ctest 11/11, verify_packages 9/9, event order self-test and 18/18. UE suites: run with
the next host commit.

