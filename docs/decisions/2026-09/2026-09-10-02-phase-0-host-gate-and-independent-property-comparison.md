## 2026-09-10: Phase 0 host gate and independent property comparison

The UE5.8.2 host probe rendered the extracted `Env_Ash.Mesh.Ash_Road01` with
its verified `p_Diffuse` texture in `/Game/Phase0/Phase0`. The first visual
gate screenshot is user-verified. The probe remains a diagnostic import spike:
its material is two-sided and unlit/emissive so asset visibility is independent
of lighting-bake and winding issues.

The installed-package census completed with 2,010 candidate files, including
two identified UHD texture sidecars, and read all 2,008 UE packages. It found
4,751,329 serialized exports. These are serialized copies rather than unique
assets. A UE Viewer/umodel plausibility comparison is still pending because
the reference executable is not installed locally.

The BLCMM Object Explorer dump for
`WeaponPartDefinition GD_Gladiolus_Weapons.AssaultRifle.AR_Barrel_Jakobs_Sawbar`
matches the C++ reader on the decoded property names and values, including the
three attribute effects, `WP_Barrel`, shell-casing settings, gestalt mesh name,
slot upgrades and monetary-value reference. The reader consumes 1,551 bytes
with zero trailing bytes in all three installed copies: `Gladiolus_Startup_SF`
export 1009, `Lobelia_Startup_SF` export 1020, and
`TestingZone_Combat` export 42374.

One discrepancy remains explicitly unresolved: BLCMM prints
`BehaviorProviderDefinition=None`, while the serialized stream contains a
reference to `...AR_Barrel_Jakobs_Sawbar.BehaviorProviderDefinition_13`.
This may reflect BLCMM's reference-dump/default presentation of a generated
subobject, but it is not treated as a confirmed match until that representation
is checked independently.

