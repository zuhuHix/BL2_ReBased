## 2026-11-16: Rust mission dialog wired through the tracker and the bridge

AI-assisted (Claude Code). W5-DIALOG landed as 413eafd (PR #333, decision 2026-11-15-32), and lane W6-MISSION wires it
into the mission tracker and the script bridge (branch `fix/rust-mission-dialog-wiring`). No parsing behaviour changes;
every dialog rule keeps W5-DIALOG's labels (UNVERIFIED in the running game).

Edits: docs/decisions/2026-11/2026-11-15-34-rust-mission-tracker-script-bridge-and-mission-run-as.md

Decision 34 said the dialog was `NotYet` (the dialog behavior unhandled, the dialog steps and the director's dialog
refused) and gave the #334 counts. It now carries an addendum, "Addendum (2026-11-16): the dialog wired", with:
- the wiring: one `Rc<RefCell<DialogSystem>>` per mission, the handler on the mission's own provider, shared with the
  bridge's `TriggerEvent`;
- the tests;
- the new counts;
- what stays open.

The hub chose this route: a new file naming 34, as the decisions check requires.

- **Wiring.** `MissionSystem` owns one `Rc<RefCell<DialogSystem>>` over the mission's package:
  - `dialog::handler(rc)` runs `GearboxFramework.Behavior_TriggerDialogEvent` on the mission's own provider;
  - the provider's time hook drives the dialog clock;
  - the status routine sets the tracked mission's floor;
  - every chosen line is a `Dialog` effect.

  The director's `GearboxDialogComponent.TriggerEvent` (bridge) runs on that same `Rc`. `--mission-run`'s dialog steps
  run as in the C++.
- **Fixture-only change (granted by the hub).** `tests/mission_script_test.py`'s toy Engine declares the class
  `AkEvent`, the class of the dialog scenario's `Ak_*` exports. Without it the Off-mode definition store reads a
  `TalkAkEvent` naming them as None. No value or assertion changed; the C++ binary passes the file as before.
- **Counts (automated, local, pc-hummer).**
  - `mission_script_test.py`, `--mission-run` commands byte-identical to the C++: 34 of 34 in Reference mode, 33 of 34
    in Off mode (scenario W, the lent weapon's unread condition, by design).
  - Installed Sanctuary missions:
    - Reference: tracker 73 of 77, bridge 1 of 77;
    - Off: tracker 58 of 77, bridge 1 of 77.

  The breakdown is in decision 34's addendum.
