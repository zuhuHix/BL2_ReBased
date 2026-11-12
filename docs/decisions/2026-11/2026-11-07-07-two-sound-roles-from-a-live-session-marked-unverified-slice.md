## 2026-11-07: two sound roles from a live session, marked UNVERIFIED (slice audio lane)

AI-assisted (Claude). Record: `docs/verification/SLICE_AUDIO_CHAIN.md` section 5; the session is in `docs/verification/SLICE_AUDIO_LIVE.md` on `feature/slice-audio-capture`.

- **Added:** manifest entries `role:phaselock_cast` and `role:hit`, and the prepare tool maps them. `pickup` stays unresolved: the session did not exercise it. `fire` still comes from the lent pistol's data, because the session's hooks saw no fire event.
- **Limit:** `hit` is a surface sound, not shown to be the enemy, with 96 sources. The host picks at random, so a hit can sound like any surface until the switch value is read.
- **Not stock:** one session on a modded install, Dialog Skipper included. No role here comes from a dialog event.
- **Not verified:** nothing has been heard, and no game sound can play until a decoder is approved. No parsing behaviour changed.
