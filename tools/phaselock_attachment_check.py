#!/usr/bin/env python3
"""Compare where the live game puts the Phaselock target's worn pieces with where the host puts them: each piece's pivot position on its bone.

AI-assisted (Claude), 2026-11-08. Inputs are ignored local files: the probe's session folder
(tools/real_game/phaselock_integrity_capture.ps1 -> local/realgame/phaselock_integrity/: session.json names the hit attempt it kept, hit or
hit_try<N>, and <attempt>.jsonl holds that attempt's rows) and the host's placement (local/phaselock/target/host_target.json, written by
tools/prepare_phaselock_target.py attachments).

Only the kept attempt counts, and only if session.json says it lifted the target; its pieces are read from a row in which the target is the
lifted (held) pawn. A failed first cast is never compared (PR #182 review r1).

Why the bounds: LocalToWorld read null through the SDK. A piece mesh's pivot is its bounds' centre (checked on the exported glTF of all
four meshes), so the world bounds' origin is the piece's world position, and taking it into the bone's frame gives the offset the host
should use. UE3 matrices are row-vector: rows X, Y, Z are the axes (scaled), row W the origin.

What it checks: the pivot position at one held frame, against PASS_TOLERANCE. Not the pieces' orientation or scale (the extents are not
compared), and not other poses. Exit 0 when every piece is within the tolerance, 1 otherwise, 2 when the session cannot be used.

Usage: python tools/phaselock_attachment_check.py --session local/realgame/phaselock_integrity --host local/phaselock/target/host_target.json
"""
import argparse
import json
import math
import sys
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


# Host choice (UNVERIFIED): how far, in units, a live pivot may sit from the host's and still pass. The bounds and the bone matrix are read on
# the same frame, but a component's bounds may trail its bone by a frame of the hold's loop motion; 8 units leaves room for that and is
# small against the pieces (about 25 to 115 units long).
PASS_TOLERANCE = 8.0


class SessionError(Exception):
    pass


def held_rows(folder, stage='hit'):
    """(rows of the kept attempt in which the target is lifted, the target's name). Raises SessionError when the session cannot be used."""
    folder = Path(folder)
    try:
        session = json.loads((folder / 'session.json').read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        raise SessionError(f'session.json unreadable in {folder}: {error}')
    attempt = session.get(f'{stage}_attempt')
    if not attempt:
        raise SessionError(f'session.json names no {stage}_attempt')
    result = session.get(f'{attempt}_result') or {}
    if result.get('target_lifted') is not True or not result.get('target'):
        raise SessionError(f'the kept attempt {attempt} did not lift the target ({result.get("lifted")})')
    path = folder / f'{attempt}.jsonl'
    if not path.is_file():
        raise SessionError(f'{path} is missing')
    try:
        rows = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines() if line.strip()]
    except (OSError, ValueError) as error:
        raise SessionError(f'{path} unreadable: {error}')
    if not all(isinstance(row, dict) for row in rows):
        raise SessionError(f'{path} has a row that is not an object')
    held = [row for row in rows if row.get('lifted') == result['target']]
    if not held:
        raise SessionError(f'{attempt}: no row shows {result["target"]} lifted')
    return held, result['target']


def live_pieces(rows):
    """{bone: [offset in bone frame]} from the newest of the given rows that has an attachment with both a bone matrix and bounds."""
    found = {}
    for row in rows:
        for piece in ((row.get('assets') or {}).get('attachments') or []):
            bounds, bone = piece.get('bounds'), piece.get('bone_matrix')
            if bounds and bone and piece.get('bone'):
                offset = to_bone_frame(bone, bounds['origin'])
                if offset:
                    found[piece['bone']] = offset
    return found


def host_rows(path, pawn):
    """The host report's attachments for pawn. Raises SessionError when the report cannot be used, or names a bone twice."""
    try:
        rows = json.loads(Path(path).read_text(encoding='utf-8'))[pawn].get('attachments', [])
        bones = [row['bone'] for row in rows]
        for row in rows:
            location = row['location']
            numbers = isinstance(location, list) and len(location) == 3 and all(
                isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v) for v in location)
            if not isinstance(row['bone'], str) or not numbers:
                raise ValueError(f"{row['bone']!r}: needs a bone name and a location of three finite numbers")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        raise SessionError(f'host report {path} unusable for {pawn}: {error!r}')
    if len(set(bones)) != len(bones):
        raise SessionError(f'host report {path} names a bone twice: {bones}')
    return rows


def compare(live, host_rows):
    """One row per piece on either side. A piece only the game shows, or only the host shows, fails: a partial report must not pass."""
    out = []
    for piece in host_rows:
        mine = piece['location']
        theirs = live.get(piece['bone'])
        gap = math.dist(mine, theirs) if theirs else None
        out.append({'bone': piece['bone'], 'host': mine, 'live': theirs, 'gap': gap,
                    'pass': gap is not None and gap <= PASS_TOLERANCE})
    named = {piece['bone'] for piece in host_rows}
    for bone in sorted(set(live) - named):
        out.append({'bone': bone, 'host': None, 'live': live[bone], 'gap': None, 'pass': False})
    return out


def vector(v):
    return '-' if v is None else '(' + ', '.join('%.1f' % c for c in v) + ')'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--session', required=True, type=Path, help='the capture folder holding session.json and <attempt>.jsonl')
    parser.add_argument('--host', required=True, type=Path)
    parser.add_argument('--pawn', default='Bullymong')
    args = parser.parse_args()
    try:
        rows, target = held_rows(args.session)
        host = host_rows(args.host, args.pawn)
    except SessionError as error:
        print(f'cannot compare: {error}')
        return 2
    results = compare(live_pieces(rows), host)
    print(f'held target {target}; pivot position on the bone at one held frame; tolerance {PASS_TOLERANCE} units')
    for r in results:
        gap = '-' if r['gap'] is None else '%.2f' % r['gap']
        print(f"{r['bone']:<8} host {vector(r['host'])}  live {vector(r['live'])}  gap {gap}  {'pass' if r['pass'] else 'FAIL'}")
    passed = bool(results) and all(r['pass'] for r in results)
    print(f"verdict: {'PASS' if passed else 'FAIL'} ({sum(r['pass'] for r in results)} of {len(results)} pieces)")
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
