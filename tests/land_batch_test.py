"""Synthetic checks for tools/land_batch.sh: throwaway git repositories, a local bare origin and a stub for gh.

No network and no game data. The stub answers the gh calls the script makes from the bare origin's refs, and returns
check runs as raw JSON, so the batch, its stops, the conflict stop, the rerere replay, main's decisions check and the
wait for the required checks run as they would against GitHub, without GitHub. tools/land_batch_checks.py, which picks
the verdict from that JSON, is also tested directly on synthetic answers.
"""
import datetime
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / 'tools' / 'land_batch.sh'
CHECKS_TOOL = REPO / 'tools' / 'land_batch_checks.py'
DECISIONS_TOOL = REPO / 'tools' / 'decisions_index.py'

# Every subprocess here gets stdin=subprocess.DEVNULL: started by a parent with no usable standard input (a
# hidden-desktop wrapper), an inherited invalid handle makes subprocess fail with WinError 6. The stub runs under
# sys.executable, never the py launcher's choice.
# The gh stub. State: GH_STUB_ORIGIN (bare repository), GH_STUB_PRS ("<n>=<branch>" lines), GH_STUB_LOG (one line per
# call), GH_STUB_CHECK (the answers to the check-run calls in turn, comma-separated, the last one repeating: "none",
# "in_progress", "completed:<conclusion>", or "error", which makes gh fail) with its call counter in
# GH_STUB_CHECK_COUNT. The --jq filters it knows are those of tools/land_batch.sh; check runs come back as raw JSON.
STUB = r'''
import datetime, json, os, subprocess, sys
args = sys.argv[1:]
with open(os.environ['GH_STUB_LOG'], 'a') as log:
    log.write(' '.join(args) + '\n')
origin = os.environ['GH_STUB_ORIGIN']
def git(*a):
    return subprocess.run(['git', '--git-dir', origin] + list(a), stdin=subprocess.DEVNULL, capture_output=True,
                          text=True)
if args[:2] == ['repo', 'view']:
    print('synthetic/repo')
    sys.exit(0)
if args[0] != 'api':
    sys.exit('stub: unexpected gh ' + ' '.join(args))
path = args[1]
prs = dict(line.split('=', 1) for line in open(os.environ['GH_STUB_PRS']).read().split())
if '/pulls/' in path:
    jq = args[args.index('--jq') + 1]
    branch = prs[path.rsplit('/', 1)[1]]
    head = git('rev-parse', '-q', '--verify', 'refs/heads/' + branch).stdout.strip()
    if jq.startswith('[.state'):
        print('\t'.join(['open', head, branch, 'main', 'synthetic/repo']))
    elif jq == '.head.sha':
        # GH_STUB_MAIN_BACK=<sha>: someone force-pushes main back to that commit while the script waits for the heads.
        back = os.environ.get('GH_STUB_MAIN_BACK')
        if back:
            git('update-ref', 'refs/heads/main', back)
        print(head)
    elif jq == '.merged':
        print('true' if git('merge-base', '--is-ancestor', head, 'refs/heads/main').returncode == 0 else 'false')
    else:
        sys.exit('stub: unknown filter ' + jq)
    sys.exit(0)
if '/check-runs?check_name=' in path:
    counter = os.environ['GH_STUB_CHECK_COUNT']
    calls = int(open(counter).read()) if os.path.exists(counter) else 0
    open(counter, 'w').write(str(calls + 1))
    answers = os.environ['GH_STUB_CHECK'].split(',')
    answer = answers[min(calls, len(answers) - 1)]
    if answer == 'error':
        sys.stderr.write('HTTP 502: synthetic outage\n')
        sys.exit(1)
    sha = path.split('/')[4]
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    runs = []
    if answer != 'none':
        status, _, conclusion = answer.partition(':')
        runs.append({'name': 'Leak gate', 'head_sha': sha, 'app': {'slug': 'github-actions'}, 'status': status,
                     'conclusion': conclusion or None, 'started_at': now,
                     'completed_at': now if status == 'completed' else None,
                     'html_url': 'https://example.invalid/run/1', 'check_suite': {'id': 7}})
    print(json.dumps({'total_count': len(runs), 'check_runs': runs}))
    sys.exit(0)
if '/actions/runs?head_sha=' in path:
    print(json.dumps({'workflow_runs': [{'check_suite_id': 7, 'event': 'pull_request'}]}))
    sys.exit(0)
sys.exit('stub: unknown path ' + path)
'''

