## 2026-11-19: Rust Pawn.IsFriendly and the WillowPawn charm layer in Off

AI-assisted (Claude Code). Lane IMPL-IS-FRIENDLY, implementer (slice link 9, Phaselock). Number 60 was given by the
hub.
- **The branch.** It is `origin/main` 4af068bf, which holds #437 (`fix/rust-sm-play`) and the note (#440). It was
  written on `origin/research/rust-is-friendly` 9df0d6c1 and moved onto #437's head da88db8f, then onto main once
  both landed. The hub's carry-over changes #437's `register_allegiance` and `register_cast_opinion`.
- **The source.** NATIVE_IS_FRIENDLY.md (decision 2026-11-19-55), read only at 07177679 and at 9df0d6c1. The second
  changes only the live-check wording.
- **Clean room.** This lane did not read the natives and opened no `local/analysis/`, Ghidra or REA output.
- **No dependency, no CLI flag, no parsing change.** There was no real-game session; every rule is **UNVERIFIED** in
  the running game.

- **New module `ow_combat::hit::opinion`** (Off only):
  - **`Pawn.IsFriendly(Other)`:** true exactly when the pawn's own opinion of `Other` is Friendly. The opinion is the
    charm layer for a `WillowPawn` and its subclasses, the generic rule for any other pawn.
  - **`WillowPawn.GetOpinion(Other)`:** its charmed child is Enemy; its charmed parent is Friendly. When it has no
    charmed parent, a `WillowPawn` other whose charmed parent is friendly to it is Enemy. Otherwise the generic rule.
    That last question cannot recurse further: it asks about a pawn with no charmed parent.
  - **`Pawn.GetOpinion(Other)`**, the generic rule:
    - no own allegiance: Neutral, before the other side is read;
    - the other side B: a `WillowPawn`'s `Allegiance`; a `PawnAllegiance` itself; none for a controller (not read
      through its pawn) and for None;
    - then `SelfOpinion`, the first `MyOpinions` match, the first `OtherOpinions` match, and the forced or default
      opinion;
    - allegiances are compared as objects, and no health is read.
  - **`NotYet { NotRead }`**, decided before any answer (nothing here changes state):
    - an other that is a pawn but not a `WillowPawn`: what the targetable interface reports is read only for link 9's
      classes;
    - an object that is none of a pawn, a `PawnAllegiance` and a controller;
    - a pawn whose charm field points at itself (UNVERIFIED in the note).

    A class lacking a field the rules read is a data error.
  - **`IsEnemy` and `IsNeutral`** keep the groundwork rule in both modes: the note does not read `WillowAIPawn`'s
    replacements.
- **One registration** (the hub's carry-over from #437's delta). `ow_combat::hit::register_allegiance` is the only
  path, used both by the hit pack and by the action skill pack's `register_cast_opinion` (the cast's worlds):
  - in Off it binds this module's three natives first, so no world in Off can get the groundwork boolean for
    `GetOpinion` or `IsFriendly`, whatever its pack order;
  - in Reference it binds that boolean, the C++ stand-in, labelled;
  - `WillowPawn.GetOpinion` stays unbound in Reference.

  #437's Off `NotYet` for `IsFriendly` is replaced by the rule.
- **Verified (automated, local, pc-hummer).**
  - **Gates:** workspace 2,065 passed / 0 failed / 45 ignored; `ow-combat` 153 / 0 / 2; `ow-skills` 96 / 0 / 1;
    `crate_edges_test.py` OK; decisions check OK (against the research base, before the move to the #437 stack: this file and #437's 52 added); leak gate clean.
  - **Synthetic tests** (`ow_package::testkit` packages, invented values):
    - `ow-combat` `hit::opinion`, 5 tests: the generic rule step by step (object comparison included); the unread
      other sides; the charm layer with its precedence and step 3; `IsFriendly` by class; the hit pack's stand-ins
      in Reference only.
    - `ow-skills`, 1 new test: `IsFriendly` is one binding in both pack orders and both modes. A controller is false
      in Off and true through the Reference stand-in. #437's cast-script test now meets the note's `NotYet` for its
      plain-pawn targets.
    - `hit::tests::the_opinion_follows_the_four_steps` now runs in Reference, because it tests the stand-in. The
      shared combat fixture was not touched.
  - **Installed, Off** (this branch before and after, and with #435 merged locally, never pushed):
    - `--slice-gate` strict: rows byte-identical, 0 / 14 / 0 on this base. Link 9 stops at `GetSMComponent`'s shared
      component, before and after, and also with #435 merged: the gate's cast makes its pawns from class defaults.
    - `--combat-run` (COMBAT_DEATH.md's hit): identical but for `elapsed_ms`.
    - The W6-SKILLS installed cast with #435 merged:
      - before: `Pawn.IsFriendly` unbound;
      - with this lane, without #437: past it, at the unbound `Pawn.IsAliveAndWell()`;
      - with #437 too (this branch): past both, at the unbound `AttributeDefinition.GetValue(object,object,object)`,
        the `IsPhaselockedAttribute` read.
- **Open:** the note's three questions (the installed allegiances; what sets the charm fields; death and the fields);
  `WillowAIPawn`'s `IsEnemy` / `IsNeutral`; `AttributeDefinition.GetValue`, the cast's next stop.
