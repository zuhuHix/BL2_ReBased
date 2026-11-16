"""AI-assisted (Claude Code). Checks the Rust workspace's shape (Rust port architecture, sections 2.2, 5.1 and 15).

Stdlib only (tomllib). Run: python tests/crate_edges_test.py

What it checks:
  - the workspace has exactly the 22 crates, and each crate's path dependencies stay inside the allowed closure of
    the edge table below (an edge outside it fails);
  - no dependency is anything but a workspace crate by path (no third-party crate; AGENTS.md rule 6), except the
    engine crates of ENGINE_CRATES, which only ow-host may declare, only in its [dependencies], pinned to exactly the
    approved version (maintainer approval 2026-11-16, THIRD_PARTY.md "Engine crates: wgpu and winit");
  - Cargo.lock holds the workspace crates, the engine crates at those versions, and registry packages that only
    ow-host's dependency tree reaches: a package with a `source` that no engine crate pulls in fails, and so does a
    `source` other than crates.io;
  - the banned patterns appear in no Rust file outside the places allowed: N2 `mul_add`; N3 the float functions
    (powers, exponentials, logarithms, roots, trigonometry) outside ow-core/src/num.rs; N7 the std HashMap/HashSet
    outside ow-core/src/detmap.rs (which holds the fixed-hasher aliases); N6 a float format spec (`{:e}`, `{:.3}`)
    outside ow-core/src/fmt.rs; R3 reading the parity mode outside the binaries' main.rs;
  - every flag of src/cli.cpp's usage text has an entry or option in the Rust CLI's dispatch table
    (crates/ow-cli/src/args.rs), and every flag src/cli.cpp's main() dispatches on has a cmd("--flag" entry.
Each check also has a case proving it fails on a deliberately bad input.
"""

import fnmatch
import pathlib
import re
import shutil
import sys
import tempfile
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Section 2.2: each crate may depend on these and on anything they may depend on.
EDGES = {
    'ow-core': [],
    'ow-container': ['ow-core'],
    'ow-package': ['ow-container'],
    'ow-object': ['ow-package'],
    'ow-script': ['ow-package'],
    'ow-defs': ['ow-object'],
    'ow-attr': ['ow-defs'],
    'ow-balance': ['ow-attr'],
    'ow-items': ['ow-balance'],
    'ow-vm': ['ow-script', 'ow-defs', 'ow-attr'],
    'ow-world': ['ow-vm', 'ow-balance'],
    'ow-inventory': ['ow-world', 'ow-items'],
    'ow-behavior': ['ow-world'],
    'ow-combat': ['ow-inventory', 'ow-behavior'],
    'ow-skills': ['ow-combat'],
    'ow-ai': ['ow-combat'],
    'ow-mission': ['ow-ai', 'ow-skills', 'ow-combat', 'ow-inventory', 'ow-behavior'],
    'ow-save': ['ow-mission'],
    'ow-ui': ['ow-mission'],
    'ow-assets': ['ow-defs'],
}
_LIBRARIES = list(EDGES)
EDGES['ow-cli'] = list(_LIBRARIES)   # everything except ow-host
EDGES['ow-host'] = list(_LIBRARIES)  # everything except ow-cli
CRATES = set(EDGES)

# Third-party crates the maintainer approved, by name: (the one crate that may declare it, the exact version). Each
# needs its THIRD_PARTY.md entry before it is added here (AGENTS.md rule 6).
ENGINE_CRATES = {
    'wgpu': ('ow-host', '30.0.1'),
    'winit': ('ow-host', '0.30.13'),
}
CRATES_IO = 'registry+https://github.com/rust-lang/crates.io-index'

