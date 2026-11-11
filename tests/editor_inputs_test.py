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

# Any of these in a call path is a change to content: imports, deletes, creates, saves, and every material-instance mutation the zone step
# makes (clear_all_material_instance_parameters, set_material_instance_parent and the *_parameter_value setters all contain "material").
FORBIDDEN = ('import_asset_tasks', 'delete_directory', 'delete_asset', 'create_asset', 'save_loaded_asset', 'save_directory',
             'set_editor_property', 'set_material', 'clear_all_material_instance_parameters')


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


class StubClassMeta(type):
    """An unreal class (capitalised name): a real type, so isinstance() works; its attributes are recorders, so calls are logged."""
    def __getattr__(cls, name):
        return Recorder(cls._log, f'unreal.{cls.__name__}.{name}')


def stub_attribute(log, name):
    if name[:1].isupper():
        cls = StubClassMeta(name, (Recorder,), {'_log': log})
        original = cls.__init__
        cls.__init__ = lambda self, *args, **kwargs: (log.append(f'unreal.{name}()'), original(self, log, f'unreal.{name}()'))[1]
        return cls
    return Recorder(log, f'unreal.{name}')


def run_with_stub(script, job, mode=None):
    log = []
    stub = types.ModuleType('unreal')
    classes = {}
    stub.__getattr__ = lambda name: classes.setdefault(name, stub_attribute(log, name))
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
        except Exception as raised:   # a stopped step raises RuntimeError; anything else is reported the same way, with what was touched
            error = f'{type(raised).__name__}: {raised}'
        finally:
            sys.modules.pop('unreal', None)
            os.environ.clear()
            os.environ.update(saved)
    return error, log


def touched(log):
    return [call for call in log if any(word in call for word in FORBIDDEN)]


def test_zone_step_stops_before_any_change():
    # A valid zoned slot comes first, so a check placed after (or inside) the loop would already have changed that slot's instance.
    with tempfile.TemporaryDirectory() as tmp:
        good, empty, absent = files(tmp)
        valid = {'slot': 'Good', 'material': 'Mati_Good', 'textures': {'p_Diffuse': good, 'p_Masks': good, 'p_Normal': good}, 'vectors': {},
                 'scalars': {}}
        broken = {'slot': 'Bad', 'material': 'Mati_Bad', 'textures': {'p_Diffuse': good, 'p_Masks': empty}, 'vectors': {}, 'scalars': {}}
        job = {'tool': 'tools/prepare_phaselock_target.py', 'npcs': [
            {'id': 'A', 'dest': '/Game/A', 'mesh_name': 'M', 'master': 'Master_Creature', 'slots': [valid]},
            {'id': 'B', 'dest': '/Game/B', 'mesh_name': 'M', 'master': 'Master_Creature', 'slots': [broken]}]}
        error, log = run_with_stub('phaselock_target_editor.py', job)
    assert error and 'zone: inputs missing' in error and 'B/Bad p_Masks' in error and not touched(log), (error, touched(log))


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



# --- repository-relative job paths (tools/repo_paths.py) and the launchers' checkout guard -----------------------------------------

import repo_paths as rp  # noqa: E402
import shutil  # noqa: E402
import subprocess  # noqa: E402


def synthetic_checkout(base, name):
    """A folder that looks like a checkout of this repository: a .git entry and the project file."""
    root = Path(base) / name
    (root / 'host/ue5/OpenWillow').mkdir(parents=True)
    (root / 'host/ue5/OpenWillow/OpenWillow.uproject').write_text('{}', encoding='utf-8')
    (root / '.git').mkdir()
    return root


def stored_job(root):
    tex = root / 'local/external/umodel/slice-npc/TargetDummy/Textures/T_Mask.png'
    tex.parent.mkdir(parents=True)
    tex.write_bytes(b'x')
    return {'tool': 'tools/slice_npc_assets.py', 'slice_dir': str(root / 'local/slice'), 'npcs': [
        {'id': 'TargetDummy', 'gltf': None, 'tracks': str(root / 'local/slice/anim_tracks_TargetDummy.json'),
         'reference_out': str(root / 'local/slice/ref_pose_TargetDummy.json'),
         'slots': [{'slot': 'S', 'material': 'Mati_X', 'textures': {'p_Masks': str(tex), 'p_Diffuse': str(tex)}}]}]}


