"""Bake one frame of a UModel MD5 character export to a camera-space OBJ.

Used for Maya's first-person arms (`Hands_Siren`). The arms skeleton carries a
`Camera` bone; vertices are written relative to that bone so the host can
attach the mesh to the player camera at the origin. That the game places its
view at this bone is UNVERIFIED (inferred from the bone name and placement).
Reads the MD5 text interchange format, not a cooked UE3 layout.
"""
import argparse
import json
import math
from pathlib import Path
import re

from prepare_sanctuary_pillar import JOINT, TRI, VERT, WEIGHT, block, multiply, numbers, quaternion, rotate


def conjugate(q):
    return (-q[0], -q[1], -q[2], q[3])


def read_joints(text):
    joints = []
    for line in block(text, 'joints').splitlines():
        if line.strip():
            match = JOINT.match(line)
            if not match:
                raise ValueError(f'Unsupported MD5 joint: {line}')
            name, parent, position, orientation = match.groups()
            joints.append((name, int(parent), numbers(position), quaternion(numbers(orientation))))
    if any(not -1 <= parent < i for i, (_, parent, _, _) in enumerate(joints)):
        raise ValueError('MD5 joints are not parent ordered')
    return joints


def read_mesh(path):
    text = Path(path).read_text(encoding='utf-8')
    if 'MD5Version 10' not in text:
        raise ValueError('Not an MD5 version 10 mesh')
    sections = []
    for body in re.findall(r'(?m)^mesh\s*\{([^{}]*)\}', text):
        verts, tris, weights = {}, {}, {}
        for line in body.splitlines():
            if match := VERT.match(line):
                index, uv, first, count = match.groups()
                verts[int(index)] = (numbers(uv), int(first), int(count))
            elif match := TRI.match(line):
                index, a, b, c = match.groups()
                tris[int(index)] = (int(a), int(b), int(c))
            elif match := WEIGHT.match(line):
                index, joint, bias, position = match.groups()
                weights[int(index)] = (int(joint), float(bias), numbers(position))
        for parsed in (verts, tris, weights):
            if sorted(parsed) != list(range(len(parsed))):
                raise ValueError('MD5 mesh indices have gaps')
        if any(v not in verts for t in tris.values() for v in t):
            raise ValueError('MD5 triangle references a missing vertex')
        shader = re.search(r'\bshader\s+"([^"]+)"', body)
        sections.append((shader.group(1) if shader else '', verts, tris, weights))
    if not sections:
        raise ValueError('MD5 mesh has no sections')
    return read_joints(text), sections


def read_frame(path, frame, joints):
    text = Path(path).read_text(encoding='utf-8')
    count = re.search(r'\bnumJoints\s+(\d+)', text)
    frames = re.search(r'\bnumFrames\s+(\d+)', text)
    if count is None or int(count.group(1)) != len(joints):
        raise ValueError('Animation and mesh joint counts differ')
    names = [line.split('"')[1] for line in block(text, 'hierarchy').splitlines() if line.strip()]
    flags = [int(line.split()[2]) for line in block(text, 'hierarchy').splitlines() if line.strip()]
    if names != [joint[0] for joint in joints] or set(flags) != {63}:
        raise ValueError('Animation bones or component layout differ from the mesh')
    if frames is None or not 0 <= frame < int(frames.group(1)):
        raise ValueError('Animation frame is out of range')
    values = [numbers(line) for line in block(text, f'frame {frame}').splitlines() if line.strip()]
    if len(values) != len(joints) or any(len(value) != 6 for value in values):
        raise ValueError('Animation frame has an unexpected shape')
    # As with the pillar export, frame transforms are local to the mesh's own
    # joint parents (UModel flattens the anim hierarchy's parent column).
    world = []
    for (_, parent, _, _), local in zip(joints, values):
        position, orientation = local[:3], quaternion(local[3:])
        if parent >= 0:
            parent_position, parent_rotation = world[parent]
            position = tuple(a + b for a, b in zip(parent_position, rotate(parent_rotation, position)))
            orientation = multiply(parent_rotation, orientation)
        world.append((position, orientation))
    return world


def bake(mesh_path, animation_path, frame, output, anchor='Camera'):
    joints, sections = read_mesh(mesh_path)
    world = read_frame(animation_path, frame, joints)
    names = [joint[0] for joint in joints]
    if anchor not in names:
        raise ValueError(f'No {anchor} bone in the mesh')
    anchor_position, anchor_rotation = world[names.index(anchor)]
    inverse = conjugate(anchor_rotation)
    lines = [f'# source: {Path(mesh_path).stem} {Path(animation_path).stem} frame {frame}, {anchor} bone space']
    offset = 0
    lows, highs = [math.inf] * 3, [-math.inf] * 3
    for slot, (shader, verts, tris, weights) in enumerate(sections):
        lines.append(f'g {Path(mesh_path).stem}_s{slot}')
        for uv, first, count in (verts[i] for i in range(len(verts))):
            point, total = [0.0, 0.0, 0.0], 0.0
            for bone, bias, local in (weights[i] for i in range(first, first + count)):
                position, orientation = world[bone]
                skinned = tuple(a + b for a, b in zip(position, rotate(orientation, local)))
                point = [p + bias * s for p, s in zip(point, skinned)]
                total += bias
            if abs(total - 1.0) > 0.002:
                raise ValueError('Vertex weights do not sum to one')
            x, y, z = rotate(inverse, tuple(p - a for p, a in zip(point, anchor_position)))
            # MD5 is right handed; mirror Y into UE3 space (as the pillar does).
            vertex = (x, -y, z)
            lows = [min(a, b) for a, b in zip(lows, vertex)]
            highs = [max(a, b) for a, b in zip(highs, vertex)]
            lines.append('v ' + ' '.join(format(v, '.9g') for v in vertex))
        # MD5 V is top-down; OBJ V is bottom-up (see src/assets.cpp).
        lines.extend(f'vt {format(verts[i][0][0], ".9g")} {format(1 - verts[i][0][1], ".9g")}'
                     for i in range(len(verts)))
        # The Y mirror reverses triangle order, matching ow-package's OBJ convention.
        lines.extend(f'f {c+offset+1}/{c+offset+1} {b+offset+1}/{b+offset+1} {a+offset+1}/{a+offset+1}'
                     for a, b, c in (tris[i] for i in range(len(tris))))
        offset += len(verts)
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return {'obj': str(output), 'anchor': anchor, 'anchor_world_md5': anchor_position,
            'bounds_min': lows, 'bounds_max': highs, 'sections': len(sections)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mesh', type=Path, required=True)
    parser.add_argument('--animation', type=Path, required=True)
    parser.add_argument('--frame', type=int, default=0)
    parser.add_argument('--anchor', default='Camera')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(bake(args.mesh, args.animation, args.frame, args.output, args.anchor), indent=2))


if __name__ == '__main__':
    main()
