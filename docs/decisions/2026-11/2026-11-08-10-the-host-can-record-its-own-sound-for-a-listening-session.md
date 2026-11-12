## 2026-11-08: The host can record its own sound for a listening session (slice audio lane)

AI-assisted (Claude). The maintainer asked to hear our build's decoded sounds with a step-by-step guide. Added test-only switches: `-owrecordaudio=<wav>`
records the main submix through UE's audio mixer recorder (engine `AudioMixer` module; no external tool), and `-owslicegatepause=<s>` waits before each slice
gate step so a step's sounds stand apart (0, the default, leaves the gate as it was; the world keeps running during the wait). While recording, the host keeps submixes rendering when silent (`au.NeverDisableSubmixes`) and plays at full volume without
window focus; without both, two runs recorded nothing. It puts both back when the recording stops. Any graceful end stops the recording and waits for the
file; a crash loses it (out of scope). Driver
`tools/record_audio_listen.ps1`, report `tools/listen_report.py` (synthetic test). No parsing change. A recording measures what the host sent to the mixer;
whether any sound is right is for the maintainer's ears and stays UNVERIFIED until they report. Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 6.