# The float functions every rule reaches only through ow_core::num (5.1 N3): powers, exponentials, logarithms, roots,
# trigonometry. The method form (`x.sqrt()`) and the path forms (`f64::sqrt(x)`, `<f64>::sqrt(x)`) are refused.
# The method form matches any method of these names, so rule code must not name its own methods after them (a
# `tracer.log(line)` would fail here): call it `record`, `trace` or similar instead.
_N3_FUNCTIONS = ('powf|powi|exp|exp2|exp_m1|ln|ln_1p|ln_gamma|log|log2|log10|sqrt|cbrt|hypot|sin|cos|tan|sin_cos|asin|'
                 'acos|atan|atan2|sinh|cosh|tanh|asinh|acosh|atanh')
# Where the parity mode may be read (architecture rule R3): the mode's own module and the binaries' main.rs.
_PARITY_READERS = {'crates/ow-core/src/parity.rs', 'crates/ow-cli/src/main.rs', 'crates/ow-host/src/main.rs'}
# Tests that drive a binary may also name the variable, to set it for the child process (fnmatch patterns).
_PARITY_VAR_NAMERS = _PARITY_READERS | {'crates/*/tests/*.rs'}

# Sections 5.1 and 15. Each entry: name, regex, files (relative to the root, '/' separators, fnmatch patterns allowed)
# where it is allowed.
BANNED = [
    ('N2 fused multiply-add', re.compile(r'\bmul_add\b'), set()),
    ('N3 float function outside ow_core::num',
     re.compile(rf'\.(?:{_N3_FUNCTIONS})\s*\(|(?:\b(?:f32|f64)|<\s*(?:f32|f64)\s*>)\s*::\s*(?:{_N3_FUNCTIONS})\b'),
     {'crates/ow-core/src/num.rs'}),
    # HashMap/HashSet by name at all: ::new, ::default, ::with_capacity, ::from, collect() into one, all hidden behind
    # one identifier. The fixed-hasher aliases live in detmap.rs (FixedHashMap, FixedHashSet).
    ('N7 std HashMap/HashSet outside ow_core::detmap', re.compile(r'\b(?:HashMap|HashSet)\b'),
     {'crates/ow-core/src/detmap.rs'}),
    # The identifier parity_mode at all, not only a call: it also catches `let f = parity::parity_mode;`,
    # `use ow_core::parity::parity_mode as pm;` (the later `pm()` cannot be read without it) and
    # `.get_or_init(parity_mode)`. A library names the mode by its type, `ParityMode`, which this does not match.
    ('R3 parity mode read outside a binary',
     re.compile(r'\b(?:parity_mode|init_parity_mode|mode_from_env|ENV_VAR)\b'),
     _PARITY_READERS),
    ('R3 parity variable named outside a binary or its test', re.compile(r'"OW_PARITY"'), _PARITY_VAR_NAMERS),
    # N6: a format spec with an exponent (`{:e}`, `{x:E}`) or a precision (`{:.3}`, `{:.*}`, `{:8.2e}`) is float
    # text in Rust's own form, which matches no oracle; ow_core::fmt is the one place that builds output floats
    # from it. A float behind a bare `{}` cannot be seen here (a review item). A string cut by precision fails too:
    # slice it instead.
    ('N6 float text outside ow_core::fmt',
     re.compile(r'\{[A-Za-z0-9_]*:[^{}"\s]*(?:\.(?:[0-9]+\$?|[A-Za-z_][A-Za-z0-9_]*\$|\*)[^{}"\s]*|[eE])\}'),
     {'crates/ow-core/src/fmt.rs'}),
]


def _allowed(rel, allowed):
    return any(fnmatch.fnmatchcase(rel, pattern) for pattern in allowed)


def closure(crate, edges=EDGES):
    seen, todo = set(), list(edges[crate])
    while todo:
        dep = todo.pop()
        if dep not in seen:
            seen.add(dep)
            todo.extend(edges[dep])
    return seen


def _dependency_tables(manifest):
    for key in ('dependencies', 'dev-dependencies', 'build-dependencies'):
        yield key, manifest.get(key, {})
    for target, table in manifest.get('target', {}).items():
        for key in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            yield f'target.{target}.{key}', table.get(key, {})


