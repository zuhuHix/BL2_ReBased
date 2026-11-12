"""Check a suite map (the hub's suites.txt format) against a checkout, so a suite's file list cannot quietly miss files.

AI-assisted (Claude Opus 5.5), 2026-11-12.

    python tools/suite_map_lint.py --map <suites.txt> [--repo <checkout>] [--commit <rev>] [--smoke-only <file>]
                                   [--switch-only <file>]

The map: one suite per line, "name | minutes | paths | command"; lines starting with # are comments; a line named *ignore*
lists paths no suite depends on. Paths are separated by spaces: "src/" or "src" (a folder), "tools/run.ps1" (a file),
"*.py" (a suffix), "." (everything). The matching is the hub helper's (suites-due.sh): a file a suite's paths match is
covered unless an *ignore* path also matches it, and a suite that names an ignored path itself (the file, a suffix, or a
folder inside an ignored one) still covers it.

Errors, each one line; the exit code is 1 when there is any, 2 when the map or the checkout cannot be read:
1. unmatched: a path of a suite that matches no tracked file at <rev> (before *ignore* is applied, as the helper warns).
2. not-covered: a file the suite's own script loads from its folder that the suite does not cover. The script is the
   .ps1 after -File in the command; a load is Join-Path $PSScriptRoot '<file>' (dot-sourced, run or read). Loaded .ps1
   files are followed in turn, so a helper's helper counts. Only tracked files count (a folder or a local/ path does not).
   The lint does not read switches: a load behind a switch the suite never passes is reported too. List such a pair in
   --switch-only (one "<suite> <repo path>" per line, # comments allowed) to accept it.
3. uncovered-core: a tracked file under host/ue5/OpenWillow/Source/ or src/ that no suite covers except a whole-tree one
   (a suite whose paths include "."), unless --smoke-only lists it (one repo path per line, # comments allowed).
Reads only git objects at <rev> (default HEAD), not the working tree.
"""
import argparse
import posixpath
import re
import subprocess
import sys
from pathlib import Path

CORE_PREFIXES = ('host/ue5/OpenWillow/Source/', 'src/')
SCRIPT = re.compile(r'-File\s+["\']?([^"\'\s;]+\.ps1)', re.I)
LOAD = re.compile(r'Join-Path\s+\$PSScriptRoot\s+["\']([^"\']+)["\']', re.I)


def match1(path, pattern):
    """One path against one pattern, as suites-due.sh's match1."""
    if pattern == '.':
        return True
    if pattern.startswith('*'):
        return path.endswith(pattern[1:])
    if path == pattern:
        return True
    if not pattern.endswith('/'):
        pattern += '/'
    return path.startswith(pattern)


def hit(path, patterns):
    return any(p and match1(path, p) for p in patterns)


def covers(path, include, ignore):
    """Whether a suite with these include patterns covers the path, *ignore* applied (suites-due.sh's covered())."""
    if not hit(path, include):
        return False
    if not hit(path, ignore):
        return True
    for s in include:   # claimed: the suite names the ignored path itself
        if not s or s == '.' or not match1(path, s):
            continue
        if s == path or s.startswith('*') or hit(s, ignore) or hit(s + '/', ignore):
            return True
    return False


def read_map(path):
    """[(name, minutes, [paths], command)] in map order; the same trimming as the helper's suites()."""
    suites = []
    for raw in Path(path).read_text(encoding='utf-8-sig').splitlines():
        line = re.sub(r'\s*\|\s*', '|', raw.replace('\r', '')).strip()
        if not line or line.startswith('#'):
            continue
        fields = line.split('|')
        while len(fields) < 4:
            fields.append('')
        suites.append((fields[0], fields[1], fields[2].split(), fields[3]))
    return suites


def git(repo, *args):
    done = subprocess.run(['git', '-C', str(repo), *args], capture_output=True)
    if done.returncode:
        raise RuntimeError(f"git {' '.join(args)}: {done.stderr.decode('utf-8', 'replace').strip()}")
    return done.stdout


