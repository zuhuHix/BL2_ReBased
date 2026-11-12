## 2026-09-30: Stock inventory target, camera-space framing and native item inspection

AI-assisted. The maintainer selected stock inventory and default Maya as the
target; the custom appearance in the older screenshot is not the appearance
target. Existing UModel 1590 exports, local SWF conversion and Ruffle
0.7.0-nightly.2026.9.26 remain the external payload path. Project code owns
stable inventory identity, host validation, layout and input adaptation.

The menu display now rotates with the camera, including pitch, so looking
up/down before opening does not tilt the display copy out of its framing.
Distance 300 cm / ScreenX 0.79 fits in fresh captures after this correction;
key/rim 1.5/3, f-stop 16 and lighter backdrop grading expose the world through
the curved glass. Affine panel layout and native glass alpha are adjusted
against the stock screenshot. Native movie favorite/trash icons replace HTML
symbols and have explicit hit targets. Drag/drop uses stable IDs and the
existing authoritative equip/unequip path, rejecting wrong gear types and
locked slots. The host starts its new-session purse at zero; this is host
state, not a decoded BL2 save or economy implementation.

`prepare_inventory_gear.py` consumes existing SDK callback observations,
keeping interleaved card transactions separate and ending at SetHeight.
Benchmark: two ignored traces, 87 gear observations, 11 unique cards,
76 duplicates; four shields, four class mods, three relics, zero grenade mods;
0.287 s. Output: `local/inventory/observed_gear.json`. No failed input reads.
Unknown types and unfinished transactions are omitted (synthetic coverage).
No package identity, rolled parts or mesh is inferred. Observed stat icons and
Flash flavour formatting are preserved, including red text among white bonuses.
Formatting is passed only to the movie TextField, never browser HTML.

Inspect reuses the existing UE preview actor and imported recipe mesh rather
than a static PNG. The host resolves the current inventory instance, clamps
orbit pitch and rate-limits capture to 10 Hz. PNG frames stay in memory; close
destroys the actor. Gear without resolved visual identity is unavailable.
The render target uses isolated channel-2 lights and manual exposure.
[Epic's image utility API](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/FImageUtils)
supports render-target readback and PNG encoding; no third-party code was added.

Verified: UE 5.8 build; 32/32 in-engine actions with Slate keys and synthetic DOM
drag/pointer events; rotated Inspect captures; currency zero; yellow suit and
unclipped elbow. CTest 6/6, all nine package byte/count/export comparisons match,
weapon-stat tests 11/11 and gear-observation tests 4/4. The initial Inspect test
used an unmapped uppercase helper key and was corrected to lowercase before
the passing run. Lighting refinement is checked separately in the verification
record. All game-derived payloads/captures remain ignored.

UNVERIFIED: 1:1 parity, physical drag input, continuous Inspect latency, original
Inspect composition, BL2 material graphs, exact 3D panel projection, stock
focus/swap and sorting behaviour, compare-card placement, unresolved gear art,
and an independent critic re-score. The old 5.5/10 score remains historical.

