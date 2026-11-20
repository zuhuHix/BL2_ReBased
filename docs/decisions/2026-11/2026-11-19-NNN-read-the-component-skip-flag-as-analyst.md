## 2026-11-19: The component skip check is BulletListener, and the check before it is a component virtual

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKIP-FLAG, analyst only (clean room: a different lane implements
it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md` records:
  - **the skip check:** the box routine skips a primitive whose collision bool word has bit 10 set. The name is
    `BulletListener`, by the packages' declaration order, between native anchors in the same word (the bits
    `SetActorCollision` and `SetTraceBlocking` write, and the transform update's Absolute bits). The behaviour fits:
    projectiles ignore touches from it, ordinary collision walkers skip it, and the bounds leave it out. WillowGame's
    `BulletListenerComponent` exists for bullet touches. The candidate is confirmed, not rejected; high confidence,
    UNVERIFIED in game;
  - **the check before it:** a different predicate, a component virtual asked before the primitive cast. It is
    always true for `PrimitiveComponent`, `CylinderComponent` and `BulletListenerComponent` (read from their
    vtables). It is not read for the other primitive classes;
  - **a proposed Off answer:** lift refusal S by reading `BulletListener` (set: skipped; clear: on to the bounds), if
    the hub agrees. The virtual passes for the read classes; for the others, the hub chooses between the
    pass-through and a `NotRead`.
- **Sources:** the script, through `research/script_disasm.py` (every script function scanned), and the class
  defaults through `ow-package --object-dump`. `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` under the
  machine lock, plus a local reader for vtable slots. Raw output stays under ignored `local/analysis/skipflag/`. No
  addresses, decompiler output, table offsets or listing structure are in tracked files. No bl-sdk session was
  granted, and no recorded capture covers this.
- **Verified:** nothing in the running game. Every rule is UNVERIFIED.
- **Index:** a NATIVE_INDEX row was added.
