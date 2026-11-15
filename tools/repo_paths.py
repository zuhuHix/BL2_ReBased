"""Repository-relative file paths in the slice-npc, ambient-npc and Phaselock-target job files, and their resolution when read.

AI-assisted (Claude Opus 5.5), 2026-11-11. Plain Python (no ``unreal``), tested by tests/editor_inputs_test.py.

Why: ``local/slice/editor_job.json`` and ``local/phaselock/target/editor_job.json`` held absolute paths. Copied into another worktree
(by tools/provision_worktree_assets.ps1 or by hand) they pointed into the worktree that wrote them, and once that one was deleted the
target dummy's zone step found no textures (2026-11-10). The writers now store paths relative to the repository root, and a reader
resolves a job against its own checkout (``resolve-job`` below) into plain absolute paths. The readers were the retired UE5 host's
launchers and editor scripts (git show 08bcf3e:tools/checkout_guard.ps1); the writers and ``resolve-job`` stay.

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

# The two ow-package builds (lane W3-SWITCH, 2026-11-15). The Rust binary (cargo build --release -p ow-cli) is the default
# reader of every tool whose ow-package flags are all ported; the C++ one (build/Release, frozen, see src/package.hpp) stays the
# default of a tool that still calls a flag the Rust binary answers with "not ported to Rust yet" (exit 3).
EXE = '.exe' if os.name == 'nt' else ''
RUST_READER = ROOT / 'target' / 'release' / f'ow-package{EXE}'
CXX_READER = ROOT / 'build' / 'Release' / f'ow-package{EXE}'


def default_reader(waits_for=None):
    """The ``--reader`` default of a tool: the Rust binary, or the C++ one when ``waits_for`` names the ow-package flag the tool
    waits for: one not ported yet (``default_reader(waits_for='--mission-run')``), or a ported one whose Rust output still
    changes the tool's result (the tool says how next to the call). The switch lane that ports a flag finds its tools with
    ``grep -n "waits_for='--flag'" tools/*.py`` and drops the argument. A path the caller passes with ``--reader`` always wins.

    When the Rust binary is not built, a tool run as the program with no ``--reader`` stops here with one line that names the
    path and the build command; it never falls back to the C++ binary. The check is left out when the tool is imported (a
    test calling its ``main()``, a module-level constant), since the default is then not what runs."""
    if waits_for:
        return CXX_READER
    caller = sys._getframe(1).f_globals.get('__name__')
    if caller == '__main__' and not reader_given(sys.argv[1:]) and not RUST_READER.is_file():
        sys.exit(f'{Path(sys.argv[0]).name}: no Rust ow-package at {RUST_READER}; build it with '
                 f'"cargo build --release -p ow-cli --locked", or pass --reader {CXX_READER} for the C++ one')
    return RUST_READER


def reader_given(argv):
    """Whether a command line sets --reader itself (``--reader X`` or ``--reader=X``)."""
    return any(arg == '--reader' or arg.startswith('--reader=') for arg in argv)

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
    entry and ``tools/repo_paths.py``. None when there is none. (Until the UE5 host left the tree the marker was its .uproject.)"""
    folder = Path(os.path.realpath(path))
    for candidate in (folder, *folder.parents):
        if (candidate / '.git').exists() and (candidate / 'tools/repo_paths.py').is_file():
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
        return 'python tools/ambient_npc_assets.py editor-job, or attachments for attach_job.json'
    if 'phaselock' in str(job.get('tool', '')).lower():
        return 'python tools/prepare_phaselock_target.py editor-job'
    return 'python tools/slice_npc_assets.py editor-job'


def resolve_job_file(job_file, out_file, root=ROOT):
    """Reads a stored job, writes the resolved one (plain absolute paths). Raises StalePath."""
    job = json.loads(Path(job_file).read_text(encoding='utf-8'))
    resolved = job_from_repo(job, root, source=str(job_file), regenerate=regenerate_command(job))
    Path(out_file).write_text(json.dumps(resolved, indent=1), encoding='utf-8')
    return resolved


def main(argv=None):
    parser = argparse.ArgumentParser(description='Resolve a stored editor job against this checkout.')
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
