"""Repository-relative file paths in the slice-npc and Phaselock-target job files, and their resolution when read.

AI-assisted (Claude Opus 5.5), 2026-11-11. Plain Python (no ``unreal``), tested by tests/editor_inputs_test.py.

Why: ``local/slice/editor_job.json`` and ``local/phaselock/target/editor_job.json`` held absolute paths. Copied into another worktree
(by tools/provision_worktree_assets.ps1 or by hand) they pointed into the worktree that wrote them, and once that one was deleted the
target dummy's zone step found no textures (2026-11-10). The writers now store paths relative to the repository root; the
PowerShell launchers resolve a job against their own checkout (``resolve-job`` below) before the editor reads it, so the editor
scripts keep reading plain absolute paths.

An old job with absolute paths is still read: an input that exists is used as it is; an input that does not, and any output outside
this checkout, stops with the job file, the path and the command that rewrites the job. Nothing is skipped silently.
"""
import argparse
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]

# Job keys holding a file the editor reads (inputs) or writes (outputs). ``textures`` maps a parameter to a file.
INPUT_KEYS = ('gltf', 'tracks', 'sample_gltf', 'candidates_gltf', 'props')
OUTPUT_KEYS = ('slice_dir', 'reference_out')


class StalePath(Exception):
    """A path in a job or manifest that does not resolve in this checkout."""


def _inside(path, root):
    try:
        Path(os.path.abspath(path)).relative_to(Path(os.path.abspath(root)))
        return True
    except ValueError:
        return False


def to_repo(path, root=ROOT):
    """A path below ``root`` as a forward-slash relative string; anything else (outside, None, empty) unchanged."""
    if path is None or str(path).strip() in ('', 'None'):
        return path
    if not Path(path).is_absolute() or not _inside(path, root):
        return str(path)
    return Path(os.path.abspath(path)).relative_to(Path(os.path.abspath(root))).as_posix()


def from_repo(value, root=ROOT, output=False, source='the job', regenerate='its producer'):
    """The absolute path for a stored value. Relative: below ``root``. Absolute (an old job): kept if it lies in this checkout; outside
    it, an input is used only if it exists and an output is refused. None / "" / "None" are returned unchanged (tools/editor_inputs.py
    reports them)."""
    if value is None or str(value).strip() in ('', 'None'):
        return value
    path = Path(value)
    if not path.is_absolute():
        return str(Path(root) / path)
    if _inside(path, root):
        return str(path)     # this checkout's own path; a file not written yet (anim tracks) is editor_inputs.py's business
    if output:
        raise StalePath(f'{source}: output path {value} is outside this checkout ({root}); the job was written in another worktree. '
                        f'Rewrite it here: {regenerate}')
    if path.exists():
        return str(path)
    raise StalePath(f'{source}: {value} does not exist (an absolute path from an older job, probably written in another worktree). '
                    f'Rewrite the job here: {regenerate}')


def _walk(node, convert):
    if isinstance(node, list):
        return [_walk(item, convert) for item in node]
    if not isinstance(node, dict):
        return node
    out = {}
    for key, value in node.items():
        if key in INPUT_KEYS and isinstance(value, str):
            out[key] = convert(value, False)
        elif key in OUTPUT_KEYS and isinstance(value, str):
            out[key] = convert(value, True)
        elif key == 'textures' and isinstance(value, dict):
            out[key] = {p: convert(v, False) if isinstance(v, str) else v for p, v in value.items()}
        else:
            out[key] = _walk(value, convert)
    return out


def job_to_repo(job, root=ROOT):
    """The job as written: every file path under ``root`` made relative."""
    return _walk(job, lambda value, output: to_repo(value, root))


def job_from_repo(job, root=ROOT, source='the job', regenerate='its producer'):
    """The job as the editor reads it: every path absolute in this checkout. Raises StalePath on the first path that does not resolve."""
    return _walk(job, lambda value, output: from_repo(value, root, output, source, regenerate))


def regenerate_command(job):
    if 'phaselock' in str(job.get('tool', '')).lower():
        return 'python tools/prepare_phaselock_target.py editor-job (or powershell -File tools/import_phaselock_target.ps1 -Steps prepare)'
    return 'python tools/slice_npc_assets.py editor-job (or powershell -File tools/seed_slice_npc_assets.ps1 -Steps import)'


def resolve_job_file(job_file, out_file, root=ROOT):
    """Reads a stored job, writes the resolved one the editor reads. Raises StalePath."""
    job = json.loads(Path(job_file).read_text(encoding='utf-8'))
    resolved = job_from_repo(job, root, source=str(job_file), regenerate=regenerate_command(job))
    Path(out_file).write_text(json.dumps(resolved, indent=1), encoding='utf-8')
    return resolved


def main(argv=None):
    parser = argparse.ArgumentParser(description='Resolve a stored editor job against this checkout for the editor.')
    parser.add_argument('step', choices=['resolve-job'])
    parser.add_argument('job')
    parser.add_argument('out')
    args = parser.parse_args(argv)
    try:
        resolve_job_file(args.job, args.out)
    except StalePath as problem:
        print(f'resolve-job: {problem}', file=sys.stderr)
        return 2
    print(f'resolve-job: {args.out} (paths from {ROOT})')
    return 0


if __name__ == '__main__':
    sys.exit(main())
