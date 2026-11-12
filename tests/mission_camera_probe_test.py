"""Synthetic tests for the camera part of the mission screen probe (no game: unrealsdk is stubbed, and the camera, pawn and clock are invented).

Run: python tests/mission_camera_probe_test.py   (or python -m pytest tests/mission_camera_probe_test.py -q)
"""
import importlib.util
import math
import sys
import tempfile
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CAPTURE = ROOT / 'tools/real_game/mission_screen_capture.ps1'


def load_probe():
    sdk = types.ModuleType('unrealsdk')
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = types.SimpleNamespace(PRE=0, POST=1)
    hooks.add_hook = hooks.remove_hook = lambda *a: None
    sdk.find_object = lambda *a: None
    sdk.make_struct = lambda kind: kind   # GetPlayerViewPoint's two out parameters; the stand-in controller ignores them
    sys.modules.update({'unrealsdk': sdk, 'unrealsdk.hooks': hooks})
    spec = importlib.util.spec_from_file_location('mission_screen_probe_camera', ROOT / 'tools/real_game/scripts/mission_screen_probe.py')
    probe = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(probe)
    return probe


def ns(**fields):
    return types.SimpleNamespace(**fields)


def vec(x, y, z):
    return ns(X=x, Y=y, Z=z)


def rot(pitch, yaw, roll):
    return ns(Pitch=pitch, Yaw=yaw, Roll=roll)


# ms_view_geometry

def test_a_point_straight_ahead_is_the_centre():
    probe = load_probe()
    out = probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [10, 0, 0])
    assert out['ndc'] == [0.0, 0.0] and out['forward'] == 10.0 and out['distance'] == 10.0, out


def test_yaw_a_quarter_turn_puts_the_y_axis_forward():
    probe = load_probe()
    out = probe.ms_view_geometry([0, 0, 0], [0, 16384, 0], 90, [0, 10, 0])
    assert out['ndc'] == [0.0, 0.0] and out['forward'] == 10.0, out


def test_right_and_up_are_positive_x_and_y():
    probe = load_probe()
    out = probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [10, 2, 1], aspect=16 / 9)
    # Half the frame is tan(45) = 1 at depth 10: x = 2/10, and y = (1/10) / (1 / (16/9)).
    assert out['ndc'][0] > 0 and out['ndc'][1] > 0, out
    assert out['ndc'] == [0.2, 0.1778], out


def test_the_frame_edge_is_one():
    probe = load_probe()
    out = probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [5, 5, 0])
    assert out['ndc'] == [1.0, 0.0], out
    # The edge of the vertical frame: the half height is the half width over the aspect.
    tan_half_v = math.tan(math.radians(90) / 2) / (16 / 9)
    out = probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [1, 0, tan_half_v])
    assert out['ndc'] == [0.0, 1.0], out


def test_an_unsigned_pitch_reads_as_the_negative_one():
    probe = load_probe()
    unsigned = probe.ms_view_geometry([0, 0, 0], [58384, 0, 0], 90, [10, 3, -4])
    signed = probe.ms_view_geometry([0, 0, 0], [-7152, 0, 0], 90, [10, 3, -4])
    assert unsigned == signed, (unsigned, signed)
    # 58384 is -7152 units, about 39 degrees nose down.
    assert probe._ms_angle(58384) == probe._ms_angle(-7152) < 0


def test_a_point_behind_the_camera_has_no_ndc():
    probe = load_probe()
    out = probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [-10, 0, 0])
    assert out['ndc'] is None and out['forward'] == -10.0 and out['distance'] == 10.0, out
    assert probe.ms_view_geometry([0, 0, 0], [0, 0, 0], 90, [0, 5, 0])['ndc'] is None   # level with the camera


def test_the_camera_position_is_taken_off_the_point():
    probe = load_probe()
    out = probe.ms_view_geometry([100, 200, 300], [0, 0, 0], 90, [110, 200, 300])
    assert out['distance'] == 10.0 and out['ndc'] == [0.0, 0.0], out


# The camera trace

class Clock:
    now = 0.0


def fresh(probe):
    probe.MS.update(rows=[], t0=0.0, on=True, movie_path=None, down=None, missing=[], cash_before=None, cam=None, hook_errors=0)
    Clock.now = 0.0
    probe._ms_now = lambda: Clock.now


def camera_pc(loc=(1.0, 2.0, 3.0), rotation=(1, 2, 3), fov=80.0):
    """The player's own controller as BL2 has it (session 1 analysis, A1): no PlayerCamera; GetPlayerViewPoint fills the location and
    rotation, GetFOVAngle gives the field of view."""
    return ns(GetPlayerViewPoint=lambda a, b: (None, vec(*loc), rot(*rotation)), GetFOVAngle=lambda: fov)


