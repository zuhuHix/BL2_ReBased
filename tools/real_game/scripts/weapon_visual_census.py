"""Live visual census of the weapon parts, read from the running game (the RTSE way: find_all on loaded objects).

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1) with saves blocked. Our own code; the output is game
data and goes under RG_OUT (normally the ignored local/realgame/wpa), never into a tracked file. It reads:
- every GestaltSkeletalMeshDefinition: GestaltInfos (per LOD: fragment name, material index, first index, triangles),
  the gestalt skeletal mesh and its material slots;
- every WeaponPartDefinition: the gestalt fragment names, Material, MaterialVectorParameterValues, PartType;
- every WeaponTypeDefinition: the bones it hides;
- every MaterialInstanceConstant a material part names, with its parent chain and parameter values.
tools/weapon_part_census.py --live compares it with the cooked census.
"""
import enum
import json
from pathlib import Path

import unrealsdk


def path(value):
    try:
        return value._path_name() if value is not None else None
    except Exception:
        return None


def deep(value, depth=0):
    if isinstance(value, enum.Enum):
        return value.name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if hasattr(value, "_path_name") and hasattr(value, "Class"):
        return path(value)
    if depth > 6:
        return repr(value)[:120]
    try:
        props = value._type._properties()
        return {p.Name: deep(getattr(value, p.Name), depth + 1) for p in props}
    except Exception:
        pass
    try:
        return [deep(v, depth + 1) for v in list(value)]
    except Exception:
        return repr(value)[:120]


def field(obj, name):
    try:
        return deep(getattr(obj, name))
    except Exception:
        return None


def gestalts():
    out = {}
    for definition in unrealsdk.find_all("GestaltSkeletalMeshDefinition", exact=False):
        if definition.Name.startswith("Default__"):
            continue
        infos = []
        for info in list(definition.GestaltInfos):
            infos.append([{"name": str(p.SkeletalMeshFragmentName), "material": int(p.MaterialIndex),
                           "first_index": int(p.FirstIndex), "triangles": int(p.NumPrimitives)} for p in list(info.Parts)])
        mesh = getattr(definition, "GestaltSkeletalMesh", None)
        materials = []
        try:
            materials = [path(m) for m in list(mesh.Materials)] if mesh is not None else []
        except Exception:
            pass
        out[path(definition)] = {"lods": infos, "mesh": path(mesh), "mesh_materials": materials}
    return out


def mic_record(mic):
    chain, current = [], mic
    while current is not None and len(chain) < 8:
        record = {"path": path(current), "class": current.Class.Name}
        for name in ("VectorParameterValues", "ScalarParameterValues", "TextureParameterValues"):
            record[name] = field(current, name)
        chain.append(record)
        current = getattr(current, "Parent", None)
    return chain


def parts_types_mics():
    parts, mics = {}, {}
    for part in unrealsdk.find_all("WeaponPartDefinition", exact=False):
        if part.Name.startswith("Default__"):
            continue
        names = []
        try:
            names = [str(n) for n in list(part.AdditionalGestaltModeSkeletalMeshNames)]
        except Exception:
            pass
        material = getattr(part, "Material", None)
        parts[path(part)] = {"gestalt": str(part.GestaltModeSkeletalMeshName), "additional": names,
                             "material": path(material), "part_type": field(part, "PartType"),
                             "vectors": field(part, "MaterialVectorParameterValues")}
        if material is not None and path(material) not in mics:
            mics[path(material)] = mic_record(material)
    types = {}
    for kind in unrealsdk.find_all("WeaponTypeDefinition", exact=False):
        if kind.Name.startswith("Default__"):
            continue
        types[path(kind)] = {"bones_to_hide": [field(kind, "BoneToHideOnMesh"), field(kind, "AdditionalBoneToHideOnMesh")],
                             "material_class": path(getattr(kind, "MaterialClass", None))}
    return parts, types, mics


target = Path(RG_OUT)  # noqa: F821 (set by Invoke-GamePyFile)
target.mkdir(parents=True, exist_ok=True)
census = {"gestalts": gestalts()}
census["parts"], census["weapon_types"], census["mics"] = parts_types_mics()
(target / "live_visual_census.json").write_text(json.dumps(census, sort_keys=True), encoding="utf-8")
print(json.dumps({key: len(value) for key, value in census.items()}))
