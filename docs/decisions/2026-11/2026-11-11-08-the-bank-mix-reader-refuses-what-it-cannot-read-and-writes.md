## 2026-11-11: The bank mix reader refuses what it cannot read and writes null, never a guessed gain (audio mix lane, PR #217 review 1)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 7.

- **Reader.** Records are read in order by the fitted layout (effect block: a bypass byte and 7 bytes per effect, fitted on all 89 objects with effects; a source plugin's parameter block). The reader no longer searches for a plausible bus and parent pair. It refuses an unsupported layout, a parent or bus that does not resolve, a bundle past its record, a volume that is not finite or lies outside -96.3..+12 dB, and a parent or bus cycle.
- **Switch.** Only the perspective switch (the `firstperson` / `thirdperson` pair) filters chains. Rejected chains never come back, and an action wholly on the other perspective adds nothing. A switch list that does not fit, or several levels with nothing to choose between them, leaves the source unresolved: no more "highest wins".
- **Gains.** An unresolved file, and every file when no game data is given, gets `null` (the host warns and plays it at 0 dB). A clamp is noted. A `--game` path that is not a folder is an error. The listening report pairs each gain with its own file before the median, and shows "no game prediction" for a role without a game gain.
- **Host self-test.** Five rows: 0 dB is 1.0, -6 dB about 0.501, NaN and +13 dB refused, the mono-to-stereo byte layout. `check_audio.py` expects 27 rows.
- **Verified.** On this install every slice gain is unchanged. Python tests (synthetic): prepare 62/62, listen report 29/29, check_audio 14/14. **UNVERIFIED:** the C++ is still not compiled or run, and the effect entry's 7 bytes are fitted, not read as fields.

