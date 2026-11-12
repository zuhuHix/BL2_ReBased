"""Synthetic checks for tools/land_batch.sh: throwaway git repositories, a local bare origin and a stub for gh.

No network and no game data. The stub answers the few gh calls the script makes from the bare origin's refs, so the
batch and its stops run as they would against GitHub, without GitHub.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / 'tools' / 'land_batch.sh'

# The gh stub. State: GH_STUB_ORIGIN (bare repository), GH_STUB_PRS ("<n>=<branch>" lines), GH_STUB_LOG (one line per
# call). It knows only the --jq filters tools/land_batch.sh passes.
STUB = r'''
import os, subprocess, sys
args = sys.argv[1:]
with open(os.environ['GH_STUB_LOG'], 'a') as log:
    log.write(' '.join(args) + '\n')
origin = os.environ['GH_STUB_ORIGIN']
def git(*a):
    return subprocess.run(['git', '--git-dir', origin] + list(a), capture_output=True, text=True)
if args[:2] == ['repo', 'view']:
    print('synthetic/repo')
    sys.exit(0)
if args[0] != 'api':
    sys.exit('stub: unexpected gh ' + ' '.join(args))
path, jq = args[1], args[args.index('--jq') + 1]
prs = dict(line.split('=', 1) for line in open(os.environ['GH_STUB_PRS']).read().split())
if '/pulls/' in path:
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
sys.exit('stub: unknown path ' + path)
'''



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

    def __init__(self, tmp):
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
        self.write(self.seed / 'DECISIONS.md', '# Decisions\n\n## 2026-01-01: invented first entry\n\nBody.\n')
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
                        GH_STUB_PRS=str(self.prs), GH_STUB_LOG=str(self.calls))
        # The script finds its log through its own location, so it runs from a copy inside the throwaway folder.
        (tmp / 'tools').mkdir()
        shutil.copy(SCRIPT, tmp / 'tools' / 'land_batch.sh')
        self.bash = find_bash()

    def git(self, cwd, *args):
        result = subprocess.run(['git'] + list(args), cwd=str(cwd), env=self.env, capture_output=True, text=True)
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
                                    'refs/heads/' + branch], env=self.env, capture_output=True).stdout.strip())

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

    def landing(self, name):
        path = self.tmp / name
        self.git(self.tmp, 'clone', '-q', str(self.origin), str(path))
        return path

    def land(self, landing, items, extra_env=None, flags=('-D',)):
        env = dict(self.env, **(extra_env or {}))
        cmd = [self.bash, (self.tmp / 'tools' / 'land_batch.sh').as_posix(), '-w', landing.as_posix()] + list(flags)
        return subprocess.run(cmd + [' '.join('%d:%s' % item for item in items)], env=env, capture_output=True,
                              text=True, timeout=300)


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
        done = world.land(landing, [(1, one), (2, two)])
        assert done.returncode == 0, done.stdout + done.stderr
        assert world.main() != start
        tree = world.git(world.tmp, '--git-dir', str(world.origin), 'ls-tree', '-r', '--name-only', 'refs/heads/main')
        assert 'docs/one.md' in tree and 'docs/two.md' in tree, tree
        assert not world.branch_exists('pr-1') and not world.branch_exists('pr-2'), 'landed branches kept'
        assert done.stdout.count('OK #') == 2, done.stdout


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
        assert stopped.returncode == 1 and 'has changes to tracked files' in stopped.stderr, (stopped.returncode, stopped.stdout, stopped.stderr)
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


CASES = [check_two_prs_land, check_dirty_worktree_cannot_rewrite_main, check_failed_rebase_stops,
         check_main_moved_back_is_refused, check_rebase_that_leaves_head_elsewhere_stops]

if __name__ == '__main__':
    for case in CASES:
        case()
        print('ok', case.__name__)
    print('land_batch_test: passed (%d cases)' % len(CASES))