def test_job_paths_round_trip_through_the_repository_root():
    with tempfile.TemporaryDirectory() as tmp:
        writer = synthetic_checkout(tmp, 'writer')
        job = stored_job(writer)
        stored = rp.job_to_repo(job, writer)
        npc = stored['npcs'][0]
        assert stored['slice_dir'] == 'local/slice' and npc['reference_out'] == 'local/slice/ref_pose_TargetDummy.json'
        assert npc['slots'][0]['textures']['p_Masks'] == 'local/external/umodel/slice-npc/TargetDummy/Textures/T_Mask.png'
        assert npc['gltf'] is None and npc['slots'][0]['material'] == 'Mati_X'          # no path: unchanged; names: untouched
        assert rp.job_from_repo(stored, writer) == job
        # Copied into another worktree, the same stored job resolves there, and the zone check sees that worktree's files.
        reader = synthetic_checkout(tmp, 'reader')
        shutil.copytree(writer / 'local', reader / 'local')
        shutil.rmtree(writer)
        resolved = rp.job_from_repo(stored, reader)
        assert resolved['slice_dir'] == str(reader / 'local/slice')
        assert e.zone_problem(resolved) is None


def test_old_job_with_absolute_paths():
    with tempfile.TemporaryDirectory() as tmp:
        writer = synthetic_checkout(tmp, 'writer')
        reader = synthetic_checkout(tmp, 'reader')
        old = stored_job(reader)                          # absolute and present in this checkout: read as it is
        assert rp.job_from_repo(old, reader) == old
        gone = stored_job(writer)                         # absolute into a worktree that was deleted afterwards
        gone['slice_dir'] = str(reader / 'local/slice')
        for npc in gone['npcs']:
            npc['reference_out'] = str(reader / 'local/slice/ref.json')
        shutil.rmtree(writer)
        job_file = reader / 'local/slice/editor_job.json'
        job_file.parent.mkdir(parents=True, exist_ok=True)
        job_file.write_text(json.dumps(gone), encoding='utf-8')
        try:
            rp.resolve_job_file(job_file, reader / 'local/slice/editor_job.resolved.json', reader)
            raise AssertionError('a missing absolute path was accepted')
        except rp.StalePath as problem:
            text = str(problem)
        assert str(job_file) in text and str(writer) in text and 'does not exist' in text and 'python tools/slice_npc_assets.py editor-job' in text, text
        assert not (reader / 'local/slice/editor_job.resolved.json').exists()


def test_old_job_writing_into_another_checkout_is_refused():
    with tempfile.TemporaryDirectory() as tmp:
        other = synthetic_checkout(tmp, 'other')          # still exists: the report would land there (2026-11-10)
        here = synthetic_checkout(tmp, 'here')
        job = {'tool': 'tools/prepare_phaselock_target.py', 'slice_dir': str(other / 'local/phaselock/target'), 'npcs': []}
        try:
            rp.job_from_repo(job, here, source='job.json', regenerate=rp.regenerate_command(job))
            raise AssertionError('an output in another checkout was accepted')
        except rp.StalePath as problem:
            assert 'inside another checkout' in str(problem) and 'prepare_phaselock_target.py editor-job' in str(problem), problem


def refused(value, root, output=False):
    try:
        rp.from_repo(value, root, output, source='job.json', regenerate='REWRITE')
    except rp.StalePath as problem:
        assert 'job.json' in str(problem) and 'REWRITE' in str(problem), problem
        return str(problem)
    raise AssertionError(f'{value!r} was accepted')


