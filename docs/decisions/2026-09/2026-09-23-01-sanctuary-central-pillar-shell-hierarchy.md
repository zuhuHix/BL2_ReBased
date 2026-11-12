## 2026-09-23: Sanctuary central-pillar shell hierarchy

The Sanctuary pillar's UModel build 1590 MD5 mesh names the actual 17-bone
hierarchy, but its `Open.md5anim` hierarchy lists the child tracks under Root.
The animation frame values for those tracks are local to the **mesh** parents:
composing them through the mesh hierarchy returns all 17 frame-0 joints to
their mesh bind positions within 0.11 cm, with quaternion alignment above
0.999. Composing through the animation header instead placed the front and
upper TopShell plates thousands of centimetres below the cap.

`prepare_sanctuary_pillar.py` now uses the mesh hierarchy for the bounded
17-joint `Open` bake and rejects a changed hierarchy or a frame-0 bind-pose
mismatch. The lower Base and Tile OBJ sections remain byte-identical; only
the Top and Topinner sections change. UModel remains the external mesh and
animation payload source. The project still owns package identity, placement,
material selection and the frame-0 verification. Corrected UE5 host captures
are under ignored `local/center-pillar/`; original-game visual parity and
runtime animation remain **UNVERIFIED** pending paired capture review.

