#!/usr/bin/env python3
"""Compare where the live game puts the Phaselock target's worn pieces with where the host puts them.

AI-assisted (Claude), 2026-11-08. Inputs are ignored local files: the probe's rows
(tools/real_game/phaselock_integrity_capture.ps1 -> local/realgame/phaselock_integrity/hit.jsonl, whose 'assets' rows list each attached
piece with its bone's world matrix and, since 2026-11-08, its world bounds) and the host's placement
(local/phaselock/target/host_target.json, written by tools/prepare_phaselock_target.py attachments).

Why the bounds: LocalToWorld read null through the SDK. A piece mesh's pivot is its bounds' centre (checked on the exported glTF of
all four meshes), so the world bounds' origin is the piece's world position, and taking it into the bone's frame gives the offset the
host should use. UE3 matrices are row-vector: rows X, Y, Z are the axes (scaled), row W the origin. Prints one row per piece: the live
offset in the bone's frame, the host's, and the distance between them. The verdict needs a real-game run; this tool only does the arithmetic.

Usage: python tools/phaselock_attachment_check.py --rows <hit.jsonl> --host <host_target.json> [--pawn Bullymong]
"""
import argparse
import json
import math
from pathlib import Path


def to_bone_frame(bone_matrix, point):
    """point (world) in the frame of bone_matrix ([[X], [Y], [Z], [W]] rows): solve point = W + a X + b Y + c Z."""
    x, y, z, w = bone_matrix
    d = [point[i] - w[i] for i in range(3)]
    det = (x[0] * (y[1] * z[2] - y[2] * z[1]) - x[1] * (y[0] * z[2] - y[2] * z[0]) + x[2] * (y[0] * z[1] - y[1] * z[0]))
    if abs(det) < 1e-12:
        return None

    def replaced(column):
        rows = [list(x), list(y), list(z)]
        for i in range(3):
            rows[column][i] = d[i]
        a, b, c = rows
        return a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
    return [replaced(i) / det for i in range(3)]


def live_pieces(rows):
    """{bone: [offset in bone frame]} from the newest probe row that has an attachment with both a bone matrix and bounds."""
    found = {}
    for row in rows:
        for piece in ((row.get('assets') or {}).get('attachments') or []):
            bounds, bone = piece.get('bounds'), piece.get('bone_matrix')
            if bounds and bone and piece.get('bone'):
                offset = to_bone_frame(bone, bounds['origin'])
                if offset:
                    found[piece['bone']] = offset
    return found


def compare(live, host_rows):
    out = []
    for piece in host_rows:
        mine = piece['location']
        theirs = live.get(piece['bone'])
        gap = math.dist(mine, theirs) if theirs else None
        out.append({'bone': piece['bone'], 'host': mine, 'live': theirs, 'gap': gap})
    return out


def vector(v):
    return '-' if v is None else '(' + ', '.join('%.1f' % c for c in v) + ')'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--rows', required=True, type=Path)
    parser.add_argument('--host', required=True, type=Path)
    parser.add_argument('--pawn', default='Bullymong')
    args = parser.parse_args()
    rows = [json.loads(line) for line in args.rows.read_text(encoding='utf-8').splitlines() if line.strip()]
    host = json.loads(args.host.read_text(encoding='utf-8'))[args.pawn].get('attachments', [])
    for r in compare(live_pieces(rows), host):
        gap = '-' if r['gap'] is None else '%.1f' % r['gap']
        print(f"{r['bone']:<8} host {vector(r['host'])}  live {vector(r['live'])}  gap {gap}")


if __name__ == '__main__':
    main()