def test_relative_values_must_stay_in_the_root():
    with tempfile.TemporaryDirectory() as tmp:
        here = synthetic_checkout(tmp, 'runner-z5')
        synthetic_checkout(tmp, 'runner-z55')
        for output in (False, True):
            assert 'leaves the repository root' in refused('../runner-z55/local/slice', here, output)
            assert 'leaves the repository root' in refused('local/../../runner-z55/local', here, output)
            assert 'rooted or drive-relative' in refused(r'\local\x', here, output)
            assert 'rooted or drive-relative' in refused('C:foo', here, output)
        assert rp.from_repo('local/./slice/../slice/x.json', here) == str(here / 'local/slice/x.json')


def test_absolute_input_in_another_checkout_is_refused_and_outside_every_checkout_is_used():
    with tempfile.TemporaryDirectory() as tmp:
        here = synthetic_checkout(tmp, 'here')
        other = synthetic_checkout(tmp, 'runner-z55')
        held = other / 'local/x.png'
        held.parent.mkdir()
        held.write_bytes(b'x')
        assert 'inside another checkout' in refused(str(held), here)            # exists, but it is another worktree's file
        export = Path(tmp) / 'exports/T.png'                                       # the game install, an export folder: allowed
        export.parent.mkdir()
        export.write_bytes(b'x')
        assert rp.from_repo(str(export), here) == str(export)
        assert 'outside this checkout' in refused(str(export), here, output=True)


def junction(link, target):
    """A directory junction (no elevation needed); None when this machine cannot make one, with the reason printed by the caller."""
    made = subprocess.run(f'mklink /J "{link}" "{target}"', shell=True, capture_output=True, text=True)
    return None if made.returncode == 0 else f'{made.stdout.strip()} {made.stderr.strip()}'


def test_junction_rules():
    """The junction rules of tools/worktree-assets.md, (a) to (d), each pinned on synthetic checkouts."""
    with tempfile.TemporaryDirectory() as tmp:
        here = synthetic_checkout(tmp, 'here')
        main = synthetic_checkout(tmp, 'main')
        (main / 'local/slice').mkdir(parents=True)
        (main / 'local/slice/in.png').write_bytes(b'x')
        failed = junction(here / 'local', main / 'local')
        if failed:
            print(f'test_junction_rules: skipped, mklink /J failed: {failed}')
            return
        # (a) local/ junctioned to another checkout's local/ (the shared data root): inputs, outputs, files not written yet are ours.
        assert rp.from_repo('local/slice/in.png', here) == str(here / 'local/slice/in.png')
        assert rp.from_repo('local/slice', here, output=True) == str(here / 'local/slice')
        assert rp.from_repo('local/slice/new/report.json', here, output=True) == str(here / 'local/slice/new/report.json')   # (d)
        assert rp.from_repo(str(here / 'local/slice/in.png'), here) == str(here / 'local/slice/in.png')
        # (a) also for a local/ junctioned to a plain data folder.
        os.rmdir(here / 'local')
        shared = Path(tmp) / 'shared-data'
        shared.mkdir()
        assert junction(here / 'local', shared) is None
        assert rp.from_repo('local/slice', here, output=True) == str(here / 'local/slice')
        # (b) a junction elsewhere into another checkout is refused: local/slice alone, and a folder outside local/.
        os.rmdir(here / 'local')
        (here / 'local').mkdir()
        assert junction(here / 'local/slice', main / 'local/slice') is None
        assert 'inside another checkout' in refused('local/slice/in.png', here)
        assert 'inside another checkout' in refused('local/slice', here, output=True)
        assert junction(here / 'host/data', main / 'local') is None
        assert 'inside another checkout' in refused('host/data/slice/in.png', here)
        # (c) delete the checkout the junction points into: the dangling junction is refused, relative and absolute, input and output.
        shutil.rmtree(main)
        for value, output in (('local/slice/in.png', False), ('local/slice', True), (str(here / 'local/slice/x.json'), True),
                              ('host/data/slice/in.png', False)):
            assert 'target is gone' in refused(value, here, output), value
        # (d) a file and folders not written yet under real, verified folders are fine.
        assert rp.from_repo('local/fresh/editor_report_zone.json', here, output=True) == str(here / 'local/fresh/editor_report_zone.json')
        for link in (here / 'local/slice', here / 'host/data'):
            os.rmdir(link)                                  # junctions only; the temp folder is removed afterwards


