"""Synthetic tests for tools/phaselock_attachment_check.py (invented matrices and offsets, no game data).

Run: python tests/phaselock_attachment_check_test.py
"""
import json
import math
import sys
import tempfile
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
    assert abs(rows[0]['gap'] - 5.0) < 1e-9 and rows[0]['pass'] is (5.0 <= c.PASS_TOLERANCE)
    assert rows[1]['live'] is None and rows[1]['gap'] is None and rows[1]['pass'] is False


def piece_row(lifted, offset, bone_name='Head'):
    bone = rotated_bone(0.3, 1.0, (50.0, 0.0, 100.0))
    return {'lifted': lifted, 'assets': {'attachments': [{'bone': bone_name, 'bone_matrix': bone, 'bounds': {'origin': world_of(bone, offset)}}]}}


def session_folder(tmp, session, files):
    folder = Path(tmp)
    (folder / 'session.json').write_text(json.dumps(session), encoding='utf-8')
    for name, rows in files.items():
        (folder / name).write_text(chr(10).join(json.dumps(r) for r in rows), encoding='utf-8')
    return folder


def test_only_the_kept_attempt_with_the_target_held_counts():
    with tempfile.TemporaryDirectory() as tmp:
        # A failed first cast (hit) has a wrong placement; the kept retry (hit_try2) holds the target with the right one.
        folder = session_folder(tmp, {'hit_attempt': 'hit_try2', 'hit_result': {'target': 'Pawn_A', 'target_lifted': False},
                                      'hit_try2_result': {'target': 'Pawn_A', 'target_lifted': True}},
                                {'hit.jsonl': [piece_row(None, [90.0, 0.0, 0.0])],
                                 'hit_try2.jsonl': [piece_row(None, [70.0, 0.0, 0.0]), piece_row('Pawn_A', [1.0, 2.0, 2.0])]})
        rows, target = c.held_rows(folder)
        assert target == 'Pawn_A' and all(r['lifted'] == 'Pawn_A' for r in rows)
        result = c.compare(c.live_pieces(rows), [{'bone': 'Head', 'location': [0.0, 0.0, 0.0]}])[0]
        assert abs(result['gap'] - 3.0) < 1e-9 and result['pass']


def test_unusable_sessions_are_refused():
    cases = [({}, {}),
             ({'hit_attempt': 'hit', 'hit_result': {'target': 'Pawn_A', 'target_lifted': False}}, {'hit.jsonl': [piece_row('Pawn_A', [0, 0, 0])]}),
             ({'hit_attempt': 'hit', 'hit_result': {'target': 'Pawn_A', 'target_lifted': True}}, {}),
             ({'hit_attempt': 'hit', 'hit_result': {'target': 'Pawn_A', 'target_lifted': True}}, {'hit.jsonl': [piece_row('Pawn_B', [0, 0, 0])]})]
    for session, files in cases:
        with tempfile.TemporaryDirectory() as tmp:
            try:
                c.held_rows(session_folder(tmp, session, files))
            except c.SessionError:
                continue
            raise AssertionError(f'accepted {session}')


def test_a_gap_beyond_the_tolerance_fails():
    rows = [piece_row('Pawn_A', [c.PASS_TOLERANCE + 1.0, 0.0, 0.0])]
    assert c.compare(c.live_pieces(rows), [{'bone': 'Head', 'location': [0.0, 0.0, 0.0]}])[0]['pass'] is False


def test_a_piece_missing_from_the_host_report_fails():
    rows = [piece_row('Pawn_A', [0.0, 0.0, 0.0], 'Head'), piece_row('Pawn_A', [0.0, 0.0, 0.0], 'Spine2')]
    result = c.compare(c.live_pieces(rows), [{'bone': 'Head', 'location': [0.0, 0.0, 0.0]}])
    assert [r['bone'] for r in result] == ['Head', 'Spine2'] and result[0]['pass'] and result[1]['pass'] is False


def test_unusable_host_reports_are_refused():
    cases = ['{', json.dumps({'Other': {}}), json.dumps({'Bullymong': {'attachments': [{'bone': 'Head'}]}}),
             json.dumps({'Bullymong': {'attachments': [{'bone': 'Head', 'location': [0, 0, 0]}, {'bone': 'Head', 'location': [1, 1, 1]}]}})]
    for text in cases:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'host.json'
            path.write_text(text, encoding='utf-8')
            try:
                c.host_rows(path, 'Bullymong')
            except c.SessionError:
                continue
            raise AssertionError(f'accepted {text}')


def test_a_truncated_session_row_is_refused():
    with tempfile.TemporaryDirectory() as tmp:
        folder = session_folder(tmp, {'hit_attempt': 'hit', 'hit_result': {'target': 'Pawn_A', 'target_lifted': True}}, {})
        (folder / 'hit.jsonl').write_text(json.dumps(piece_row('Pawn_A', [0, 0, 0])) + chr(10) + '{"lifted": "Pa', encoding='utf-8')
        try:
            c.held_rows(folder)
        except c.SessionError:
            return
        raise AssertionError('accepted a truncated row')


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'phaselock_attachment_check_test: {len(tests)} passed')
