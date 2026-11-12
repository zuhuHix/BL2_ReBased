## 2026-11-06: the Phaselock stock colours' source, GammaOut removed (maya lane)

AI-assisted (Claude), maintainer's decision. Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "The stock colour constants".

- **Option C:** `tools/build_phaselock_fx_constants.py` rebuilds the orb's blue and the tattoo glow from regenerable local data; the bubble twirls' violet stays
  hand-kept in the ignored `fx_constants.json`, because it exists only as a literal in the compiled shader. Without an existing value the tool stops. A tracked shader-map reader (option B) comes after the slice.
- **`GammaOut` removed:** round 24 showed no gain.
- **Probe:** the live read also records the target's mesh scale, draw scale and bounds radius; its rotation stays the priority.
- **Live read (one cast, one pawn):** the lifted target faces Maya, its rotation does not change, and its scale is 1.0. The default facing stays +180 (hub decision). The "too large" gap is neither scale nor a bubble value; its cause is open.

