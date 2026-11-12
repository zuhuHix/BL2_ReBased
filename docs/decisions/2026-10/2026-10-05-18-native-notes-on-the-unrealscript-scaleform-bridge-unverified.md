## 2026-10-05: native notes on the UnrealScript–Scaleform bridge (UNVERIFIED)

AI-assisted (Claude), analyst lane G5, Ghidra; own-words note `docs/verification/NATIVE_GFX_BRIDGE.md`, no listing text.
The contract the host's Ruffle adapter must meet: the `ActionScript*` family forwards the calling script function's own
parameters and converts the result to its return type (conversion tables in the note; missing movie or path is a silent
no-op); `Invoke`/`SetVariable*`/`GetVariable*` conversions by `ASType`; ActionScript→script calls through
`ExternalInterface.call` (named script function on the player's `ExternalInterface` object), `SetFunction` bindings,
`fscommand` and the CLIK widget hooks (the VM must be re-entrant there); wrapper lifetime and `Close` event order; markup
translation in `SetText`; `PlayUISound`; `FocusOn` (presentation only); `QuestAcceptGFxMovie.UpdateMissionTextList` (the
accept screen's category headers). Scaleform's own semantics (paths, sticky variables, conversions) were not read. All
UNVERIFIED in game.

