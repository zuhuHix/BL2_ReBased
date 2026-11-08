"""Synthetic tests for tools/phaselock_attachment_check.py (invented matrices and offsets, no game data).

Run: python tests/phaselock_attachment_check_test.py
"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import phaselock_attachment_check as c  # noqa: E402


def rotated_bone(angle, scale, origin):
    """A bone turned by angle about Z, scaled, at origin: rows X, Y, Z, W (UE3 row-vector)."""
    co, si = math.cos(angle), math.sin(angle)
    return [[co * scale, si * scale, 0.0], [-si * scale, co * scale, 0.0], [0.0, 0.0, scale], list(origin)]


def world_of(bone, offset):
    x, y, z, w = bone
    return [w[i] + offset[0] * x[i] + offset[1] * y[i] + offset[2] * z[i] for i in range(3)]


def test_round_trip():
    bone = rotated_bone(0.7, 1.3, (100.0, -40.0, 12.0))
    back = c.to_bone_frame(bone, world_of(bone, [13.0, 33.0, -4.0]))
    assert all(abs(a - b) < 1e-9 for a, b in zip(back, [13.0, 33.0, -4.0]))


def test_degenerate_bone_is_none():
    assert c.to_bone_frame([[0, 0, 0], [0, 1, 0], [0, 0, 1], [0, 0, 0]], [1, 2, 3]) is None


def test_compare_uses_the_newest_row_and_reports_missing():
    bone = rotated_bone(-1.1, 1.0, (5.0, 6.0, 7.0))
    old = {'assets': {'attachments': [{'bone': 'Head', 'bone_matrix': bone, 'bounds': {'origin': world_of(bone, [0, 0, 0])}}]}}
    new = {'assets': {'attachments': [{'bone': 'Head', 'bone_matrix': bone, 'bounds': {'origin': world_of(bone, [3, 4, 0])}},
                                      {'bone': 'Spine2', 'bone_matrix': bone, 'bounds': None}]}}
    rows = c.compare(c.live_pieces([old, {'other': 1}, new]),
                     [{'bone': 'Head', 'location': [0, 0, 0]}, {'bone': 'Spine2', 'location': [1, 1, 1]}])
    assert abs(rows[0]['gap'] - 5.0) < 1e-9
    assert rows[1]['live'] is None and rows[1]['gap'] is None


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'phaselock_attachment_check_test: {len(tests)} passed')
