## 2026-11-07: vgmstream r2168 decodes the game's audio, as an external tool (slice audio lane)

Maintainer decision; AI-assisted (Claude). Provenance: `THIRD_PARTY.md`. Benchmark: `docs/verification/SLICE_AUDIO_CHAIN.md` section 3.

- **Added:** `tools/decode_slice_audio.py` drives `vgmstream-cli` from an explicit path or `OPENWILLOW_VGMSTREAM`, writing PCM WAV to ignored `local/`. It keeps a file only after the host's parser rules and a length check, and refuses a folder elsewhere in the repository. Nothing links or vendors the tool, and the script never downloads it. `tools/test_slice_gate.ps1` gains an optional `-AudioPlay`.
- **Measured here:** 153 of 153 slice sources and 30 of 30 others decoded, with frame counts equal to the `.wem` headers on the 153, none silent.
- **Not verified:** nobody has listened. Length and signal do not show that a sound is right, and the roles behind them stay as recorded. No parsing behaviour changed.

