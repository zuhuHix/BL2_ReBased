"""Which heavy suites still need to run on a commit, and record the ones that passed.

AI-assisted (Claude Opus 5.5), 2026-11-12. A port of the hub's suites-due.sh, with the same commands, output and ledger.

    python tools/suites_due.py due <commit>                 suites that need a run on <commit> (name, minutes, command)
    python tools/suites_due.py record <commit> <suite>...   these suites passed on <commit>
    python tools/suites_due.py why <commit> <suite>         covered files that changed since that suite's last recorded pass
    python tools/suites_due.py list                         the suite map as read
    --map <file> reads another map (default: tools/suites.txt next to this script).

A pass is kept per suite, keyed by the content of the files the suite covers plus its command, so a suite is due only
when one of those files, or its own map line, differs from every version it passed on. A rebase, a merge, a docs fix or a
second look at the same tree makes nothing due. The key is the git blob hash of the suite's covered "git ls-tree -r"
entries (one per line, a newline inside a file name written as "?") followed by "#cmd <command>", exactly as suites-due.sh
builds it, so passes it recorded stay valid.

Run it inside the repository (any worktree). Passes are kept in <main checkout>/local/hub/test-ledger.txt, one
"suite|key|commit|time" line each; set HUB_DIR to use another folder. The ledger is machine state and stays untracked.
The map format and the path matching are tools/suite_map_lint.py's (docs/TOOLING.md, "Heavy suites: the suite map").
Exit codes: 0 done; 1 from "due" when a suite covers no file at the commit; 2 a refused map or a wrong call.
"""
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from suite_map_lint import covers, map_problems, match1, read_map  # noqa: E402

DEFAULT_MAP = Path(__file__).resolve().parent / 'suites.txt'


class Refused(Exception):
    pass


def git(*args, data=None):
    done = subprocess.run(['git', *args], input=data, capture_output=True)
    if done.returncode:
        raise Refused(f"git {' '.join(args)}: {done.stderr.decode('utf-8', 'replace').strip()}")
    return done.stdout


def commit_of(rev):
    done = subprocess.run(['git', 'rev-parse', '--verify', '--quiet', rev + '^{commit}'], capture_output=True, text=True)
    if done.returncode:
        raise Refused(f'unknown commit: {rev}')
    return done.stdout.strip()


def short(rev):
    return git('rev-parse', '--short', rev).decode().strip()


def load(map_path):
    """The suites in map order, with *ignore* split off; refuses what suites-due.sh refuses."""
    if not Path(map_path).is_file():
        raise Refused(f'no suite map at {map_path}')
    problems = []
    suites = read_map(map_path, problems)
    problems += map_problems(suites)
    if problems:
        raise Refused(f'fix {map_path}:\n' + '\n'.join(problems))
    ignore = next((s[2] for s in suites if s[0] == '*ignore*'), [])
    return [s for s in suites if s[0] != '*ignore*'], ignore


def tree(commit):
    """[(ls-tree entry as bytes, path)] at a commit, in git's order; a newline inside a name becomes '?'."""
    out = []
    for entry in git('ls-tree', '-r', '-z', '--full-tree', commit).split(b'\0'):
        if entry:
            entry = entry.replace(b'\n', b'?')
            out.append((entry, entry.split(b'\t', 1)[1].decode('utf-8', 'surrogateescape')))
    return out


def key(entries, paths, command, ignore, name=None):
    """The ledger key of a suite at a commit, or None when it covers no file there. With a name, warns on stderr about a
    path of the suite that matches no file (before *ignore*, as the shell helper did)."""
    cov = [e for e, p in entries if covers(p, paths, ignore)]
    if name:
        for pattern in paths:
            if not any(match1(p, pattern) for _, p in entries):
                print(f'suites-due: warning: "{pattern}" of suite {name} matches no file at this commit', file=sys.stderr)
    if not cov:
        return None
    blob = b'\n'.join(cov) + b'\n#cmd ' + command.encode('utf-8') + b'\n'
    return git('hash-object', '--stdin', data=blob).decode().strip()


def ledger_path():
    hub = os.environ.get('HUB_DIR')
    if not hub:
        common = Path(git('rev-parse', '--git-common-dir').decode().strip()).resolve()
        hub = common.parent / 'local' / 'hub'
    return Path(hub) / 'test-ledger.txt'


def ledger_lines(ledger):
    if not ledger.is_file():
        return []
    return [l.rstrip('\r').split('|') for l in ledger.read_text(encoding='utf-8', errors='replace').split('\n') if l]


