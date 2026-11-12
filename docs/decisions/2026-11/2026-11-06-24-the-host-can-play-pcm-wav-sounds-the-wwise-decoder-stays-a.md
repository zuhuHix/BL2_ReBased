## 2026-11-06: the host can play PCM WAV sounds; the Wwise decoder stays a maintainer decision (slice audio lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 5.

- **Added:** `OpenWillowAudio` plays role -> WAV through UE's `USoundWaveProcedural`, and `tools/prepare_slice_audio.py` writes the role map after a format and length check.
  One hook line each at the shot, the successful Phaselock lift and an item pickup; hits use the existing `OnHit` delegate. No new module, library or tool. Off unless `-owaudioplay=` is given.
- **Not decided:** the decoder. Nothing recorded in `THIRD_PARTY.md` decodes Wwise Vorbis, so the options went to the maintainer and no decoder was downloaded. Until then no game sound plays.
- **Evidence:** `fire` comes from the lent pistol's data and is played for every gun for now. `hit`, `phaselock_cast` and `pickup` have no sound assigned: they stay unresolved rather than guessed.
- **Not verified:** nothing has been heard. The synthetic tone test can only show the mixer pulled the bytes. No parsing behaviour changed.

