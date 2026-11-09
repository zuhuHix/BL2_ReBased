"""Synthetic tests for tools/editor_inputs.py and the editor steps that use it (no game data, no Unreal: a stub ``unreal`` module).

AI-assisted (Claude), 2026-11-09 (PR #195 review r1). The pure checks: an invalid source in any form is reported, a plain slot the zone step
keeps on purpose is accepted. The editor scripts (zone, attach, the slice npcs mode) are then run against the stub with a bad job: each must
stop with the problem named and make no import, delete, create or save call.
Run: python tests/editor_inputs_test.py
"""
import json
import os
import runpy
import sys
import tempfile
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import editor_inputs as e  # noqa: E402


def files(tmp):
    good = Path(tmp) / 'good.png'
    good.write_bytes(b'x')
    empty = Path(tmp) / 'empty.png'
    empty.write_bytes(b'')
    return str(good), str(empty), str(Path(tmp) / 'absent.png')


def test_file_problem_covers_every_bad_form():
    with tempfile.TemporaryDirectory() as tmp:
        good, empty, absent = files(tmp)
        assert e.file_problem(good) is None
        for bad in (None, 'None', '', '  '):
            assert 'no file in the job' in e.file_problem(bad)
        assert 'missing' in e.file_problem(absent) and 'empty' in e.file_problem(empty)


def test_zone_checks_only_what_it_imports():
    with tempfile.TemporaryDirectory() as tmp:
        good, empty, absent = files(tmp)
        plain = {'slot': 'Body', 'textures': {'p_Diffuse': good, 'p_Normal': good}}          # no p_Masks: kept plain, not checked
        plain_broken = {'slot': 'Head', 'textures': {'p_Diffuse': absent}}                    # the zone step never imports it
        zoned = {'slot': 'Skin', 'textures': {'p_Diffuse': good, 'p_Masks': good, 'p_Normal': good, 'p_CustomPattern': absent}}
        job = {'tool': 'tools/slice_npc_assets.py', 'npcs': [{'id': 'A', 'slots': [plain, plain_broken, zoned]}]}
        assert e.zone_problem(job) is None                                                    # p_CustomPattern is not imported either
        zoned['textures']['p_Masks'] = empty
        assert 'A/Skin p_Masks' in e.zone_problem(job) and 'seed_slice_npc_assets' in e.zone_problem(job)


def test_zone_reports_an_expected_mask_that_was_not_exported():
    job = {'tool': 'tools/prepare_phaselock_target.py', 'npcs': [{'id': 'B', 'slots': [
        {'slot': 'S', 'textures': {}, 'missing_textures': ['p_Masks', 'p_Diffuse', 'p_Normal']}]}]}
    problem = e.zone_problem(job)
    assert 'p_Masks, p_Diffuse not in the UModel output' in problem and 'import_phaselock_target' in problem


def test_npcs_and_attach():
    with tempfile.TemporaryDirectory() as tmp:
        good, empty, absent = files(tmp)
        job = {'npcs': [{'id': 'A', 'gltf': good, 'slots': [{'slot': 'S', 'textures': {'p_Diffuse': good, 'p_Masks': absent}}],
                         'attachment_meshes': [{'id': 'P', 'gltf': good}]}]}
        assert e.npcs_problem(job) is None and e.attach_problem(job) is None                  # npcs does not import p_Masks
        job['npcs'][0]['gltf'] = 'None'
        job['npcs'][0]['slots'][0]['textures']['p_Normal'] = empty
        job['npcs'][0]['attachment_meshes'][0]['gltf'] = None
        assert 'A mesh' in e.npcs_problem(job) and 'p_Normal' in e.npcs_problem(job)
        assert 'A/P' in e.attach_problem(job)


# --- the editor scripts against a stub unreal ------------------------------------------------------------------------------------

FORBIDDEN = ('import_asset_tasks', 'delete_directory', 'delete_asset', 'create_asset', 'save_loaded_asset', 'save_directory',
             'set_editor_property', 'set_material')


class Recorder:
    """Any attribute or call on the stub returns another recorder and logs its dotted path."""
    def __init__(self, log, path):
        self._log, self._path = log, path

    def __getattr__(self, name):
        return Recorder(self._log, f'{self._path}.{name}')

    def __call__(self, *args, **kwargs):
        self._log.append(self._path)
        return Recorder(self._log, self._path + '()')

    def __iter__(self):
        return iter(())


def run_with_stub(script, job, mode=None):
    log = []
    stub = types.ModuleType('unreal')
    stub.__getattr__ = lambda name: Recorder(log, f'unreal.{name}')
    with tempfile.TemporaryDirectory() as tmp:
        job_file = Path(tmp) / 'job.json'
        job.setdefault('slice_dir', tmp)
        job_file.write_text(json.dumps(job), encoding='utf-8')
        saved = dict(os.environ)
        os.environ['OPENWILLOW_SLICE_JOB'] = str(job_file)
        if mode:
            os.environ['OPENWILLOW_SLICE_MODE'] = mode
        sys.modules['unreal'] = stub
        try:
            runpy.run_path(str(ROOT / 'tools' / script), run_name='__main__')
            error = None
        except RuntimeError as raised:
            error = str(raised)
        finally:
            sys.modules.pop('unreal', None)
            os.environ.clear()
            os.environ.update(saved)
    return error, log


def touched(log):
    return [call for call in log if any(word in call for word in FORBIDDEN)]


def test_zone_step_stops_before_any_change():
    job = {'tool': 'tools/prepare_phaselock_target.py', 'npcs': [{'id': 'B', 'dest': '/Game/X', 'mesh_name': 'M', 'master': 'Master_Creature',
           'slots': [{'slot': 'S', 'material': 'Mati_X', 'textures': {}, 'missing_textures': ['p_Masks', 'p_Diffuse']}]}]}
    error, log = run_with_stub('phaselock_target_editor.py', job)
    assert error and 'zone: inputs missing' in error and not touched(log), (error, touched(log))


def test_attach_step_stops_before_any_import():
    job = {'tool': 'tools/prepare_phaselock_target.py', 'npcs': [{'id': 'B', 'dest': '/Game/X',
           'attachment_meshes': [{'id': 'Piece', 'gltf': 'None', 'material': 'Mati_X'}]}]}
    error, log = run_with_stub('phaselock_target_attach_editor.py', job)
    assert error and 'attach: inputs missing' in error and 'B/Piece' in error and not touched(log), (error, touched(log))


def test_npcs_mode_stops_before_clean_or_import():
    with tempfile.TemporaryDirectory() as tmp:
        good, empty, absent = files(tmp)
        job = {'tool': 'tools/slice_npc_assets.py', 'npcs': [
            {'id': 'Fine', 'dest': '/Game/A', 'gltf': good, 'slots': [{'slot': 'S', 'material': 'M', 'textures': {'p_Diffuse': good}}]},
            {'id': 'Broken', 'dest': '/Game/B', 'gltf': good, 'slots': [{'slot': 'S', 'material': 'M', 'textures': {'p_Diffuse': absent}}]}]}
        error, log = run_with_stub('slice_npc_editor.py', job, mode='npcs')
    # The first NPC is fine, but nothing may happen to it either: the check covers the whole job before the first clean().
    assert error and 'Broken/S p_Diffuse' in error and not touched(log), (error, touched(log))


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'editor_inputs_test: {len(tests)} passed')