POWERSHELL = shutil.which('powershell') or shutil.which('pwsh')


def guard(cwd):
    call = f". '{ROOT / 'tools' / 'checkout_guard.ps1'}'; $r = Assert-OwCheckout '{ROOT / 'tools'}'; Write-Output \"returned=$r\""
    return subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', call], cwd=cwd,
                          capture_output=True, text=True, encoding='utf-8', errors='replace')


def test_checkout_guard():
    if not POWERSHELL:
        print('test_checkout_guard: skipped (no PowerShell)')
        return
    with tempfile.TemporaryDirectory() as tmp:
        other = synthetic_checkout(tmp, 'other')
        (other / 'tools').mkdir()
        refused = guard(other / 'tools')                  # cwd in another checkout of this repository
        assert refused.returncode != 0 and 'another checkout of this repository' in refused.stderr + refused.stdout, refused
        assert f'Repository root: {ROOT}' in refused.stdout, refused.stdout
        plain = guard(tmp)                                 # cwd in no checkout at all: allowed
        assert plain.returncode == 0 and f'returned={ROOT}' in plain.stdout, plain
    ours = guard(ROOT / 'tools')                           # cwd in the script's own checkout
    assert ours.returncode == 0 and f'returned={ROOT}' in ours.stdout, ours
    # The same checkout reached through a junction is the same checkout (compared by physical path, not by text).
    with tempfile.TemporaryDirectory() as tmp:
        link = Path(tmp) / 'via-junction'
        made = subprocess.run(f'mklink /J "{link}" "{ROOT}"', shell=True, capture_output=True, text=True)
        if made.returncode != 0:
            print(f'test_checkout_guard: junction case skipped, mklink /J failed: {made.stdout.strip()} {made.stderr.strip()}')
            return
        try:
            through = guard(link / 'tools')
            assert through.returncode == 0 and f'returned={ROOT}' in through.stdout, through
        finally:
            os.rmdir(link)                                 # removes the junction only, never the checkout behind it


def test_launcher_resolves_the_job_through_the_shared_function():
    """The wiring the three launchers use: Resolve-OwEditorJob runs the resolver, returns the resolved file for OPENWILLOW_SLICE_JOB,
    and throws on a job that does not resolve. Runs without Unreal."""
    if not POWERSHELL:
        print('test_launcher_resolves_the_job_through_the_shared_function: skipped (no PowerShell)')
        return
    with tempfile.TemporaryDirectory() as tmp:
        job = Path(tmp) / 'editor_job.json'
        job.write_text(json.dumps({'tool': 'tools/slice_npc_assets.py', 'slice_dir': 'local/slice', 'npcs': []}), encoding='utf-8')
        call = (f". '{ROOT / 'tools' / 'checkout_guard.ps1'}'; $env:OPENWILLOW_SLICE_JOB = Resolve-OwEditorJob '{sys.executable}' "
                f"'{job}'; Write-Output \"job=$env:OPENWILLOW_SLICE_JOB\"")
        run = lambda: subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', call], cwd=tmp,
                                     capture_output=True, text=True, encoding='utf-8', errors='replace')
        done = run()
        resolved = Path(tmp) / 'editor_job.resolved.json'
        assert done.returncode == 0 and f'job={resolved}' in done.stdout, done
        assert json.loads(resolved.read_text(encoding='utf-8'))['slice_dir'] == str(ROOT / 'local/slice')
        job.write_text(json.dumps({'tool': 'tools/slice_npc_assets.py', 'slice_dir': '../runner-z55/local/slice'}), encoding='utf-8')
        resolved.unlink()
        stale = run()
        assert stale.returncode != 0 and 'leaves the repository root' in stale.stderr and 'does not resolve' in stale.stderr, stale
        assert 'job=' not in stale.stdout and not resolved.exists(), stale


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'editor_inputs_test: {len(tests)} passed')
