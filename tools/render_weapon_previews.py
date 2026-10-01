"""Render transparent PNG weapon thumbnails from UModel's glTF exports.

Inputs (all ignored local/ payloads, nothing here is committed):
  local/items/<id>.gltf/.bin   filtered UModel gestalt mesh (one primitive)
  local/items/<id>.json        weapon recipe; its "material" names the cooked
                               MaterialInstanceConstant
  <materials>/**/<MIC>.props.txt + Texture2D/*.png
                               UModel's export of that MIC and its textures
                               (UModel build 1590, `-export -png`)

The cooked Master_Gun shader graph is stripped (UModel logs "Ignoring
Material3'Master_Gun' due to empty parameters"), so the paint model below is
the UNVERIFIED reading in tools/weapon_paint_model.py, recovered from the
compiled Master_Gun shaders in the install's shader cache. Without --paint the
base-Material defaults and the static channel choices are unknown, so missing
values fall back to neutral ones. Lighting here is a look-alike, not the game's.

Pure Python + Pillow (no numpy is installed): a small orthographic z-buffer
rasteriser, deferred per-pixel shading, supersampling, ink outlines.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import re
import struct
import subprocess
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter

from weapon_paint_model import PAINT_PARAMETERS, albedo, decal_uv, mask_uvs, pattern_uv

ROOT = Path(__file__).resolve().parents[1]
ITEMS = ROOT / "local" / "items"
OUTPUT = ROOT / "local" / "ui" / "run" / "previews"
UMODEL_ROOT = ROOT / "local" / "external" / "umodel"
COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
FORMATS = {5121: ("B", 1), 5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
ID_PATTERN = re.compile(r"[A-Za-z0-9_]+")

# Detail atlas channel per weapon class. UNVERIFIED inference from the atlas
# names (Launcher/Shotgun/Pistol and AssaultRifle/SubMachineGun/Sniper packed
# R/G/B); host/ue5/import_infinity_proxy.py makes the same guess for pistols.
DETAIL_CHANNELS = {
    ("Weap_LauncherShotgunPistol_Comp", "pistol"): 2,
    ("Weap_LauncherShotgunPistol_Comp", "shotgun"): 1,
    ("Weap_LauncherShotgunPistol_Comp", "launcher"): 0,
    ("Weap_AssaultSubSniper_Comp", "assaultrifle"): 0,
    ("Weap_AssaultSubSniper_Comp", "smg"): 1,
    ("Weap_AssaultSubSniper_Comp", "sniper"): 2,
}


# --------------------------------------------------------------------------
# glTF reading and skinning (bind pose of the exported skeleton)
# --------------------------------------------------------------------------
def read_accessor(doc, blob, accessor_index):
    accessor = doc["accessors"][accessor_index]
    view = doc["bufferViews"][accessor["bufferView"]]
    fmt, size = FORMATS[accessor["componentType"]]
    width = COMPONENTS[accessor["type"]]
    stride = view.get("byteStride", width * size)
    base = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    values = []
    for item in range(accessor["count"]):
        raw = struct.unpack_from("<" + fmt * width, blob, base + item * stride)
        if accessor.get("normalized") and accessor["componentType"] == 5121:
            raw = tuple(value / 255.0 for value in raw)
        values.append(raw)
    return values


def multiply(a, b):
    return [sum(a[k * 4 + row] * b[col * 4 + k] for k in range(4))
            for col in range(4) for row in range(4)]


def node_matrix(node):
    if "matrix" in node:
        return node["matrix"]
    x, y, z, w = node.get("rotation", (0, 0, 0, 1))
    sx, sy, sz = node.get("scale", (1, 1, 1))
    tx, ty, tz = node.get("translation", (0, 0, 0))
    return [
        (1-2*y*y-2*z*z)*sx, (2*x*y+2*z*w)*sx, (2*x*z-2*y*w)*sx, 0,
        (2*x*y-2*z*w)*sy, (1-2*x*x-2*z*z)*sy, (2*y*z+2*x*w)*sy, 0,
        (2*x*z+2*y*w)*sz, (2*y*z-2*x*w)*sz, (1-2*x*x-2*y*y)*sz, 0,
        tx, ty, tz, 1,
    ]


def point(matrix, xyz):
    x, y, z = xyz
    return (matrix[0]*x + matrix[4]*y + matrix[8]*z + matrix[12],
            matrix[1]*x + matrix[5]*y + matrix[9]*z + matrix[13],
            matrix[2]*x + matrix[6]*y + matrix[10]*z + matrix[14])


def vector(matrix, xyz):
    x, y, z = xyz
    return (matrix[0]*x + matrix[4]*y + matrix[8]*z,
            matrix[1]*x + matrix[5]*y + matrix[9]*z,
            matrix[2]*x + matrix[6]*y + matrix[10]*z)


def unit(v):
    length = math.sqrt(sum(x*x for x in v)) or 1.0
    return tuple(x / length for x in v)


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def cross(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])


def skin_matrices(doc, blob):
    skin = doc["skins"][0]
    inverse = read_accessor(doc, blob, skin["inverseBindMatrices"])
    globals_ = [None] * len(doc["nodes"])

    def visit(index, parent):
        current = multiply(parent, node_matrix(doc["nodes"][index]))
        globals_[index] = current
        for child in doc["nodes"][index].get("children", []):
            visit(child, current)

    roots = set(range(len(doc["nodes"])))
    for node in doc["nodes"]:
        roots.difference_update(node.get("children", []))
    identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    for root in roots:
        visit(root, identity)
    return [multiply(globals_[joint], inv) for joint, inv in zip(skin["joints"], inverse)]


def skin_vertex(matrices, transform, value, bone_ids, bone_weights):
    total = sum(bone_weights) or 1.0
    out = [0.0, 0.0, 0.0]
    for bone_id, weight in zip(bone_ids, bone_weights):
        if weight <= 0:
            continue
        moved = transform(matrices[bone_id], value)
        for axis in range(3):
            out[axis] += moved[axis] * weight / total
    return tuple(out)


class Mesh:
    """Only the vertices the filtered recipe actually references."""

    def __init__(self, item_id):
        source = ITEMS / f"{item_id}.gltf"
        if not source.is_file():
            raise FileNotFoundError(source)
        doc = json.loads(source.read_text(encoding="utf-8"))
        binary_name = doc["buffers"][0]["uri"]
        if Path(binary_name).name != binary_name:
            raise ValueError(f"unsafe glTF buffer path in {source.name}")
        blob = (ITEMS / binary_name).read_bytes()
        primitive = doc["meshes"][0]["primitives"][0]
        attrs = primitive["attributes"]
        indices = [int(v[0]) for v in read_accessor(doc, blob, primitive["indices"])]
        used = sorted(set(indices))
        remap = {old: new for new, old in enumerate(used)}
        self.triangles = [(remap[indices[i]], remap[indices[i + 1]], remap[indices[i + 2]])
                          for i in range(0, len(indices) - 2, 3)]

        def pick(name, default_width):
            if name not in attrs:
                return [(0.0,) * default_width] * len(used)
            data = read_accessor(doc, blob, attrs[name])
            return [data[i] for i in used]

        positions = pick("POSITION", 3)
        normals = pick("NORMAL", 3)
        tangents = pick("TANGENT", 4)
        joints = pick("JOINTS_0", 4)
        weights = pick("WEIGHTS_0", 4)
        self.uv0 = pick("TEXCOORD_0", 2)
        self.uv1 = pick("TEXCOORD_1", 2)
        matrices = skin_matrices(doc, blob)
        self.positions = [skin_vertex(matrices, point, p, j, w)
                          for p, j, w in zip(positions, joints, weights)]
        self.normals = [unit(skin_vertex(matrices, vector, n, j, w))
                        for n, j, w in zip(normals, joints, weights)]
        self.tangents = [unit(skin_vertex(matrices, vector, t[:3], j, w))
                         for t, j, w in zip(tangents, joints, weights)]
        self.tangent_sign = [t[3] if t[3] else 1.0 for t in tangents]


# --------------------------------------------------------------------------
# Material: UModel props.txt -> parameters, PNG textures
# --------------------------------------------------------------------------
def parse_props(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    parent = re.search(r"^Parent = (\w+)'([^']+)'", text, re.M)
    params = {"scalar": {}, "vector": {}, "texture": {}}
    pattern = re.compile(r"ParameterValue = (.+?)\s*\r?\n\s*ParameterName = (\w+)")
    for header, key in (("ScalarParameterValues", "scalar"), ("TextureParameterValues", "texture"),
                        ("VectorParameterValues", "vector")):
        start = re.search(rf"^{header}\[\d+\] =\s*\r?\n\{{", text, re.M)
        if not start:
            continue
        end = re.search(r"^\}", text[start.end():], re.M)
        block = text[start.end(): start.end() + (end.start() if end else len(text))]
        for value, name in pattern.findall(block):
            if key == "scalar":
                params[key][name] = float(value)
            elif key == "vector":
                found = dict(re.findall(r"([RGBA])=([-\d.eE+]+)", value))
                params[key][name] = tuple(float(found.get(c, 0.0)) for c in "RGBA")
            else:
                quoted = re.search(r"'([^']+)'", value)
                params[key][name] = quoted.group(1).split(".")[-1] if quoted else None
    return (parent.group(2).split(".")[-1] if parent else None), params


def find_props(mic_name, roots):
    for root in roots:
        for hit in root.glob(f"**/MaterialInstanceConstant/{mic_name}.props.txt"):
            return hit
    return None


def resolve_material(mic_name, roots, base_defaults=None):
    """Child overrides parent; returns (params, texture_dir, chain) or None.

    UModel exports MICs but not the base Material their chain ends at, so a
    parameter no MIC sets is absent. `base_defaults(name)` may return that
    Material's own parameter defaults ({"path", "scalar", "vector", "source"});
    they fill only what every MIC left unset. params["source"] records, per
    kind and name, the MIC or base-Material expression each value came from.
    """
    path = find_props(mic_name, roots)
    if path is None:
        return None
    chain = []
    merged = {"scalar": {}, "vector": {}, "texture": {}}
    source = {kind: {} for kind in merged}
    current, guard, base = (mic_name, path), 0, None
    while current and guard < 8:
        name, props = current
        parent, params = parse_props(props)
        chain.append(name)
        for kind in merged:
            for key, value in params[kind].items():
                if key not in merged[kind]:
                    merged[kind][key] = value
                    source[kind][key] = name
        nxt = find_props(parent, roots) if parent else None
        current = (parent, nxt) if nxt else None
        base = parent if nxt is None else None
        guard += 1
    defaults = base_defaults(base) if base and base_defaults else None
    if defaults:
        for kind in ("scalar", "vector"):  # texture defaults are engine stubs with no local PNG
            for key, value in defaults[kind].items():
                if key not in merged[kind]:
                    merged[kind][key] = value
                    source[kind][key] = defaults["source"][kind][key]
    merged["source"] = source
    texture_dir = path.parents[1] / "Texture2D"
    return merged, texture_dir, chain


def load_channels(path, size=None):
    """Return (width, height, [bytes per channel]) for a PNG; RGBA keeps its alpha channel."""
    image = Image.open(path)
    image = image.convert("RGBA" if "A" in image.getbands() else "RGB")
    if size and max(image.size) > size:
        image = image.resize((min(size, image.width), min(size, image.height)), Image.BOX)
    return image.width, image.height, [c.tobytes() for c in image.split()]


class Texture:
    def __init__(self, path, size=2048, address=None):
        self.width, self.height, self.channels = load_channels(path, size)
        self.pixels = None
        self.clamp = [mode == "TA_Clamp" for mode in (address or ["TA_Wrap", "TA_Wrap"])]

    def rgb(self):
        if self.pixels is None:
            self.pixels = Image.merge("RGB", [Image.frombytes("L", (self.width, self.height), c)
                                              for c in self.channels[:3]]).tobytes()
        return self.pixels

    def sample(self, u, v):
        """Nearest texel as floats 0..1, (r, g, b, a); alpha is 1 without an alpha channel."""
        def index(value, size, clamp):
            value = min(max(value, 0.0), 0.999999) if clamp else value % 1.0
            return int(value * size)
        i = index(v, self.height, self.clamp[1]) * self.width + index(u, self.width, self.clamp[0])
        values = [c[i] / 255.0 for c in self.channels]
        return tuple(values) + ((1.0,) if len(values) == 3 else ())


def weapon_class(recipe, mic_params):
    text = " ".join(str(recipe.get(k, "")) for k in ("weapon_type", "gestalt", "balance")).lower()
    for token, name in (("pistol", "pistol"), ("shotgun", "shotgun"), ("launcher", "launcher"),
                        ("assaultrifle", "assaultrifle"), ("assault", "assaultrifle"),
                        ("smg", "smg"), ("sniper", "sniper")):
        if token in text:
            return name
    return "pistol"


class Paint:
    """Master_Gun reading of tools/weapon_paint_model.py. UNVERIFIED (see that module).

    Built from a recipe and UModel exports (thumbnails: no base-Material defaults and no static
    parameters, so missing values fall back to neutral ones and the detail channel to the
    DETAIL_CHANNELS guess) or, with `prepared`, from prepare_weapon_paint.py's output, which carries
    the base defaults and the channels the MIC's static parameters select.
    """

    def __init__(self, recipe, materials_roots, detail_override=None, patterns=True, prepared=None):
        self.ok = False
        self.note = ""
        if prepared is not None:
            params, chain = prepared["params"], prepared["parent_chain"]
            paths = prepared["textures"]
            texture = lambda name, address=None: Texture(Path(paths[name]), address=address) if name in paths else None
            decal = prepared.get("decal") or {}
            self.detail_channel = prepared["detail_channel"]
            self.pattern_channels = prepared.get("pattern_channels", "RGB")
            self.decal_channels = prepared.get("decal_channels", "RGB")
            decal_address = decal.get("address") if decal.get("used") else None
        else:
            mic = str(recipe.get("material") or "").split(".")[-1]
            resolved = resolve_material(mic, materials_roots) if mic else None
            if resolved is None:
                self.note = f"no UModel export of {mic or 'material'}"
                return
            params, texdir, chain = resolved
            leaf = params["texture"]

            def texture(name, address=None):
                path = texdir / f"{leaf.get(name)}.png"
                return Texture(path, address=address) if leaf.get(name) and path.is_file() else None
            klass = weapon_class(recipe, params)
            self.detail_channel = DETAIL_CHANNELS.get((leaf.get("p_Diffuse"), klass), 2)
            self.pattern_channels = self.decal_channels = "RGB"
            decal_address = None
        if detail_override is not None:
            self.detail_channel = detail_override
        self.chain = chain
        self.masks = texture("p_Masks")
        self.detail = texture("p_Diffuse")
        self.normal = texture("p_NormalScopesEmissive")
        self.pattern = texture("p_Pattern") if patterns else None
        self.decal = texture("p_Decal", decal_address) if prepared is not None and decal_address else None
        if self.masks is None:
            self.note = "missing p_Masks texture"
            return
        vec, sca = dict(params["vector"]), dict(params["scalar"])
        white, neutral = (1.0, 1.0, 1.0, 1.0), {"p_HighlightsIntensity": 0.0, "p_ShadowsIntensity": 0.0}
        for name in PAINT_PARAMETERS["vector"]:
            vec.setdefault(name, white)
        for name, value in neutral.items():
            sca.setdefault(name, value)
        self.params = {"vector": vec, "scalar": sca, "pattern_channels": self.pattern_channels,
                       "decal_channels": self.decal_channels}
        names = lambda group: all(n in vec for n in PAINT_PARAMETERS[group + "_vector"]) and all(
            n in sca for n in PAINT_PARAMETERS[group + "_scalar"])
        if self.pattern is not None and not names("pattern"):
            self.pattern = None
        if self.decal is not None and not names("decal"):
            self.decal = None
        self.ok = True

    def texel(self, u0, v0, u1, v1):
        """(r, g, b, zone coverage) for one point; linear and possibly above 1."""
        (lu, lv), (mu, mv) = mask_uvs(u0, v0)
        light = self.masks.sample(lu, lv)
        mask = self.masks.sample(mu, mv)[:3]
        detail = self.detail.sample(u0, v0)[self.detail_channel] if self.detail else 0.5
        vec, sca = self.params["vector"], self.params["scalar"]
        pattern = decal = None
        if self.pattern is not None:
            pattern = self.pattern.sample(*pattern_uv(u1, v1, vec["p_PatternScalePosition"]))[:3]
        if self.decal is not None:
            decal = self.decal.sample(*decal_uv(u1, v1, vec["p_DecalScalePosition"], sca["p_DecalRotate"]))
        colour = albedo(light, mask, detail, self.params, pattern, decal)
        return colour + (min(1.0, sum(mask)),)


def srgb(value):
    value = 0.0 if value < 0 else value
    if value <= 0.0031308:
        return 12.92 * value
    return 1.055 * value ** (1 / 2.4) - 0.055


SRGB_LUT = [int(round(255 * min(1.0, srgb(i / 4095.0)))) for i in range(4096)]


def to_byte(value):
    return SRGB_LUT[int(min(max(value, 0.0), 1.0) * 4095.0)]


# Display grade, applied to the lit linear colour before sRGB encoding. The MIC colours are HDR
# (values above 1, e.g. the Infinity's p_PatternColor 5.06/5.66/1.93) and the old clamp turned them
# into near-white. The game's own tone mapping, bloom and post-process are not decoded, so this
# grade is a look-alike chosen by eye against local reference captures of the real inventory
# (darker, more saturated, higher contrast than the first pass). UNVERIFIED: it is not the game's
# pipeline and no colour value is claimed to match.
HDR_PEAK_POWER = 0.75  # 1.0 = pure hue-preserving normalise; chosen by eye, UNVERIFIED
GRADE = {"exposure": 0.78, "white": 1.7, "saturation": 1.28, "contrast": 0.22}


def grade_colour(out, grade):
    exposure, white = grade["exposure"], grade["white"]
    # Reinhard with a white point keeps hue where a per-channel clamp would bleach it
    mapped = [c * exposure * (1.0 + c * exposure / (white * white)) / (1.0 + c * exposure)
              if c > 0.0 else 0.0 for c in out]
    lum = 0.2126 * mapped[0] + 0.7152 * mapped[1] + 0.0722 * mapped[2]
    sat = grade["saturation"]
    return [lum + (c - lum) * sat for c in mapped]


def contrast_lut(amount):
    """S-curve on the encoded value: 0 is identity."""
    table = []
    for i in range(256):
        x = i / 255.0
        smooth = x * x * (3.0 - 2.0 * x)
        table.append(int(round(255 * (x + (smooth - x) * amount))))
    return table


# --------------------------------------------------------------------------
# Rasteriser
# --------------------------------------------------------------------------
def view_basis(yaw_deg, pitch_deg, side):
    yaw, pitch = math.radians(yaw_deg), math.radians(pitch_deg)
    # Long axis is Z with the muzzle towards -Z; the reference shows the muzzle
    # on the left, so the camera sits on -X (side=-1) looking at the gun.
    eye = unit((side * math.cos(yaw) * math.cos(pitch), math.sin(pitch),
                -math.sin(yaw) * math.cos(pitch)))
    right = unit(cross(tuple(-x for x in eye), (0, 1, 0)))
    up = unit(cross(right, tuple(-x for x in eye)))
    return eye, right, up


def rasterise(mesh, screen, width, height):
    zbuf = [-1e30] * (width * height)
    ids = [-1] * (width * height)
    for tid, (ia, ib, ic) in enumerate(mesh.triangles):
        x0, y0, z0 = screen[ia]
        x1, y1, z1 = screen[ib]
        x2, y2, z2 = screen[ic]
        denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)
        if abs(denom) < 1e-9:
            continue
        # depth plane z = za + zx*x + zy*y
        l0x, l0y = (y1 - y2) / denom, (x2 - x1) / denom
        l1x, l1y = (y2 - y0) / denom, (x0 - x2) / denom
        zx = z0 * l0x + z1 * l1x + z2 * (-l0x - l1x)
        zy = z0 * l0y + z1 * l1y + z2 * (-l0y - l1y)
        zc = z2 - zx * x2 - zy * y2
        ymin = max(0, int(math.floor(min(y0, y1, y2) - 0.5)))
        ymax = min(height - 1, int(math.ceil(max(y0, y1, y2) - 0.5)))
        edges = ((x0, y0, x1, y1), (x1, y1, x2, y2), (x2, y2, x0, y0))
        for y in range(ymin, ymax + 1):
            py = y + 0.5
            # x where the triangle's edges cross this scanline (half-open in y)
            crossings = []
            for ax, ay, bx, by in edges:
                if (ay <= py < by) or (by <= py < ay):
                    crossings.append(ax + (py - ay) * (bx - ax) / (by - ay))
            if len(crossings) < 2:
                continue
            xa, xb = min(crossings), max(crossings)
            xs = max(0, int(math.ceil(xa - 0.5)))
            xe = min(width - 1, int(math.floor(xb - 0.5)))
            base = y * width
            zrow = zc + zy * py
            for x in range(xs, xe + 1):
                z = zrow + zx * (x + 0.5)
                i = base + x
                if z > zbuf[i]:
                    zbuf[i] = z
                    ids[i] = tid
    return zbuf, ids


# --------------------------------------------------------------------------
# Shading
# --------------------------------------------------------------------------
def shade_pixels(mesh, screen, ids, width, height, paint, basis, grade=None):
    grade = grade or GRADE
    eye, right, up = basis

    def cam(a, b, c):
        return unit(tuple(a * right[i] + b * up[i] + c * eye[i] for i in range(3)))

    key_dir = cam(-0.55, 0.75, 0.62)      # upper-left, towards the viewer
    fill_dir = cam(0.75, 0.05, 0.45)
    rim_dir = cam(0.35, 0.35, -0.85)      # from behind
    key_col = (1.0, 0.94, 0.86)
    fill_col = (0.30, 0.38, 0.55)
    rim_col = (0.55, 0.75, 1.0)
    sky = (0.42, 0.45, 0.55)
    ground = (0.13, 0.12, 0.14)

    R = [bytearray(width * height) for _ in range(3)]
    A = bytearray(width * height)
    D = bytearray(width * height)         # relative depth for crease lines

    tri_data = []
    for ia, ib, ic in mesh.triangles:
        x0, y0, _ = screen[ia]
        x1, y1, _ = screen[ib]
        x2, y2, _ = screen[ic]
        denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2)
        if abs(denom) < 1e-9:
            tri_data.append(None)
            continue
        tri_data.append((x2, y2, (y1 - y2) / denom, (x2 - x1) / denom,
                         (y2 - y0) / denom, (x0 - x2) / denom))

    uv0, uv1 = mesh.uv0, mesh.uv1
    nrm, tan, sign = mesh.normals, mesh.tangents, mesh.tangent_sign
    zs = [s[2] for s in screen]
    zmin, zmax = min(zs), max(zs)
    zscale = 255.0 / max(zmax - zmin, 1e-9)

    ok = paint is not None and paint.ok
    if ok:
        ntex = paint.normal.rgb() if paint.normal else None
        nw = paint.normal.width if paint.normal else 1
        nh = paint.normal.height if paint.normal else 1
    for idx, tid in enumerate(ids):
        if tid < 0:
            continue
        td = tri_data[tid]
        if td is None:
            continue
        ia, ib, ic = mesh.triangles[tid]
        x2, y2, ax_, bx_, cx_, ex_ = td
        px, py = (idx % width) + 0.5, (idx // width) + 0.5
        l0 = ax_ * (px - x2) + bx_ * (py - y2)
        l1 = cx_ * (px - x2) + ex_ * (py - y2)
        l2 = 1.0 - l0 - l1
        n = [nrm[ia][k] * l0 + nrm[ib][k] * l1 + nrm[ic][k] * l2 for k in range(3)]
        t = [tan[ia][k] * l0 + tan[ib][k] * l1 + tan[ic][k] * l2 for k in range(3)]
        nl = math.sqrt(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) or 1.0
        n = [c / nl for c in n]
        # geometric facing: flip when the interpolated normal faces away
        if n[0] * eye[0] + n[1] * eye[1] + n[2] * eye[2] < 0:
            n = [-c for c in n]
            t = [-c for c in t]
        u = uv0[ia][0] * l0 + uv0[ib][0] * l1 + uv0[ic][0] * l2
        v = uv0[ia][1] * l0 + uv0[ib][1] * l1 + uv0[ic][1] * l2

        albedo_r = albedo_g = albedo_b = 0.0
        metal = 0.3
        rough = 0.5
        if ok:
            u %= 1.0
            v %= 1.0
            u1 = uv1[ia][0] * l0 + uv1[ib][0] * l1 + uv1[ic][0] * l2
            v1 = uv1[ia][1] * l0 + uv1[ib][1] * l1 + uv1[ic][1] * l2
            albedo_r, albedo_g, albedo_b, coverage = paint.texel(u, v, u1, v1)
            # HDR colours (> 1) keep their hue: divide by the peak instead of clamping each channel.
            peak = max(albedo_r, albedo_g, albedo_b)
            if peak > 1.0:
                norm = peak ** HDR_PEAK_POWER
                albedo_r, albedo_g, albedo_b = albedo_r / norm, albedo_g / norm, albedo_b / norm
            bare = 1.0 - coverage
            metal = 0.25 + 0.6 * bare
            rough = 0.55 - 0.25 * bare
            if ntex is not None:
                ni = (int(v * nh) * nw + int(u * nw)) * 3
                nx = ntex[ni] / 127.5 - 1.0
                ny = ntex[ni + 1] / 127.5 - 1.0
                nz2 = 1.0 - nx * nx - ny * ny
                nz = math.sqrt(nz2) if nz2 > 0 else 0.0
                # tangent frame; the green-channel direction is UNVERIFIED
                tl = math.sqrt(t[0] * t[0] + t[1] * t[1] + t[2] * t[2]) or 1.0
                t = [c / tl for c in t]
                dt = t[0] * n[0] + t[1] * n[1] + t[2] * n[2]
                t = [t[k] - n[k] * dt for k in range(3)]
                tl = math.sqrt(t[0] * t[0] + t[1] * t[1] + t[2] * t[2]) or 1.0
                t = [c / tl for c in t]
                b = (n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0])
                s = sign[ia]
                strength = 0.8
                n = [n[k] + strength * (t[k] * nx + b[k] * ny * s) for k in range(3)]
                nl = math.sqrt(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) or 1.0
                n = [c / nl for c in n]
        else:
            albedo_r = albedo_g = albedo_b = 0.28

        ndk = max(0.0, n[0] * key_dir[0] + n[1] * key_dir[1] + n[2] * key_dir[2])
        ndf = max(0.0, n[0] * fill_dir[0] + n[1] * fill_dir[1] + n[2] * fill_dir[2])
        ndr = max(0.0, n[0] * rim_dir[0] + n[1] * rim_dir[1] + n[2] * rim_dir[2])
        hemi = 0.5 + 0.5 * n[1]
        # half vector towards the viewer for a broad highlight
        hx = key_dir[0] + eye[0]
        hy = key_dir[1] + eye[1]
        hz = key_dir[2] + eye[2]
        hl = math.sqrt(hx * hx + hy * hy + hz * hz) or 1.0
        ndh = max(0.0, (n[0] * hx + n[1] * hy + n[2] * hz) / hl)
        shine = 8.0 + 60.0 * (1.0 - rough)
        spec = (ndh ** shine) * (0.15 + 0.6 * metal) * ndk
        rim = (1.0 - max(0.0, n[0] * eye[0] + n[1] * eye[1] + n[2] * eye[2])) ** 3 * ndr
        out = []
        for c, alb in enumerate((albedo_r, albedo_g, albedo_b)):
            amb = sky[c] * hemi + ground[c] * (1.0 - hemi)
            light = amb * 0.42 + key_col[c] * ndk * 1.15 + fill_col[c] * ndf * 0.45
            diffuse = alb * light * (1.0 - 0.45 * metal)
            spec_col = (alb * 0.7 + 0.3) if metal > 0.4 else 1.0
            out.append(diffuse + spec * spec_col + rim * rim_col[c] * 0.55 * (0.4 + alb))
        out = grade_colour(out, grade)
        R[0][idx], R[1][idx], R[2][idx] = to_byte(out[0]), to_byte(out[1]), to_byte(out[2])
        A[idx] = 255
        z = screen_z(px, py, tid, mesh, screen, l0, l1, l2)
        D[idx] = int(min(255, max(0, (z - zmin) * zscale)))
    size = (width, height)
    rgb = Image.merge("RGB", [Image.frombytes("L", size, bytes(c)) for c in R])
    alpha = Image.frombytes("L", size, bytes(A))
    depth = Image.frombytes("L", size, bytes(D))
    return rgb, alpha, depth


def screen_z(px, py, tid, mesh, screen, l0, l1, l2):
    ia, ib, ic = mesh.triangles[tid]
    return screen[ia][2] * l0 + screen[ib][2] * l1 + screen[ic][2] * l2


def ink_outline(rgb, alpha, depth, ss):
    """BL2 draws dark ink lines on silhouettes and depth creases."""
    width = max(1, int(round(ss * 1.1)))
    grown = alpha.filter(ImageFilter.MaxFilter(2 * width + 1))
    silhouette = ImageChops.subtract(grown, alpha)
    edge = depth.filter(ImageFilter.FIND_EDGES)
    creases = edge.point(lambda v: 255 if v > 9 else 0)
    creases = ImageChops.multiply(creases, alpha)
    creases = creases.filter(ImageFilter.MaxFilter(max(3, 2 * (width // 2) + 1)))
    line = ImageChops.lighter(silhouette, ImageChops.multiply(creases, alpha.point(lambda v: 255 if v else 0)))
    ink = Image.new("RGB", rgb.size, (10, 10, 16))
    # inside creases blend the ink over the paint; the silhouette grows the alpha
    inked = Image.composite(ink, rgb, line)
    return inked, ImageChops.lighter(alpha, silhouette)


# --------------------------------------------------------------------------
# Driver
# --------------------------------------------------------------------------
def tight_crop(image, margin):
    """Crop to the visible pixels, then pad every side by the same transparent margin.

    Faint resampling fringe (alpha <= 12) does not count as content, so every thumbnail's
    visible art sits exactly `margin` px from its edges and the page can fit it to a cell
    with object-fit:contain and one common inset."""
    alpha = image.getchannel("A").point(lambda v: 255 if v > 12 else 0)
    box = alpha.getbbox()
    if box is None:
        return image
    cropped = image.crop(box)
    padded = Image.new("RGBA", (cropped.width + 2 * margin, cropped.height + 2 * margin), (0, 0, 0, 0))
    padded.paste(cropped, (margin, margin))
    return padded


def render_item(item_id, materials_roots, max_size=(512, 256), ss=3, yaw=14.0, pitch=7.0,
                side=-1, margin=6, detail_override=None, flat=False, patterns=True,
                grade=None, final_margin=4, prepared=None):
    started = time.time()
    mesh = Mesh(item_id)
    recipe_path = ITEMS / f"{item_id}.json"
    recipe = json.loads(recipe_path.read_text(encoding="utf-8")) if recipe_path.is_file() else {}
    paint = None if flat else Paint(recipe, materials_roots, detail_override, patterns, prepared)
    note = "flat neutral" if paint is None else ("textured" if paint.ok else f"neutral ({paint.note})")
    basis = view_basis(yaw, pitch, side)
    eye, right, up = basis
    center = [sum(p[i] for p in mesh.positions) / len(mesh.positions) for i in range(3)]
    proj = [(dot([p[i] - center[i] for i in range(3)], right),
             dot([p[i] - center[i] for i in range(3)], up),
             dot([p[i] - center[i] for i in range(3)], eye)) for p in mesh.positions]
    x_lo, x_hi = min(p[0] for p in proj), max(p[0] for p in proj)
    y_lo, y_hi = min(p[1] for p in proj), max(p[1] for p in proj)
    pad = margin
    scale = min((max_size[0] - 2 * pad) / max(x_hi - x_lo, 1e-6),
                (max_size[1] - 2 * pad) / max(y_hi - y_lo, 1e-6))
    out_w = int((x_hi - x_lo) * scale) + 2 * pad
    out_h = int((y_hi - y_lo) * scale) + 2 * pad
    width, height = out_w * ss, out_h * ss
    screen = [((p[0] - x_lo) * scale * ss + pad * ss, (y_hi - p[1]) * scale * ss + pad * ss, p[2])
              for p in proj]
    zbuf, ids = rasterise(mesh, screen, width, height)
    rgb, alpha, depth = shade_pixels(mesh, screen, ids, width, height, paint, basis, grade)
    rgb, alpha = ink_outline(rgb, alpha, depth, ss)
    lut = contrast_lut((grade or GRADE)["contrast"])
    rgb = rgb.point(lut * 3)
    rgba = rgb.convert("RGBA")
    rgba.putalpha(alpha)
    small = rgba.convert("RGBa").resize((out_w, out_h), Image.LANCZOS).convert("RGBA")
    small = tight_crop(small, final_margin)
    buffer = BytesIO()
    small.save(buffer, format="PNG", optimize=True)
    return item_id, buffer.getvalue(), small.size, note, time.time() - started


def _job(args):
    item_id, kwargs = args
    kwargs = dict(kwargs)
    try:
        global ITEMS
        ITEMS = kwargs.pop("items", ITEMS)
        kwargs["prepared"] = kwargs.pop("prepared_by_id", {}).get(item_id)
        return render_item(item_id, **kwargs)
    except Exception as error:  # report per item, keep going
        return item_id, None, None, f"{type(error).__name__}: {error}", 0.0


def contact_sheet(paths, target, cell=(512, 256), columns=3, background=(38, 70, 95)):
    rows = (len(paths) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * cell[0], rows * cell[1]), background)
    for n, path in enumerate(paths):
        img = Image.open(path).convert("RGBA")
        img.thumbnail(cell, Image.LANCZOS)
        x = (n % columns) * cell[0] + (cell[0] - img.width) // 2
        y = (n // columns) * cell[1] + (cell[1] - img.height) // 2
        sheet.paste(img, (x, y), img)
    target.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(target)


def export_missing_materials(recipes, umodel, cooked, out_dir):
    """Bounded UModel run for MICs that have no props export yet."""
    seen = set()
    for recipe in recipes:
        mic = str(recipe.get("material") or "").split(".")[-1]
        if not mic or mic in seen or find_props(mic, [UMODEL_ROOT]):
            continue
        seen.add(mic)
        started = time.time()
        command = [str(umodel), f"-path={cooked}", "-game=border", "-export", "-png",
                   f"-out={out_dir}", "Startup", mic, "MaterialInstanceConstant"]
        result = subprocess.run(command, capture_output=True, text=True)
        print(f"umodel {mic}: exit {result.returncode} in {time.time() - started:.2f}s")
        print("  " + " ".join(command))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("ids", nargs="*", help="recipe IDs; default: every local/items/*.gltf")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--materials", type=Path, action="append",
                        help="UModel export root(s); default: local/external/umodel")
    parser.add_argument("--max-width", type=int, default=512)
    parser.add_argument("--max-height", type=int, default=256)
    parser.add_argument("--supersample", type=int, default=3)
    parser.add_argument("--yaw", type=float, default=14.0, help="degrees the muzzle turns toward the camera")
    parser.add_argument("--pitch", type=float, default=7.0, help="degrees the camera looks down")
    parser.add_argument("--side", type=int, choices=(-1, 1), default=-1,
                        help="-1: camera on -X (muzzle on the left, as in the reference)")
    parser.add_argument("--detail-channel", type=int, choices=(0, 1, 2))
    parser.add_argument("--no-pattern", action="store_true", help="skip the UNVERIFIED p_Pattern layer")
    parser.add_argument("--flat", action="store_true", help="old untextured neutral shading")
    parser.add_argument("--paint", type=Path, help="prepare_weapon_paint.py output: use its parameters, "
                        "base defaults and static channels instead of the UModel-only fallback")
    parser.add_argument("--items", type=Path, help="folder holding <id>.gltf/.json (default local/items)")
    parser.add_argument("--exposure", type=float, default=GRADE["exposure"], help="display grade (UNVERIFIED look)")
    parser.add_argument("--saturation", type=float, default=GRADE["saturation"])
    parser.add_argument("--contrast", type=float, default=GRADE["contrast"], help="0 = none")
    parser.add_argument("--white", type=float, default=GRADE["white"], help="tone-map white point")
    parser.add_argument("--margin", type=int, default=4, help="transparent px kept around the cropped art")
    parser.add_argument("--jobs", type=int, default=min(8, os.cpu_count() or 1))
    parser.add_argument("--sheet", type=Path, help="also write a contact sheet PNG")
    parser.add_argument("--umodel", type=Path, default=os.environ.get("OPENWILLOW_UMODEL"),
                        help="umodel.exe; with --cooked, exports MICs that are missing")
    parser.add_argument("--cooked", type=Path, default=os.environ.get("OPENWILLOW_COOKED"),
                        help="Borderlands 2 WillowGame/CookedPCConsole directory")
    args = parser.parse_args()
    global ITEMS
    if args.items:
        ITEMS = args.items
    prepared = {}
    if args.paint:
        loaded = json.loads(args.paint.read_text(encoding="utf-8"))
        prepared = {entry["recipe_id"]: entry for entry in (loaded if isinstance(loaded, list) else [loaded])}
    ids = args.ids or [path.stem for path in sorted(ITEMS.glob("*.gltf"))]
    if not ids or any(not ID_PATTERN.fullmatch(item_id) for item_id in ids):
        parser.error("provide valid item IDs backed by local/items/*.gltf")
    roots = args.materials or [UMODEL_ROOT]
    if args.umodel and args.cooked:
        recipes = [json.loads((ITEMS / f"{i}.json").read_text(encoding="utf-8"))
                   for i in ids if (ITEMS / f"{i}.json").is_file()]
        export_missing_materials(recipes, args.umodel, args.cooked,
                                 UMODEL_ROOT / f"weapon-materials-{time.strftime('%Y%m%d')}")
    args.output.mkdir(parents=True, exist_ok=True)
    kwargs = dict(materials_roots=roots, max_size=(args.max_width, args.max_height),
                  ss=args.supersample, yaw=args.yaw, pitch=args.pitch, side=args.side,
                  detail_override=args.detail_channel, flat=args.flat, patterns=not args.no_pattern,
                  grade={"exposure": args.exposure, "white": args.white,
                         "saturation": args.saturation, "contrast": args.contrast},
                  final_margin=args.margin, items=ITEMS, prepared_by_id=prepared)
    started = time.time()
    rendered, failed, written = 0, 0, []
    jobs = [(item_id, kwargs) for item_id in ids]
    if args.jobs > 1 and len(jobs) > 1:
        with ProcessPoolExecutor(max_workers=args.jobs) as pool:
            results = list(pool.map(_job, jobs))
    else:
        results = [_job(job) for job in jobs]
    for item_id, data, size, note, seconds in results:
        if data is None:
            failed += 1
            print(f"{item_id}: FAILED {note}", file=sys.stderr)
            continue
        target = args.output / f"{item_id}.png"
        target.write_bytes(data)
        written.append(target)
        rendered += 1
        print(f"{item_id}: {size[0]}x{size[1]} {len(data)} bytes {seconds:.1f}s [{note}] -> {target}")
    if args.sheet and written:
        contact_sheet(written, args.sheet)
        print(f"contact sheet -> {args.sheet}")
    print(f"rendered={rendered} failed={failed} elapsed={time.time() - started:.1f}s")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
