## 2026-09-23: Maya's animated first-person arms and eye height on the Sanctuary walker

UModel can export `AnimSet` animation as glTF only from its viewer
(`glTF animation could be exported from mesh viewer only`), so `Hands_Siren`
and `1st_Person_Pistol` were exported as MD5.
`tools/prepare_character_anims.py` converts six clips (Idle, Run_F, Sprint,
Jump_Start, Jump_Idle, Jump_End; 30 fps) into bone tracks for the UE skeleton
imported from the same mesh's glTF. The MD5-to-UE map is fitted from the 47
bind positions and must be a pure Y mirror (it is). Each bone's axis
convention is absorbed by comparing the MD5 and UE bind poses.
`host/ue5/import_character_anims.py` writes the tracks as AnimSequences through
the editor's animation data controller.

A per-frame root correction keeps the arms skeleton's `Camera` bone at the
player camera. In all six clips that bone does not move relative to the root
(range 0.1 cm or less), so this is equivalent to a fixed attachment. That BL2
views from this bone is UNVERIFIED. Check: the right hand's camera-space
position from forward kinematics of the written tracks equals a direct MD5
computation (47, 17, -21.5 cm, frames 0/30/60 of Idle). At the converted FOV
this projects to about 64% across the screen, which matches the pistol hand
in a maintainer capture outside Scooter's. The earlier static OBJ bake of the
same frame rendered the hand near 85-90% across, so that render was wrong and
the static path has been removed; `prepare_character_pose.py` remains as the
MD5 reader and a single-frame OBJ baker.

The walker's `-owmaya` mode plays Idle, Run_F (horizontal speed over 50),
Jump_Idle (falling) and Jump_End (landing) with hard cuts. BL2's AnimTree
blending is not reproduced. The clips themselves are subtle: the hand moves
about 2 cm running and 5 cm jumping. The strong running gun bob seen in the
game is assumed to come from native weapon code, which is not implemented.

Eye height and collision come from `GD_Siren_Streaming.Pawn_Siren`: the
`CylinderComponent` (properties at offset 8, not 4) gives CollisionRadius 42
and CollisionHeight 80. The pawn gives BaseEyeHeight 70 and a serialized
EyeHeight of 77. `-owmaya` uses a 42/80 capsule and puts the camera 70 above
its centre (150 cm standing eye). Choosing BaseEyeHeight over EyeHeight is
UE3 behaviour assumed, not checked in the game.

FOV: `DefaultEngine.ini` sets `[Engine.LocalPlayer]
AspectRatioAxisConstraint=AspectRatio_MaintainYFOV`. A BL2 FOV value is
treated as horizontal at 4:3 and converted to UE's horizontal FOV at the
actual aspect. `-owfov=` defaults to 90 (106 degrees at 16:9). This matched
the Scooter's capture by eye; the player's actual setting was not read.

Automated: host build; import commandlets exit 0; `OpenWillow.Walking` passes
(without `-owmaya`). Runtime: an unattended `-owautowalk` run logged the state
sequence Idle, Jump_Idle, Jump_End, Run_F, then Run_F/Idle alternating once
per 6 s lap as the pawn bumped geometry. Visual: idle and walking captures
compared by eye only. No gun is held.

