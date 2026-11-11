"""Synthetic tests for tools/leak_gate.py: every rule caught and a near miss passed, the baseline, the allow-list,
the exit codes, per-commit added-lines scanning, the trusted lists, tags and identities, and the pre-push hook
refusing through temporary repositories.

Trigger strings are built at run time (joined from parts) so this file itself passes the gate without an
exemption. No game data; no network (the remotes are local bare repositories).
"""
import base64
import hashlib
import importlib.util
import os
import random
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('leak_gate', ROOT / 'tools' / 'leak_gate.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def j(*parts):
    return ''.join(parts)


AT = j('@')
FUN = j('FU', 'N_', '0040a1b2')
PARAM = j('par', 'am_', '1')
ADDRESS = j('0', 'x', '00a3b4c5')
EMAIL = j('jane.doe', AT, 'mail', '.net')
USER_PATH = j('C:', '\\', 'Us', 'ers', '\\', 'alice', '\\', 'Tools')
STEAM = j('7656', '119', '8000000001')
TOKEN = j('gh', 'p_', 'A' * 36)
NOREPLY = j('1+bot', AT, 'users.noreply.github.com')
PRIVATE = j('zz', 'qx', 'name')
BACKSLASH = '\\'
TRIGGERS = (EMAIL, 'jane', 'alice', TOKEN, STEAM, PRIVATE)

ALLOW = '\n'.join([
    'unknown-type *.py -- code',
    'unknown-type *.cpp -- code',
    'unknown-type *.ps1 -- code',
    'unknown-type *.md -- docs',
    'unknown-type *.txt -- text',
    'unknown-type *.json -- data',
    'unknown-type *.svg -- art',
    'unknown-type *.js -- code',
    'forbidden-type cfg/*.ini -- own config',
    'unknown-type .githooks/* -- hooks',
    'unknown-type .gitignore -- git',
    'unknown-type .gitattributes -- git',
    j('allowed-address noreply', AT, 'example-trailer.org -- a trailer'),
])


def config(allow=ALLOW, baseline=None):
    return gate.Config(allow, baseline, 'test')


def findings_of(path, text, cfg=None, new=True):
    data = text if isinstance(text, bytes) else text.encode('utf-8')
    return gate.scan_file(path, data, cfg or config(), new_file=new)[0]


def rules(path, text, cfg=None, new=True):
    return sorted({f.rule for f in findings_of(path, text, cfg, new)})


def random_bytes(count, seed=7):
    rng = random.Random(seed)
    return bytes(rng.randrange(256) for _ in range(count))


class RuleTests(unittest.TestCase):
    def setUp(self):
        gate.load_names([])

    def test_forbidden_type(self):
        self.assertIn('forbidden-type', rules('maps/Fake_P.upk', 'x'))
        self.assertIn('forbidden-type', rules('out/mesh.psk', 'x'))
        self.assertIn('forbidden-type', rules('proj.rep/idx', 'x'))
        self.assertIn('forbidden-type', rules('dump/Thing.props.txt', 'x'))
        self.assertNotIn('forbidden-type', rules('docs/upk_notes.md', 'about packages'))

    def test_game_config_location_and_content(self):
        self.assertIn('forbidden-type', rules('Engine.ini', '[/Script/Engine.Engine]\n'))
        self.assertEqual(rules('cfg/DefaultEngine.ini', '[/Script/Engine.RendererSettings]\nr.X=1\n'), [])
        self.assertEqual(rules('cfg/DefaultEngine.ini', '[Core.Log]\nLogX=Log\n[Core.System]\nPaths=x\n'), [])
        self.assertIn('game-config', rules('cfg/DefaultEngine.ini', '[Engine.Engine]\nFoo=1\n'))
        self.assertIn('game-config', rules('cfg/Default.ini', '[IniVersion]\n0=1\n'))
        self.assertIn('game-config', rules('cfg/Default.ini', '﻿[Engine.Engine] ; tuned\nFoo=1\n'))
        two = '[Engine.Engine]\nFoo=1\n[WillowGame.WillowGameInfo]\nBar=2\n'
        self.assertIn('game-config', rules('notes/settings.txt', two))
        self.assertEqual(sorted(f.line for f in findings_of('notes/settings.txt', two) if f.rule == 'game-config'), [1, 3])
        self.assertEqual(rules('docs/n.md', 'The section\n[Engine.Engine]\nholds Foo.\n'), [])

    def test_unknown_type_is_a_warning(self):
        self.assertEqual(rules('tools/x.toml', 'a = 1'), ['unknown-type'])
        self.assertEqual(gate.RULES['unknown-type'][0], gate.WARN)
        self.assertEqual(rules('tools/x.py', 'a = 1'), [])
        self.assertEqual(rules('tools/x.toml', 'a = 1', new=False), [])

    def test_binary_size_lfs(self):
        self.assertIn('binary', rules('tools/blob.py', b'abc\0def'))
        self.assertEqual(rules('tools/text.py', 'café = 1'), [])
        self.assertIn('oversize', rules('tools/big.txt', 'a' * (gate.SIZE_LIMIT + 1)))
        self.assertNotIn('oversize', rules('tools/big.txt', 'a' * gate.SIZE_LIMIT))
        pointer = j('version https://git-lfs', '.github.com/spec/v1\noid sha256:', 'ab' * 32, '\nsize 12\n')
        self.assertIn('lfs-pointer', rules('docs/big.md', pointer))
        self.assertEqual(rules('docs/n.md', 'We do not use git-lfs.\n'), [])

    def test_utf32_and_bom_binary(self):
        self.assertIn('decompiler-mark', rules('docs/n.md', ('﻿' + f'see {FUN}\n').encode('utf-32-le')))
        self.assertIn('decompiler-mark', rules('docs/n.md', ('﻿' + f'see {FUN}\n').encode('utf-32-be')))
        self.assertIn('binary', rules('docs/n.md', b'\xff\xfe' + 'a\0b'.encode('utf-16-le')))

    def test_type_names_are_normalised(self):
        for name in ('maps/Fake_P.upk.', 'maps/Fake_P.upk ', 'maps/Fake_P.upk:stream', 'maps/Fake_P.UPK. .'):
            self.assertIn('forbidden-type', rules(name, 'x'), name)

    def test_strict_decoding(self):
        self.assertIn('decompiler-mark', rules('docs/n.md', ('﻿' + f'see {FUN}\n').encode('utf-16-le')))
        self.assertNotIn('binary', rules('docs/n.md', ('﻿' + 'plain\n').encode('utf-16-le')))
        self.assertIn('decompiler-mark', rules('docs/n.md', b'\xef\xbb\xbf' + f'see {FUN}\n'.encode()))
        noise = bytes(b for b in random_bytes(2000) if b) + b'\xff\xfe\xfd'
        self.assertIn('undecodable', rules('docs/n.md', b'x' + noise))
        self.assertEqual(rules('docs/n.md', 'plain UTF-8 café\n'), [])

    def test_pinned_binary_needs_its_hash(self):
        data = b'\x89PNG\0fake'
        digest = hashlib.sha256(data).hexdigest()
        pinned = config(ALLOW + f'\nbinary art/a.png sha256={digest} -- art\nforbidden-type art/a.png sha256={digest} -- art')
        self.assertEqual(rules('art/a.png', data, pinned), [])
        self.assertEqual(rules('art/a.png', data + b'x', pinned), ['binary', 'forbidden-type'])

    def test_local_and_private_paths(self):
        self.assertIn('local-path', rules('local/p2/out.json', '{}'))
        self.assertIn('local-path', rules('LOCAL/p2/out.json', '{}'))
        self.assertIn('local-path', rules('host/ue5/OpenWillow/Saved/x.txt', 'x'))
        self.assertIn('local-path', rules('docs/Bl2Screens/a.txt', 'x'))
        self.assertIn('local-path', rules('PromptUpgrade.md', 'x'))
        self.assertEqual(rules('tools/local_paths.py', 'x = 1'), [])
        self.assertEqual(rules('docs/local/notes.md', 'x'), [])

    def test_identifiers_in_file_names(self):
        self.assertIn('email', rules(f'docs/{EMAIL}.md', 'x'))
        self.assertIn(f'docs/{EMAIL}.md', gate.SENSITIVE_PATHS)

    def test_decompiler_marks(self):
        self.assertIn('decompiler-mark', rules('src/a.cpp', f'int x = {FUN}();'))
        self.assertIn('decompiler-mark', rules('docs/n.md', j('i', 'Var', '3 = 2')))
        self.assertIn('decompiler-mark', rules('docs/n.md', j('undefined', '4 x')))
        self.assertEqual(rules('src/a.cpp', 'int FUN_GAME = 0; auto v = undefined;'), [])

    def test_note_marks_only_in_prose_and_this_in_fences(self):
        self.assertIn('decompiler-note-mark', rules('docs/n.md', f'reads {PARAM} first'))
        self.assertIn('decompiler-note-mark', rules('docs/n.md', j('calls ', 'this', '->Run')))
        self.assertEqual(rules('src/a.cpp', j('this', '->Run(', PARAM, ');')), [])
        self.assertEqual(rules('docs/n.md', 'Left shield->class mod, 4->1->2'), [])
        self.assertEqual(rules('docs/n.md', j('Quoted:\n```cpp\n', 'this', '->Run();\n```\n')), [])
        self.assertIn('decompiler-note-mark', rules('docs/n.md', j('Quoted:\n```\nx = ', PARAM, ';\n```\n')))
        self.assertEqual(rules('docs/n.md', j('The call `', 'this', '->Run()` sets it.')), [])
        self.assertIn('decompiler-note-mark', rules('docs/n.md', j('The `x` call ', 'this', '->Run sets it.')))

    def test_address_hex_is_a_prose_warning(self):
        self.assertEqual(rules('docs/n.md', f'at {ADDRESS}'), ['address-hex'])
        self.assertEqual(rules('docs/n.md', j('flag 0', 'x', '02000000')), [])
        self.assertEqual(rules('src/a.cpp', f'auto k = {ADDRESS};'), [])

    def test_numeric_dumps(self):
        row = ' '.join(['4d', '5a', '90', '00', '03', '00', '00', '00'])
        self.assertIn('dump', rules('docs/n.md', '\n'.join([row] * 10)))  # byte rows with a-f digits
        self.assertEqual(rules('docs/n.md', '\n'.join([row] * 6)), [])
        self.assertIn('dump', rules('tests/x.py', 'x = "' + 'ab' * 100 + '"'))
        self.assertEqual(rules('docs/n.md', 'sha256 ' + 'ab' * 32), [])
        # Structured numeric tables are reported (dump-like, a warning) and refused only at about ten times the
        # largest table the tree holds.
        self.assertEqual(rules('docs/n.md', '\n'.join(['7'] * 30)), ['dump-like'])
        md_table = '| a | b | c | d | e | f |\n|---|---|---|---|---|---|\n' + ''.join(
            '| ' + ' | '.join(str(r * 10 + c) for c in range(6)) + ' |\n' for r in range(10))
        self.assertEqual(rules('docs/verification/n.md', md_table), ['dump-like'])
        cpp = 'static const int T[] = {\n' + ''.join(
            '    ' + ', '.join(f'0x{(r * 8 + c) * 3:04x}' for c in range(8)) + ',\n' for r in range(20)) + '};\n'
        self.assertNotIn('dump', rules('src/table.cpp', cpp))
        fixture = '[\n' + ''.join(f'  {n},\n' for n in range(30)) + '  0\n]\n'
        self.assertEqual(rules('tests/fixture.json', fixture), ['dump-like'])
        self.assertEqual(rules('tests/fixture.json', '[\n' + ''.join(f'  {n},\n' for n in range(10)) + '  0\n]\n'), [])
        hashes = '\n'.join(hashlib.sha256(str(n).encode()).hexdigest() for n in range(9))
        self.assertEqual(rules('docs/n.md', hashes + '\n'), ['dump-like'])
        huge = '\n'.join(' '.join(str(r * 9 + c) for c in range(9)) for r in range(200))
        self.assertIn('dump', rules('docs/n.md', huge))
        self.assertIn('dump', rules('src/table.cpp', '\n'.join(['1, 2, 3, 4, 5, 6, 7, 8,'] * 130)))

    def test_quoted_and_split_blobs(self):
        data = random_bytes(600)
        hexed, b64 = data.hex(), base64.b64encode(data).decode()
        cases = {
            'hex at 16 columns': '\n'.join(hexed[i:i + 16] for i in range(0, len(hexed), 16)),
            'base64 at 20 columns': '\n'.join(b64[i:i + 20] for i in range(0, len(b64), 20)),
            'python strings': 'BLOB = (\n' + ''.join(f'    "{b64[i:i + 40]}"\n' for i in range(0, len(b64), 40)) + ')\n',
            'json array': '[\n' + ''.join(f'  "{b64[i:i + 50]}",\n' for i in range(0, len(b64), 50)) + '  "x"\n]\n',
            'split escapes': 'b = (\n' + ''.join('    b"' + ''.join('\\x%02x' % c for c in data[i:i + 12]) + '"\n'
                                                 for i in range(0, 300, 12)) + ')\n',
            'split data URI': 'src = "data:image/png;base64," +\n' + ''.join(
                f'    "{b64[i:i + 30]}" +\n' for i in range(0, 120, 30)) + '    ""\n',
            'base85': '\n'.join(base64.b85encode(data).decode()[i:i + 60] for i in range(0, 600, 60)),
        }
        for name, text in cases.items():
            self.assertIn('dump', rules('tools/x.py', text), name)
        words = '\n'.join(['    "material_assignment", "weapon_parity2_probe",'] * 12)
        self.assertEqual(rules('tools/x.py', words), [])
        cmake_list = '\n'.join(['  audio_census blcmm_dumps block_saves bsp_calibration bsp build_phaselock_fx_constants'] * 10)
        self.assertEqual(rules('CMakeLists.txt', cmake_list), [])

    def test_wrapped_hex_and_base64(self):
        data = random_bytes(3000)
        plain_hex = data.hex()
        xxd_p = '\n'.join(plain_hex[i:i + 60] for i in range(0, len(plain_hex), 60))
        xxd = '\n'.join('%08x: %s  %s' % (i, ' '.join(data[i:i + 16][k:k + 2].hex() for k in range(0, 16, 2)),
                                          '.' * 16) for i in range(0, len(data), 16))
        b64 = base64.encodebytes(data).decode()
        urlsafe = '\n'.join(base64.urlsafe_b64encode(data).decode()[i:i + 64] for i in range(0, 4000, 64))
        for name, text in (('xxd -p', xxd_p), ('xxd', xxd), ('base64', b64), ('url-safe', urlsafe)):
            self.assertIn('dump', rules('docs/n.md', 'Intro\n' + text + '\nEnd\n'), name)
        short = '\n'.join(b64.split('\n')[:3])  # 228 characters: under the joined base64 size
        self.assertEqual(rules('docs/n.md', 'Intro\n' + short + '\nEnd\n'), [])
        self.assertEqual(rules('docs/n.md', '\n'.join(['-' * 60] * 10)), [])

    def test_svg_keeps_blob_checks(self):
        self.assertEqual(rules('art/a.svg', ' '.join(['1'] * 100)), [])
        payload = base64.b64encode(random_bytes(300)).decode()
        self.assertIn('dump', rules('art/a.svg', f'<svg><image href="data:image/png;base64,{payload}"/></svg>'))
        self.assertIn('dump', rules('tools/page.js', f'const img = "data:image/png;base64,{payload}";'))
        self.assertEqual(rules('tools/page.js', "if (s.startsWith('data:image/png;base64,')) {}"), [])

    def test_email_forms(self):
        self.assertIn('email', rules('docs/n.md', f'write to {EMAIL}'))
        self.assertEqual(rules('docs/n.md', f'{NOREPLY}, a{AT}example.com, icon{AT}2x.png'), [])
        self.assertIn('email', rules('docs/n.md', j('see jane', AT, 'mail.png')))  # only the density form passes
        self.assertEqual(rules('docs/n.md', j('trailer <noreply', AT, 'example-trailer.org>')), [])
        for near in (j('x-noreply-y', AT, 'mail.net'), j('git', AT, 'github.com'),
                     j('bot', AT, 'users.noreply.github.com'), j('noreply', AT, 'other-vendor.net')):
            self.assertIn('email', rules('docs/n.md', f'see {near}'), near)

    def test_user_path(self):
        self.assertIn('user-path', rules('tools/a.ps1', f"$p = '{USER_PATH}'"))
        self.assertIn('user-path', rules('docs/n.md', j('/ho', 'me/bob/x')))
        quad = j('"C:', BACKSLASH * 4, 'Users', BACKSLASH * 4, 'alice', BACKSLASH * 4, 'x"')
        self.assertIn('user-path', rules('tests/x.json', '{"p": ' + quad + '}'))
        self.assertIn('user-path', rules('tools/a.sh', j('cd /c/', 'Users/alice/x')))
        self.assertIn('user-path', rules('docs/n.md', j(BACKSLASH * 2, 'server', BACKSLASH, 'Users', BACKSLASH, 'alice')))
        self.assertIn('user-path', rules('docs/n.md', j('D:', BACKSLASH, 'Documents and Settings', BACKSLASH, 'alice')))
        self.assertEqual(rules('docs/n.md', j('C:', '\\Users\\', 'Public\\x and C:', '\\Users\\', '<name>\\x')), [])
        self.assertEqual(rules('tools/a.ps1', j('C:', '\\Users\\', '$env:USERNAME')), [])
        self.assertEqual(rules('docs/n.md', j('see https://example.org', '/ho', 'me/index and github.com',
                                              '/us', 'ers/someone/repo and docs/us', 'ers/list')), [])

    def test_account_and_credential(self):
        self.assertIn('account-id', rules('docs/n.md', f'id {STEAM}'))
        self.assertEqual(rules('docs/n.md', 'id 1234567890123456'), [])
        self.assertIn('credential', rules('tools/a.py', f'T = "{TOKEN}"'))
        self.assertEqual(rules('tools/a.py', j('T = "gh', 'p_short"')), [])
        jwt = j('ey', 'J', 'hbGciOiJIUzI1NiJ9', '.', 'ey', 'J', 'zdWIiOiIxMjM0NTY3ODkwIn0', '.', 'abcdefghij0123456789')
        self.assertIn('credential', rules('docs/n.md', jwt))
        self.assertIn('credential', rules('docs/n.md', j('https://hooks.slack', '.com/services/T0000/B0000/', 'x' * 20)))
        self.assertIn('credential', rules('docs/n.md', j('-----BEGIN ENCRYPTED ', 'PRIVATE KEY-----')))
        self.assertEqual(rules('docs/n.md', 'We keep no private key in the tree.'), [])

    def test_private_names_list(self):
        with tempfile.TemporaryDirectory() as folder:
            names = Path(folder, 'names.txt')
            names.write_text('# comment\n' + PRIVATE + '\n', encoding='utf-8')
            gate.load_names([str(names)])
            try:
                self.assertIn('private-name', rules('docs/n.md', f'the {PRIVATE.upper()} PC'))
                self.assertEqual(rules('docs/n.md', f'the {PRIVATE}s2 PC'), [])
                self.assertIn('1 name(s)', gate.NAMES['status'])
                self.assertNotIn(folder, gate.NAMES['status'])
            finally:
                gate.load_names([])
        self.assertIn('none found', gate.NAMES['status'])

    def test_excerpt_never_shows_the_match(self):
        for path, text in (('docs/n.md', f'mail {EMAIL}'), ('tools/a.py', f'T = "{TOKEN}"'),
                           ('docs/n.md', f'path {USER_PATH}')):
            for f in findings_of(path, text):
                for secret in TRIGGERS:
                    self.assertNotIn(secret, f.excerpt)
        email = [f for f in findings_of('docs/n.md', f'mail {EMAIL}') if f.rule == 'email']
        self.assertEqual(email[0].excerpt, f'[e-mail] ({len(EMAIL)} chars)')

    def test_messages(self):
        found = {f.rule for f in gate.scan_message('pr', f'Fixes it.\n\nSee {USER_PATH} and {FUN}.', config())}
        self.assertEqual(found, {'user-path', 'decompiler-mark'})
        self.assertEqual(gate.scan_message('pr', 'Plain description, 3 files.', config()), [])

    def test_mergetag_header_is_judged(self):
        ident = f'bot <{NOREPLY}> 1700000000 +0000'
        tag_text = (f'object {"a" * 40}\ntype commit\ntag v1\ntagger {ident}\n\nrelease notes {USER_PATH}\n')
        merge = (f'tree {"b" * 40}\nparent {"c" * 40}\nparent {"d" * 40}\nauthor {ident}\ncommitter {ident}\n'
                 'mergetag ' + tag_text.rstrip('\n').replace('\n', '\n ') + '\n\nMerge tag v1\n').encode()
        found = gate.identity_and_message(None, 'e' * 40, merge, 'commit:eeee', config())
        self.assertIn(('user-path', 'commit:eeee:mergetag'), {(f.rule, f.path) for f in found})

    def test_added_lines_by_content(self):
        self.assertEqual(gate.added_line_numbers('a\nb\n', 'a\nb\nc\n'), {3})
        self.assertEqual(gate.added_line_numbers('a\nb\n', 'b\na\n'), set())  # a move adds nothing
        self.assertEqual(gate.added_line_numbers('a\nb\n', 'a\nb\na\n'), {1, 3})  # an extra copy is added
        self.assertEqual(gate.added_line_numbers(None, 'a\n'), {1, 2})


class BaselineTests(unittest.TestCase):
    def setUp(self):
        gate.load_names([])

    def write(self, found, folder):
        out = os.path.join(folder, 'b.txt')
        gate.write_baseline(found, out, 'test')
        return Path(out).read_text(encoding='utf-8')

    def test_baseline_counts_lines(self):
        text = f'one {EMAIL}\n'
        found = findings_of('docs/n.md', text)
        with tempfile.TemporaryDirectory() as folder:
            baseline = self.write(found, folder)
        self.assertNotIn(EMAIL, baseline)
        cfg = config(baseline=baseline)
        kept, baselined = gate.apply_baseline(findings_of('docs/n.md', text, cfg), cfg)
        self.assertEqual((kept, baselined), ([], 1))
        kept, baselined = gate.apply_baseline(findings_of('docs/n.md', text + text, cfg), cfg)
        self.assertEqual((len(kept), baselined), (1, 1))
        # Range judging: an added line is never baselined, even when its content is.
        added = gate.added_line_numbers(text, text + text)
        kept, _ = gate.judge(findings_of('docs/n.md', text + text, cfg), added, cfg)
        self.assertEqual(len(kept), 2)

    def test_stale_entries_warn(self):
        with tempfile.TemporaryDirectory() as folder:
            cfg = config(baseline=self.write(findings_of('docs/n.md', f'one {EMAIL}\n'), folder))
        kept, _ = gate.apply_baseline([], cfg, report_stale=True)
        self.assertEqual([f.rule for f in kept], ['stale-baseline'])
        self.assertEqual(gate.RULES['stale-baseline'][0], gate.WARN)

    def test_private_entries_stay_out_of_the_tracked_baseline(self):
        with tempfile.TemporaryDirectory() as folder:
            Path(folder, 'names.txt').write_text(PRIVATE + '\n', encoding='utf-8')
            gate.load_names([str(Path(folder, 'names.txt'))])
            try:
                found = findings_of('docs/n.md', f'the {PRIVATE} PC and {EMAIL}\n')
                out = os.path.join(folder, 'tracked.txt')
                self.assertEqual(gate.write_baseline(found, out, 'test'), (1, 1))
                self.assertNotIn('private-name', Path(out).read_text(encoding='utf-8'))
                self.assertIn('private-name', Path(folder, gate.NAMES_BASELINE).read_text(encoding='utf-8'))
            finally:
                gate.load_names([])

    def test_bad_config_is_an_error(self):
        with self.assertRaises(gate.GateError):
            gate.parse_allow('binary *.png')
        with self.assertRaises(gate.GateError):
            gate.parse_allow('no-such-rule *.png -- why')
        with self.assertRaises(gate.GateError):
            gate.parse_baseline('email\tdocs/n.md\tabc')


# Every git call here (and the gate's, through gate_env) runs with an empty global config and no system config,
# so a developer's core.hooksPath, signing or other settings neither break the fixtures nor receive the hook.
EMPTY_CONFIG = os.path.join(tempfile.mkdtemp(prefix='leak_gate_cfg_'), 'empty.gitconfig')
Path(EMPTY_CONFIG).write_text('', encoding='utf-8')


def controlled_env(**extra):
    env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_') and not k.startswith('OPENWILLOW_')}
    env.update(GIT_CONFIG_GLOBAL=EMPTY_CONFIG, GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0')
    env.update(extra)
    return env


def git(cwd, *args, env=None, check=True, data=None):
    proc = subprocess.run(['git', '-c', 'core.autocrlf=false', *args], cwd=cwd, capture_output=True,
                          input=data, env=env or controlled_env())
    if check and proc.returncode != 0:
        raise AssertionError(f'git {args} failed: {proc.stderr.decode(errors="replace")}')
    proc.text = (proc.stdout + proc.stderr).decode('utf-8', 'replace')
    return proc


def gate_env(**extra):
    env = controlled_env(OPENWILLOW_LEAK_NAMES='', OPENWILLOW_PYTHON=sys.executable)
    env.update(extra)
    return env


def run_gate(cwd, *args, stdin='', names='', **env):
    script = str(Path(cwd, 'tools', 'leak_gate.py'))
    proc = subprocess.run([sys.executable, script, *args], cwd=cwd, input=stdin.encode(), capture_output=True,
                          env=gate_env(OPENWILLOW_LEAK_NAMES=names, **env))
    return proc.returncode, (proc.stdout + proc.stderr).decode('utf-8', 'replace')


class RepoFixture:
    """A clone whose origin/main holds the gate and its lists, as after this change lands. A mixin, so
    tests/leak_gate_hook_test.py can share it without collecting these tests twice."""

    def setUp(self):
        gate.load_names([])
        self.folder = tempfile.mkdtemp()
        self.repo = os.path.join(self.folder, 'repo')
        self.remote = os.path.join(self.folder, 'remote.git')
        git(self.folder, 'init', '-q', '--bare', self.remote)
        git(self.folder, 'init', '-q', '-b', 'main', 'repo')
        for key, value in (('user.name', 'bot'), ('user.email', NOREPLY), ('core.autocrlf', 'false')):
            git(self.repo, 'config', key, value)
        git(self.repo, 'remote', 'add', 'origin', self.remote)
        self.write('.gitignore', '/local/\n')
        self.write('notes.md', f'old line {EMAIL}\nsecond line\n')
        self.install_gate()
        git(self.repo, 'add', '-A')
        git(self.repo, 'commit', '-q', '-m', 'base')
        self.base = self.head()
        git(self.repo, 'push', '-q', 'origin', 'main')

    def tearDown(self):
        shutil.rmtree(self.folder, ignore_errors=True)

    def install_gate(self, allow=ALLOW):
        os.makedirs(os.path.join(self.repo, 'tools'), exist_ok=True)
        os.makedirs(os.path.join(self.repo, '.githooks'), exist_ok=True)
        shutil.copy(ROOT / 'tools' / 'leak_gate.py', os.path.join(self.repo, 'tools'))
        shutil.copy(ROOT / '.githooks' / 'pre-push', os.path.join(self.repo, '.githooks'))
        self.write('tools/leak_gate_allow.txt', allow + '\n')

    def write(self, name, text):
        Path(self.repo, name).parent.mkdir(parents=True, exist_ok=True)
        Path(self.repo, name).write_bytes(text.encode('utf-8') if isinstance(text, str) else text)

    def head(self):
        return git(self.repo, 'rev-parse', 'HEAD').stdout.decode().strip()

    def commit(self, name, text, message='change', add_all=True):
        self.write(name, text)
        git(self.repo, 'add', '-A' if add_all else name)
        git(self.repo, 'commit', '-q', '-m', message)
        return self.head()

    def commit_tree_entry(self, mode, sha, path, message='plumbing'):
        """Adds one index entry by plumbing (for names the file system cannot hold) and commits it."""
        git(self.repo, '-c', 'core.protectNTFS=false', 'update-index', '--add', '--cacheinfo', f'{mode},{sha},{path}')
        tree = git(self.repo, 'write-tree').stdout.decode().strip()
        commit = git(self.repo, 'commit-tree', tree, '-p', 'HEAD', '-m', message).stdout.decode().strip()
        git(self.repo, 'update-ref', 'HEAD', commit)
        return commit

    def blob(self, text):
        return git(self.repo, 'hash-object', '-w', '--stdin', data=text.encode()).stdout.decode().strip()

    def range_gate(self, base=None, *extra):
        return run_gate(self.repo, '--range', f'{base or self.base}..HEAD', *extra)




class MoreRuleTests(unittest.TestCase):
    def setUp(self):
        gate.load_names([])

    def test_decoded_binary_cannot_be_read_as_text(self):
        decoded = gate.decode(b'a\0b')
        self.assertTrue(decoded.binary)
        with self.assertRaises(gate.GateError):
            decoded.text
        self.assertEqual([f.rule for f in decoded.findings('msg')], ['binary'])
        self.assertEqual(rules('docs/n.md', 'plain\0text'), ['binary'])

    def test_signed_tag_and_object_headers(self):
        ident = f'bot <{NOREPLY}> 1700000000 +0000'
        body = '\n'.join(base64.b64encode(random_bytes(48, seed=n)).decode() for n in range(12))
        armour = f'-----BEGIN PGP SIGNATURE-----\n\n{body}\n=abcd\n-----END PGP SIGNATURE-----\n'
        tag = f'object {"a" * 40}\ntype commit\ntag v1\ntagger {ident}\n\nRelease one.\n{armour}'
        self.assertEqual(gate.identity_and_message(None, 'e' * 40, tag.encode(), 'tag', config(), kind='tag'), [])
        bad = tag.replace('=abcd', f'token {TOKEN}')
        rules_found = {f.rule for f in gate.identity_and_message(None, 'e' * 40, bad.encode(), 'tag', config(), kind='tag')}
        self.assertIn('object-format', rules_found)
        named = tag.replace('tag v1', f'tag v1-{TOKEN}')
        self.assertIn('credential', {f.rule for f in gate.identity_and_message(None, 'e' * 40, named.encode(), 'tag',
                                                                             config(), kind='tag')})
        commit = f'tree {"b" * 40}\nauthor {ident}\ncommitter {ident}\nx-extra {TOKEN}\n\nmessage\n'
        found = {f.rule for f in gate.identity_and_message(None, 'e' * 40, commit.encode(), 'c', config())}
        self.assertIn('object-format', found)
        self.assertIn('credential', found)
        sig = (f'tree {"b" * 40}\nauthor {ident}\ncommitter {ident}\ngpgsig -----BEGIN PGP SIGNATURE-----\n \n'
               f' {TOKEN}\n -----END PGP SIGNATURE-----\n\nmessage\n')
        self.assertIn('object-format', {f.rule for f in gate.identity_and_message(None, 'e' * 40, sig.encode(), 'c',
                                                                                config())})
        nul = f'tree {"b" * 40}\nauthor {ident}\ncommitter {ident}\n\nmes\0sage\n'
        self.assertIn('binary', {f.rule for f in gate.identity_and_message(None, 'e' * 40, nul.encode(), 'c', config())})

    def test_batch_framing_is_checked(self):
        class Proc:
            def __init__(self, out):
                self.stdout, self.returncode = out, 0
        oid = 'a' * 40
        g = gate.Git()
        for out in (f'{oid} blob 100\nshort\n'.encode(), f'{"b" * 40} blob 1\nx\n'.encode(),
                    f'{oid} blob 1\nx\nextra'.encode()):
            g.run = lambda *a, out=out, **k: Proc(out)
            with self.assertRaises(gate.GateError):
                g.objects([oid])
        g.run = lambda *a, **k: Proc(f'{oid} blob 1\nx\n'.encode())
        self.assertEqual(g.objects([oid]), {oid: ('blob', b'x')})

    def test_unreadable_local_folder_is_reported(self):
        from unittest import mock
        gate.INCOMPLETE.clear()
        with tempfile.TemporaryDirectory() as folder:
            os.makedirs(os.path.join(folder, 'local', 'sub'))
            g = gate.Git(cwd=folder)
            real = os.scandir

            def failing(path):
                if str(path).endswith('sub'):
                    raise PermissionError('denied')
                return real(path)
            with mock.patch.object(gate, 'local_roots', lambda git, root: [os.path.join(folder, 'local')]), \
                    mock.patch.object(gate.os, 'scandir', failing):
                gate.local_copies(g, folder, {'a.md': b'x' * 100})
        self.assertTrue(gate.INCOMPLETE)
        gate.INCOMPLETE.clear()

    def test_round3_dump_details(self):
        row = ', '.join(['0x4d', '0x5a', '0x90', '0x00', '0x03', '0x00', '0x0f', '0xff']) + ','
        self.assertIn('dump', rules('src/blob.cpp', 'static const unsigned char B[] = {\n' + '\n'.join([row] * 10) + '\n};\n'))
        self.assertNotIn('dump', rules('src/blob.cpp', '\n'.join([row] * 6)))
        rows = ''.join((f'| {n} | {n * 3} | {n * 7} | {n * 11} |\n' if n % 30 else '| a | b | c | d |\n') for n in range(1, 1001))
        self.assertIn('dump', rules('docs/n.md', rows))  # 4000 numbers cut into chunks of 30 rows
        floats = ''.join((f'    {n * 0.5:.3f}, {n * 0.25:.3f}, {n * 0.125:.3f},\n' if n % 7 else '    # block\n')
                         for n in range(1, 1144))
        self.assertIn('dump', rules('tools/table.py', floats))  # about 2940 floats with a label every seven lines
        b64 = base64.b64encode(random_bytes(600)).decode()
        lines = [b64[i:i + 30] for i in range(0, len(b64), 30)]
        lines[3] = 'AbCdEfGhIjKlMnOpQrStUvWxYzAbCd'  # one line with no digit
        self.assertIn('dump', rules('tools/x.py', 'X = (\n' + ''.join(f'    "{line}"\n' for line in lines) + ')\n'))
        names = ['VectorToString', 'ReadPackageHeader', 'WriteSceneManifest', 'CheckSliceGate', 'LoadWeaponParts',
                 'BuildPhaselockFx', 'ResolveObjectPath', 'DecodeNameTable', 'OpenWillowMayaHud', 'TraceRealGame',
                 'PrepareLevelData', 'CrossCheckUmodel', 'InventoryActions', 'MissionDispatch2', 'TerrainWeightmap']
        self.assertNotIn('dump', rules('tools/x.py', 'NAMES = [\n' + ''.join(f'    "{n}",\n' for n in names * 2) + ']\n'))

    def test_signature_body_is_bounded(self):
        ident = f'bot <{NOREPLY}> 1700000000 +0000'
        body = '\n'.join(base64.b64encode(random_bytes(48, seed=n)).decode() for n in range(200))
        tag = (f'object {"a" * 40}\ntype commit\ntag v1\ntagger {ident}\n\nRelease.\n'
               f'-----BEGIN PGP SIGNATURE-----\n\n{body}\n-----END PGP SIGNATURE-----\n')
        found = {f.rule for f in gate.identity_and_message(None, 'e' * 40, tag.encode(), 'tag', config(), kind='tag')}
        self.assertIn('object-format', found)

    def test_names_get_the_content_rules(self):
        ident = lambda name: f'{name} <{NOREPLY}> 1700000000 +0000'
        for name, rule in ((f'bot {FUN}', 'decompiler-mark'),
                           (base64.b64encode(random_bytes(300)).decode(), 'dump'),
                           ('x' * 200, 'object-format')):
            commit = f'tree {"b" * 40}\nauthor {ident(name)}\ncommitter {ident("bot")}\n\nmessage\n'
            found = {f.rule for f in gate.identity_and_message(None, 'e' * 40, commit.encode(), 'c', config())}
            self.assertIn(rule, found, name[:20])
        self.assertEqual(gate.name_findings('ref-name', 'refs/heads/feature/leak-gate', config()), [])

    def test_credentials_inside_armour_are_refused(self):
        ident = f'bot <{NOREPLY}> 1700000000 +0000'
        key = j('AKIA', 'ABCDEFGHIJKLMNOP')
        body = '\n'.join([base64.b64encode(random_bytes(48, seed=n)).decode() for n in range(4)] + [key])
        tag = (f'object {"a" * 40}\ntype commit\ntag v1\ntagger {ident}\n\nRelease.\n'
               f'-----BEGIN PGP SIGNATURE-----\n\n{body}\n-----END PGP SIGNATURE-----\n')
        found = {f.rule for f in gate.identity_and_message(None, 'e' * 40, tag.encode(), 'tag', config(), kind='tag')}
        self.assertIn('credential', found)

if __name__ == '__main__':
    unittest.main()