# Two PRs each append one of these to NOTES.md, a plain shared file kept the way the old log was.
ENTRY_ONE = '## 2026-01-02: invented entry one\n\nOne.\n'
ENTRY_TWO = '## 2026-01-02: invented entry two\n\nTwo.\n'
BOTH = ENTRY_ONE + '\n' + ENTRY_TWO
OLD_LOG = '# Notes\n\n## 2026-01-01: invented first note\n\nBody.\n'
STUB_LOG = '# Decisions\n\nThis file holds no entries.\n'
FIRST = 'docs/decisions/2026-01/2026-01-01-01-invented-first.md'
NEW_LAYOUT = {'DECISIONS.md': STUB_LOG, FIRST: '## 2026-01-01: Invented first\n\nAI-assisted (synthetic). Body.\n'}


def find_bash():
    git = shutil.which('git')
    if os.name == 'nt' and git:
        for parent in Path(git).resolve().parents:
            if (parent / 'bin' / 'bash.exe').exists():
                return str(parent / 'bin' / 'bash.exe')
    found = shutil.which('bash')
    if not found:
        sys.exit('land_batch_test: no bash found')
    return found


class World:
    """A bare origin, a seed clone that makes the PR branches, and the gh stub."""

    def __init__(self, tmp, files=None):
        self.tmp = tmp
        empty = tmp / 'gitconfig'
        empty.write_text('')
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL=str(empty), GIT_CONFIG_NOSYSTEM='1',
                        GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                        GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
        self.origin = tmp / 'origin.git'
        self.seed = tmp / 'seed'
        self.git(tmp, 'init', '-q', '--bare', '-b', 'main', str(self.origin))
        self.git(tmp, 'clone', '-q', str(self.origin), str(self.seed))
        self.git(self.seed, 'checkout', '-q', '-b', 'main')
        self.write(self.seed / 'README.md', '# Synthetic\n')
        for name, text in (files or dict(NEW_LAYOUT, **{'NOTES.md': OLD_LOG})).items():
            self.write(self.seed / name, text)
        # main's copy of the decisions check, which the script runs on every rebased PR.
        (self.seed / 'tools').mkdir()
        shutil.copy(DECISIONS_TOOL, self.seed / 'tools' / 'decisions_index.py')
        self.commit(self.seed, 'start')
        self.git(self.seed, 'push', '-q', 'origin', 'main')
        stub_dir = tmp / 'stub'
        stub_dir.mkdir()
        (stub_dir / 'gh_stub.py').write_text(STUB)
        (stub_dir / 'gh').write_text('#!/bin/sh\nexec "%s" "%s" "$@"\n'
                                     % (Path(sys.executable).as_posix(), (stub_dir / 'gh_stub.py').as_posix()))
        os.chmod(stub_dir / 'gh', 0o755)
        self.prs = tmp / 'prs.txt'
        self.prs.write_text('')
        self.calls = tmp / 'gh_calls.log'
        self.env.update(PATH=str(stub_dir) + os.pathsep + os.environ['PATH'], GH_STUB_ORIGIN=str(self.origin),
                        GH_STUB_PRS=str(self.prs), GH_STUB_LOG=str(self.calls), GH_STUB_CHECK='completed:success',
                        GH_STUB_CHECK_COUNT=str(tmp / 'check_count'), PYTHON=Path(sys.executable).as_posix(),
                        CHECK_POLL='0', CHECK_WAIT='60')
        # The script finds its log and its helper through its own location, so it runs from a copy in the throwaway
        # folder.
        (tmp / 'tools').mkdir()
        shutil.copy(SCRIPT, tmp / 'tools' / 'land_batch.sh')
        shutil.copy(CHECKS_TOOL, tmp / 'tools' / 'land_batch_checks.py')
        self.bash = find_bash()

    def git(self, cwd, *args):
        result = subprocess.run(['git'] + list(args), cwd=str(cwd), env=self.env, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True)
        assert result.returncode == 0, (args, result.stdout, result.stderr)
        return result.stdout.strip()

    @staticmethod
    def write(path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode('utf-8'))

    def commit(self, cwd, message):
        self.git(cwd, 'add', '-A')
        self.git(cwd, 'commit', '-q', '-m', message)
        return self.git(cwd, 'rev-parse', 'HEAD')

    def main(self):
        return self.git(self.tmp, '--git-dir', str(self.origin), 'rev-parse', 'refs/heads/main')

    def branch_exists(self, branch):
        return bool(subprocess.run(['git', '--git-dir', str(self.origin), 'rev-parse', '-q', '--verify',
                                    'refs/heads/' + branch], env=self.env, stdin=subprocess.DEVNULL,
                                   capture_output=True).stdout.strip())

    def pr(self, number, files, base='origin/main'):
        """Push branch pr-<number> off base with one commit that writes each {path: text}; returns its head."""
        branch = 'pr-%d' % number
        self.git(self.seed, 'fetch', '-q', 'origin')
        self.git(self.seed, 'checkout', '-q', '-B', branch, base)
        for name, text in files.items():
            self.write(self.seed / name, text)
        head = self.commit(self.seed, 'PR %d' % number)
        self.git(self.seed, 'push', '-q', 'origin', branch)
        with open(self.prs, 'a') as out:
            out.write('%d=%s\n' % (number, branch))
        return head

    def head(self, branch):
        return self.git(self.tmp, '--git-dir', str(self.origin), 'rev-parse', 'refs/heads/' + branch)

    def check_calls(self):
        """The check-run paths the script asked the stub for."""
        if not self.calls.exists():
            return []
        return [line.split()[1] for line in self.calls.read_text().splitlines() if '/check-runs?' in line]

    def landing(self, name):
        path = self.tmp / name
        self.git(self.tmp, 'clone', '-q', str(self.origin), str(path))
        return path

    def land(self, landing, items, extra_env=None, flags=('-D',)):
        env = dict(self.env, **(extra_env or {}))
        if os.path.exists(env['GH_STUB_CHECK_COUNT']):
            os.remove(env['GH_STUB_CHECK_COUNT'])
        cmd = [self.bash, (self.tmp / 'tools' / 'land_batch.sh').as_posix(), '-w', landing.as_posix()] + list(flags)
        return subprocess.run(cmd + [' '.join('%d:%s' % item for item in items)], env=env, stdin=subprocess.DEVNULL,
                              capture_output=True, text=True, timeout=300)


