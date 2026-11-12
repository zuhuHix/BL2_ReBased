## 2026-10-22: Weapon paint fallback fix and first-person parity rounds for four slice guns (lane E)

AI-assisted (Claude, with Codex sub-agents). `host/ue5/import_weapon_items.py`, `tools/weapon_item_materials.py`, `OpenWillowWalker.*`,
`OpenWillowShotFx.*`, `tools/weapon_view_model.py`, `tools/weapon_reload_anims.py`, `tools/weapon_capture_items.py`,
`host/ue5/import_weapon_reload_anims.py`, `host/ue5/import_weapon_fx.py`, `tools/real_game/scripts/weapon_parity_probe.py`.

- **White Wild Rifle (bounded item set):** the item importer gave every gun except the Infinity the neutral stand-in, even when a prepared
  per-item paint material existed. It now uses `/Game/OpenWillow/Weapons/Paint/<id>/M_OW_PaintApprox` when present and warns with the missing
  path otherwise (synthetic tests in `tests/weapon_paint_test.py`). The Inspect preview and the held weapon load the same mesh, so both are fixed;
  confirmed by a UE preview render, not yet by an Inspect capture.
- **Live probe first:** one SDK session read the four guns' type fields and captured idle, ADS, burst and reload frames (`weapon_parity_probe.py`,
  output under ignored `local/realgame/laneE/`). A live skill effect doubled fire rate and halved reload on held guns, so the host keeps card base
  values; base values read live equal the cards. Values used and their sources: `docs/verification/WEAPON_VISUALS.md` sections 11-15.
- **Host:** hold-to-aim on the right mouse (live zoom FOV and zoom time, eye-socket alignment fitted to the real frames), per-type weapon and arms
  reload clips chosen by the fast-reload threshold and scaled to the card reload time, a host-drawn flame flash at the cooked muzzle socket, brass
  casings, a procedural SMG scope, data-driven kick. Rule versus fit is labelled per item; every fit and the reload-rate rule are `UNVERIFIED`.
- **Visual review:** critic 5.8, 5.5, 6.3, 6.5/10 over four rounds; 8/10 not reached. Open items (SMG flash drawn at the rear, suspected Y mirror of
  `Root`-parented sockets; SMG hip size; reload poses; ADS widths; metal tint; sound) in `WEAPON_VISUALS.md` section 15.
- **Checks:** CTest 10/10; packages 9/9; Python tests 53/53; JS tests 2/2; quest suite 79/0 and resume 11/0; inventory suite 49 / 0 / 0 / 0
  (bounded item set).
- `ReloadCompletePercent` now drives the one-time magazine transfer and keeps the missing-field `1.0` fallback for the previous full-duration
  refill; the host check covered cancellations before and after that point.
- Successful Phaselock lifts cancel reload before the cast action, and arms cleanup only clears an active reload clip; the host check passed with a
  test-only slower cast rate so the clip spans the former reload end. Original-game notes confirm only successful-cast abort behavior.

