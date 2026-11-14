"""Synthetic tests for tools/editor_inputs.py and tools/repo_paths.py (no game data, no editor).

AI-assisted (Claude), 2026-11-09 (PR #195 review r1). The pure checks: an invalid source in any form is reported, a plain slot the zone step
keeps on purpose is accepted. The runs of the retired UE5 host's editor scripts against a stub ``unreal`` module, and of the launchers'
checkout guard, left with those scripts (git show 08bcf3e:tests/editor_inputs_test.py).
Run: python tests/editor_inputs_test.py
"""
import json
import os
import sys
import tempfile
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
        assert 'A/Skin p_Masks' in e.zone_problem(job) and 'slice_npc_assets.py editor-job' in e.zone_problem(job)


def test_zone_reports_an_expected_mask_that_was_not_exported():
    job = {'tool': 'tools/prepare_phaselock_target.py', 'npcs': [{'id': 'B', 'slots': [
        {'slot': 'S', 'textures': {}, 'missing_textures': ['p_Masks', 'p_Diffuse', 'p_Normal']}]}]}
    problem = e.zone_problem(job)
    assert 'p_Masks, p_Diffuse not in the UModel output' in problem and 'prepare_phaselock_target.py editor-job' in problem


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


# --- repository-relative job paths (tools/repo_paths.py) ---------------------------------------------------------------------------

import repo_paths as rp  # noqa: E402
import shutil  # noqa: E402
import subprocess  # noqa: E402


def synthetic_checkout(base, name):
    """A folder that looks like a checkout of this repository: a .git entry and tools/repo_paths.py."""
    root = Path(base) / name
    (root / 'tools').mkdir(parents=True)
    (root / 'tools/repo_paths.py').write_text('', encoding='utf-8')
    (root / 'other').mkdir()
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
        assert junction(here / 'other/data', main / 'local') is None
        assert 'inside another checkout' in refused('other/data/slice/in.png', here)
        # (c) delete the checkout the junction points into: the dangling junction is refused, relative and absolute, input and output.
        shutil.rmtree(main)
        for value, output in (('local/slice/in.png', False), ('local/slice', True), (str(here / 'local/slice/x.json'), True),
                              ('other/data/slice/in.png', False)):
            assert 'target is gone' in refused(value, here, output), value
        # (d) a file and folders not written yet under real, verified folders are fine.
        assert rp.from_repo('local/fresh/editor_report_zone.json', here, output=True) == str(here / 'local/fresh/editor_report_zone.json')
        for link in (here / 'local/slice', here / 'other/data'):
            os.rmdir(link)                                  # junctions only; the temp folder is removed afterwards


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'editor_inputs_test: {len(tests)} passed')