def appended(text):
    return OLD_LOG + '\n' + text


def advance_main(world, files, message):
    """Add one commit to origin/main (empty when files is empty); returns the commit it started from."""
    world.git(world.seed, 'fetch', '-q', 'origin')
    start = world.git(world.seed, 'rev-parse', 'origin/main')
    world.git(world.seed, 'checkout', '-q', '-B', 'main', 'origin/main')
    for name, text in files.items():
        world.write(world.seed / name, text)
    world.git(world.seed, 'add', '-A')
    world.git(world.seed, 'commit', '-q', '--allow-empty', '-m', message)
    world.git(world.seed, 'push', '-q', 'origin', 'main')
    return start


def check_two_prs_land():
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        one = world.pr(1, {'docs/one.md': 'one\n'})
        two = world.pr(2, {'docs/two.md': 'two\n'})
        start = world.main()
        landing = world.landing('landing')
        dry = world.land(landing, [(1, one), (2, two)], {'DRY': '1'})
        assert dry.returncode == 0, dry.stdout + dry.stderr
        assert 'DRY: every check passed' in dry.stdout, dry.stdout
        assert world.main() == start, 'DRY moved main'
        assert world.check_calls() == [], 'DRY asked GitHub for checks'
        done = world.land(landing, [(1, one), (2, two)])
        assert done.returncode == 0, done.stdout + done.stderr
        assert world.main() != start
        tree = world.git(world.tmp, '--git-dir', str(world.origin), 'ls-tree', '-r', '--name-only', 'refs/heads/main')
        assert 'docs/one.md' in tree and 'docs/two.md' in tree, tree
        assert not world.branch_exists('pr-1') and not world.branch_exists('pr-2'), 'landed branches kept'
        assert done.stdout.count('OK #') == 2, done.stdout
        assert world.check_calls() == ['repos/synthetic/repo/commits/%s/check-runs?check_name=Leak%%20gate'
                                       '&filter=latest&per_page=100' % world.main()], world.check_calls()


