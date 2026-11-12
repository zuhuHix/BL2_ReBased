## 2026-09-26: Menus for Maya: trace Gearbox's UI code in the real game, then build to the trace

The maintainer asked to "run Gearbox's code and base off that" to get Maya's
menus working faster. Running it inside OpenWillow needs the Phase 2 VM and
the natives the menu code reaches, which is months away. Reading the scripts
and porting them is forbidden (docs/LEGAL.md, "UI movies" and clean-room rule
3). The approved route is observation: run the real game with the community
mod SDK the player already has installed, record everything the UI code does,
and build host controllers that reproduce the recording against the same
movies. The traces also become golden files for the VM later.

- The SDK is in the player's install: unrealsdk v3.2.0, pyunrealsdk v1.10.0,
  mod manager 3.8, recorded in THIRD_PARTY.md with maintainer approval. Its
  log shows ProcessEvent and CallFunction detoured, so script calls to native
  functions are hookable too.
- `tools/sdk_trace/openwillow_uitrace` is our own logging-only mod, installed
  with maintainer approval into the game's `sdk_mods` folder. When enabled
  from the mod menu, it hooks every function declared on classes inheriting
  `GFxMoviePlayer` or `GFxObject`. That covers the controllers, the Scaleform
  bridge and the `ext*` callbacks. It writes JSONL to `local/ui/traces`, the
  path taken from `trace_dir.txt` beside the installed mod. After 200 detailed
  records per function it only counts, so per-frame HUD traffic stays bounded.
- `tools/sdk_trace/summarize.py` reports classes, bridge calls and callbacks,
  and prints a per-class timeline. Checked on a synthetic trace only.

Subsequent real-game runs produced two local traces: 2,919 functions hooked,
60,200 and 37,502 JSONL records over 90.4 and 40.2 seconds, respectively,
with zero trace errors. Performance cost while enabled is not measured.
Controllers built from a trace must still be checked side by side with the
real game.

