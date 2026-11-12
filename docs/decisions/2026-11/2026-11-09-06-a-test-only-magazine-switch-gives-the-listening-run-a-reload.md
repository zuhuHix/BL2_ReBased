## 2026-11-09: A test-only magazine switch gives the listening run a reload (slice audio lane)

AI-assisted (Claude Haiku 5.5). The slice gate reloads only when a shot finds the magazine empty, and only in editor builds, and nothing in the gate empties the lent pistol's magazine, so the listening run had no reload. Added `-owslicegatemag=<n>` (test only; absent or 0 = the gate as before): before the first ShootDummy shot it sets the held gun's magazine to n rounds, once, leaving the reserve alone, and logs one line. `tools/record_audio_listen.ps1` passes `-owslicegatemag=1` by default (`-Mag`, 0 = off). `tools/listen_report.py` states the fixture and flags a fixture run with no logged reload cue. `UnrealEditor.exe -game` runs the editor target, so the reload press (`WITH_EDITOR`) is reached in the listening run.

- **Verified (automated):** the synthetic listen-report suite (18 tests, including the fixture cases); the UE module build through the slot; ctest.
- **Not verified:** the switch in engine (the log line, the reload cues in a run). It stays UNVERIFIED until a listening run shows them. Reload timing and the game's reload sequence stay UNVERIFIED as in the 2026-11-09 equip and reload entry.
- **Record:** `docs/verification/SLICE_AUDIO_CHAIN.md` section 5a.