def passed(ledger, name, k):
    return any(len(f) > 1 and f[0] == name and f[1] == k for f in ledger_lines(ledger))


def find(suites, name, map_path):
    for s in suites:
        if s[0] == name:
            return s
    raise Refused(f"no suite named '{name}' in {map_path}")


def cmd_due(rev, map_path):
    suites, ignore = load(map_path)
    c = commit_of(rev)
    entries, ledger = tree(c), ledger_path()
    n = total = 0
    bad = 0
    for name, mins, paths, command in suites:
        k = key(entries, paths, command, ignore, name)
        if k is None:
            print(f'{name:<24} {mins:>5} min   !! covers no file at this commit: fix its paths in suites.txt')
            n += 1
            bad = 1
            continue
        if passed(ledger, name, k):
            continue
        print(f'{name:<24} {mins:>5} min   {command}')
        n += 1
        if mins.isdigit() and mins.isascii():
            total += int(mins)
    if n == 0:
        print(f'nothing due on {short(c)}: every suite already passed on this content')
    else:
        print(f'-- {n} due, about {total} min, on {short(c)} (tree {short(c + "^{tree}")})')
    return bad


def cmd_record(rev, names, map_path):
    suites, ignore = load(map_path)
    c = commit_of(rev)
    entries, ledger = tree(c), ledger_path()
    for name in names:
        _, _, paths, command = find(suites, name, map_path)
        k = key(entries, paths, command, ignore, name)
        if k is None:
            raise Refused(f"suite '{name}' covers no file at {short(c)}: fix its paths before recording")
        if not passed(ledger, name, k):
            ledger.parent.mkdir(parents=True, exist_ok=True)
            with open(ledger, 'a', encoding='utf-8', newline='') as f:
                f.write(f"{name}|{k}|{c}|{time.strftime('%Y-%m-%dT%H:%MZ', time.gmtime())}\n")
        print(f'recorded: {name} passed on {short(c)}')
    return 0


def cmd_why(rev, name, map_path):
    suites, ignore = load(map_path)
    c = commit_of(rev)
    _, _, paths, _ = find(suites, name, map_path)
    last = next((f[2] for f in reversed(ledger_lines(ledger_path())) if len(f) > 2 and f[0] == name), None)
    if not last:
        print(f'{name} has no recorded pass yet')
        return 0
    if subprocess.run(['git', 'cat-file', '-e', last + '^{commit}'], capture_output=True).returncode:
        print(f'{name} last passed on {last[:9]}, which this clone no longer has')
        return 0
    print(f'covered files that differ between its last pass ({short(last)}) and {short(c)}:')
    for p in git('diff', '--name-only', '--no-renames', '-z', last, c).split(b'\0'):
        p = p.replace(b'\n', b'?').decode('utf-8', 'surrogateescape')
        if p and covers(p, paths, ignore):
            print(f'  {p}')
    return 0


def cmd_list(map_path):
    load(map_path)   # refuses a bad map first
    for name, mins, paths, _ in read_map(map_path):
        print(f"{name:<24} {mins:>5} min   {' '.join(paths)}")
    return 0


def main(argv=None):
    # Plain \n line ends and lines in order on a shared log, as the shell helper printed them.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(newline='\n', line_buffering=True)
    args = list(sys.argv[1:] if argv is None else argv)
    map_path = DEFAULT_MAP
    if len(args) >= 2 and args[0] == '--map':
        map_path, args = Path(args[1]), args[2:]
    cmd, rest = (args[0], args[1:]) if args else ('', [])
    try:
        if subprocess.run(['git', 'rev-parse', '--git-dir'], capture_output=True).returncode:
            raise Refused('not inside a git repository')
        if cmd == 'due' and len(rest) == 1:
            return cmd_due(rest[0], map_path)
        if cmd == 'record' and len(rest) >= 2:
            return cmd_record(rest[0], rest[1:], map_path)
        if cmd == 'why' and len(rest) == 2:
            return cmd_why(rest[0], rest[1], map_path)
        if cmd == 'list' and not rest:
            return cmd_list(map_path)
        if cmd in ('due', 'record', 'why', 'list'):
            raise Refused(f'usage: suites_due.py {cmd} ' + {'due': '<commit>', 'record': '<commit> <suite>...',
                                                            'why': '<commit> <suite>', 'list': ''}[cmd])
    except Refused as e:
        print(f'suites-due: {e}', file=sys.stderr)
        return 2
    print(__doc__.split('\n\n')[1])
    return 64


if __name__ == '__main__':
    sys.exit(main())
