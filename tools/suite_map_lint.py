"""Check a suite map (the hub's suites.txt format) against a checkout, so a suite's file list cannot quietly miss files.

AI-assisted (Claude Opus 5.5), 2026-11-12.

    python tools/suite_map_lint.py [--map <suites.txt>] [--repo <checkout>] [--commit <rev>] [--smoke-only <file>]
                                   [--switch-only <file>]

The map: one suite per line, "name | minutes | paths | command"; lines starting with # are comments; a line named *ignore*
lists paths no suite depends on. Paths are separated by spaces: "src/" or "src" (a folder), "tools/run.ps1" (a file),
"*.py" (a suffix), "." (everything). The matching is the hub helper's (suites-due.sh): a file a suite's paths match is
covered unless an *ignore* path also matches it, and a suite that names an ignored path itself (the file, a suffix, or a
folder inside an ignored one) still covers it.

Errors, each one line; the exit code is 1 when there is any, 2 when the map or the checkout cannot be read:
0. map: what suites-due.sh refuses: a byte-order mark, a name with other than letters, digits, dot, dash and
   underscore, a name listed twice (or twice in different case), a suite with no paths, a "|" inside the command, a * anywhere but the start of a path, a path written with ./,
   a leading / or a backslash, and a map with no suite. covers-nothing: a suite that covers no tracked file once *ignore*
   is applied (the helper's "covers no file at this commit").
1. unmatched: a path of a suite that matches no tracked file at <rev> (before *ignore* is applied, as the helper warns).
   missing-script: the .ps1 after -File in a suite's command is not a tracked file.
2. not-covered: a file the suite's own script loads that the suite does not cover. The script is the .ps1 after -File
   in the command. A load is Join-Path $PSScriptRoot '<file>' (from the script's folder) or Join-Path $repo '<file>'
   (from the repository root, where the script sets $repo with Split-Path -Parent $PSScriptRoot, Assert-OwCheckout
   $PSScriptRoot, (Resolve-Path (Join-Path $PSScriptRoot '..')).Path, or two Split-Path -Parent; a dot-sourced helper
   that does not set $repo uses its caller's), in single or double quotes without a $ inside; a path built from a
   variable ("host/ue5/$job") is not read. Loaded .ps1 files are followed in turn, so a helper's helper counts. Only
   tracked files count (a folder or a local/ path does not), matched without regard to case as PowerShell does.
   Two edge cases: a script at the repository root that sets $repo with Split-Path -Parent is treated as if $repo were
   the root (the real value would be the folder above); a helper that sets $repo in a spelling the lint does not know
   keeps its caller's base instead.
   The lint does not read switches: a load behind a switch the suite never passes is reported too. List such a pair in
   --switch-only (one "<suite> <repo path>" per line, # comments allowed) to accept it. Without --map and --switch-only
   the lint checks the tracked tools/suites.txt with the tracked tools/suites_switch_only.txt (2026-11-13: before, the
   plain command left the list out and reported its pairs, so it disagreed with the CTest entry). --switch-only '' reads none.
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
LOAD = re.compile(r'''Join-Path\s+\$(PSScriptRoot|repo)\s+(?:'([^']+)'|"([^"$`]+)")''', re.I)
NAME = re.compile(r'[A-Za-z0-9._-]+')
# How a script sets $repo, and how many folders above the script's own that is.
REPO_DEFS = [
    (re.compile(r'^\s*\$repo\s*=\s*Split-Path\s+-Parent\s+\(Split-Path\s+-Parent\s+\$PSScriptRoot\)\s*$', re.I | re.M), 2),
    (re.compile(r'^\s*\$repo\s*=\s*(Split-Path\s+-Parent\s+\$PSScriptRoot|Assert-OwCheckout\s+\$PSScriptRoot'
                r'''|\(Resolve-Path\s+\(Join-Path\s+\$PSScriptRoot\s+['"]\.\.['"]\)\)\.Path)\s*$''', re.I | re.M), 1),
]


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


def read_map(path, problems=None, data=None):
    """[(name, minutes, [paths], command)] in map order; the same trimming as the helper's suites(). What the helper
    would refuse while reading (a byte-order mark, a suite with no paths, a "|" inside the command) goes to problems, a
    list, when one is given. data: the map's bytes, when they do not come from the file (a map read from a commit)."""
    suites = []
    if data is None:
        data = Path(path).read_bytes()
    if problems is not None and data.startswith(b'\xef\xbb\xbf'):
        problems.append('map: the file starts with a byte-order mark; suites-due.sh reads it as part of the first line')
    for raw in data.decode('utf-8', 'replace').splitlines():
        line = re.sub(r'\s*\|\s*', '|', raw.replace('\r', '')).strip()
        if not line or line.startswith('#'):
            continue
        fields = line.split('|')
        if problems is not None and (len(fields) < 3 or fields[2] == ''):
            problems.append(f'map: suite "{fields[0]}" has no paths')
        if problems is not None and len(fields) > 4:
            problems.append(f'map: suite "{fields[0]}" has more than 4 fields: keep "|" out of the command')
        while len(fields) < 4:
            fields.append('')
        # Paths split on ASCII spaces and tabs only, as the shell helper's awk did: a no-break space stays in a file name.
        suites.append((fields[0], fields[1], [p for p in re.split('[ \t]+', fields[2]) if p], fields[3]))
    return suites