def test_the_trace_samples_each_tick_until_its_time_is_up():
    probe = load_probe()
    fresh(probe)
    probe.get_pc = lambda: camera_pc()
    assert probe.ms_cam_trace(2.0) == 2.0
    for t in (0.0, 0.5, 1.9):
        Clock.now = t
        probe._ms_tick(None, None, None, None)
    Clock.now = 2.5
    probe._ms_tick(None, None, None, None)
    rows = [r for r in probe.MS['rows'] if r.get('tag') == 'cam']
    assert [r['t'] for r in rows] == [0.0, 0.5, 1.9], rows
    assert rows[1]['cam'] == [[1.0, 2.0, 3.0], [1, 2, 3], 80.0], rows[1]


def test_the_trace_is_capped_per_trace():
    probe = load_probe()
    fresh(probe)
    probe.get_pc = lambda: camera_pc()
    probe.ms_cam_trace(100.0)
    for i in range(probe.MS_CAM_MAX + 50):
        Clock.now = i * 0.01
        probe._ms_tick(None, None, None, None)
    rows = [r for r in probe.MS['rows'] if r.get('tag') == 'cam']
    assert len(rows) == probe.MS_CAM_MAX, len(rows)
    # A new trace starts its own count.
    Clock.now = 7.0               # the first trace ran to 6.49 s; a new one starts from here
    probe.ms_cam_trace(1.0)
    Clock.now = 7.5
    probe._ms_tick(None, None, None, None)
    assert len([r for r in probe.MS['rows'] if r.get('tag') == 'cam']) == probe.MS_CAM_MAX + 1


def test_a_camera_error_is_a_row_not_a_failed_tick():
    probe = load_probe()
    fresh(probe)
    probe.get_pc = lambda: ns()   # no PlayerCamera in this build
    probe.ms_cam_trace(1.0)
    probe._ms_tick(None, None, None, None)
    row = probe.MS['rows'][-1]
    assert row['tag'] == 'cam' and 'error' in row and 'cam' not in row, row


def test_no_trace_armed_means_no_rows_and_the_down_job_still_runs():
    probe = load_probe()
    fresh(probe)
    calls = []
    probe.get_pc = lambda: calls.append('pc') or ns()
    probe._ms_tick(None, None, None, None)
    assert calls == [] and probe.MS['rows'] == []   # no job and no trace: nothing read
    # With a down job, the tick reads the pawn as before: an injured pawn is marked down.
    pawn = ns(bIsInjured=True)
    probe.get_pc = lambda: ns(Pawn=pawn)
    probe.MS['down'] = {'hits': 2, 'next': 0.0, 'done': False, 'injured': False}
    probe._ms_tick(None, None, None, None)
    assert probe.MS['down']['done'] and probe.MS['down']['injured']
    assert probe.MS['rows'][-1] == {'t': 0.0, 'tag': 'mark', 'func': 'down', 'args': {'hits': 2}}, probe.MS['rows']


def test_the_trace_stops_when_the_recorder_stops():
    probe = load_probe()
    fresh(probe)
    probe.get_pc = lambda: camera_pc()
    probe.ms_cam_trace(2.0)
    probe.MS['on'] = False
    probe._ms_tick(None, None, None, None)
    assert probe.MS['rows'] == []


# ms_read: the camera, the viewport, Marcus and the focus geometry

MARCUS = 'Sanctuary_Dynamic.TheWorld:PersistentLevel.WillowAIPawn_13'


def marcus_pawn():
    bone = vec(1000.0, 0.0, 60.0)
    return ns(Location=vec(1000.0, 0.0, 0.0), Rotation=rot(0, 32768, 0), EyeHeight=50.0, BaseEyeHeight=60.0,
              Mesh=ns(GetBoneLocation=lambda name, space: bone if name == 'Head' else None),
              AIClass=ns(FocusRadius=250.0, FocusOffset=vec(0.0, 0.0, 40.0)))


def read_pc(viewport=(1280, 720)):
    pc = camera_pc(loc=(0.0, 0.0, 0.0), rotation=(0, 0, 0), fov=90.0)
    pc.DefaultFOV = 90.0   # the controller's own field
    pc.FocusObject = ns(_path_name=lambda: 'Sanctuary.Marcus')
    pc.Player = ns(ViewportClient=ns(Viewport=ns(SizeX=viewport[0], SizeY=viewport[1])))
    pc.Pawn = ns(Location=vec(0.0, 0.0, 0.0))
    pc.WorldInfo = ns()           # read at the top of ms_read, outside any try: the fields under it fail, each with its own error
    return pc


def setup_read(probe, pc, pawn=None, viewport=(1280, 720)):
    fresh(probe)
    probe.get_pc = lambda: pc
    probe.MARCUS_PATH = MARCUS
    probe.RG_OUT = tempfile.mkdtemp()
    pawn = pawn or marcus_pawn()
    sdk = sys.modules['unrealsdk']
    sdk.find_object = lambda cls, path: pawn if (cls, path) == ('WillowAIPawn', MARCUS) else None
    probe.unrealsdk = sdk


