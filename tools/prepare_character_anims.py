"""Convert UModel MD5 animations of Maya's arms into UE bone tracks (JSON).

Inputs: the MD5 mesh/animations and the reference pose of the UE skeleton that
the same mesh's glTF export produced (dumped by host/ue5/import_character.py).
Each UE bone transform is taken as T_i(t) = C W_i(t) D_i: W is the MD5 world
transform, C maps MD5 space to UE space (fitted from bind positions; observed
as a Y mirror) and D_i = W_i(bind)^-1 C^-1 T_i(bind) absorbs each imported
bone's axis convention. A per-frame root correction then places the `Camera`
bone at the component origin, reproducing the camera-space framing of
prepare_character_pose.py, so the arms can be attached to the player camera.
That BL2 views from this bone is UNVERIFIED.
"""
import argparse
import json
from pathlib import Path
import re

import numpy as np

from prepare_character_pose import read_joints
from prepare_sanctuary_pillar import block, numbers, quaternion, rotate, multiply

MIRROR = np.diag([1.0, -1.0, 1.0, 1.0])


def matrix(position, q):
    m = np.eye(4)
    for axis in range(3):
        m[:3, axis] = rotate(q, tuple(np.eye(3)[axis]))
    m[:3, 3] = position
    return m


def ue_matrix(loc, quat):
    x, y, z, w = quat
    r = np.array([[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
                  [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
                  [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]])
    m = np.eye(4)
    m[:3, :3], m[:3, 3] = r, loc
    return m


def to_quat(r):
    u, _, vt = np.linalg.svd(r)
    r = u @ vt
    if np.linalg.det(r) < 0:
        raise ValueError('Improper rotation in a UE bone transform')
    t = np.trace(r)
    if t > 0:
        s = 2*np.sqrt(t+1)
        q = [(r[2, 1]-r[1, 2])/s, (r[0, 2]-r[2, 0])/s, (r[1, 0]-r[0, 1])/s, s/4]
    else:
        i = int(np.argmax(np.diag(r)))
        j, k = (i+1) % 3, (i+2) % 3
        s = 2*np.sqrt(1+r[i, i]-r[j, j]-r[k, k])
        q = [0.0]*4
        q[i], q[j], q[k], q[3] = s/4, (r[j, i]+r[i, j])/s, (r[k, i]+r[i, k])/s, (r[k, j]-r[j, k])/s
    return [float(v) for v in q]


def frames(path, joints):
    text = Path(path).read_text(encoding='utf-8')
    names = [line.split('"')[1] for line in block(text, 'hierarchy').splitlines() if line.strip()]
    if names != [joint[0] for joint in joints]:
        raise ValueError(f'{path}: animation bones differ from the mesh')
    count = int(re.search(r'\bnumFrames\s+(\d+)', text).group(1))
    rate = float(re.search(r'\bframeRate\s+(\S+)', text).group(1))
    result = []
    for index in range(count):
        values = [numbers(line) for line in block(text, f'frame {index}').splitlines() if line.strip()]
        world = []
        for (_, parent, _, _), local in zip(joints, values):
            position, orientation = local[:3], quaternion(local[3:])
            if parent >= 0:
                parent_position, parent_rotation = world[parent]
                position = tuple(a + b for a, b in zip(parent_position, rotate(parent_rotation, position)))
                orientation = multiply(parent_rotation, orientation)
            world.append((position, orientation))
        result.append([matrix(p, q) for p, q in world])
    return rate, result


def convert(mesh_path, reference_path, animations, anchor='Camera'):
    joints = read_joints(Path(mesh_path).read_text(encoding='utf-8'))
    reference = {bone['name']: bone for bone in json.loads(Path(reference_path).read_text(encoding='utf-8'))}
    names = [joint[0] for joint in joints]
    if set(names) != set(reference):
        raise ValueError('UE skeleton and MD5 mesh bones differ')
    bind_md5 = [matrix(position, q) for _, _, position, q in joints]
    bind_ue = [ue_matrix(reference[name]['loc'], reference[name]['quat']) for name in names]
    # Fit C from bind positions; require an orthonormal map without translation.
    source = np.array([m[:3, 3] for m in bind_md5])
    target = np.array([m[:3, 3] for m in bind_ue])
    fitted, *_ = np.linalg.lstsq(np.c_[source, np.ones(len(source))], target, rcond=None)
    linear, offset = fitted[:3].T, fitted[3]
    if np.abs(linear - MIRROR[:3, :3]).max() > 1e-3 or np.abs(offset).max() > 0.05:
        raise ValueError(f'MD5 to UE map is not the expected Y mirror: {linear} {offset}')
    c = MIRROR
    inverse_c = np.linalg.inv(c)
    fix = [np.linalg.inv(w) @ inverse_c @ t for w, t in zip(bind_md5, bind_ue)]
    parents = [joint[1] for joint in joints]
    camera = names.index(anchor)
    output = {}
    for label, path in animations.items():
        rate, clip = frames(path, joints)
        tracks = {name: {'pos': [], 'rot': []} for name in names}
        for world in clip:
            ue = [c @ w @ d for w, d in zip(world, fix)]
            view = MIRROR @ np.linalg.inv(world[camera]) @ inverse_c
            ue = [view @ t for t in ue]
            for i, name in enumerate(names):
                local = ue[i] if parents[i] < 0 else np.linalg.inv(ue[parents[i]]) @ ue[i]
                tracks[name]['pos'].append([float(v) for v in local[:3, 3]])
                tracks[name]['rot'].append(to_quat(local[:3, :3]))
        output[label] = {'rate': rate, 'frames': len(clip), 'tracks': tracks}
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mesh', type=Path, required=True)
    parser.add_argument('--reference', type=Path, required=True, help='UE reference pose JSON')
    parser.add_argument('--animset', type=Path, required=True, help='Folder of .md5anim files')
    parser.add_argument('--clips', nargs='+', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    clips = {clip: args.animset / f'{clip}.md5anim' for clip in args.clips}
    result = convert(args.mesh, args.reference, clips)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result), encoding='utf-8')
    print(json.dumps({k: {'frames': v['frames'], 'rate': v['rate']} for k, v in result.items()}))


if __name__ == '__main__':
    main()