def check_edges(root):
    """Returns a list of problems with the workspace members and their dependencies."""
    problems = []
    workspace = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))
    members = workspace.get('workspace', {}).get('members', [])
    names = {pathlib.PurePosixPath(m).name for m in members}
    if names != CRATES:
        problems.append(f'workspace members differ from the 22 crates: missing {sorted(CRATES - names)}, '
                        f'extra {sorted(names - CRATES)}')
    for member in members:
        crate = pathlib.PurePosixPath(member).name
        manifest_path = root / member / 'Cargo.toml'
        if not manifest_path.is_file():
            problems.append(f'{member}: no Cargo.toml')
            continue
        manifest = tomllib.loads(manifest_path.read_text(encoding='utf-8'))
        if manifest.get('package', {}).get('name') != crate:
            problems.append(f'{member}: package name is not {crate}')
        allowed = closure(crate) if crate in EDGES else set()
        for table_name, table in _dependency_tables(manifest):
            for dep, spec in table.items():
                path = spec.get('path') if isinstance(spec, dict) else None
                if dep in ENGINE_CRATES and path is None:
                    problems += _engine_problems(crate, table_name, dep, spec)
                elif path is None or dep not in CRATES or pathlib.PurePosixPath(path).name != dep:
                    problems.append(f'{crate}: [{table_name}] {dep} is not a workspace crate by path '
                                    '(third-party crates need a THIRD_PARTY.md entry, AGENTS.md rule 6)')
                elif dep not in allowed:
                    problems.append(f'{crate}: [{table_name}] {dep} is an edge outside the allowed table')
    lock = root / 'Cargo.lock'
    if lock.is_file():
        problems += _lock_problems(tomllib.loads(lock.read_text(encoding='utf-8')).get('package', []))
    else:
        problems.append('Cargo.lock is missing')
    return problems


def _engine_problems(crate, table_name, dep, spec):
    owner, version = ENGINE_CRATES[dep]
    where = f'{crate}: [{table_name}] {dep}'
    if crate != owner:
        return [f'{where}: engine crates are allowed only in {owner} (architecture section 2.2)']
    if table_name != 'dependencies':
        return [f'{where}: engine crates belong in [dependencies] only']
    requirement = spec if isinstance(spec, str) else spec.get('version') if isinstance(spec, dict) else None
    extra = sorted(set(spec) - {'version', 'features'}) if isinstance(spec, dict) else []
    problems = []
    if requirement != f'={version}':
        problems.append(f'{where}: must be pinned "={version}", not {requirement!r}')
    if extra:
        problems.append(f'{where}: only version and features may be set, not {extra} (crates.io, no git or path)')
    return problems


def _lock_problems(packages):
    """Every registry package must be reachable from an engine crate at its approved version; nothing else may have
    a source. Dependency entries in Cargo.lock are "name" or "name version" (the version only when two are locked)."""
    problems = []
    by_name = {}
    for package in packages:
        by_name.setdefault(package.get('name'), []).append(package)
    for package in packages:
        name, source = package.get('name'), package.get('source')
        if source is None:
            if name not in CRATES:
                problems.append(f'Cargo.lock: {name} is neither a workspace crate nor from a registry')
        elif source != CRATES_IO:
            problems.append(f'Cargo.lock: {name} comes from {source}, not crates.io')
        elif name in ENGINE_CRATES and package.get('version') != ENGINE_CRATES[name][1]:
            problems.append(f'Cargo.lock: {name} {package.get("version")} is not the approved {ENGINE_CRATES[name][1]}')
    reached, todo = set(), [p for name in ENGINE_CRATES for p in by_name.get(name, [])
                            if p.get('source') == CRATES_IO and p.get('version') == ENGINE_CRATES[name][1]]
    while todo:
        package = todo.pop()
        key = (package.get('name'), package.get('version'))
        if key in reached:
            continue
        reached.add(key)
        for entry in package.get('dependencies', []):
            dep_name, _, dep_version = entry.partition(' ')
            candidates = by_name.get(dep_name, [])
            if not dep_version:
                # Cargo names the version only when two of a name are locked, so a bare name with several
                # candidates is a hand-edited lock: it reaches none of them, and each is reported below.
                candidates = candidates if len(candidates) == 1 else []
            todo.extend(p for p in candidates if not dep_version or p.get('version') == dep_version)
    for package in packages:
        if package.get('source') is not None and (package.get('name'), package.get('version')) not in reached:
            problems.append(f'Cargo.lock: {package.get("name")} {package.get("version")} is not a workspace crate '
                            'and no approved engine crate depends on it (third-party crates need a THIRD_PARTY.md '
                            'entry, AGENTS.md rule 6)')
    return problems