def check_dirty_worktree_cannot_rewrite_main():
    """The reviewer's case: main is B then an empty M, the PR is B then P, a tracked file in the landing worktree is
    modified. Before 2026-11-12 the rebase refused to start, its failure was ignored, and the leased push replaced M
    with P. Now the script refuses to start, and nothing moves."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        b = advance_main(world, {}, 'empty M')
        m = world.main()
        p = world.pr(1, {'docs/p.md': 'p\n'}, base=b)
        landing = world.landing('landing')
        world.write(landing / 'README.md', '# Changed by hand\n')
        stopped = world.land(landing, [(1, p)])
        assert stopped.returncode == 1 and 'has changes to tracked files' in stopped.stderr, \
            (stopped.returncode, stopped.stdout, stopped.stderr)
        assert world.main() == m and world.head('pr-1') == p, 'a refused run moved something'


def check_failed_rebase_stops():
    """A rebase that fails without leaving a conflict open (here an untracked file is in its way) stops with exit 10
    before anything is pushed."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        b = advance_main(world, {'docs/m.md': 'm\n'}, 'M adds docs/m.md')
        m = world.main()
        p = world.pr(1, {'docs/p.md': 'p\n'}, base=b)
        landing = world.landing('landing')
        world.git(landing, 'checkout', '-q', '--detach', b)
        world.write(landing / 'docs' / 'm.md', 'untracked, in the way\n')
        stopped = world.land(landing, [(1, p)])
        assert stopped.returncode == 10 and 'left no conflict open' in stopped.stdout, (stopped.returncode, stopped.stdout)
        assert world.main() == m and world.head('pr-1') == p, 'a failed rebase moved something'


