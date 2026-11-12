## 2026-09-30: Scaleform 3D projection probe

AI-assisted developer-only benchmark, no menu runtime change. Autodesk's
primary 3D guide documents AS2 _z/_xrotation/_yrotation/_matrix3d/_perspfov as
Scaleform extensions (https://help.autodesk.com/cloudhelp/ENU/Scaleform-Help/scaleform_help/3di.html).
The current Ruffle 0.7.0-nightly.2026.9.26 bridge was tested with original
synthetic 100x100 geometry, not game art. `probe_scaleform_3d.js` is never loaded
by the inventory page; paste/run it in the ready browser, then await
`owProbeScaleform3D()`. Four cases / 405.5 ms including four 100 ms waits,
with gfxExtensions=true. Flat, _yrotation=45 and _z=-300 all remain 100x100;
ordinary _rotation=45 produces 141.4x141.4 bounds. Three cases execute but two
3D mutations have no rendered effect; ordinary rotation is the positive
control. The temporary clip is removed in finally. No output duplicates,
exports or new external tools. A first attempt to load the probe by HTTP failed;
executing its repository source through preview_evaluate succeeded.

Both existing SDK traces contain Get/SetDisplayInfo observations, but zero
nonzero Z/XRotation/YRotation SetDisplayInfo calls. Thus they do not establish
the stock projection values. This confirms a renderer support gap rather than
proving an intended transform. Do not compensate by inventing 3D parameter
values or claim the current affine presentation is 1:1. A projection adapter
requires a separate rendering/interaction benchmark and original transform
observations before replacing the current path. Other parity work can continue.

Verification: developer probe above; CTest 6/6 (8.64 s), all nine package checks
match, JS syntax/diff checks pass. No new engine run (runtime unchanged);
previous 46-action run is retained as earlier evidence, not this probe's scope.