def _strip_comments(text):
    # Line comments only (doc comments included): a doc line may name a banned call to forbid it. Naive about
    # '//' inside string literals, which errs toward checking less, never toward a false failure.
    return '\n'.join(line.split('//', 1)[0] for line in text.splitlines())


def check_banned(root):
    problems = []
    for path in sorted((root / 'crates').rglob('*.rs')):
        rel = path.relative_to(root).as_posix()
        if '/target/' in f'/{rel}':
            continue
        code = _strip_comments(path.read_text(encoding='utf-8'))
        for name, pattern, allowed in BANNED:
            if _allowed(rel, allowed):
                continue
            for number, line in enumerate(code.splitlines(), 1):
                if pattern.search(line):
                    problems.append(f'{rel}:{number}: {name}: {line.strip()}')
    return problems


def cxx_usage_flags(root):
    source = (root / 'src' / 'cli.cpp').read_text(encoding='utf-8')
    body = re.search(r'void usage\(\) \{\s*throw std::runtime_error\((.*?)\);\s*\}', source, re.S)
    if not body:
        raise AssertionError('src/cli.cpp: usage() not found')
    text = ''.join(re.findall(r'"((?:[^"\\]|\\.)*)"', body.group(1)))
    return set(re.findall(r'--[a-z][a-z-]*', text))


def cxx_dispatch_flags(root):
    """The command flags src/cli.cpp's main() dispatches on, whether or not the usage text names them."""
    source = (root / 'src' / 'cli.cpp').read_text(encoding='utf-8')
    return set(re.findall(r'(?:\bmode|std::string\(argv\[1\]\))\s*==\s*"(--[a-z][a-z-]*)"', source))


def check_cli_flags(root):
    table = _strip_comments((root / 'crates' / 'ow-cli' / 'src' / 'args.rs').read_text(encoding='utf-8'))
    known = set(re.findall(r'"(--[a-z][a-z-]*)"', table))
    commands = set(re.findall(r'\bcmd\(\s*"(--[a-z][a-z-]*)"', table))
    problems = [f'crates/ow-cli/src/args.rs: no dispatch entry or option for {flag} (src/cli.cpp usage)'
                for flag in sorted(cxx_usage_flags(root) - known)]
    problems += [f'crates/ow-cli/src/args.rs: no cmd("{flag}" dispatch entry (src/cli.cpp main)'
                 for flag in sorted(cxx_dispatch_flags(root) - commands)]
    return problems


