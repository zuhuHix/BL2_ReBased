## 2026-11-09: Missing UModel exports stop the Phaselock target import with a message, not an Unreal assertion

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Missing exports stop the target import instead of crashing Unreal".

- **Cause:** a worktree without the UModel exports wrote "None" for every glTF; the attach step's import asserted (exit 3), and the zone step kept plain materials silently.
- **Change:** the job writer refuses an incomplete job; the zone, attach and mesh editor steps check every file they will import before they change anything (`tools/editor_inputs.py`). No parsing or host change.
- **Checked:** synthetic tests of the job check and of the three editor steps against a stub `unreal` (no import, delete or save on a bad job). Not run in the editor.