def tracked(repo, commit):
    out = git(repo, 'ls-tree', '-r', '-z', '--name-only', '--full-tree', commit)
    return [p.decode('utf-8', 'replace').replace('\n', '?') for p in out.split(b'\0') if p]


def loads(repo, commit, script, files):
    """{tracked file: the script that loads it} through Join-Path $PSScriptRoot, following loaded .ps1 files."""
    found, todo, done = {}, [script], set()
    while todo:
        current = todo.pop()
        if current in done:
            continue
        done.add(current)
        text = git(repo, 'show', f'{commit}:{current}').decode('utf-8-sig', 'replace')
        code = '\n'.join(l for l in text.splitlines() if not l.lstrip().startswith('#'))
        for name in LOAD.findall(code):
            target = posixpath.normpath(posixpath.join(posixpath.dirname(current), name.replace('\\', '/')))
            if target not in files or target == current:
                continue
            found.setdefault(target, current)
            if target.lower().endswith('.ps1'):
                todo.append(target)
    return found


def lint(map_path, repo, commit='HEAD', smoke_only=(), switch_only=()):
    suites = read_map(map_path)
    files = tracked(repo, commit)
    fileset = set(files)
    ignore = next((s[2] for s in suites if s[0] == '*ignore*'), [])
    errors = []
    real = [s for s in suites if s[0] != '*ignore*']
    for name, _, include, command in real:
        for pattern in include:
            if not any(match1(f, pattern) for f in files):
                errors.append(f'unmatched: suite {name}: "{pattern}" matches no tracked file')
        m = SCRIPT.search(command)
        if m:
            script = posixpath.normpath(m.group(1).replace('\\', '/'))
            if script not in fileset:
                continue   # error 1 or the helper reports a command whose script is gone; nothing to follow
            for f, loader in loads(repo, commit, script, fileset).items():
                if not covers(f, include, ignore) and (name, f) not in switch_only:
                    via = '' if loader == script else f' (in {loader})'
                    errors.append(f'not-covered: suite {name}: {script} loads {f}{via}, which its paths do not cover')
    narrow = [s for s in real if '.' not in s[2]]
    allowed = set(smoke_only)
    for f in files:
        if f.startswith(CORE_PREFIXES) and f not in allowed and not any(covers(f, s[2], ignore) for s in narrow):
            errors.append(f'uncovered-core: {f} is covered only by a whole-tree suite (add it to a suite or to --smoke-only)')
    return errors, len(real)


def list_file(path):
    if not path:
        return []
    return [l.strip() for l in Path(path).read_text(encoding='utf-8-sig').splitlines() if l.strip() and not l.strip().startswith('#')]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--map', required=True, help='the suite map to check')
    ap.add_argument('--repo', default=str(Path(__file__).resolve().parents[1]), help='the checkout (default: this one)')
    ap.add_argument('--commit', default='HEAD', help='the revision whose tracked files count (default HEAD)')
    ap.add_argument('--smoke-only', help='file of core paths that only a whole-tree suite needs to cover')
    ap.add_argument('--switch-only', help='file of "<suite> <path>" loads that sit behind a switch the suite never passes')
    a = ap.parse_args(argv)
    try:
        smoke = list_file(a.smoke_only)
        pairs = set()
        for line in list_file(a.switch_only):
            parts = line.split()
            if len(parts) != 2:
                raise OSError(f'{a.switch_only}: want "<suite> <path>", got "{line}"')
            pairs.add((parts[0], parts[1]))
        errors, count = lint(a.map, a.repo, a.commit, smoke, pairs)
    except (OSError, RuntimeError) as e:
        print(f'suite_map_lint: {e}', file=sys.stderr)
        return 2
    for e in errors:
        print(e)
    print(f'suite_map_lint: {count} suites, {len(errors)} error(s)')
    return 1 if errors else 0


if __name__ == '__main__':
    sys.exit(main())
