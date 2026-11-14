"""AI-assisted (Claude Code). Checks the Rust workspace's shape (Rust port architecture, sections 2.2, 5.1 and 15).

Stdlib only (tomllib). Run: python tests/crate_edges_test.py

What it checks:
  - the workspace has exactly the 22 crates, and each crate's path dependencies stay inside the allowed closure of
    the edge table below (an edge outside it fails);
  - no dependency is anything but a workspace crate by path (no third-party crate; AGENTS.md rule 6), and Cargo.lock
    holds no package with a `source`;
  - the banned patterns appear in no Rust file outside the places allowed: N2 `mul_add`; N3 the float functions
    (powers, exponentials, logarithms, roots, trigonometry) outside ow-core/src/num.rs; N7 the std HashMap/HashSet
    outside ow-core/src/detmap.rs (which holds the fixed-hasher aliases); R3 reading the parity mode outside the
    binaries' main.rs;
  - every flag of src/cli.cpp's usage text has an entry or option in the Rust CLI's dispatch table
    (crates/ow-cli/src/args.rs), and every flag src/cli.cpp's main() dispatches on has a cmd("--flag" entry.
Each check also has a case proving it fails on a deliberately bad input.
"""

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

# The float functions every rule reaches only through ow_core::num (5.1 N3): powers, exponentials, logarithms, roots,
# trigonometry. Both the method form (`x.sqrt()`) and the path form (`f64::sqrt(x)`) are refused.
_N3_FUNCTIONS = ('powf|powi|exp|exp2|exp_m1|ln|ln_1p|log|log2|log10|sqrt|cbrt|hypot|sin|cos|tan|sin_cos|asin|acos|atan|'
                 'atan2|sinh|cosh|tanh')
_PARITY_READERS = {'crates/ow-core/src/parity.rs', 'crates/ow-cli/src/main.rs', 'crates/ow-host/src/main.rs',
                   # drives the binary with OW_PARITY set and unset; it reads nothing itself
                   'crates/ow-cli/tests/not_ported.rs'}

# Sections 5.1 and 15. Each entry: name, regex, files (relative to the root, '/' separators) where it is allowed.
BANNED = [
    ('N2 fused multiply-add', re.compile(r'\bmul_add\b'), set()),
    ('N3 float function outside ow_core::num',
     re.compile(rf'\.(?:{_N3_FUNCTIONS})\s*\(|\b(?:f32|f64)\s*::\s*(?:{_N3_FUNCTIONS})\b'),
     {'crates/ow-core/src/num.rs'}),
    # HashMap/HashSet by name at all: ::new, ::default, ::with_capacity, ::from, collect() into one, all hidden behind
    # one identifier. The fixed-hasher aliases live in detmap.rs (FixedHashMap, FixedHashSet).
    ('N7 std HashMap/HashSet outside ow_core::detmap', re.compile(r'\b(?:HashMap|HashSet)\b'),
     {'crates/ow-core/src/detmap.rs'}),
    ('R3 parity mode read outside a binary',
     re.compile(r'\b(?:parity_mode\s*\(|init_parity_mode\b|mode_from_env\b|ENV_VAR\b)|"OW_PARITY"'),
     _PARITY_READERS),
]


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
                if path is None or dep not in CRATES or pathlib.PurePosixPath(path).name != dep:
                    problems.append(f'{crate}: [{table_name}] {dep} is not a workspace crate by path '
                                    '(third-party crates need a THIRD_PARTY.md entry, AGENTS.md rule 6)')
                elif dep not in allowed:
                    problems.append(f'{crate}: [{table_name}] {dep} is an edge outside the allowed table')
    lock = root / 'Cargo.lock'
    if lock.is_file():
        for package in tomllib.loads(lock.read_text(encoding='utf-8')).get('package', []):
            if 'source' in package or package.get('name') not in CRATES:
                problems.append(f'Cargo.lock: {package.get("name")} is not a workspace crate')
    else:
        problems.append('Cargo.lock is missing')
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
            if rel in allowed:
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
            'fn s(v: &[u8]) -> Option<u8> { v.iter().max().copied() }\n', encoding='utf-8')
        found = check_banned(self.tmp)
        names = [p.split(': ')[1] for p in found]
        self.assertEqual(names.count('N2 fused multiply-add'), 1)
        # powf/exp on one line, sqrt, sin (with a space before the bracket), the f64::atan2 path form
        self.assertEqual(names.count('N3 float function outside ow_core::num'), 4, found)
        # h, k, o, p, q: every way of naming the std containers; r (the fixed-hasher alias) passes
        self.assertEqual(names.count('N7 std HashMap/HashSet outside ow_core::detmap'), 5, found)
        self.assertFalse(any('bad.rs:12:' in p or 'bad.rs:13:' in p for p in found), found)
        num = self.tmp / 'crates' / 'ow-core' / 'src'
        num.mkdir(parents=True)
        (num / 'num.rs').write_text('pub fn p(a: f64) -> f64 { a.powf(2.0) + f64::sqrt(a) }\n', encoding='utf-8')
        (num / 'detmap.rs').write_text('pub type M<K, V> = std::collections::HashMap<K, V>;\n', encoding='utf-8')
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
            'fn g() { let _ = ow_core::mode_from_value(Some("reference")); }\n', encoding='utf-8')
        found = [p for p in check_banned(self.tmp) if 'R3 parity' in p]
        self.assertEqual([p.split(':')[1] for p in found], ['2', '3', '4', '5', '6'], found)
        main = self.tmp / 'crates' / 'ow-cli' / 'src'
        main.mkdir(parents=True)
        (main / 'main.rs').write_text('fn main() { let _ = ow_core::parity::init_parity_mode(); }\n',
                                      encoding='utf-8')
        self.assertFalse(any('ow-cli' in p for p in check_banned(self.tmp)))

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