class WorkspaceShape(unittest.TestCase):
    def test_edges_of_the_tree(self):
        self.assertEqual(check_edges(ROOT), [])

    def test_banned_patterns_in_the_tree(self):
        self.assertEqual(check_banned(ROOT), [])

    def test_every_cxx_flag_has_a_rust_entry(self):
        self.assertGreater(len(cxx_usage_flags(ROOT)), 40)
        dispatched = cxx_dispatch_flags(ROOT)
        self.assertGreaterEqual(len(dispatched), 36)
        self.assertIn('--mover-event', dispatched)   # dispatched, though the usage text does not name it
        self.assertIn('--native-selftest', dispatched)
        self.assertEqual(check_cli_flags(ROOT), [])

    def test_the_host_declares_exactly_the_engine_crates(self):
        host = tomllib.loads((ROOT / 'crates' / 'ow-host' / 'Cargo.toml').read_text(encoding='utf-8'))
        third_party = {dep: spec for dep, spec in host['dependencies'].items() if dep not in CRATES}
        self.assertEqual(third_party, {name: f'={version}' for name, (_, version) in ENGINE_CRATES.items()})
        locked = {(p['name'], p['version']) for p in tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
                  ['package'] if p['name'] in ENGINE_CRATES}
        self.assertEqual(locked, {(name, version) for name, (_, version) in ENGINE_CRATES.items()})

    def test_the_table_itself(self):
        self.assertEqual(len(CRATES), 22)
        for forbidden_user in ('ow-attr', 'ow-balance', 'ow-items'):
            self.assertNotIn('ow-vm', closure(forbidden_user))
        self.assertNotIn('ow-object', closure('ow-package'))
        for forbidden in ('ow-combat', 'ow-skills', 'ow-items'):
            self.assertNotIn(forbidden, closure('ow-behavior'))
        for crate in CRATES:
            self.assertNotIn('ow-host', closure(crate))
            if crate != 'ow-host':
                self.assertNotIn('ow-cli', closure(crate))


