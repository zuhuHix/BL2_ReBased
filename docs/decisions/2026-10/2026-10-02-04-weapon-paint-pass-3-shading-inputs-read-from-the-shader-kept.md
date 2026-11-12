## 2026-10-02: weapon paint pass 3: shading inputs read from the shader, kept behind a flag

AI-assisted. Importer and notes only. The compiled base and light passes of Master_Gun multiply the material colour
by 0.4 (a factor most other shaders in the cache do not have) and take no specular from the material (the light
pass's specular is the engine override only). Mapped to UE5 that is base colour 0.4 x colour, metallic 0, specular
0, roughness 1 (`USE_SHADER_SHADING` in `host/ue5/import_weapon_paint.py`, UNVERIFIED). With the host's uncalibrated
Sanctuary lighting those inputs render the guns 4-7x darker than the reference screenshots, and the red-down/blue-up
tint measured on the stills (about x0.8-0.9 red, x1.1-1.27 blue against the unlit albedo) is unchanged by them, so the
tint comes from the host scene lighting. Maintainer-facing choice made by the orchestrator: the importer keeps the
pass-2 host stand-in (scale 1, metallic 0.35, roughness 0.55) until the scene lighting is calibrated; the six guns were
re-imported with it (0 errors).

