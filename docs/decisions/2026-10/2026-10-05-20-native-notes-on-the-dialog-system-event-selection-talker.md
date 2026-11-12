## 2026-10-05: native notes on the dialog system: event selection, talker, priority, line end (UNVERIFIED)

AI-assisted (Claude), analyst lane G6, Ghidra; own-words note `docs/verification/NATIVE_DIALOG.md`, no listing text.
`Behavior_TriggerDialogEvent` selects Out (id 0) on its first run and Finished (id 1) only once the event's talk act is no
longer live (polled every 0.1 s; at once when no line starts; immediate mode selects both), so the host stand-in that
selects both at once is wrong in timing. Event selection within a group, template-event wiring (a data-layout check on
the Fire group agrees with SLICE_AUDIO_CHAIN's pairing), talker resolution (talker variable, instigator, random
`TalkData` by name tag; echo callers), the priority rule (index in the globals' `Priorities`, tracked-mission floors), and
the line end (Wwise playing id stops, then `OutputDelay`). Predicts that the Fire mission's first objective set starts
after Marcus's first line ends, not at the kickoff. All UNVERIFIED in game.

