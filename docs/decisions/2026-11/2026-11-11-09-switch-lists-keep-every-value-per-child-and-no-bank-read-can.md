## 2026-11-11: Switch lists keep every value per child, and no bank read can stop the preparation (audio mix lane, PR #217 review 2)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 7.

- **Switch.** A switch container's child may be listed under several values; the reader keeps the set, so entry order no longer decides between `null` and a number. The child-list search is bounds-checked, and any read past a record becomes one unresolved source instead of a stopped tool. A record that names neither a parent nor an output bus is refused.
- **Report.** The level table shows "from k of n file(s)" next to a partial prediction in LISTEN.md, and its cue count counts every cue (with how many had a level).
- **Host self-test.** A `null` gain entry plays at 0 dB with a reason to log (`GainDbFromJson`); `check_audio.py` expects 28 rows.
- **Verified.** On this install every slice gain is unchanged. Python tests (synthetic): prepare 66/66, listen report 30/30, check_audio 14/14. **UNVERIFIED:** the C++ is still not compiled or run.

