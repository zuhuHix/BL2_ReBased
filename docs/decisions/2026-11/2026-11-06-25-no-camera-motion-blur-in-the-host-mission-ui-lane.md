## 2026-11-06: no camera motion blur in the host (mission UI lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_VISUALS.md` section 1. `DefaultEngine.ini` sets `r.DefaultFeature.MotionBlur=False`; the engine's source documents 0 as "sets MotionBlurAmount to 0", and no
camera or volume in the host's code overrides that amount. The installed game's config has `MotionBlur=False`, a config read only. Host rendering only; no parsing change. In-engine frame: UNVERIFIED until the batch runs it.