def test_ms_read_records_the_camera_the_viewport_marcus_and_the_focus():
    probe = load_probe()
    setup_read(probe, read_pc())
    row = probe.ms_read('invented')
    assert row['camera'] == {'location': [0.0, 0.0, 0.0], 'rotation': [0, 0, 0], 'fov': 90.0}, row
    assert row['camera_fov_default'] == 90.0 and row['focus_object'] == 'Sanctuary.Marcus', row
    assert row['viewport'] == [1280, 720], row
    m = row['marcus']
    assert m['location'] == [1000.0, 0.0, 0.0] and m['rotation'] == [0, 32768, 0] and m['head_bone'] == [1000.0, 0.0, 60.0], m
    assert m['EyeHeight'] == 50.0 and m['BaseEyeHeight'] == 60.0 and m['FocusRadius'] == 250.0 and m['FocusOffset'] == [0.0, 0.0, 40.0], m
    # The head is dead ahead, 60 up: x 0 and y 0.06 * (16/9) at 16:9.
    focus = row['focus']
    assert focus['aspect'] == round(16 / 9, 4) and focus['head']['ndc'] == [0.0, round(0.06 * 16 / 9, 4)], focus
    assert focus['eye']['distance'] == round(math.sqrt(1000 ** 2 + 60 ** 2), 4), focus
    assert not [k for k in ('camera_error', 'camera_fov_default_error', 'focus_object_error', 'viewport_error', 'marcus_error', 'focus_error') if k in row], row


def test_each_field_that_fails_records_its_own_error():
    probe = load_probe()
    pc = read_pc()
    del pc.FocusObject            # no such property in this build
    pc.Player = ns()              # no viewport client
    pawn = marcus_pawn()
    del pawn.Mesh                 # no head bone
    setup_read(probe, pc, pawn=pawn)
    row = probe.ms_read('invented')
    assert 'focus_object_error' in row and 'viewport_error' in row, row
    assert row['marcus']['head_bone_error'] and 'location' in row['marcus'] and 'head_bone' not in row['marcus'], row['marcus']
    # Without a head bone the focus cannot be computed, and that is said rather than raised.
    assert 'focus_error' in row and 'focus' not in row, row


def test_no_marcus_path_and_no_pawn_are_errors_in_the_row():
    probe = load_probe()
    setup_read(probe, read_pc())
    del probe.MARCUS_PATH
    row = probe.ms_read('invented')
    assert row['marcus_error'] == 'MARCUS_PATH is not set' and 'marcus' not in row and 'focus' not in row, row
    probe.MARCUS_PATH = 'Nowhere.Pawn'
    row = probe.ms_read('invented')
    assert 'no pawn' in row['marcus_error'] and 'focus' not in row, row


def test_the_viewport_falls_back_to_the_size_call_and_defaults_the_aspect():
    probe = load_probe()
    pc = read_pc()
    pc.Player = ns(ViewportClient=ns(GetViewportSize=lambda: ns(X=800, Y=800)))   # no viewport fields: the size call answers
    setup_read(probe, pc)
    row = probe.ms_read('invented')
    assert row['viewport'] == [800, 800] and row['focus']['aspect'] == 1.0, row
    # No size at all: the aspect is 16:9 and the error is recorded.
    bare = read_pc()
    bare.Player = ns(ViewportClient=ns())
    setup_read(probe, bare)
    row = probe.ms_read('invented')
    assert 'viewport_error' in row and row['focus']['aspect'] == round(16 / 9, 4), row


def test_the_capture_arms_the_trace_before_the_presses_it_describes():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    open_body = text[text.index('function Open-Screen('):text.index('\n}\n', text.index('function Open-Screen('))]
    arm = open_body.index("Py 'print(ms_cam_trace(2.0))'")
    assert arm < open_body.index('$null = Send-Key E'), open_body
    assert "if ($Label -in 'offer', 'turnin')" in open_body
    stages = text[text.index("$log.hooks = PyJson 'ms_start()'"):text.index('\n} catch {\n    $runError')]
    # The two closes of this run, the accept on the offer and the reward taken, are traced: each arm comes right before its Enter.
    assert ("if ($row.status -eq 0) { Py 'print(ms_cam_trace(2.0))' | Out-Null }\n"
            "    if ($row.status -eq 0) { $accept = Test-Key 'offer' 'Enter' }") in stages
    reward = stages.index("Py \"print(ms_mark('reward_press_Enter'))\"")
    assert stages.index("ms_cam_trace(2.0)", reward) < stages.index("$after = Test-Key 'reward' 'Enter'", reward)
    assert stages.count('ms_cam_trace(2.0)') == 2
    # No key was added: Esc is still never sent.
    assert 'Send-Key Esc' not in stages


if __name__ == '__main__':
    for name, fn in list(globals().items()):
        if name.startswith('test_') and callable(fn):
            fn()
    print('ok')