class FailsOnBadInput(unittest.TestCase):
    """Copies the manifests (and one source file) into a scratch tree, breaks one thing, expects a problem."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix='ow-crate-edges-'))
        shutil.copy(ROOT / 'Cargo.toml', self.tmp / 'Cargo.toml')
        shutil.copy(ROOT / 'Cargo.lock', self.tmp / 'Cargo.lock')
        for manifest in (ROOT / 'crates').glob('*/Cargo.toml'):
            target = self.tmp / 'crates' / manifest.parent.name
            target.mkdir(parents=True)
            shutil.copy(manifest, target / 'Cargo.toml')
        self.assertEqual(check_edges(self.tmp), [])

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _append(self, crate, text):
        with open(self.tmp / 'crates' / crate / 'Cargo.toml', 'a', encoding='utf-8') as out:
            out.write(text)

    def test_an_offline_rule_crate_depending_on_the_vm_fails(self):
        self._append('ow-attr', '\n[build-dependencies]\now-vm = { path = "../ow-vm" }\n')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-attr' in p and 'ow-vm' in p and 'outside' in p for p in problems), problems)

    def test_a_game_crate_depending_on_the_host_fails(self):
        manifest = self.tmp / 'crates' / 'ow-world' / 'Cargo.toml'
        text = manifest.read_text(encoding='utf-8').replace(
            '[dependencies]\n', '[dependencies]\now-host = { path = "../ow-host" }\n', 1)
        manifest.write_text(text, encoding='utf-8')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-world' in p and 'ow-host' in p for p in problems), problems)

    def test_a_third_party_crate_fails(self):
        self._append('ow-core', '\n[dev-dependencies]\nserde = "1"\n')
        self.assertTrue(any('serde' in p for p in check_edges(self.tmp)))
        lock = self.tmp / 'Cargo.lock'
        lock.write_text(lock.read_text(encoding='utf-8') + '\n[[package]]\nname = "serde"\nversion = "1.0.0"\n'
                        'source = "registry+https://github.com/rust-lang/crates.io-index"\n', encoding='utf-8')
        self.assertTrue(any('Cargo.lock: serde' in p for p in check_edges(self.tmp)))

    def test_an_engine_crate_outside_the_host_fails(self):
        self._append('ow-core', '\n[dependencies.wgpu]\nversion = "=30.0.1"\n')
        self._append('ow-ui', '\n[target.\'cfg(windows)\'.dependencies]\nwinit = "=0.30.13"\n')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-core' in p and 'wgpu' in p and 'only in ow-host' in p for p in problems), problems)
        self.assertTrue(any('ow-ui' in p and 'winit' in p and 'only in ow-host' in p for p in problems), problems)

    def test_an_engine_crate_in_the_host_s_other_tables_fails(self):
        self._append('ow-host', '\n[build-dependencies]\nwinit = "=0.30.13"\n')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-host' in p and 'build-dependencies' in p and 'winit' in p for p in problems), problems)

    def test_an_engine_crate_not_pinned_or_not_from_crates_io_fails(self):
        manifest = self.tmp / 'crates' / 'ow-host' / 'Cargo.toml'
        text = manifest.read_text(encoding='utf-8')
        manifest.write_text(text.replace('wgpu = "=30.0.1"', 'wgpu = "30"'), encoding='utf-8')
        self.assertTrue(any('wgpu' in p and 'pinned' in p for p in check_edges(self.tmp)))
        manifest.write_text(text.replace('wgpu = "=30.0.1"', 'wgpu = "=30.0.2"'), encoding='utf-8')
        self.assertTrue(any('wgpu' in p and 'pinned' in p for p in check_edges(self.tmp)))
        manifest.write_text(text.replace(
            'winit = "=0.30.13"', 'winit = { version = "=0.30.13", git = "https://example.invalid/winit" }'),
            encoding='utf-8')
        self.assertTrue(any('winit' in p and 'git' in p for p in check_edges(self.tmp)))

    def test_another_third_party_crate_in_the_host_fails(self):
        self._append('ow-host', '\n[dependencies.glam]\nversion = "=0.33.12"\n')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-host' in p and 'glam' in p and 'THIRD_PARTY' in p for p in problems), problems)

    def test_a_lock_entry_outside_the_engine_closure_fails(self):
        lock = self.tmp / 'Cargo.lock'
        text = lock.read_text(encoding='utf-8')
        # A registry package that no engine crate depends on (the host pulling in glam by some other route).
        lock.write_text(text + f'\n[[package]]\nname = "glam"\nversion = "0.33.12"\nsource = "{CRATES_IO}"\n',
                        encoding='utf-8')
        self.assertTrue(any('Cargo.lock: glam 0.33.12' in p and 'no approved engine crate' in p
                            for p in check_edges(self.tmp)))
        # A source other than crates.io, and an engine crate at a version other than the approved one.
        lock.write_text(text + '\n[[package]]\nname = "glam"\nversion = "0.33.12"\n'
                        'source = "git+https://example.invalid/glam#0123"\n', encoding='utf-8')
        self.assertTrue(any('glam comes from git+' in p for p in check_edges(self.tmp)))
        lock.write_text(text.replace('name = "winit"\nversion = "0.30.13"', 'name = "winit"\nversion = "0.30.12"'),
                        encoding='utf-8')
        self.assertTrue(any('winit 0.30.12 is not the approved 0.30.13' in p for p in check_edges(self.tmp)))

    def test_behavior_depending_on_skills_fails(self):
        # A second [dev-dependencies] header would be invalid TOML; a dotted sub-table is not.
        self._append('ow-behavior', '\n[dev-dependencies.ow-skills]\npath = "../ow-skills"\n')
        problems = check_edges(self.tmp)
        self.assertTrue(any('ow-behavior' in p and 'ow-skills' in p and 'outside' in p for p in problems), problems)

    def test_a_missing_member_fails(self):
        top = self.tmp / 'Cargo.toml'
        top.write_text(top.read_text(encoding='utf-8').replace('    "crates/ow-ui",\n', ''), encoding='utf-8')
        self.assertTrue(any('missing' in p and 'ow-ui' in p for p in check_edges(self.tmp)))

    def test_banned_patterns_fail_outside_their_place(self):
        src = self.tmp / 'crates' / 'ow-attr' / 'src'
        src.mkdir(parents=True)
        (src / 'bad.rs').write_text(
            '//! a doc line may say mul_add and powf\n'
            'fn f(a: f32) -> f32 { a.mul_add(2.0, 1.0) }\n'
            'fn g(a: f64) -> f64 { a.powf(2.0) + a.exp() }\n'
            'fn h() { let m: std::collections::HashMap<u8, u8> = std::collections::HashMap::new(); }\n'
            'fn k() { let s = HashSet::<u8>::new(); }\n'
            'fn l() -> f64 { 2.0f64.sqrt() }\n'
            'fn m(a: f32) -> f32 { a.sin () }\n'
            'fn n(y: f64, x: f64) -> f64 { f64::atan2(y, x) }\n'
            'fn o() { let m: HashMap<u8, u8> = HashMap::default(); }\n'
            'fn p() { let s: HashSet<u8> = [1].into_iter().collect(); }\n'
            'fn q() { let m = Vec::<HashMap<u8, Vec<u8>>>::with_capacity(1); }\n'
            'fn r() { let m: ow_core::FixedHashMap<u8, u8> = Default::default(); }\n'
            'fn s(v: &[u8]) -> Option<u8> { v.iter().max().copied() }\n'
            'fn t(x: f64) -> f64 { <f64>::sqrt(x) }\n'
            'fn u(x: f64) -> f64 { x.asinh() + < f32 >::atanh(0.5) as f64 }\n'
            'fn v(x: f64) -> String { format!("{:.3} {x:e}", x) }\n'
            'fn w(x: f64) -> String { format!("{:>8.2e}|{:.*}", x, 2, x) }\n'
            'fn y(n: u32) -> String { format!("{n:08x} {n:>4} {:?} {}", n, n) }\n'
            'fn z(x: f64, p: usize) -> String { format!("{:.prec$} {x:.p$}", x, prec = p) }\n', encoding='utf-8')
        found = check_banned(self.tmp)
        names = [p.split(': ')[1] for p in found]
        self.assertEqual(names.count('N2 fused multiply-add'), 1)
        # powf/exp on one line, sqrt, sin (with a space before the bracket), the f64::atan2 path form, the <f64>::
        # path form (line 14), asinh and < f32 >::atanh (line 15)
        self.assertEqual(names.count('N3 float function outside ow_core::num'), 6, found)
        # h, k, o, p, q: every way of naming the std containers; r (the fixed-hasher alias) passes
        self.assertEqual(names.count('N7 std HashMap/HashSet outside ow_core::detmap'), 5, found)
        self.assertFalse(any('bad.rs:12:' in p or 'bad.rs:13:' in p for p in found), found)
        # v and w (precision and exponent specs) and z (named precision, `{:.prec$}` and `{x:.p$}`); y (hex, width,
        # Debug, Display) passes
        self.assertEqual(names.count('N6 float text outside ow_core::fmt'), 3, found)
        self.assertFalse(any('bad.rs:18:' in p for p in found), found)
        num = self.tmp / 'crates' / 'ow-core' / 'src'
        num.mkdir(parents=True)
        (num / 'num.rs').write_text('pub fn p(a: f64) -> f64 { a.powf(2.0) + f64::sqrt(a) }\n', encoding='utf-8')
        (num / 'detmap.rs').write_text('pub type M<K, V> = std::collections::HashMap<K, V>;\n', encoding='utf-8')
        (num / 'fmt.rs').write_text('pub fn g(x: f64) -> String { format!("{:.*e}", 3, x) }\n', encoding='utf-8')
        self.assertEqual([p for p in check_banned(self.tmp) if 'ow-core' in p], [])

    def test_reading_the_parity_mode_in_a_library_fails(self):
        src = self.tmp / 'crates' / 'ow-vm' / 'src'
        src.mkdir(parents=True)
        (src / 'bad.rs').write_text(
            '//! parity_mode() in a doc line is fine\n'
            'fn a() -> ow_core::ParityMode { ow_core::parity::parity_mode() }\n'
            'fn b() { let _ = ow_core::parity::init_parity_mode(); }\n'
            'fn c() { let _ = std::env::var(ow_core::parity::ENV_VAR); }\n'
            'fn d() { let _ = std::env::var("OW_PARITY"); }\n'
            'fn e() { let _ = ow_core::parity::mode_from_env(); }\n'
            'fn f(m: ow_core::ParityMode) -> bool { m == ow_core::ParityMode::Reference }\n'
            'fn g() { let _ = ow_core::mode_from_value(Some("reference")); }\n'
            'fn h() { let f = ow_core::parity::parity_mode; let _ = f(); }\n'
            'use ow_core::parity::parity_mode as pm;\n'
            'use ow_core::parity::{\n    parity_mode,\n};\n'
            'fn k(c: &std::sync::OnceLock<ow_core::ParityMode>) { c.get_or_init(parity_mode); }\n', encoding='utf-8')
        lines = lambda found: sorted(int(p.split(':')[1]) for p in found)
        found = [p for p in check_banned(self.tmp) if 'R3 parity' in p]
        # lines 7 and 8 (the type and mode_from_value) pass; 9-14 are the indirect reads
        self.assertEqual(lines(found), [2, 3, 4, 5, 6, 9, 10, 12, 14], found)
        # A test that drives the binary may name the variable, but still may not read the mode in-process.
        tests = self.tmp / 'crates' / 'ow-cli' / 'tests'
        tests.mkdir(parents=True)
        (tests / 'drive.rs').write_text(
            'const VAR: &str = "OW_PARITY";\n'
            'fn t() { let _ = ow_core::parity::parity_mode(); }\n', encoding='utf-8')
        found = [p for p in check_banned(self.tmp) if 'drive.rs' in p]
        self.assertEqual(lines(found), [2], found)
        main = self.tmp / 'crates' / 'ow-cli' / 'src'
        main.mkdir(parents=True)
        (main / 'main.rs').write_text('fn main() { let _ = ow_core::parity::init_parity_mode(); }\n',
                                      encoding='utf-8')
        self.assertFalse(any('ow-cli/src/main.rs' in p for p in check_banned(self.tmp)))

    def test_a_cxx_flag_without_a_rust_entry_fails(self):
        (self.tmp / 'src').mkdir()
        (self.tmp / 'src' / 'cli.cpp').write_text(
            'void usage() {\n    throw std::runtime_error(\n        "usage: ow-package <package> [--names | "\n'
            '        "--brand-new <x>]");\n}\n', encoding='utf-8')
        args = self.tmp / 'crates' / 'ow-cli' / 'src'
        args.mkdir(parents=True)
        (args / 'args.rs').write_text('const A: &str = "--names"; // "--brand-new" in a comment does not count\n',
                                      encoding='utf-8')
        self.assertEqual(check_cli_flags(self.tmp),
                         ['crates/ow-cli/src/args.rs: no dispatch entry or option for --brand-new (src/cli.cpp usage)'])

    def test_a_dispatched_cxx_flag_without_a_cmd_entry_fails(self):
        # Like --mover-event: dispatched by main() but absent from the usage text; an option string is not enough.
        (self.tmp / 'src').mkdir()
        (self.tmp / 'src' / 'cli.cpp').write_text(
            'void usage() {\n    throw std::runtime_error("usage: ow-package <package> [--names]");\n}\n'
            'int main(int argc, char** argv) {\n    if (std::string(argv[1]) == "--first") return 0;\n'
            '    if (mode == "--names") return 0;\n    if (mode == "--hidden") return 0;\n}\n', encoding='utf-8')
        args = self.tmp / 'crates' / 'ow-cli' / 'src'
        args.mkdir(parents=True)
        (args / 'args.rs').write_text('const C: &[Command] = &[cmd(\n    "--names", P, F::Tables, &["--hidden"]),\n'
                                      '    cmd("--first", First, F::Vm, &[])];\n', encoding='utf-8')
        self.assertEqual(check_cli_flags(self.tmp),
                         ['crates/ow-cli/src/args.rs: no cmd("--hidden" dispatch entry (src/cli.cpp main)'])


if __name__ == '__main__':
    result = unittest.main(exit=False, verbosity=1).result
    sys.exit(0 if result.wasSuccessful() else 1)
