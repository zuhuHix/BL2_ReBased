## 2026-09-21: sky approximation from the dome's named inputs, and an opt-in outer hull

The accepted `Sky_Dome` placement previously painted the raw
`Sky_TransitionBL2Default_Dif` strip across the dome with UV0: every
time-of-day column wrapped once around the azimuth. The master graph
`Mat_SkyTimeOfDay_Master` is stripped from the cooked package, so this entry
does not decode it. Instead `prepare_level.py` reads the named inputs that
survive along the instance chain (`Transition_Track`, `clouds`, `Masks`,
`Time_of_Day`, `sky_brightness`, `Sun_spot_brightness`, `cloud_cap_opacity`,
`p_CouldBrightness`, `Horizion_track_color_multiplier`) into a
`sky_approximation` record, and the UE5 importer builds one fixed graph from
them: visible = lerp(strip(column, dome V) × sky_brightness,
strip(column, 0.95) × sky_brightness × cloud_brightness,
saturate(clouds.R × cloud_cap_opacity)). Method name
`sky_time_of_day_strip_v1`, status `partial_unverified`.

Two readings of `Time_of_Day = 170` were compared on the extracted strip:
as a pixel column (/256) it selects the blue daytime gradient; as degrees
(/360) it lands in a sun column and gives dusk hues. The pixel-column reading
is used and recorded as `UNVERIFIED` in the manifest; nothing in the package
says which the original shader does. The sun spot, `Masks` (stars and cap
gradient), the horizon color multiplier, cloud channels G/B, cloud motion,
time-of-day animation and Kismet control are listed as omitted. The ordinary
diffuse inference stays on Base Color as the fallback, and the host's blue
`OpenWillow_SkyFallback` sphere, which sits inside the dome and hid it, is
now only spawned when no accepted dome carries this record.

Separately, the `Sanctuary_Outer` hull (`Prop_Skybox.Meshes.SanctuarySky` and
its two antennas) is placed with masked `_Teleported` phase-in overrides whose
graphs Material v1 cannot recover, so it imported invisible. With the new
opt-in `--outer-shell` flag the preparer drops only those `_Teleported`
overrides whose mesh-default material resolved a diffuse and records each
replacement; every other override is kept. This is a substitution, not the
placed material, and whether the running game shows `_Outer` or `_Land` is
Kismet state that is still not interpreted. Default behaviour is unchanged.
Record: docs/verification/NATIVE_SKY_APPROXIMATION.md.

The editor fly-through after this slice added two facts. First, the hull's
tower sits off-centre and above the town's own; the actor transform matches
the game dump and the vertices match umodel, and the sublevel's Kismet shows
why: `SeqAct_Interp_0` (`SanctuaryLiftoff`) binds the hull to a
`RelativeToInitial` move track that jumps it 1.7 km south and lifts it
100–150 m before hiding it at 36 s. The placed transform may be a parking pose
for a cutscene prop, but its in-game position remains unverified; the mismatch
is left as-is and recorded. Status for
`StaticMeshComponent_393` / `InterpActor_29` is **decode verified, in-game
position unverified, observed off in editor**. The first-key import is an
opt-in experiment recorded in the verification note; the serialized placement
remains the default and no correction is committed. Second, two
`Blocking_Plane` placements on the horizon carried
`Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01`, a sibling of the
already-hidden `Mat_CloudLayer_Light`, and tiled a dust sprite as yellow/black
stripes; the hide rule now names both instances and `refresh_materials.py`
re-evaluates it.