def check_main_moved_back_is_refused():
    """The reviewer's case: main is B then Y, the PR is based on B, and while the script waits for GitHub to show the
    new PR head someone force-pushes main back to B. The chain still contains B, so a plain push would be a fast-forward
    and bring Y back; the lease on Y refuses it (exit 8), and main stays at B."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        b = advance_main(world, {'docs/y.md': 'taken off again\n'}, 'Y')
        p = world.pr(1, {'docs/p.md': 'p\n'}, base=b)
        landing = world.landing('landing')
        stopped = world.land(landing, [(1, p)], {'GH_STUB_MAIN_BACK': b})
        assert stopped.returncode == 8 and 'main push refused' in stopped.stdout, (stopped.returncode, stopped.stdout)
        assert 'new tested commit' in stopped.stdout, stopped.stdout
        assert world.main() == b, 'main did not stay where it was moved back to'


def check_rebase_that_leaves_head_elsewhere_stops():
    """A rebase that reports success but leaves HEAD off the chain (here a post-rewrite hook in the landing worktree
    puts HEAD back on the PR's own head) stops with exit 10 at the check after the rebase. The final check, that the
    chain contains main, follows from the per-rebase checks (the chain starts at main and every step contains the one
    before), so no setup reaches it alone; with both checks removed this case lands the PR over main's empty M through
    the lease, which only checks what main is, not that the push is a fast-forward."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        b = advance_main(world, {}, 'empty M')
        m = world.main()
        p = world.pr(1, {'docs/p.md': 'p\n'}, base=b)
        landing = world.landing('landing')
        hook = landing / '.git' / 'hooks' / 'post-rewrite'
        hook.write_bytes(('#!/bin/sh\ngit checkout -q --detach %s\n' % p).encode())
        os.chmod(hook, 0o755)
        stopped = world.land(landing, [(1, p)])
        assert stopped.returncode == 10 and 'does not contain' in stopped.stdout, (stopped.returncode, stopped.stdout)
        assert world.main() == m and world.head('pr-1') == p, 'a rebase off the chain moved something'


def check_shared_file_conflicts_without_union():
    """Two PRs appending to one file used to merge as a union; now the second one stops (exit 4) with diff3 markers."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        one = world.pr(1, {'NOTES.md': appended(ENTRY_ONE)})
        two = world.pr(2, {'NOTES.md': appended(ENTRY_TWO)})
        start = world.main()
        landing = world.landing('landing')
        stopped = world.land(landing, [(1, one), (2, two)])
        assert stopped.returncode == 4, (stopped.returncode, stopped.stdout, stopped.stderr)
        assert 'conflict in: NOTES.md' in stopped.stdout, stopped.stdout
        marked = (landing / 'NOTES.md').read_text()
        assert '|||||||' in marked, marked
        assert world.main() == start and world.branch_exists('pr-2'), 'a conflict pushed something'
        world.git(landing, 'rebase', '--abort')


def record_fix(world, landing, one, two, style):
    """Record, under the given conflict style, how two's change rebases onto one (both entries, one first)."""
    world.git(landing, 'config', 'rerere.enabled', 'true')
    world.git(landing, 'checkout', '-q', '--detach', two)
    rebase = subprocess.run(['git', '-c', 'merge.conflictStyle=' + style, 'rebase', '-q', one], cwd=str(landing),
                            env=world.env, stdin=subprocess.DEVNULL, capture_output=True, text=True)
    assert rebase.returncode != 0, 'the synthetic rebase did not conflict'
    world.write(landing / 'NOTES.md', appended(BOTH))
    world.git(landing, 'add', 'NOTES.md')
    world.git(landing, 'rerere')
    world.git(landing, 'rebase', '--abort')


def check_rerere_replay_needs_the_same_style():
    """A fix recorded under diff3 is replayed by the script's rebase, which then finishes; the identity check stops the
    batch (exit 5), because the second PR's diff now has the first entry as context, so two PRs that append to one file
    cannot share a batch. A fix recorded under the default style is not replayed (exit 4): that style moves the blank
    line both entries start with out of the conflict and diff3 keeps it in, so the two conflicts differ."""
    for style, code, message in (('diff3', 5, 'the rebased change differs from the reviewed one'),
                                 ('merge', 4, 'conflict in: NOTES.md')):
        with tempfile.TemporaryDirectory() as tmp:
            world = World(Path(tmp))
            one = world.pr(1, {'NOTES.md': appended(ENTRY_ONE)})
            two = world.pr(2, {'NOTES.md': appended(ENTRY_TWO)})
            start = world.main()
            landing = world.landing('landing')
            record_fix(world, landing, one, two, style)
            done = world.land(landing, [(1, one), (2, two)])
            assert done.returncode == code and message in done.stdout, (style, done.returncode, done.stdout)
            assert world.main() == start, 'a stopped batch moved main'
            if code == 4:
                world.git(landing, 'rebase', '--abort')


def check_main_decisions_check_judges_each_pr():
    """A PR that changes a decision file without an Edits: line stops the batch (exit 5) before anything is pushed,
    though it also rewrites its own copy of the check to pass everything, and though an untracked file in the landing
    worktree carries the Edits: line (the reviewer's case: the check reads the commit, not the worktree). A PR that
    adds a decision file lands."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp), NEW_LAYOUT)
        good = world.pr(1, {'docs/decisions/2026-01/2026-01-02-01-invented-second.md':
                            '## 2026-01-02: Invented second\n\nAI-assisted (synthetic).\n'})
        bad = world.pr(2, {FIRST: NEW_LAYOUT[FIRST] + 'Quietly changed.\n',
                           'tools/decisions_index.py': 'import sys\nsys.exit(0)\n'})
        start = world.main()
        landing = world.landing('landing')
        world.write(landing / 'docs' / 'decisions' / '2026-01' / '2026-01-03-01-untracked-reason.md',
                    '## 2026-01-03: Untracked reason\n\nAI-assisted (synthetic).\n\nEdits: %s\n' % FIRST)
        stopped = world.land(landing, [(1, good), (2, bad)], flags=('-t', bad))
        assert stopped.returncode == 5, (stopped.returncode, stopped.stdout, stopped.stderr)
        assert "#2 fails main's decisions check" in stopped.stdout, stopped.stdout
        assert 'changed: an edit needs' in stopped.stdout, stopped.stdout
        assert world.main() == start and world.head('pr-2') == bad, 'a refused batch pushed something'
        done = world.land(landing, [(1, good)])
        assert done.returncode == 0, done.stdout + done.stderr


def check_required_check_gates_main():
    """main moves only once the required check passed on the chain's tip. A failed run stops with exit 13; a run that
    never completes, and gh failing throughout, stop with exit 14 (the second naming gh's last error); in all three the
    PR branches are already on their rebased commits and main stays. Then main gains a commit, and the rerun with the
    new heads (the second one sits on the first one's old rebased commit) waits through a pending run and lands."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        one = world.pr(1, {'docs/one.md': 'one\n'})
        two = world.pr(2, {'docs/two.md': 'two\n'})
        start = world.main()
        landing = world.landing('landing')
        failed = world.land(landing, [(1, one), (2, two)], {'GH_STUB_CHECK': 'in_progress,completed:failure'})
        assert failed.returncode == 13, (failed.returncode, failed.stdout, failed.stderr)
        assert "check 'Leak gate' failed" in failed.stdout and 'gh run rerun' in failed.stdout, failed.stdout
        assert 'https://example.invalid/run/1' in failed.stdout, failed.stdout
        assert world.main() == start, 'main moved past a failed check'
        new_one, new_two = world.head('pr-1'), world.head('pr-2')
        assert new_one == one and new_two != two, 'the PR branches did not keep their rebased commits'
        assert world.check_calls()[-1].split('/')[4] == new_two, (world.check_calls(), new_two)
        silent = world.land(landing, [(1, new_one), (2, new_two)], {'GH_STUB_CHECK': 'none', 'CHECK_WAIT': '0'})
        assert silent.returncode == 14 and 'after 0s' in silent.stdout, (silent.returncode, silent.stdout)
        down = world.land(landing, [(1, new_one), (2, new_two)], {'GH_STUB_CHECK': 'error', 'CHECK_WAIT': '0'})
        assert down.returncode == 14 and 'last gh error: HTTP 502: synthetic outage' in down.stdout, down.stdout
        assert world.main() == start
        advance_main(world, {'docs/other.md': 'landed meanwhile\n'}, 'main moves')
        moved = world.main()
        before = len(world.check_calls())
        done = world.land(landing, [(1, new_one), (2, new_two)],
                          {'GH_STUB_CHECK': 'none,in_progress,completed:success'})
        assert done.returncode == 0, done.stdout + done.stderr
        assert len(world.check_calls()) - before == 3, world.check_calls()
        landed = world.main()
        assert world.git(world.tmp, '--git-dir', str(world.origin), 'merge-base', '--is-ancestor', moved, landed) == ''
        tree = world.git(world.tmp, '--git-dir', str(world.origin), 'ls-tree', '-r', '--name-only', landed)
        assert all(name in tree for name in ('docs/one.md', 'docs/two.md', 'docs/other.md')), tree


def check_empty_required_checks_refused():
    """With REQUIRED_CHECKS emptied, the script refuses to run rather than move main unchecked."""
    with tempfile.TemporaryDirectory() as tmp:
        world = World(Path(tmp))
        one = world.pr(1, {'docs/one.md': 'one\n'})
        script = world.tmp / 'tools' / 'land_batch.sh'
        text = script.read_bytes().decode()
        assert text.count('REQUIRED_CHECKS=("Leak gate")') == 1
        script.write_bytes(text.replace('REQUIRED_CHECKS=("Leak gate")', 'REQUIRED_CHECKS=()').encode())
        refused = world.land(world.landing('landing'), [(1, one)])
        assert refused.returncode == 64 and 'REQUIRED_CHECKS' in refused.stderr, (refused.returncode, refused.stderr)


def load_checks_tool():
    spec = importlib.util.spec_from_file_location('land_batch_checks', CHECKS_TOOL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_run_selection():
    """tools/land_batch_checks.py on synthetic GitHub answers: one case per rule."""
    tool = load_checks_tool()
    sha, other = 'a' * 40, 'b' * 40
    now = datetime.datetime(2026, 1, 20, 12, 0, 0, tzinfo=datetime.timezone.utc)
    recent, stale = '2026-01-20T11:00:00Z', '2026-01-10T11:00:00Z'
    events = {'workflow_runs': [{'check_suite_id': 1, 'event': 'pull_request'},
                                {'check_suite_id': 2, 'event': 'workflow_dispatch'},
                                {'check_suite_id': 3, 'event': 'push'}]}

    def run(status='completed', conclusion='success', suite=1, at=recent, app='github-actions', head=sha,
            name='Leak gate', started=True):
        return {'name': name, 'head_sha': head, 'app': {'slug': app}, 'status': status,
                'conclusion': conclusion if status == 'completed' else None,
                'started_at': at if started else None, 'completed_at': at if status == 'completed' else None,
                'html_url': 'https://example.invalid/%d' % suite, 'check_suite': {'id': suite}}

    def judge(*runs, workflow=events):
        return tool.verdict({'check_runs': list(runs)}, workflow, sha, 'Leak gate', now)[0]

    assert judge(run()) == 'success'
    assert judge(run(conclusion='failure')) == 'failure'
    assert judge(run(conclusion='cancelled')) == 'failure'
    assert judge(run(conclusion='skipped')) == 'failure'
    assert judge(run(status='in_progress')) == 'pending'
    assert judge() == 'none'
    # A later manual success does not outweigh the PR's failed run, in either order.
    assert judge(run(conclusion='failure'), run(suite=2)) == 'failure'
    assert judge(run(suite=2), run(conclusion='failure')) == 'failure'
    # An older green run and a queued run that has not started yet: wait for the queued one.
    assert judge(run(), run(status='queued', suite=3, started=False)) == 'pending'
    # Only a manual run, only another app's run, or a run on another commit or under another name: nothing counted.
    assert judge(run(suite=2)) == 'none'
    assert judge(run(app='some-other-app')) == 'none'
    assert judge(run(head=other)) == 'none'
    assert judge(run(name='Leak gate (copy)')) == 'none'
    # A run completed more than seven days ago is left out, whether it passed or failed.
    old = tool.verdict({'check_runs': [run(at=stale)]}, events, sha, 'Leak gate', now)
    assert old[0] == 'none' and 'older than 7 days' in old[1], old
    assert judge(run(at=stale, conclusion='failure'), run()) == 'success'
    # A failed push run on the same commit blocks too.
    assert judge(run(), run(suite=3, conclusion='failure')) == 'failure'
    # Answers that cannot be read.
    assert judge(run(), workflow={'message': 'Not Found'}) == 'error'
    assert tool.verdict({'message': 'Bad credentials'}, events, sha, 'Leak gate', now)[0] == 'error'
    with tempfile.TemporaryDirectory() as tmp:
        bad, good = Path(tmp) / 'bad.json', Path(tmp) / 'good.json'
        bad.write_text('<html>502</html>')
        good.write_text(json.dumps(events))
        out = subprocess.run([sys.executable, '-I', str(CHECKS_TOOL), '--sha', sha, '--name', 'Leak gate',
                              '--check-runs', str(bad), '--workflow-runs', str(good)], stdin=subprocess.DEVNULL,
                             capture_output=True, text=True)
        assert out.returncode == 0 and out.stdout.startswith('error\t'), out.stdout


CASES = [check_run_selection, check_two_prs_land, check_dirty_worktree_cannot_rewrite_main, check_failed_rebase_stops,
         check_main_moved_back_is_refused, check_rebase_that_leaves_head_elsewhere_stops,
         check_shared_file_conflicts_without_union, check_rerere_replay_needs_the_same_style,
         check_main_decisions_check_judges_each_pr, check_required_check_gates_main, check_empty_required_checks_refused]

if __name__ == '__main__':
    for case in CASES:
        case()
        print('ok', case.__name__)
    print('land_batch_test: passed (%d cases)' % len(CASES))