def map_problems(suites):
    """The rest of the helper's refusals: names, duplicates, path spellings, a map with no suite."""
    out, seen = [], set()
    for name, _, paths, _ in suites:
        if name != '*ignore*' and not NAME.fullmatch(name):
            out.append(f'map: suite name "{name}" may hold only letters, digits, dot, dash and underscore')
        if name.lower() in {n.lower() for n in seen} and name not in seen:
            out.append(f'map: suite "{name}" differs from another suite only in case (the driver would run one twice)')
        if name in seen:
            out.append(f'map: suite "{name}" is listed twice')
        seen.add(name)
        for s in paths:
            if '*' in s[1:]:
                out.append(f'map: suite "{name}": "{s}": a * is allowed only at the start (a suffix such as *.py)')
            if s.startswith('./') or s.startswith('/'):
                out.append(f'map: suite "{name}": "{s}": write paths from the repo root, without ./ or a leading /')
            if '\\' in s:
                out.append(f'map: suite "{name}": "{s}": use / in paths, not a backslash')
    if not any(s[0] != '*ignore*' for s in suites):
        out.append('map: lists no suite')
    return out


def git(repo, *args):
    done = subprocess.run(['git', '-C', str(repo), *args], capture_output=True)
    if done.returncode:
        raise RuntimeError(f"git {' '.join(args)}: {done.stderr.decode('utf-8', 'replace').strip()}")
    return done.stdout


def tracked(repo, commit):
    out = git(repo, 'ls-tree', '-r', '-z', '--name-only', '--full-tree', commit)
    return [p.decode('utf-8', 'replace').replace('\n', '?') for p in out.split(b'\0') if p]


def up(path, n):
    for _ in range(n):
        path = posixpath.dirname(path)
    return path


def loads(repo, commit, script, files):
    """{tracked file: the script that loads it} through Join-Path $PSScriptRoot or $repo, following loaded .ps1 files.
    files maps each tracked path in lower case to its spelling."""
    found, todo, done = {}, [(script, None)], set()
    while todo:
        current, inherited = todo.pop()
        if current in done:
            continue
        done.add(current)
        text = git(repo, 'show', f'{commit}:{current}').decode('utf-8-sig', 'replace')
        code = '\n'.join(l for l in text.splitlines() if not l.lstrip().startswith('#'))
        here = posixpath.dirname(current)
        base = next((up(here, n) for pattern, n in REPO_DEFS if pattern.search(code)), inherited)
        for var, single, double in LOAD.findall(code):
            if var.lower() == 'repo' and base is None:
                continue   # $repo is not set in a way this lint reads
            start = here if var.lower() == 'psscriptroot' else base
            target = files.get(posixpath.normpath(posixpath.join(start, (single or double).replace('\\', '/'))).lower())
            if target is None or target == current:
                continue
            found.setdefault(target, current)
            if target.lower().endswith('.ps1'):
                todo.append((target, base))
    return found


def lint(map_path, repo, commit='HEAD', smoke_only=(), switch_only=()):
    errors = []
    suites = read_map(map_path, errors)
    errors += map_problems(suites)
    files = tracked(repo, commit)
    lower = {f.lower(): f for f in files}
    ignore = next((s[2] for s in suites if s[0] == '*ignore*'), [])
    real = [s for s in suites if s[0] != '*ignore*']
    for name, _, include, command in real:
        for pattern in include:
            if not any(match1(f, pattern) for f in files):
                errors.append(f'unmatched: suite {name}: "{pattern}" matches no tracked file')
        if include and not any(covers(f, include, ignore) for f in files):
            errors.append(f'covers-nothing: suite {name} covers no tracked file once *ignore* is applied')
        m = SCRIPT.search(command)
        if m:
            named = posixpath.normpath(m.group(1).replace('\\', '/'))
            script = lower.get(named.lower())
            if script is None:
                # Nothing else checks a command before it runs: a typo or a deleted script would fail only at run time.
                errors.append(f'missing-script: suite {name}: its command runs {named}, which is not a tracked file')
                continue
            for f, loader in loads(repo, commit, script, lower).items():
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


# The pairs of the tracked map, read by default with it.
SWITCH_ONLY = Path(__file__).resolve().parent / 'suites_switch_only.txt'


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--map', default=str(Path(__file__).resolve().parent / 'suites.txt'),
                    help='the suite map to check (default: the tracked tools/suites.txt)')
    ap.add_argument('--repo', default=str(Path(__file__).resolve().parents[1]), help='the checkout (default: this one)')
    ap.add_argument('--commit', default='HEAD', help='the revision whose tracked files count (default HEAD)')
    ap.add_argument('--smoke-only', help='file of core paths that only a whole-tree suite needs to cover')
    ap.add_argument('--switch-only', help='file of "<suite> <path>" loads that sit behind a switch the suite never passes (default '
                    'with the default map: the tracked tools/suites_switch_only.txt; '' for none)')
    a = ap.parse_args(argv)
    if a.switch_only is None and '--map' not in (argv if argv is not None else sys.argv[1:]):
        a.switch_only = str(SWITCH_ONLY)
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
