## 2026-09-29: Maya's inventory preview uses the game's Idle_Inventory clip and an ink outline

AI-assisted. UModel 1590 MD5 export of `GD_Siren_Streaming_SF` `Skel_SirenBody`
and AnimSets `Base_Siren`, `Unarmed_Siren`, `Rifle_Siren` (0.9 s, 68 clips, no
failures). `Base_Siren.Idle_Inventory` (271 frames, 30 fps) is converted by
`tools/prepare_character_anims.py --anchor none` (new option: skips the
first-person camera correction; the MD5-to-UE Y-mirror check still passes) and
imported by `import_character_anims.py` onto the body skeleton
(`OPENWILLOW_CHARACTER_MESH` / `_ANIM_FOLDER`). The head follows through leader
pose (same bone names). `host/ue5/import_character_menu_look.py` adds an
inverted-hull outline material and Specular 0.15 / Roughness 0.85 on
`M_OW_Character`; both are art-direction approximations, UNVERIFIED against
BL2's stripped `Master_Player` shaders.

Visual (engine capture at DistanceCm 345): hand-on-hip idle on the same side as
the 1920x1080 reference, which also supports the earlier UNVERIFIED body
handedness. The final framing (DistanceCm 300, ScreenX 0.79) is built but its
capture run stalled at editor start-up, so it is not visually verified. The
backdrop post-process still darkens Maya with the world (suit reads brown);
that belongs to the backdrop pass. No critic re-score of this change.

