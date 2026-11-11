"""Repository-relative file paths in the slice-npc, ambient-npc and Phaselock-target job files, and their resolution when read.

AI-assisted (Claude Opus 5.5), 2026-11-11. Plain Python (no ``unreal``), tested by tests/editor_inputs_test.py.

Why: ``local/slice/editor_job.json`` and ``local/phaselock/target/editor_job.json`` held absolute paths. Copied into another worktree
(by tools/provision_worktree_assets.ps1 or by hand) they pointed into the worktree that wrote them, and once that one was deleted the
target dummy's zone step found no textures (2026-11-10). The writers now store paths relative to the repository root; the
PowerShell launchers resolve a job against their own checkout (``resolve-job`` below) before the editor reads it, so the editor
scripts keep reading plain absolute paths.

A stored path must stay inside this checkout: a relative value that climbs out (``../other/local``), a rooted one (``/local/x``) or a
drive-relative one (``C:foo``) is refused. Junctions follow tools/worktree-assets.md: (a) the real location of ``<root>/local`` is this
checkout's data root wherever it resolves (a shared data root may be another checkout's ``local``), so paths under it are ours; (b) any
other path that really lies in another checkout of this repository is refused, input or output; (c) a junction or symlink on the way
whose target is gone is refused; (d) a file or folder not written yet, below the deepest existing folder, is fine. An old job with
absolute paths is still read when they lie in this checkout; an absolute input outside every checkout (the game install, an export
folder) is used if it exists. Every refusal names the job file, the path and the command that rewrites the job. Nothing is skipped
silently.
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
    """Lexical containment after normalisation (``..`` folded)."""
    try:
        Path(os.path.normpath(os.path.abspath(path))).relative_to(Path(os.path.normpath(os.path.abspath(root))))
        return True
    except ValueError:
        return False


def _same(a, b):
    return os.path.normcase(os.path.realpath(a)) == os.path.normcase(os.path.realpath(b))


def checkout_of(path):
    """The checkout of this repository that really holds ``path`` (junctions followed): the nearest folder upwards with a ``.git``
    entry and ``host/ue5/OpenWillow/OpenWillow.uproject``, as tools/checkout_guard.ps1 tests it. None when there is none."""
    folder = Path(os.path.realpath(path))
    for candidate in (folder, *folder.parents):
        if (candidate / '.git').exists() and (candidate / 'host/ue5/OpenWillow/OpenWillow.uproject').is_file():
            return candidate
    return None


def _refuse_dangling(full, value, kind, source, regenerate):
    """Rule (c): every component of ``full`` that exists as an entry must resolve; a junction or symlink whose target is gone is
    refused. Rule (d): components after the deepest existing one (a file or folder not written yet) are fine, since nothing that does
    not exist can redirect the path."""
    parts = Path(full).parts
    for count in range(1, len(parts) + 1):
        prefix = os.path.join(*parts[:count])
        if not os.path.lexists(prefix):
            return
        if not os.path.exists(prefix):
            raise StalePath(f'{source}: {kind} path {value} goes through {prefix}, a junction or symlink whose target is gone. '
                            f'Rewrite the job here: {regenerate}')


def _in_data_root(full, root):
    """Rule (a): ``full`` is lexically under ``<root>/local`` and really inside the real location of ``<root>/local``."""
    local = os.path.join(root, 'local')
    if not os.path.exists(local) or not _inside(full, local):
        return False
    return _inside(os.path.normcase(os.path.realpath(full)), os.path.normcase(os.path.realpath(local)))


def to_repo(path, root=ROOT):
    """A path below ``root`` as a forward-slash relative string; anything else (outside, None, empty) unchanged."""
    if path is None or str(path).strip() in ('', 'None'):
        return path
    if not Path(path).is_absolute() or not _inside(path, root):
        return str(path)
    return Path(os.path.normpath(os.path.abspath(path))).relative_to(Path(os.path.normpath(os.path.abspath(root)))).as_posix()


def from_repo(value, root=ROOT, output=False, source='the job', regenerate='its producer'):
    """The absolute path for a stored value, or StalePath (see the module text for the rules). None / "" / "None" are returned
    unchanged (tools/editor_inputs.py reports them)."""
    if value is None or str(value).strip() in ('', 'None'):
        return value
    kind = 'output' if output else 'input'
    path = Path(value)
    if path.is_absolute():
        full = os.path.normpath(value)
    elif path.drive or path.root:
        raise StalePath(f'{source}: {kind} path {value} is rooted or drive-relative, not relative to the repository root. '
                        f'Rewrite the job here: {regenerate}')
    else:
        full = os.path.normpath(os.path.join(root, value))
        if not _inside(full, root):
            raise StalePath(f'{source}: {kind} path {value} leaves the repository root ({root}). Rewrite the job here: {regenerate}')
    _refuse_dangling(full, value, kind, source, regenerate)
    if _in_data_root(full, root):
        return full          # rule (a): under <root>/local, wherever it really lives (a shared data root may be another checkout's local)
    home = checkout_of(full)
    if home is not None and not _same(home, root):
        raise StalePath(f'{source}: {kind} path {value} is inside another checkout ({home}), not this one ({root}); the job was '
                        f'written there, or a folder on the way is a junction into it. Rewrite the job here: {regenerate}')
    if _inside(full, root):
        return full          # this checkout's own path; a file not written yet (anim tracks) is editor_inputs.py's business
    if output:
        raise StalePath(f'{source}: output path {value} is outside this checkout ({root}); the job was written in another worktree. '
                        f'Rewrite it here: {regenerate}')
    if os.path.exists(full):
        return full          # outside every checkout: the game install, an export folder
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
    if 'ambient' in str(job.get('tool', '')).lower():
        return ('python tools/ambient_npc_assets.py editor-job, or attachments for attach_job.json '
                '(or powershell -File tools/seed_ambient_npc_assets.ps1)')
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
