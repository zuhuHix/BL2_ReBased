#!/usr/bin/env python3
"""Leak gate: refuse what must never be published from this repository.

Scans a commit range, the staged changes, a whole tree or free text for game-derived files, decompiler
output, dumps and personal identifiers. Rules, scopes and thresholds are described in docs/TOOLING.md
("Leak gate"); every rule has an id (RULES below) and a synthetic test in tests/leak_gate_test.py.

Exit codes: 0 clean (warnings may be printed), 1 at least one error, 2 usage or internal error, 3 (with --strict
only) no error but at least one warning other than stale-baseline.
A finding prints its rule, where it is and a short masked excerpt, never the flagged text in full; paths that
are themselves the finding are masked too (--show-paths prints them, for local use only).

Two files configure it:
  tools/leak_gate_allow.txt     maintainer-owned: '<rule> <path glob> [sha256=<hex>] -- <reason>' per line,
                                and 'allowed-address <address> -- <reason>' for the exact addresses allowed.
  tools/leak_gate_baseline.txt  existing findings on main, by rule, path and a hash of the line.
In range, staged, pre-push and text mode both are read from a trusted commit, never from the change being
judged: --config-from REV, or by default refs/remotes/origin/main, whatever the merge base. When that commit cannot
be found or holds no lists the gate refuses (exit 2); there is no fallback. A bootstrap run names the commit to use
with --config-from, and the run says so. A range that edits the gate, its lists, a hook, a workflow or a
.gitattributes gets a gate-change warning: such a change needs the maintainer's yes before landing. --strict makes
any warning but stale-baseline fail the run with exit 3. Plain Python, no dependency, no network.
"""
import argparse
import bisect
import collections
import fnmatch
import hashlib
import os
import re
import subprocess
import sys
import tempfile

ALLOW_FILE = 'tools/leak_gate_allow.txt'
BASELINE_FILE = 'tools/leak_gate_baseline.txt'
GATE_FILE = 'tools/leak_gate.py'
TRUSTED_REF = 'refs/remotes/origin/main'
HOOK_MARKER = 'OPENWILLOW_LEAK_GATE'
ZERO_SHA = '0' * 40
# Changes to these need the maintainer's yes before landing (rule gate-change).
GATE_PATHS = (GATE_FILE, ALLOW_FILE, BASELINE_FILE)
GATE_PREFIXES = ('.githooks/', '.github/workflows/')
GATE_NAMES = ('.gitattributes', '.gitmodules')  # in any folder: they change how git presents content

ERROR, WARN = 'ERROR', 'WARN'
RULES = {
    'forbidden-type': (ERROR, 'a game file, an exported asset or an analysis database type'),
    'unknown-type': (WARN, 'a file type the tree has never held; ask for an allow-list entry if it is meant'),
    'binary': (ERROR, 'a binary file outside the allow-list'),
    'undecodable': (ERROR, 'a file without NUL bytes that is neither UTF-8 nor BOM-marked UTF-16'),
    'oversize': (ERROR, 'a file over the size limit outside the allow-list'),
    'gitlink': (ERROR, 'a submodule entry (its content is outside this repository and unchecked)'),
    'lfs-pointer': (ERROR, 'a Git LFS pointer (the content it names is stored outside the tree, unchecked)'),
    'local-path': (ERROR, 'a path inside local/ or another ignored output folder'),
    'local-copy': (WARN, 'byte-identical to a file under local/ (game-derived output lives there)'),
    'decompiler-mark': (ERROR, 'a name that decompilers and disassemblers generate'),
    'decompiler-note-mark': (ERROR, 'a decompiler variable or member form in prose (this-> is skipped inside '
                                    'fenced code blocks and inline code spans, which quote the '
                                    'project\'s own code)'),
    'address-hex': (WARN, 'a value shaped like an address in the executable, in prose'),
    'dump': (ERROR, 'rows of hex bytes, a hex dump, hex, base64 or base85 text however wrapped or quoted, a data '
                    'URI, escaped bytes, or a numeric table far larger than any the tree holds'),
    'dump-like': (WARN, 'a numeric table or a list of hash-length hex lines larger than the tree holds; refused only '
                        'at the dump sizes'),
    'email': (ERROR, 'an e-mail address that is not an approved form (GitHub noreply, the allow-list, a '
                     'reserved example domain)'),
    'user-path': (ERROR, 'an absolute path inside a user profile'),
    'account-id': (ERROR, 'a Steam account id'),
    'credential': (ERROR, 'a well-known credential, webhook or private-key form'),
    'game-config': (ERROR, 'configuration in the game\'s (UE3) section form, not the project\'s UE5 form'),
    'private-name': (ERROR, 'a token from the private names list in ignored local/'),
    'gate-change': (WARN, 'the change edits the gate, its lists, a hook, a workflow or git attributes: it needs '
                          'the maintainer\'s yes before landing'),
    'stale-baseline': (WARN, 'a baseline entry whose line is gone; remove it from the baseline'),
    'object-format': (ERROR, 'a commit or tag header the gate does not know, or a signature block of the wrong '
                             'shape: it cannot be judged, so it is refused'),
    'inspection-incomplete': (WARN, 'an optional inspection (the local/ copy check) could not finish'),
}
# Rules about a piece of identity, also run over file names, ref names, author, committer and tagger names.
IDENT_RULES = ('email', 'user-path', 'account-id', 'credential', 'private-name')
# Rules decided by the path: their findings print the path masked (it is the finding).
PATH_RULES = ('forbidden-type', 'local-path', 'unknown-type')

SIZE_LIMIT = 1024 * 1024  # bytes; the largest text file on main on 2026-11-11 is about 570 KB.

# --- file kinds -------------------------------------------------------------------------------------------
CODE_EXT = {'py', 'pyw', 'cpp', 'cc', 'cxx', 'c', 'h', 'hpp', 'hh', 'inl', 'cs', 'java', 'js', 'mjs', 'cjs',
            'ts', 'tsx', 'jsx', 'ps1', 'psm1', 'psd1', 'sh', 'bash', 'bat', 'cmd', 'cmake', 'as', 'lua', 'rs', 'go'}
CODE_NAMES = {'cmakelists.txt'}
PROSE_EXT = {'md', 'markdown', 'txt', 'rst', 'html', 'htm', 'adoc'}
# Commit messages, tag messages and PR descriptions are scanned as prose.

FORBIDDEN_EXT = {
    # Game files (docs/LEGAL.md, "What this repository does not contain"): packages including script
    # packages, texture caches, audio banks, movies, UI movies, saves and configuration. A .ini is allowed only
    # where the allow-list names the project's own UE5 configuration, and is content-checked there too.
    'upk', 'umap', 'u', 'tfc', 'pck', 'bnk', 'wem', 'bik', 'bk2', 'usm', 'gfx', 'swf', 'uncompressed_size',
    'xxx', 'sav', 'ini',
    # Decompiler and disassembler databases (the edit hook's list plus IDA's unpacked parts).
    'gzf', 'gpr', 'rep', 'idb', 'i64', 'id0', 'id1', 'id2', 'nam', 'til', 'bndb',
    # Unreal asset files and UModel exports: meshes, skeletons, animations, materials.
    'uasset', 'uexp', 'ubulk', 'psk', 'pskx', 'psa', 'mat', 'fbx', 'obj', 'gltf', 'glb', 'dae', '3ds', 'blend',
    'ply', 'stl', 'bvh', 'md5mesh', 'md5anim',
    # Textures and images (the README banner is pinned by hash in the allow-list).
    'dds', 'tga', 'bmp', 'png', 'jpg', 'jpeg', 'webp', 'gif', 'tif', 'tiff', 'exr', 'hdr', 'ktx', 'ktx2', 'psd',
    # Decoded audio and video.
    'ogg', 'wav', 'mp3', 'flac', 'opus', 'm4a', 'aac', 'wma', 'mp4', 'webm', 'avi', 'mov', 'mkv', 'wmv', 'flv',
}
FORBIDDEN_SUFFIXES = ('.props.txt',)  # UModel's property dumps
FORBIDDEN_DIR_SUFFIXES = ('.rep',)    # a Ghidra project folder
# A section header in the game's (UE3) form, [Package.Class] or [IniVersion], rather than UE5's
# [/Script/Module.Class]. UE5 itself still uses [Core.Log] and [Core.System], so Core sections pass. A BOM before
# it and a trailing ; or # comment are allowed. One such header refuses a .ini; any other text file needs two
# (a doc may quote a single section name). Every header is reported, so each added one is judged on its own.
GAME_INI_SECTION_RE = re.compile(r'^\ufeff?\s*\[(?!/Script/)(?!Core\.)(?:[A-Za-z_]\w*\.[A-Za-z_]\w*|IniVersion)\]'
                                 r'\s*(?:[;#].*)?$')
LFS_POINTER = b'version https://git-lfs.github.com/spec/'

# Ignored output folders (.gitignore and the hub preamble): committing into one needs `git add -f`. Compared
# without regard to letter case (Windows file systems ignore it).
LOCAL_PREFIXES = ('local/', 'build/', 'host/ue5/openwillow/binaries/', 'host/ue5/openwillow/intermediate/',
                  'host/ue5/openwillow/saved/', 'host/ue5/openwillow/deriveddatacache/',
                  'host/ue5/openwillow/content/', 'host/ue5/openwillow 5.8/')
LOCAL_RE = re.compile(r'^build-[^/]*/')
# Refused by name wherever they sit: the maintainer's screenshot folder and prompt notes.
PRIVATE_DIR_NAMES = ('bl2screens',)
PRIVATE_FILE_NAMES = ('promptupgrade.md',)
# A list of private tokens (account names, private repository names) the tree must not hold. It lives in
# ignored local/ and is never tracked; the gate says whether it found one. Its baseline sits next to it, so the
# tracked baseline never maps which public files hold a private token.
NAMES_FILE = 'local/leak_gate_names.txt'
NAMES_BASELINE = 'leak_gate_names_baseline.txt'

# --- text patterns ----------------------------------------------------------------------------------------
# Names decompilers generate, with an address-sized hex suffix or a fixed shape. None of them occurs on main
# (2026-11-11), so they are errors in every text file, code included.
DECOMPILER_RE = re.compile(
    r'\b(?:FUN|DAT|LAB|PTR_FUN|PTR_DAT|PTR_LAB|thunk_FUN|switchD|caseD)_[0-9a-fA-F]{6,16}\b'
    r'|\bPTR_\w+_[0-9a-fA-F]{8}\b'
    r'|\b(?:sub|loc|off|dword|qword|word|byte|unk|asc|stru|nullsub)_[0-9A-F]{6,16}\b'
    r'|\bundefined(?:1|2|4|8)\b'
    r'|\b(?:CONCAT|SUB|ZEXT|SEXT)[0-9]{2}\('
    r'|\b(?:i|u|b|c|s|l|f|d|pc|pu|pi|pp|ps|pb|pl|pf|pv|ppc|ppu|ppi|pps|au|ab|ai)Var[0-9]+\b'
    r'|\b(?:in|unaff|extraout)_(?:E?[ABCD]X|E?[SD]I|E?[SB]P|[ABCD][LH]|[CDEFGS]S|R[ABCD]X|R[0-9]{1,2}D?'
    r'|XMM[0-9]{1,2}(?:_[A-Za-z]+)?|ST[0-7])\b')
# Forms that are normal in source code but mark a decompiler listing in prose: checked in prose only.
# (A bare `->` is not one: prose uses it as an arrow.) `this->` inside a fenced code block is skipped.
NOTE_MARK_RE = re.compile(r'\bparam_[0-9]+\b|\blocal_(?=[0-9a-fA-F]*[0-9])[0-9a-fA-F]{1,8}\b|\bthis->\w')
FENCE_RE = re.compile(r'^\s{0,3}(```|~~~)')
# A hex value in the range a 32-bit image of the executable's size occupies. Constants and flags share the
# range, so this is a warning; round values (two or fewer non-zero digits) are skipped.
HEX_RE = re.compile(r'\b0x([0-9a-fA-F]{7,8})\b')
ADDRESS_LOW, ADDRESS_HIGH = 0x00400000, 0x03FFFFFF

DIGIT_RE = re.compile(r'[0-9]')
LETTER_RE = re.compile(r'[A-Za-z]')
TOKEN_RE = re.compile(r'[^\s,;\[\](){}|]+')
NUMBER_RE = re.compile(r'^[-+]?(?:0[xX][0-9a-fA-F]+|[0-9a-fA-F]{2}|[0-9]+(?:\.[0-9]+)?(?:[eE][-+]?[0-9]+)?)'
                       r'[fFuUlLhH]?:?$')
HEX_BLOB_RE = re.compile(r'[0-9a-fA-F]{160,}')
BASE64_BLOB_RE = re.compile(r'[A-Za-z0-9+/_-]{256,}={0,2}')
ESCAPED_BYTES_RE = re.compile(r'(?:\\x[0-9a-fA-F]{2}){32,}')
DATA_URI_RE = re.compile(r'data:[\w/+.-]*;base64,[A-Za-z0-9+/_-]{64,}')
# A binary's text form spread over lines, however it is wrapped or quoted: each line is reduced to a fragment
# (quotes, commas, semicolons, brackets, '+' and '\x' removed), and a run of consecutive fragments that are hex
# (spaces inside allowed: xxd groups) or base64 (mixed case and digits, no spaces) is joined and measured.
HEXDUMP_LINE_RE = re.compile(r'^\s*[0-9a-fA-F]{4,16}:?\s+(?:[0-9a-fA-F]{2,8}\s+){4,}')
FRAGMENT_STRIP_RE = re.compile(r'\\x|[\'"`,;\[\](){}]')
CONCAT_PLUS_RE = re.compile(r'^\+\s*|\s*\+$')  # a '+' joining string pieces, at either end of the line
HEX_FRAGMENT_RE = re.compile(r'[0-9a-fA-F]+')
BASE64_FRAGMENT_RE = re.compile(r'[A-Za-z0-9+/_=-]+')
BASE85_LINE_RE = re.compile(r'[0-9A-Za-z!#$%&()*+\-;<=>?@^_`{|}~]{40,}')
BASE85_PUNCT = set('!#$%&()*+-;<=>?@^_`{|}~')
BYTE_TOKEN_RE = re.compile(r'(?:0[xX])?([0-9a-fA-F]{2})')  # a byte written as 4d or 0x4d
HASH_LENGTHS = {32, 40, 64, 128}  # md5, sha1, sha256, sha512 written one per line: a hash list, not a dump
MIN_FRAGMENT = 8
JOINED_HEX, JOINED_BASE64, JOINED_DATA_URI = 160, 256, 64
# Numeric tables, measured once on main (2026-11-11, the tracked tree without SVG): the longest run of
# mostly-numeric lines is 6 lines with 12 numbers (docs/verification/WEAPON_VISUALS.md); the most numbers on one such
# line is 28 (host/ue5/import_weapon_paint.py), 24 in tests/ (tests/collision_test.py). Summed over a whole file, the
# numeric lines reach 8 lines with 28 numbers in prose (WEAPON_VISUALS.md) and 2 lines with 56 numbers in code; the
# one data file above the error sizes is research/native_by_class.json (5433 one-number lines in runs of at most
# 3), which the baseline holds as it is. A table above the warning sizes is reported (dump-like); one about ten
# times larger than anything tracked is refused, per run and per file (a table cut into chunks adds up). Rows of
# two-digit hex bytes (4d or 0x4d, with some a-f digits) are a byte dump at the warning size already.
TABLE_WARN_LINES, TABLE_WARN_NUMBERS, TABLE_WARN_COLUMN = 8, 48, 24
TABLE_ERROR = {'prose': (64, 512), 'code': (128, 1024), 'data': (128, 1024)}  # (lines, numbers)
LINE_WARN_NUMBERS, LINE_ERROR_NUMBERS = 64, 512
BLOB_RUN_LINES = 8

EMAIL_RE = re.compile(r'(?<![\w.%+-])[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,24}\b')
# The approved address forms, the same for file content and commit metadata: GitHub's per-user noreply address
# (numeric id, '+', login), the exact addresses the allow-list names, and RFC 2606 reserved example domains.
GITHUB_NOREPLY_RE = re.compile(r'^[0-9]+\+[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?@users\.noreply\.github\.com$', re.I)
RESERVED_DOMAIN_RE = re.compile(r'(?:^|\.)(?:example\.(?:com|org|net)|example|invalid|test|localhost)$', re.I)
# One narrow exception: an image named for a pixel density ("icon@2x.png"), which is a file name, not an address.
RETINA_NAME_RE = re.compile(r'^[\w.-]+@[1-4]x\.(?:png|jpg|jpeg|gif|svg|webp)$')
PLACEHOLDER_USERS = {'public', 'default', 'default user', 'all users', 'user', 'username', 'name', 'you',
                     'yourname', 'your-name', 'your_name', 'me', 'runner', 'shared', 'someone', 'example',
                     'runneradmin'}
# A user-profile path starts at a path root: a drive letter, a UNC share (\\host\ or \\host\c$\), a Git Bash drive
# (/c/), or a separator that is not part of a longer path or a URL (so docs/users/... and https://host/home/...
# pass). One to four separators cover escaped and doubly escaped forms in JSON and source strings. Profile roots:
# Users, home and the old Documents and Settings, on any drive.
USER_PATH_RE = re.compile(r'(?i)(?:\\\\[\w.-]+(?:\\[a-z]\$)?|\b[A-Z]:|(?<![\w.:/\\-])/[a-z](?=[\\/])'
                          r'|(?<![\w.:/\\-]))[\\/]{1,4}(?:Users|home|Documents and Settings)[\\/]{1,4}'
                          r'([^\\/\s"\'`<>|*?:;,)\]}]+)')
ACCOUNT_RE = re.compile(r'\b7656119[0-9]{10}\b|\[?\b[UGAI]:1:[0-9]{5,}\b\]?|\bSTEAM_[01]:[01]:[0-9]{4,}\b')
CREDENTIAL_RE = re.compile(
    r'\bgh[pousr]_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{30,}|\bsk-(?:ant-|proj-)?[A-Za-z0-9_-]{24,}'
    r'|\bAKIA[0-9A-Z]{16}\b|\bxox[abprs]-[A-Za-z0-9-]{10,}|\bAIza[0-9A-Za-z_-]{35}\b|\bglpat-[A-Za-z0-9_-]{20,}'
    r'|\bnpm_[A-Za-z0-9]{36}\b|\b[sr]k_live_[A-Za-z0-9]{20,}'
    r'|\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}'
    r'|hooks\.slack\.com/services/T[A-Za-z0-9_/]{10,}|discord(?:app)?\.com/api/webhooks/[0-9]{5,}/[A-Za-z0-9_-]{20,}'
    r'|-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----')

# Literal strings every match of a pattern contains. A pattern only runs on the lines that hold one of
# its anchors, which keeps a whole-tree scan to a few seconds. (lowercase=True: search the lowercased text.)
ANCHORS = {
    'decompiler-mark': (False, ('FUN_', 'DAT_', 'LAB_', 'PTR_', 'switchD_', 'caseD_', 'sub_', 'loc_', 'off_',
                                'word_', 'byte_', 'unk_', 'asc_', 'stru_', 'undefined', 'CONCAT', 'SUB', 'ZEXT',
                                'SEXT', 'Var', 'in_', 'unaff_', 'extraout_')),
    'decompiler-note-mark': (False, ('param_', 'local_', 'this->')),
    'address-hex': (False, ('0x',)),
    'email': (False, ('@',)),
    'user-path': (True, ('users', 'home', 'documents and settings')),
    'account-id': (False, ('7656119', ':1:', 'STEAM_')),
    'credential': (False, ('ghp_', 'gho_', 'ghu_', 'ghs_', 'ghr_', 'github_pat_', 'sk-', 'AKIA', 'xox', 'AIza',
                           'glpat-', 'npm_', 'k_live_', 'eyJ', 'hooks.slack.com', '/api/webhooks/',
                           'PRIVATE KEY')),
    'private-name': (True, ()),
}
ASCII_LOWER = str.maketrans('ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz')
IDENT_LINE_RE = re.compile(r'^(author|committer|tagger) (.*) <([^<>]*)> ([0-9]+) ([+-][0-9]{4})$')


class GateError(Exception):
    """Usage or internal error: exit 2, never read as clean. Messages never quote scanned input."""


# pseudo: the path is a label (a commit, a ref, a tag, a message), not a file, and is printed as is.
Finding = collections.namedtuple('Finding', 'rule path line excerpt key last commit pseudo', defaults=(None, None, False))
NOTES = []               # WARN lines about how the run was configured (printed before the summary)
INCOMPLETE = []          # optional inspections that could not finish (reported, never silent)
CURRENT = {'config': None, 'strict': False, 'no_trusted_exclude': False}  # run-wide options and config
SENSITIVE_PATHS = set()  # paths an identifier rule matched inside; printed masked


def mask(kind, text):
    """A short, safe stand-in for a match: its kind, at most its first four characters (fewer for a short
    match) and its length."""
    text = text.strip()
    shown = text[:max(1, min(4, len(text) // 4))].replace('\t', ' ')
    return f'[{kind}] {shown}... ({len(text)} chars)'


def line_key(rule, path, line_text):
    norm = ' '.join(line_text.split())
    return hashlib.sha256(f'{rule}\0{path}\0{norm}'.encode('utf-8', 'replace')).hexdigest()[:16]


# --- git --------------------------------------------------------------------------------------------------
class Git:
    def __init__(self, cwd=None):
        self.cwd = cwd
        self._empty_tree = None

    # Every git call reads the objects a push really sends: replace refs are ignored (worktrees share them), no
    # system or global configuration takes part (an empty file stands in for the global one), and no pager,
    # external diff, text conversion or optional lock either. Only the repository's own configuration is read.
    ENV = {'GIT_NO_REPLACE_OBJECTS': '1', 'GIT_PAGER': 'cat', 'PAGER': 'cat', 'GIT_OPTIONAL_LOCKS': '0',
           'GIT_TERMINAL_PROMPT': '0', 'LC_ALL': 'C', 'GIT_CONFIG_NOSYSTEM': '1'}
    UNSET = ('GIT_EXTERNAL_DIFF', 'GIT_DIFF_OPTS', 'GIT_REPLACE_REF_BASE', 'GIT_GRAFT_FILE', 'GIT_CONFIG',
             'GIT_CONFIG_PARAMETERS', 'GIT_CONFIG_COUNT', 'GIT_CONFIG_SYSTEM', 'GIT_DIR', 'GIT_WORK_TREE',
             'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_NAMESPACE')
    _empty_config = None

    @classmethod
    def environment(cls):
        if cls._empty_config is None:
            folder = tempfile.mkdtemp(prefix='leak_gate_')
            cls._empty_config = os.path.join(folder, 'empty.gitconfig')
            open(cls._empty_config, 'w').close()
        env = {k: v for k, v in os.environ.items() if k not in cls.UNSET and not k.startswith('GIT_CONFIG_KEY_')
               and not k.startswith('GIT_CONFIG_VALUE_')}
        env.update(cls.ENV)
        env['GIT_CONFIG_GLOBAL'] = cls._empty_config
        return env

    def run(self, *args, check=True, data=None):
        proc = subprocess.run(['git', '--no-replace-objects', *args], cwd=self.cwd, input=data, capture_output=True,
                              env=self.environment())
        if check and proc.returncode != 0:
            # git's message may quote a path or a ref; only the subcommand and the exit code are reported.
            raise GateError(f'git {args[0]} failed (exit {proc.returncode})')
        return proc

    def out(self, *args):
        return self.run(*args).stdout.decode('utf-8', 'replace')

    def toplevel(self):
        return self.out('rev-parse', '--show-toplevel').strip()

    def resolve(self, rev):
        proc = self.run('rev-parse', '--verify', '--quiet', rev + '^{commit}', check=False)
        if proc.returncode != 0:
            raise GateError('a revision given to the gate is not a commit')
        return proc.stdout.decode().strip()

    def try_resolve(self, rev):
        try:
            return self.resolve(rev)
        except GateError:
            return None

    def exists(self, spec):
        return self.run('cat-file', '-e', spec, check=False).returncode == 0

    def empty_tree(self):
        if self._empty_tree is None:
            self._empty_tree = self.run('hash-object', '-t', 'tree', '--stdin', data=b'').stdout.decode().strip()
        return self._empty_tree

    def objects(self, ids):
        """{object id: (type, bytes)} through one `git cat-file --batch`, by object id only (a path in the
        request would break the line protocol). An object that cannot be read is an internal error."""
        ids = list(dict.fromkeys(ids))
        result = {}
        if not ids:
            return result
        for oid in ids:
            if not re.fullmatch(r'[0-9a-f]{40}(?:[0-9a-f]{24})?', oid):
                raise GateError('internal: an object request that is not an object id')
        proc = self.run('cat-file', '--batch', data=''.join(i + '\n' for i in ids).encode('ascii'))
        data, pos = proc.stdout, 0
        for oid in ids:
            end = data.find(b'\n', pos)
            if end < 0:
                raise GateError('internal: a cat-file response ended early')
            header = data[pos:end].decode('ascii', 'replace').split()
            pos = end + 1
            if len(header) != 3 or header[0] != oid or not header[2].isdigit():
                raise GateError(f'cannot read object {oid[:12]} (missing from the object store, or a response '
                                'that does not match the request)')
            size = int(header[2])
            if pos + size + 1 > len(data) or data[pos + size:pos + size + 1] != b'\n':
                raise GateError(f'internal: the cat-file response for {oid[:12]} is shorter than it declares')
            result[oid] = (header[1], data[pos:pos + size])
            pos += size + 1
        if pos != len(data):
            raise GateError('internal: a cat-file response carries more than was asked for')
        return result

    def blobs(self, ids):
        got = self.objects(ids)
        for oid, (kind, _) in got.items():
            if kind != 'blob':
                raise GateError(f'object {oid[:12]} is a {kind}, not a file')
        return {oid: data for oid, (kind, data) in got.items()}

    def file_at(self, commit, path):
        """The bytes of path in commit's tree, or None: resolved through `ls-tree -z`, read by object id."""
        out = self.run('ls-tree', '-z', commit, '--', path).stdout.decode('utf-8', 'replace')
        for entry in out.split('\0'):
            if entry:
                meta, name = entry.split('\t', 1)
                mode, kind, oid = meta.split()
                if name == path and kind == 'blob':
                    return self.blobs([oid])[oid]
        return None


# --- configuration ----------------------------------------------------------------------------------------
def parse_allow(text):
    entries, addresses = [], set()
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        body, sep, reason = line.partition(' -- ')
        parts = body.split()
        if not sep or not reason.strip() or len(parts) not in (2, 3):
            raise GateError(f'{ALLOW_FILE} line {number}: expected "<rule> <path glob> [sha256=<hex>] -- <reason>"')
        if parts[0] == 'allowed-address':
            if len(parts) != 2 or '@' not in parts[1]:
                raise GateError(f'{ALLOW_FILE} line {number}: expected "allowed-address <address> -- <reason>"')
            addresses.add(parts[1].lower())
            continue
        if parts[0] not in RULES:
            raise GateError(f'{ALLOW_FILE} line {number}: unknown rule')  # the line itself is not echoed
        sha = None
        if len(parts) == 3:
            if not parts[2].startswith('sha256=') or not re.fullmatch(r'[0-9a-f]{64}', parts[2][7:]):
                raise GateError(f'{ALLOW_FILE} line {number}: the optional third field is sha256=<64 hex digits>')
            sha = parts[2][7:]
        entries.append((parts[0], parts[1], sha))
    return entries, addresses


def parse_baseline(text, where=BASELINE_FILE):
    counts = collections.Counter()
    for number, raw in enumerate(text.splitlines(), 1):
        if not raw.strip() or raw.startswith('#'):
            continue
        parts = raw.split('\t')
        if len(parts) != 4 or parts[0] not in RULES or not parts[3].isdigit():
            raise GateError(f'{where} line {number}: expected "<rule>\\t<path>\\t<hash>\\t<count>"')
        counts[(parts[0], parts[1], parts[2])] += int(parts[3])
    return counts


class Config:
    def __init__(self, allow_text, baseline_text, source):
        if allow_text is None:
            raise GateError(f'{ALLOW_FILE} not found ({source}); the gate cannot run without it')
        self.allow, self.addresses = parse_allow(allow_text)
        self.baseline = parse_baseline(baseline_text) if baseline_text is not None else collections.Counter()
        self.source = source
        self.commit = None  # the trusted commit the lists came from, when they came from one

    def allowed(self, rule, path, digest=None):
        # An entry with a sha256 covers only that exact content (a pinned binary such as the banner).
        return any(r == rule and fnmatch.fnmatchcase(path, glob) and (sha is None or sha == digest)
                   for r, glob, sha in self.allow)

    def address_ok(self, address):
        """The one e-mail policy, for file content and for commit and tag metadata alike."""
        low = address.lower()
        return bool(GITHUB_NOREPLY_RE.match(low) or low in self.addresses
                    or RESERVED_DOMAIN_RE.search(low.rpartition('@')[2]) or RETINA_NAME_RE.match(low))

    def baseline_count(self, key):
        """Tracked baseline plus the private names list's own (ignored) baseline."""
        return self.baseline[key] + NAMES['baseline'][key]

    def all_baseline(self):
        merged = collections.Counter(self.baseline)
        merged.update(NAMES['baseline'])
        return merged


def config_from_commit(git, commit):
    """The lists as `commit` holds them, or None when it holds no allow-list."""
    allow = git.file_at(commit, ALLOW_FILE)
    if allow is None:
        return None
    base = git.file_at(commit, BASELINE_FILE)
    config = Config(allow.decode('utf-8'), base.decode('utf-8') if base is not None else None,
                    f'commit {commit[:12]}')
    config.commit = commit
    return config


def config_from_files(root):
    texts = []
    for name in (ALLOW_FILE, BASELINE_FILE):
        try:
            with open(os.path.join(root, name), encoding='utf-8') as f:
                texts.append(f.read())
        except OSError:
            texts.append(None)
    return Config(texts[0], texts[1], 'working tree')


def trusted_config(git, config_rev=None):
    """The lists from a commit the change being judged cannot edit: config_rev, else origin/main, whatever the
    merge base. When that commit cannot be found or holds no lists, nothing is judged: the gate refuses. To land
    a gate that main does not hold yet, the hub names the commit whose gate and lists to use (--config-from in
    a bootstrap run, docs/TOOLING.md, "Leak gate"), and the run says so."""
    commit = git.try_resolve(config_rev or TRUSTED_REF)
    if commit is None:
        raise GateError('the trusted commit cannot be found, so nothing can be judged (fetch origin, or see '
                        '"Leak gate" in docs/TOOLING.md for a bootstrap run)')
    config = config_from_commit(git, commit)
    if config is None:
        raise GateError('the trusted commit holds no gate lists, so nothing can be judged (see "Leak gate" in '
                        'docs/TOOLING.md for a bootstrap run)')
    trusted = git.try_resolve(TRUSTED_REF)
    if config_rev and commit != trusted:
        NOTES.append(f'lists from commit {commit[:12]}, named with --config-from, not from the trusted main')
    return config


# --- the private names list -------------------------------------------------------------------------------
NAMES = {'regex': None, 'status': 'private names list: not loaded', 'baseline': collections.Counter(),
         'baseline_path': None}


def load_names(paths):
    """Reads the first names list that exists among paths: one token per line, '#' comments, at least
    three characters each. Matching ignores case and needs a non-alphanumeric character on both sides. The
    private-name baseline is read from the same folder."""
    NAMES.update(regex=None, baseline=collections.Counter(), baseline_path=None)
    ANCHORS['private-name'] = (True, ())
    for path in paths:
        if not path or not os.path.isfile(path):
            continue
        with open(path, encoding='utf-8') as f:
            names = sorted({line.strip() for line in f if line.strip() and not line.startswith('#')}, key=len,
                           reverse=True)
        short = [n for n in names if len(n) < 3]
        if short:
            raise GateError(f'the private names list has {len(short)} name(s) under 3 characters')
        if names:
            NAMES['regex'] = re.compile(r'(?i)(?<![A-Za-z0-9])(?:' + '|'.join(map(re.escape, names)) +
                                        r')(?![A-Za-z0-9])')
            ANCHORS['private-name'] = (True, tuple(n.translate(ASCII_LOWER) for n in names))
        NAMES['baseline_path'] = os.path.join(os.path.dirname(os.path.abspath(path)), NAMES_BASELINE)
        if os.path.isfile(NAMES['baseline_path']):
            with open(NAMES['baseline_path'], encoding='utf-8') as f:
                NAMES['baseline'] = parse_baseline(f.read(), NAMES_BASELINE)
        where = 'OPENWILLOW_LEAK_NAMES' if path == os.environ.get('OPENWILLOW_LEAK_NAMES') else NAMES_FILE
        NAMES['status'] = f'private names list: {len(names)} name(s) from {where}'  # never the absolute path
        return
    NAMES['status'] = (f'private names list: none found ({NAMES_FILE} or OPENWILLOW_LEAK_NAMES); '
                       'private-name was not checked')


# --- scanning ---------------------------------------------------------------------------------------------
def clean_name(name):
    """A file name as Windows would open it: an alternate-stream suffix and trailing dots and spaces dropped."""
    return name.split(':', 1)[0].rstrip('. ')


def extension(path):
    base = clean_name(path.rsplit('/', 1)[-1]).lower()
    return base.rsplit('.', 1)[-1] if '.' in base.lstrip('.') else ''


def kind_of(path):
    base = path.rsplit('/', 1)[-1].lower()
    ext = extension(path)
    if base in CODE_NAMES or ext in CODE_EXT:
        return 'code'
    if ext in PROSE_EXT:
        return 'prose'
    return 'data'


class Decoded:
    """What a run of bytes is: text, binary, or undecodable text (scanned with replacement characters). The text
    of binary content cannot be read: asking for it is an internal error, so no caller can treat binary input
    as empty clean text."""
    __slots__ = ('_text', 'binary', 'undecodable')

    def __init__(self, text, binary, undecodable):
        self._text, self.binary, self.undecodable = text, binary, undecodable

    @property
    def text(self):
        if self.binary:
            raise GateError('internal: binary content was used as text')
        return self._text

    def findings(self, label, pseudo=True):
        """The findings a message, a name or other non-file text gets for its encoding."""
        if self.binary:
            return [Finding('binary', label, 0, '[binary] NUL bytes or binary content in text', None, pseudo=pseudo)]
        if self.undecodable:
            return [Finding('undecodable', label, 0, '[encoding] not UTF-8 and no UTF-16 or UTF-32 BOM', None,
                            pseudo=pseudo)]
        return []


def decode(data):
    """UTF-8 with or without a BOM and BOM-marked UTF-16 and UTF-32 are text; decoded text holding a NUL is
    binary. Other bytes with a NUL in the first 8000 are binary (git's test), and anything else that is not
    UTF-8 is undecodable: still scanned, with replacement characters, and reported."""
    try:
        text = None
        if data.startswith(b'\xef\xbb\xbf'):
            text = data[3:].decode('utf-8')
        elif data.startswith((b'\xff\xfe\x00\x00', b'\x00\x00\xfe\xff')):  # UTF-32 first: same first bytes
            text = data.decode('utf-32')
        elif data.startswith((b'\xff\xfe', b'\xfe\xff')):
            text = data.decode('utf-16')
        if text is not None:
            return Decoded(None, True, False) if '\0' in text else Decoded(text, False, False)
    except UnicodeDecodeError:
        return Decoded(data.decode('utf-8', 'replace'), False, True)
    if b'\0' in data[:8000]:
        return Decoded(None, True, False)
    try:
        text = data.decode('utf-8')
    except UnicodeDecodeError:
        return Decoded(data.decode('utf-8', 'replace'), False, True)
    if '\0' in text:
        return Decoded(None, True, False)
    return Decoded(text, False, False)


def path_rules(path):
    """Rules decided by the path alone."""
    hits = []
    low = path.lower()
    base = clean_name(low.rsplit('/', 1)[-1])
    ext = extension(path)
    dirs = [clean_name(d) for d in low.split('/')[:-1]]
    if ext in FORBIDDEN_EXT or base.endswith(FORBIDDEN_SUFFIXES) or any(d.endswith(FORBIDDEN_DIR_SUFFIXES) for d in dirs):
        hits.append(('forbidden-type', f'[file type] .{ext or base}'))
    if low.startswith(LOCAL_PREFIXES) or LOCAL_RE.match(low):
        hits.append(('local-path', '[folder] an ignored output folder'))
    elif base in PRIVATE_FILE_NAMES or any(d in PRIVATE_DIR_NAMES for d in dirs):
        hits.append(('local-path', '[private name] a private folder or file'))
    return hits


def is_round(digits):
    return sum(1 for ch in digits if ch != '0') <= 2


def anchored_lines(text, lowered, starts, rule):
    """Sorted indexes of the lines that contain one of the rule's anchors."""
    lowercase, anchors = ANCHORS[rule]
    haystack = lowered if lowercase else text
    found = set()
    for anchor in anchors:
        pos = haystack.find(anchor)
        while pos != -1:
            found.add(bisect.bisect_right(starts, pos) - 1)
            pos = haystack.find(anchor, pos + 1)
    return sorted(found)


def fenced_lines(lines):
    inside, fenced = False, set()
    for index, line in enumerate(lines):
        if FENCE_RE.match(line):
            inside = not inside
            fenced.add(index)
        elif inside:
            fenced.add(index)
    return fenced


def text_matches(text, lines, starts, prose, config, only=None):
    """Yields (rule, line index, excerpt) for the pattern rules (only: restrict to those rule ids)."""
    lowered = text.translate(ASCII_LOWER)  # str.lower() can change the length; offsets must hold

    def wanted(rule):
        return only is None or rule in only

    def matches(rule, regex):
        for index in anchored_lines(text, lowered, starts, rule):
            for m in regex.finditer(text, starts[index], starts[index] + len(lines[index])):
                yield index, m

    if wanted('decompiler-mark'):
        for index, m in matches('decompiler-mark', DECOMPILER_RE):
            yield 'decompiler-mark', index, mask('generated name', m.group(0))
    if prose and wanted('decompiler-note-mark'):
        fenced = None
        for index, m in matches('decompiler-note-mark', NOTE_MARK_RE):
            if m.group(0).startswith('this->'):
                if fenced is None:
                    fenced = fenced_lines(lines)
                if index in fenced or lines[index][:m.start() - starts[index]].count('`') % 2 == 1:
                    continue  # quoted project code in a fenced block or an inline code span
            yield 'decompiler-note-mark', index, mask('listing form', m.group(0))
    if prose and wanted('address-hex'):
        for index, m in matches('address-hex', HEX_RE):
            if ADDRESS_LOW <= int(m.group(1), 16) <= ADDRESS_HIGH and not is_round(m.group(1)):
                yield 'address-hex', index, mask('hex value', m.group(0))
    if wanted('email'):
        for index, m in matches('email', EMAIL_RE):
            address = m.group(0)
            if config.address_ok(address):
                continue
            yield 'email', index, f'[e-mail] ({len(address)} chars)'  # no characters: CI logs are public
    if wanted('user-path'):
        for index, m in matches('user-path', USER_PATH_RE):
            name = m.group(1).strip()
            if (not name or name.lower() in PLACEHOLDER_USERS or name[0] in '$%{<[(' or name.startswith('...')
                    or (name.upper() == name and '_' in name)):
                continue  # a placeholder, a variable ($env:USERNAME, %USERNAME%, <name>) or an ALL_CAPS token
            yield 'user-path', index, mask('user profile path', m.group(0))
    if wanted('account-id'):
        for index, m in matches('account-id', ACCOUNT_RE):
            yield 'account-id', index, mask('account id', m.group(0))
    if wanted('credential'):
        for index, m in matches('credential', CREDENTIAL_RE):
            yield 'credential', index, mask('credential', m.group(0))
    if NAMES['regex'] is not None and wanted('private-name'):
        for index, m in matches('private-name', NAMES['regex']):
            yield 'private-name', index, f'[private name] ({len(m.group(0))} chars)'


def fragment(line):
    """The line reduced for the blob test: quotes, commas, semicolons, brackets and '\\x' removed, and a '+'
    that joins string pieces at either end (a '+' inside base64 text stays, so line widths hold)."""
    return CONCAT_PLUS_RE.sub('', FRAGMENT_STRIP_RE.sub('', line).strip()).strip()


def fragment_class(frag):
    """'hex', 'base64' or None for one line's fragment."""
    if len(frag) < MIN_FRAGMENT:
        return None
    compact = ''.join(frag.split())
    if HEX_FRAGMENT_RE.fullmatch(compact):
        return 'hex'
    if ' ' not in frag and '\t' not in frag and BASE64_FRAGMENT_RE.fullmatch(frag) and LETTER_RE.search(frag):
        return 'base64'  # judged as a run (blob_run): one line need not hold every character class
    return None


def mixed(text):
    return bool(DIGIT_RE.search(text) and re.search(r'[A-Z]', text) and re.search(r'[a-z]', text))


def base85_line(line):
    raw = line.strip().strip('\'"`,')
    return bool(BASE85_LINE_RE.fullmatch(raw) and DIGIT_RE.search(raw) and re.search(r'[A-Z]', raw)
                and re.search(r'[a-z]', raw) and len(BASE85_PUNCT.intersection(raw)) >= 3)


def blob_run(lines, start, end):
    """Judges lines[start:end], a run of hex or base64 fragments: (rule, excerpt) or None."""
    frags = [''.join(fragment(lines[i]).split()) for i in range(start, end)]
    joined = ''.join(frags)
    count = end - start
    data_uri = start > 0 and 'base64,' in lines[start - 1]
    if data_uri and len(joined) >= JOINED_DATA_URI:
        return 'dump', f'[data URI over lines] {count} lines, {len(joined)} chars'
    if all(HEX_FRAGMENT_RE.fullmatch(f) for f in frags):
        letters = sum(1 for ch in joined if ch in 'abcdefABCDEF')
        if all(len(f) in HASH_LENGTHS for f in frags) and all(len(lines[i].split()) == 1 for i in range(start, end)):
            return ('dump-like', f'[hash-length lines] {count} lines') if count >= BLOB_RUN_LINES else None
        if len(joined) >= JOINED_HEX and letters >= 0.15 * len(joined):
            return 'dump', f'[hex text over lines] {count} lines, {len(joined)} digits'
        return None
    if len(joined) >= JOINED_BASE64 and mixed(joined):
        # Wrapped encodings have one line width (the last line may be shorter); a list of identifiers does not.
        widths = collections.Counter(len(f) for f in frags[:-1] or frags)
        same_width = widths.most_common(1)[0][1] >= 0.8 * max(1, len(frags) - 1)
        if same_width or all(mixed(f) for f in frags):
            return 'dump', f'[base64 text over lines] {count} lines, {len(joined)} chars'
    return None


def dump_matches(lines, kind, numeric_runs=True):
    """Yields (rule, line index, excerpt, line count) for dumps: numeric tables and byte rows (unless
    numeric_runs is False, as for SVG path data), hex dumps, encoded text however wrapped, long blobs and data
    URIs."""
    run_start, run_numbers, run_tokens, run_bytes, run_letters = None, 0, 0, 0, 0
    blob_start = hexdump_start = b85_start = None
    table_lines, table_numbers = TABLE_ERROR[kind]
    file_lines = file_numbers = 0  # every numeric line of the file: a table cut into chunks adds up
    file_hash = hashlib.sha256()   # and what those lines hold: a baseline entry covers this exact content only
    run_errors = 0
    for index, line in enumerate(lines + ['']):
        if numeric_runs:
            if DIGIT_RE.search(line):
                tokens = TOKEN_RE.findall(line)
                numbers = sum(1 for t in tokens if NUMBER_RE.match(t))
                numeric = bool(tokens) and numbers >= 0.9 * len(tokens)
            else:
                tokens, numbers, numeric = [], 0, False
            if numeric:
                if run_start is None:
                    run_start, run_numbers, run_tokens, run_bytes, run_letters = index, 0, 0, 0, 0
                run_numbers += numbers
                file_lines += 1
                file_numbers += numbers
                file_hash.update(normalized(line).encode('utf-8', 'replace') + b'\n')
                run_tokens += len(tokens)
                byte_tokens = [m.group(1) for m in map(BYTE_TOKEN_RE.fullmatch, tokens) if m]
                run_bytes += len(byte_tokens)
                run_letters += sum(1 for t in byte_tokens if re.search(r'[a-fA-F]', t))
                if numbers >= LINE_ERROR_NUMBERS:
                    yield 'dump', index, f'[numeric line] {numbers} numbers', 1
                elif numbers >= LINE_WARN_NUMBERS:
                    yield 'dump-like', index, f'[numeric line] {numbers} numbers', 1
            elif run_start is not None:
                count = index - run_start
                byte_rows = (run_bytes >= 0.8 * run_tokens and run_letters >= 0.1 * run_tokens)
                if count >= TABLE_WARN_LINES and run_numbers >= TABLE_WARN_NUMBERS and byte_rows:
                    run_errors += 1
                    yield 'dump', run_start, f'[byte rows] {count} lines, {run_numbers} bytes', count
                elif count >= table_lines or run_numbers >= table_numbers:
                    run_errors += 1
                    yield 'dump', run_start, f'[numeric table] {count} lines, {run_numbers} numbers', count
                elif (count >= TABLE_WARN_LINES and run_numbers >= TABLE_WARN_NUMBERS) or count >= TABLE_WARN_COLUMN:
                    yield 'dump-like', run_start, f'[numeric table] {count} lines, {run_numbers} numbers', count
                run_start = None
        # Hex dump rows with offsets, and base85 lines: runs of whole lines.
        if HEXDUMP_LINE_RE.match(line):
            hexdump_start = index if hexdump_start is None else hexdump_start
        else:
            if hexdump_start is not None and index - hexdump_start >= BLOB_RUN_LINES:
                yield 'dump', hexdump_start, f'[hex dump] {index - hexdump_start} lines', index - hexdump_start
            hexdump_start = None
        if base85_line(line):
            b85_start = index if b85_start is None else b85_start
        else:
            if b85_start is not None and index - b85_start >= BLOB_RUN_LINES:
                yield 'dump', b85_start, f'[base85 text] {index - b85_start} lines', index - b85_start
            b85_start = None
        # Hex and base64 fragments, joined over consecutive lines.
        if fragment_class(fragment(line)) is not None and not HEXDUMP_LINE_RE.match(line):
            blob_start = index if blob_start is None else blob_start
        else:
            if blob_start is not None:
                hit = blob_run(lines, blob_start, index)
                if hit:
                    yield hit[0], blob_start, hit[1], index - blob_start
            blob_start = None
        if len(line) >= 64:
            m = DATA_URI_RE.search(line)
            if m:
                yield 'dump', index, f'[data URI] {len(m.group(0))} chars', 1
        if len(line) >= 128:
            for regex, label in ((HEX_BLOB_RE, 'hex blob'), (BASE64_BLOB_RE, 'base64 blob'),
                                 (ESCAPED_BYTES_RE, 'escaped bytes')):
                m = regex.search(line)
                if m and (regex is not BASE64_BLOB_RE or (DIGIT_RE.search(m.group(0))
                                                          and LETTER_RE.search(m.group(0)))):
                    yield 'dump', index, f'[{label}] {len(m.group(0))} chars', 1
    if numeric_runs and not run_errors and (file_lines >= table_lines or file_numbers >= table_numbers):
        # A file-level finding (index None), keyed on a hash of the numeric lines: changing any value, adding or
        # removing a row makes the baseline entry stale and the finding live again.
        yield 'dump', None, f'[numeric lines in the file] {file_lines} lines, {file_numbers} numbers', \
            f'sum:{file_hash.hexdigest()}'


def split_lines(text):
    lines = text.split('\n')
    starts, offset = [], 0
    for line in lines:
        starts.append(offset)
        offset += len(line) + 1
    return lines, starts


def scan_text(path, text, kind, config, only=None):
    """Yields (rule, first line, excerpt, line text, last line) for the text rules. The line text only feeds
    the baseline hash; it is never printed."""
    lines, starts = split_lines(text)
    for rule, index, excerpt in text_matches(text, lines, starts, kind == 'prose', config, only):
        yield rule, index + 1, excerpt, lines[index], index + 1
    if only is not None:
        return
    # SVG path data is numbers by nature: only the numeric-run checks are skipped there, not the blob checks.
    for rule, index, excerpt, count in dump_matches(lines, kind, numeric_runs=not path.lower().endswith('.svg')):
        if index is None:  # the per-file sum: a file-level finding
            yield rule, 0, excerpt, count, 0
            continue
        yield rule, index + 1, excerpt, f'{excerpt}:' + lines[index], index + count
    if '[' in text:
        sections = [i for i, line in enumerate(lines) if GAME_INI_SECTION_RE.match(line)]
        if sections and (extension(path) == 'ini' or len(sections) >= 2):
            for index in sections:
                yield 'game-config', index + 1, f'[section] one of {len(sections)} game-form section(s)', \
                    lines[index], index + 1


def ident_findings(label, text, config, commit=None, pseudo=True):
    """The identifier rules over a name field (a file name, a ref, an author, committer or tagger name)."""
    return [Finding(rule, label, 0, excerpt, None, None, commit, pseudo)
            for rule, _, excerpt, _, _ in scan_text(label, text, 'data', config, only=IDENT_RULES)]


NAME_MAX = 160  # characters: an author, committer or tagger name or a ref name longer than this is refused


def name_findings(label, text, config, commit=None):
    """Every content rule over a name field (an author, committer or tagger name, a ref name): the identifier
    rules, decompiler marks, blob shapes, and a length limit."""
    found = [Finding(rule, label, 0, excerpt, None, None, commit, True)
             for rule, _, excerpt, _, _ in scan_text(label, text, 'prose', config)]
    if len(text) > NAME_MAX:
        found.append(Finding('object-format', label, 0, f'[name] {len(text)} characters, more than {NAME_MAX}', None,
                             None, commit, True))
    return found


def scan_file(path, data, config, new_file=True):
    """All findings for one file's content (no baseline, no added-line filter): (findings, decoded text)."""
    findings = []
    digest = hashlib.sha256(data).hexdigest()

    def add(rule, line, excerpt, key_text, last=None):
        if config.allowed(rule, path, digest):
            return
        findings.append(Finding(rule, path, line, excerpt, line_key(rule, path, key_text), last or line))

    blob_id = 'blob:' + hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()
    name_hits = ident_findings(path, path, config, pseudo=False)
    if name_hits:
        SENSITIVE_PATHS.add(path)
        for f in name_hits:
            add(f.rule, 0, '[file name] ' + f.excerpt, 'name:' + path)
    for rule, excerpt in path_rules(path):
        add(rule, 0, excerpt, blob_id)
    ext = extension(path)
    if new_file and ext not in FORBIDDEN_EXT and not config.allowed('unknown-type', path, digest):
        findings.append(Finding('unknown-type', path, 0, f'[file type] .{ext}' if ext else '[file type] no extension',
                                line_key('unknown-type', path, blob_id)))
    if data.startswith(LFS_POINTER):
        add('lfs-pointer', 0, '[lfs] pointer file', blob_id)
    decoded = decode(data)
    binary, undecodable = decoded.binary, decoded.undecodable
    text = None if binary else decoded.text
    if binary:
        add('binary', 0, f'[binary] {len(data)} bytes', blob_id)
    if undecodable:
        add('undecodable', 0, f'[encoding] {len(data)} bytes, not UTF-8 and no UTF-16 BOM', blob_id)
    if len(data) > SIZE_LIMIT:
        add('oversize', 0, f'[size] {len(data)} bytes, limit {SIZE_LIMIT}', blob_id)
    if text is not None:
        text = text.replace('\r\n', '\n')
        for rule, line, excerpt, key_text, last in scan_text(path, text, kind_of(path), config):
            add(rule, line, excerpt, key_text, last)
    return findings, text


def gitlink_finding(path, sha, config):
    if config.allowed('gitlink', path):
        return []
    return [Finding('gitlink', path, 0, '[submodule] a gitlink entry', line_key('gitlink', path, sha))]


def scan_message(label, text, config):
    findings = []
    for rule, line, excerpt, key_text, last in scan_text(label, text.replace('\r\n', '\n'), 'prose', config):
        findings.append(Finding(rule, label, line, excerpt, None, last, pseudo=True))
    return findings


# --- local/ copies ----------------------------------------------------------------------------------------
def local_roots(git, root):
    roots = [os.path.join(root, 'local')]
    try:
        common = git.out('rev-parse', '--path-format=absolute', '--git-common-dir').strip()
        shared = os.path.join(os.path.dirname(common), 'local')
        if os.path.normcase(os.path.abspath(shared)) != os.path.normcase(os.path.abspath(roots[0])):
            roots.append(shared)
    except GateError:
        INCOMPLETE.append('the shared local/ folder could not be located')
    return [r for r in roots if os.path.isdir(r)]


def local_copies(git, root, files):
    """Warn for files byte-identical to a file under local/. files: {path: bytes}. Skips files under 64
    bytes and local copies that mirror the tracked path (a checkout or export of this repository)."""
    wanted = collections.defaultdict(list)
    for path, data in files.items():
        if data is not None and len(data) >= 64:
            wanted[len(data)].append(path)
    hits = []
    if not wanted:
        return hits

    def walk(top):
        """(path, size) of every file below top, skipping git checkouts; scandir gives sizes cheaply, and an
        explicit stack avoids nested generators over a deep tree."""
        stack = [top]
        while stack:
            folder = stack.pop()
            try:
                entries = list(os.scandir(folder))
            except OSError:
                INCOMPLETE.append('a folder under local/ could not be listed')
                continue
            if any(e.name == '.git' for e in entries):
                continue  # a checkout of this repository kept under local/
            for e in entries:
                try:
                    if e.is_dir(follow_symlinks=False):
                        stack.append(e.path)
                    elif e.is_file(follow_symlinks=False):
                        yield e.path, e.stat(follow_symlinks=False).st_size
                except OSError:
                    INCOMPLETE.append('an entry under local/ could not be read')
                    continue

    digests = {}
    for top in local_roots(git, root):
        for full, size in walk(top):
            if size not in wanted:
                continue
            try:
                with open(full, 'rb') as f:
                    digest = hashlib.sha256(f.read()).digest()
            except OSError:
                INCOMPLETE.append('a file under local/ could not be read')
                continue
            rel_local = os.path.relpath(full, top).replace(os.sep, '/')
            for path in wanted[size]:
                if path not in digests:
                    digests[path] = hashlib.sha256(files[path]).digest()
                if digests[path] == digest and not rel_local.endswith('/' + path) and rel_local != path:
                    hits.append(Finding('local-copy', path, 0, '[copy] same bytes as a file under local/',
                                        line_key('local-copy', path, 'copy')))
    seen, unique = set(), []
    for hit in hits:
        if hit.path not in seen:
            seen.add(hit.path)
            unique.append(hit)
    return unique


# --- diffs ------------------------------------------------------------------------------------------------
def raw_diff(git, *diff_args):
    """[(old mode, new mode, old id, new id, status, path)] from `git diff --raw -z`: NUL-separated, paths
    byte-exact, no rename pairing, submodules always listed. Git attributes and diff drivers play no part:
    the added lines are worked out from the two blobs (added_line_numbers)."""
    out = git.run('-c', 'diff.ignoreSubmodules=none', 'diff', '--raw', '-z', '--no-abbrev', '--no-renames',
                  '--ignore-submodules=none', '--no-ext-diff', *diff_args).stdout.decode('utf-8', 'replace')
    parts = out.split('\0')
    if parts and parts[-1] == '':
        parts.pop()
    entries, i = [], 0
    while i < len(parts):
        meta = parts[i]
        if not meta.startswith(':') or i + 1 >= len(parts):
            raise GateError('internal: a git diff record the gate cannot parse')
        fields = meta[1:].split()
        if len(fields) != 5:
            raise GateError('internal: a git diff record the gate cannot parse')
        entries.append((fields[0], fields[1], fields[2], fields[3], fields[4][0], parts[i + 1]))
        i += 2
    return entries


def normalized(line):
    return ' '.join(line.split())


def text_of(data):
    """An earlier version's text for the added-line comparison; None (every line new) when it is binary."""
    decoded = decode(data)
    return None if decoded.binary else decoded.text.replace('\r\n', '\n')


def tree_map(git, commit):
    """{path: blob id} of commit's tree, through `ls-tree -r -z`."""
    result = {}
    for mode, kind, sha, path in tree_entries(git, commit):
        if kind == 'blob':
            result[path] = sha
    return result


def added_line_numbers(old_text, new_text, other_texts=()):
    """Line numbers (1-based) of new_text whose content (whitespace-normalised) occurs more often than in
    old_text: new content, and every extra copy of an existing line. Moving a line is not adding it. Every
    occurrence of such content is marked, so a finding on any copy is judged."""
    new_lines = new_text.split('\n')
    if old_text is None and not other_texts:
        return set(range(1, len(new_lines) + 1))
    old_text = old_text if old_text is not None else ''
    old_counts = collections.Counter(normalized(line) for line in old_text.split('\n'))
    for other in other_texts:  # a merge: the most copies any parent holds
        old_counts |= collections.Counter(normalized(line) for line in other.split('\n'))
    new_norm = [normalized(line) for line in new_lines]
    new_counts = collections.Counter(new_norm)
    grown = {c for c, n in new_counts.items() if n > old_counts[c]}
    return {i + 1 for i, c in enumerate(new_norm) if c in grown}


def judge(findings, added, config, new_file=False):
    """Keeps line findings that touch an added line (added=None: every line is new) and file-level findings
    unless the baseline covers them. An added line is never baselined: its content occurs more often than
    before, so the baseline's count is a ceiling, never a budget. A file new in this commit (added, or removed
    and added again) gets no baseline either. Returns (kept, baselined)."""
    counts = collections.Counter(f.key for f in findings if f.key and not f.line)
    kept, baselined = [], 0
    for f in findings:
        if f.line:
            if added is None or any(n in added for n in range(f.line, (f.last or f.line) + 1)):
                kept.append(f)
            continue
        if not new_file and f.key and counts[f.key] <= config.baseline_count((f.rule, f.path, f.key)):
            baselined += 1
            continue
        kept.append(f)
    return kept, baselined


def parse_identity(raw):
    """(kind, name, address) from an author, committer or tagger header line."""
    m = IDENT_LINE_RE.match(raw)
    if not m:
        raise GateError('internal: an author, committer or tagger line the gate cannot parse')
    return m.group(1), m.group(2), m.group(3)


def parse_object(data):
    """(headers [(key, value)], message) of a raw commit or tag object."""
    head, sep, message = data.partition(b'\n\n')
    headers = []
    for raw in head.split(b'\n'):
        if raw.startswith(b' ') and headers:
            key, value = headers[-1]
            headers[-1] = (key, value + b'\n' + raw[1:])  # a continuation line (a signature, a merged tag)
            continue
        key, _, value = raw.partition(b' ')
        headers.append((key.decode('ascii', 'replace'), value))
    return headers, message


COMMIT_HEADERS = {'tree', 'parent', 'author', 'committer', 'encoding', 'gpgsig', 'gpgsig-sha256', 'mergetag'}
TAG_HEADERS = {'object', 'type', 'tag', 'tagger', 'encoding', 'gpgsig', 'gpgsig-sha256'}
OBJECT_ID_RE = re.compile(r'[0-9a-f]{40}(?:[0-9a-f]{24})?')
ARMOUR_RE = re.compile(r'-----BEGIN (PGP SIGNATURE|SSH SIGNATURE|SIGNED MESSAGE)-----\n(.*?)\n-----END \1-----\n?',
                       re.S)
ARMOUR_MAX_LINES, ARMOUR_MAX_WIDTH = 120, 80
ARMOUR_BODY_RE = re.compile(r'[A-Za-z0-9+/]+={0,2}|=[A-Za-z0-9+/]{4}')
ARMOUR_HEADER_RE = re.compile(r'(?:Version|Comment|Hash|Charset): .*')


def armour_ok(block, label, config, findings):
    """A signature block: base64 lines, an optional checksum line, optional armour headers (whose values get the
    prose rules). It is never scanned as content (it is base64 by nature); a block of any other shape is
    refused, since a credential could hide in it."""
    m = ARMOUR_RE.fullmatch(block.strip('\n') + '\n')
    if not m:
        return False
    body = m.group(2).split('\n')
    # Bounded to what a real signature is: an RSA-4096 PGP signature is about 14 lines of 64 characters, an SSH
    # one under 20 lines of 70, an S/MIME one a few dozen; 120 lines of at most 80 characters is a generous cap.
    if len(body) > ARMOUR_MAX_LINES or any(len(line) > ARMOUR_MAX_WIDTH for line in body):
        return False
    for line in body:
        if line and ARMOUR_BODY_RE.fullmatch(line):
            # Base64 by nature, so not a dump; but a recognisable credential or identifier is still refused.
            findings.extend(ident_findings(label, line, config))
            continue
        if not line:
            continue
        if ARMOUR_HEADER_RE.fullmatch(line):
            findings.extend(scan_message(f'{label}:signature-header', line, config))
            continue
        return False
    return True


def identity_and_message(git, oid, data, label, config, kind='commit'):
    """Every header of a commit or tag object, and its message: identities get the identifier rules and the
    one e-mail policy, names and messages the prose rules, object ids and types their exact shape, signatures
    their armour shape. A header the gate does not know is refused (object-format)."""
    findings = []
    known = COMMIT_HEADERS if kind == 'commit' else TAG_HEADERS

    def refuse(what):
        findings.append(Finding('object-format', label, 0, f'[{kind} object] {what}', None, pseudo=True))

    headers, message = parse_object(data)
    for key, value in headers:
        decoded = decode(value)
        if decoded.binary or decoded.undecodable:
            findings.extend(decoded.findings(f'{label}:{key}' if key in known else label))
            if decoded.binary:
                continue
        text = decoded.text
        if key not in known:
            refuse('a header the gate does not know')
            findings.extend(scan_message(f'{label}:unknown-header', text, config))
        elif key in ('tree', 'parent', 'object'):
            if not OBJECT_ID_RE.fullmatch(text):
                refuse(f'a {key} header that is not an object id')
        elif key == 'type':
            if text not in ('commit', 'tree', 'blob', 'tag'):
                refuse('a type header the gate does not know')
        elif key in ('author', 'committer', 'tagger'):
            m = IDENT_LINE_RE.match(f'{key} {text}')
            if not m:
                refuse(f'an {key} line the gate cannot parse')
                findings.extend(scan_message(f'{label}:{key}', text, config))
                continue
            _, name, address = m.group(1), m.group(2), m.group(3)
            if not config.address_ok(address):
                findings.append(Finding('email', f'{label}:{key}', 0, f'[{key} address] not an approved form', None,
                                        pseudo=True))
            findings.extend(name_findings(f'{label}:{key}-name', name, config))
        elif key in ('gpgsig', 'gpgsig-sha256'):
            if not armour_ok(text, f'{label}:{key}', config, findings):
                refuse('a signature header of the wrong shape')
        elif key == 'mergetag':  # the tag a merge commit carries: every part of it is judged too
            findings.extend(identity_and_message(git, oid, value, f'{label}:mergetag', config, kind='tag'))
        else:  # tag names and encodings: short text, judged as prose
            findings.extend(scan_message(f'{label}:{key}', text, config))
    decoded = decode(message)
    findings.extend(decoded.findings(label))
    if decoded.binary:
        return findings
    text = decoded.text
    if kind == 'tag':  # a signed tag carries its signature at the end of the message
        start = max(text.rfind('-----BEGIN PGP SIGNATURE-----'), text.rfind('-----BEGIN SSH SIGNATURE-----'),
                    text.rfind('-----BEGIN SIGNED MESSAGE-----'))
        if start >= 0 and (start == 0 or text[start - 1] == '\n'):
            if not armour_ok(text[start:], f'{label}:signature', config, findings):
                refuse('a signature block of the wrong shape')
            text = text[:start]
    findings.extend(scan_message(label, text, config))
    return findings


class Scan:
    """One judgement (a range, a ref update or the staged index), without repeated findings."""

    def __init__(self, git, root, config):
        self.git, self.root, self.config = git, root, config
        self.findings, self.baselined, self.seen = [], 0, set()
        self.scanned_blobs, self.paths, self.deleted, self.tip_files = set(), set(), set(), {}
        # The destination's trusted ref: its history is judged already. CI on a push to main turns this off
        # (--no-trusted-exclude): there the ref already sits at the pushed head, and the range's base bounds it.
        self.exclude = None if CURRENT.get('no_trusted_exclude') else git.try_resolve(TRUSTED_REF)
        self.whole_history = False

    def add(self, findings, commit=None):
        for f in findings:
            ident = (f.rule, f.path, f.key or (f.line, f.excerpt))
            if ident in self.seen:
                continue
            self.seen.add(ident)
            self.findings.append(f._replace(commit=commit) if commit else f)

    def diff(self, diff_args, commit=None, other_parents=()):
        """Judges one diff (a commit against its first parent, or the index against HEAD). For a merge, content
        that one of the other parents already holds is not added by the merge: a file identical to another
        parent's is skipped, and a line counts as added only when it occurs more often than in every parent."""
        entries = raw_diff(self.git, *diff_args)
        for e in entries:  # deletions count for gate-change before they are set aside
            (self.deleted if e[4] == 'D' else self.paths).add(e[5])
        entries = [e for e in entries if e[4] != 'D']
        if not entries:
            return
        others = [tree_map(self.git, parent) for parent in other_parents]
        ids = [e[3] for e in entries if e[1] != '160000']
        ids += [e[2] for e in entries if e[2] != ZERO_SHA and e[0] not in ('160000', '000000')]
        ids += [m[e[5]] for m in others for e in entries if e[5] in m]
        blobs = self.git.blobs(ids)
        for old_mode, new_mode, old_id, new_id, status, path in entries:
            self.paths.add(path)
            if new_mode == '160000':
                hits = ident_findings(path, path, self.config, pseudo=False)
                if hits:
                    SENSITIVE_PATHS.add(path)
                self.add(gitlink_finding(path, new_id, self.config) + hits, commit)
                continue
            data = blobs[new_id]
            self.scanned_blobs.add(new_id)
            self.tip_files[path] = data
            other_ids = [m[path] for m in others if path in m]
            if new_id in other_ids:
                continue  # the merge took this file as another parent has it: nothing new
            full, text = scan_file(path, data, self.config, new_file=status == 'A' and not other_ids)
            old_text = text_of(blobs[old_id]) if old_id != ZERO_SHA and old_id in blobs else None
            other_texts = [text_of(blobs[i]) for i in other_ids]
            added = added_line_numbers(old_text, text, [t for t in other_texts if t is not None]) \
                if text is not None else set()
            kept, baselined = judge(full, added, self.config, new_file=status == 'A' and not other_ids)
            self.baselined += baselined
            self.add(kept, commit)

    def stray_blobs(self, spec, commits):
        """Every rule on each new blob reachable in the range that no first-parent diff showed (a blob that
        reached a merge from its other side, for instance). Names come from `ls-tree -z` of the range's commits."""
        out = self.git.out('rev-list', '--objects', '--no-object-names', *spec)
        ids = [line.strip() for line in out.split('\n') if line.strip() and line.strip() not in self.scanned_blobs]
        if not ids:
            return
        check = self.git.run('cat-file', '--batch-check', data=''.join(i + '\n' for i in ids).encode('ascii'))
        kinds = dict(row.split()[:2] for row in check.stdout.decode('ascii', 'replace').splitlines() if row.strip())
        strays = {i for i in ids if kinds.get(i) == 'blob'}
        if not strays:
            return
        names = {}
        for sha, _, _ in commits:
            listing = self.git.run('ls-tree', '-r', '-z', sha).stdout.decode('utf-8', 'replace')
            for entry in listing.split('\0'):
                if entry:
                    meta, name = entry.split('\t', 1)
                    oid = meta.split()[2]
                    if oid in strays:
                        names.setdefault(oid, name)
        blobs = self.git.blobs(sorted(strays))
        for oid in sorted(strays):
            self.scanned_blobs.add(oid)
            full, _ = scan_file(names.get(oid, f'(object {oid[:12]})'), blobs[oid], self.config, new_file=False)
            kept, baselined = judge(full, None, self.config, new_file=True)  # a new blob: every line is new
            self.baselined += baselined
            self.add(kept)

    def gate_change(self):
        def gate_file(p):
            return p in GATE_PATHS or p.startswith(GATE_PREFIXES) or p.rsplit('/', 1)[-1] in GATE_NAMES

        def show(p):
            return p if p not in SENSITIVE_PATHS and not ident_findings(p, p, self.config) else '(a masked path)'
        deleted = sorted(p for p in self.deleted if gate_file(p))
        changed = sorted(p for p in self.paths - self.deleted if gate_file(p))
        if deleted:
            self.findings.append(Finding('gate-change', '(range)', 0, '[DELETES an enforcement file; needs the '
                                         'maintainer\'s yes before landing] ' + ', '.join(map(show, deleted)), None,
                                         pseudo=True))
        if changed:
            self.findings.append(Finding('gate-change', '(range)', 0, '[needs the maintainer\'s yes before landing] '
                                         + ', '.join(map(show, changed)), None, pseudo=True))


def commit_list(git, base, head, trusted=None, whole_history=False):
    """[(id, first parent, other parents)] oldest first: the commits head reaches and neither base nor the
    destination's trusted ref does (so a branch that merged main never re-judges main's history). Nothing else
    is excluded: a graph that no trusted ref or base reaches is judged whole."""
    exclude = [] if whole_history else [c for c in (base, trusted) if c]
    spec = [head, '--not'] + exclude if exclude else [head]
    out = git.out('rev-list', '--reverse', '--parents', *spec)
    commits = []
    for line in out.split('\n'):
        parts = line.split()
        if parts:
            commits.append((parts[0], parts[1] if len(parts) > 1 else None, parts[2:]))
    return commits, spec


def check_graph(git, commits, raw):
    """A shallow clone or a grafts file can hide commits (and what they added and removed). Then every commit of
    the outgoing range must have its recorded parents present and listed as recorded; otherwise nothing is
    judged."""
    shallow = git.out('rev-parse', '--is-shallow-repository').strip() == 'true'
    grafts = os.path.exists(git.out('rev-parse', '--path-format=absolute', '--git-path', 'info/grafts').strip())
    if not (shallow or grafts):
        return
    what = 'shallow' if shallow else 'has a grafts file (.git/info/grafts)'
    for sha, parent, others in commits:
        headers, _ = parse_object(raw[sha][1])
        recorded = [v.decode('ascii', 'replace') for k, v in headers if k == 'parent']
        listed = ([parent] if parent else []) + list(others)
        if recorded != listed or not all(git.exists(f'{p}^{{commit}}') for p in recorded):
            raise GateError(f'the repository is {what}, and commit {sha[:12]} has a recorded parent that is missing '
                            'or listed differently: the outgoing history is incomplete, so nothing was judged')


def scan_range(scan, base, head, check_local=True):
    """Every commit in base..head judged against its first parent (added lines), every new blob in the range,
    every commit's message, author and committer."""
    git = scan.git
    head = git.resolve(head)
    base = git.resolve(base) if base else None
    commits, spec = commit_list(git, base, head, scan.exclude, scan.whole_history)
    raw = git.objects([sha for sha, _, _ in commits])
    check_graph(git, commits, raw)
    for sha, parent, others in commits:
        scan.diff([parent or git.empty_tree(), sha], commit=sha, other_parents=others)
        scan.add(identity_and_message(git, sha, raw[sha][1], f'commit:{sha[:12]}', scan.config))
    if commits:
        scan.stray_blobs(spec, commits)
    if check_local:
        scan.add(f for f in local_copies(git, scan.root, scan.tip_files)
                 if not scan.config.allowed(f.rule, f.path))
    return len(commits)


def run_staged(scan):
    head = scan.git.try_resolve('HEAD')
    scan.diff(['--cached', head or scan.git.empty_tree()])
    scan.add(f for f in local_copies(scan.git, scan.root, scan.tip_files) if not scan.config.allowed(f.rule, f.path))


def tree_entries(git, commit):
    out = git.run('ls-tree', '-r', '-z', commit).stdout.decode('utf-8', 'replace')
    entries = []
    for entry in out.split('\0'):
        if entry:
            meta, path = entry.split('\t', 1)
            mode, kind, sha = meta.split()
            entries.append((mode, kind, sha, path))
    return entries


def run_all(git, commit, config):
    """The whole tree at commit (files read by object id)."""
    head = git.resolve(commit)
    entries = tree_entries(git, head)
    findings = []
    blobs = git.blobs([sha for _, kind, sha, _ in entries if kind == 'blob'])
    for mode, kind, sha, path in entries:
        if kind == 'commit':
            findings.extend(gitlink_finding(path, sha, config))
        elif kind == 'blob':
            findings.extend(scan_file(path, blobs[sha], config, new_file=True)[0])
    return findings, len(entries)


def apply_baseline(findings, config, report_stale=False):
    remaining = config.all_baseline()
    kept, baselined = [], 0
    for f in findings:
        k = (f.rule, f.path, f.key) if f.key else None
        if k and remaining[k] > 0:
            remaining[k] -= 1
            baselined += 1
            continue
        kept.append(f)
    if report_stale:
        for (rule, path, key), count in sorted(remaining.items()):
            if count > 0 and (rule != 'private-name' or NAMES['regex'] is not None):
                kept.append(Finding('stale-baseline', path, 0, f'[{rule}] {count} entry(ies) no longer match', None))
    return kept, baselined


def peel(git, oid, label, config, findings):
    """Follows annotated tags down to a commit, judging each tag object's tagger and message on the way."""
    for _ in range(16):
        kind, data = git.objects([oid])[oid]
        if kind == 'commit':
            return oid
        if kind != 'tag':
            raise GateError(f'a pushed ref points at a {kind}; the gate judges commits and tags only')
        findings.extend(identity_and_message(git, oid, data, f'{label}:tag-{oid[:12]}', config, kind='tag'))
        headers, _ = parse_object(data)
        targets = [v.decode('ascii', 'replace') for k, v in headers if k == 'object']
        if len(targets) != 1:
            raise GateError('internal: a tag object the gate cannot parse')
        oid = targets[0]
    raise GateError('internal: a tag chain deeper than 16')


def run_pre_push(git, root, config, remote, stdin_text, check_local=True):
    """Each ref update judged on its own, all with the one trusted config; the push fails if any fails."""
    results, scanned = [], 0
    for raw in stdin_text.splitlines():
        if not raw.strip():
            continue
        parts = raw.split(' ')
        if (len(parts) != 4 or not all(OBJECT_ID_RE.fullmatch(p) for p in parts[1::2])
                or not re.fullmatch(r'\S+', parts[0]) or not re.fullmatch(r'refs/\S+', parts[2])):
            raise GateError('internal: a pre-push input line the gate cannot parse')
        # The local source arrives as typed (refs/heads/x, HEAD, a raw id, (delete)); only its object id is used.
        local_ref, local_sha, remote_ref, remote_sha = parts
        if local_sha == ZERO_SHA:
            continue  # a branch deletion pushes no content
        scan = Scan(git, root, config)
        scan.add(name_findings('ref-name', local_ref, config) + name_findings('ref-name', remote_ref, config))
        tag_findings = []
        local_commit = peel(git, local_sha, 'ref', config, tag_findings)
        scan.add(tag_findings)
        # The destination's old value for this ref, when this clone has it; the trusted ref is excluded as well.
        base = git.try_resolve(remote_sha) if remote_sha != ZERO_SHA and git.exists(remote_sha) else None
        scanned += scan_range(scan, base, local_commit, check_local)
        scan.gate_change()
        results.append(scan)
    return results, scanned


# --- output -----------------------------------------------------------------------------------------------
def shown_path(f, show_paths):
    if show_paths:
        return f.path
    # Masking comes before anything else, labels included: an identifier or a control character inside a path or
    # a label is never printed.
    if (f.path in SENSITIVE_PATHS or any(ord(ch) < 32 or 127 <= ord(ch) < 160 for ch in f.path)
            or ident_findings(f.path, f.path, CURRENT['config'])):
        return f'[path masked, {len(f.path)} chars]'
    if f.pseudo:
        return f.path
    if f.rule in PATH_RULES:
        return mask('path', f.path)
    return f.path


def report(findings, baselined, scanned, unit='file(s)', show_paths=False, out=sys.stdout):
    unique, seen = [], set()
    for f in findings:
        ident = (f.rule, f.path, f.line, f.excerpt, f.commit)
        if ident not in seen:
            seen.add(ident)
            unique.append(f)
    errors = [f for f in unique if RULES[f.rule][0] == ERROR]
    warnings = [f for f in unique if RULES[f.rule][0] == WARN]
    for note in NOTES:
        print(f'WARN gate-config {note}', file=out)
    for what in sorted(set(INCOMPLETE)):
        unique.append(Finding('inspection-incomplete', '(local/ copy check)', 0, f'[incomplete] {what}', None,
                              pseudo=True))
    warnings = [f for f in unique if RULES[f.rule][0] == WARN]
    for f in sorted(unique, key=lambda f: (RULES[f.rule][0] != ERROR, f.path, f.line, f.rule)):
        path = shown_path(f, show_paths)
        where = f'{path}:{f.line}' if f.line else path
        commit = f' @{f.commit[:12]}' if f.commit else ''
        print(f'{RULES[f.rule][0]} {f.rule} {where}{commit} {f.excerpt}', file=out)
    warned = sorted({f.rule for f in warnings})
    print(f'leak_gate: {len(errors)} error(s), {len(warnings)} warning(s)'
          + (f' ({", ".join(warned)})' if warned else '') + f', {baselined} baselined, '
          f'{scanned} {unit} scanned; {NAMES["status"]}', file=out)
    if errors:
        rules = sorted({f.rule for f in errors})
        print('leak_gate: refused. ' + '; '.join(f'{r}: {RULES[r][1]}' for r in rules) + '.', file=out)
        print('leak_gate: fix the content (move game-derived output under ignored local/, rewrite in your own '
              'words), or ask the maintainer for an entry in tools/leak_gate_allow.txt. See docs/TOOLING.md, '
              '"Leak gate".', file=out)
        return 1
    blocking = [r for r in warned if r != 'stale-baseline']
    if CURRENT.get('strict') and blocking:
        print('leak_gate: refused under --strict: ' + ', '.join(blocking) + ' (exit 3).', file=out)
        return 3
    return 0


def write_baseline(findings, path, source):
    """The tracked baseline gets every rule but private-name; private-name entries go to the ignored file
    next to the names list, so the public tree never maps where a private token sits."""
    header = ['# Leak gate baseline: findings that were already on main when the gate arrived.',
              '# One line per rule, path and hash of the flagged line (never the line itself), with a count.',
              f'# Written by tools/leak_gate.py --write-baseline from {source}. Only findings not listed here fail.',
              '# Remove a line when its content is fixed; never add one by hand to let a new finding through.']
    counts = collections.Counter((f.rule, f.path, f.key) for f in findings if f.key)
    public = {k: v for k, v in counts.items() if k[0] != 'private-name'}
    private = {k: v for k, v in counts.items() if k[0] == 'private-name'}
    targets = [(path, public)]
    if private:
        if not NAMES['baseline_path']:
            raise GateError('private-name findings but no names list folder to hold their baseline')
        targets.append((NAMES['baseline_path'], private))
    for target, entries in targets:
        lines = list(header) + [f'{r}\t{p}\t{k}\t{c}' for (r, p, k), c in sorted(entries.items())]
        with open(target, 'w', encoding='utf-8', newline='\n') as f:
            f.write('\n'.join(lines) + '\n')
    return len(public), len(private)


def install_hook(git, root):
    source = os.path.join(root, '.githooks', 'pre-push')
    hooks = git.out('rev-parse', '--path-format=absolute', '--git-path', 'hooks').strip()
    os.makedirs(hooks, exist_ok=True)
    target = os.path.join(hooks, 'pre-push')
    if os.path.exists(target):
        with open(target, encoding='utf-8', errors='replace') as f:
            if HOOK_MARKER not in f.read():
                raise GateError('an existing pre-push hook was left untouched')
    with open(source, 'rb') as f:
        data = f.read()
    with open(target, 'wb') as f:
        f.write(data.replace(b'\r\n', b'\n'))  # a CRLF checkout must not break sh
    os.chmod(target, 0o755)
    print('installed the pre-push hook in this clone\'s hooks folder')


class QuietParser(argparse.ArgumentParser):
    """Usage errors never echo an argument back (a URL or a label can carry a secret)."""

    def error(self, message):
        self.print_usage(sys.stderr)
        print('leak_gate: error: the arguments could not be used (see --help)', file=sys.stderr)
        sys.exit(2)


def main(argv=None):
    parser = QuietParser(description=__doc__.split('\n\n')[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--range', metavar='BASE..HEAD', help='every commit in the range, its added lines and blobs')
    mode.add_argument('--staged', action='store_true', help='the staged changes')
    mode.add_argument('--all', metavar='COMMIT', nargs='?', const='HEAD', help='the whole tree at COMMIT')
    mode.add_argument('--text', metavar='FILE', help='free text from FILE, or - for stdin (a PR description)')
    mode.add_argument('--pre-push', metavar='REMOTE', help='git pre-push hook mode: reads ref lines on stdin')
    mode.add_argument('--install-hook', action='store_true', help='copy .githooks/pre-push into this clone')
    parser.add_argument('--label', default=None, help='name shown for --text findings')
    parser.add_argument('--config-from', metavar='REV', default=None,
                        help=f'read the allow-list and baseline from REV (default: {TRUSTED_REF}; with --all: COMMIT)')
    parser.add_argument('--worktree-config', action='store_true',
                        help='with --all only: read the allow-list and baseline from the working tree')
    parser.add_argument('--whole-history', action='store_true',
                        help='with --range: judge every commit HEAD reaches (a first push to an empty remote)')
    parser.add_argument('--write-baseline', metavar='FILE', help='with --all: write every finding to FILE')
    parser.add_argument('--strict', action='store_true',
                        help='exit 3 when there is no error but any warning other than stale-baseline (the publish path)')
    parser.add_argument('--no-trusted-exclude', action='store_true',
                        help='with --range: do not exclude what the trusted ref reaches (CI on a push to main)')
    parser.add_argument('--no-local', action='store_true', help='skip the local/ copy check')
    parser.add_argument('--show-paths', action='store_true', help='print masked paths in full (local use only)')
    args = parser.parse_args(argv)
    for stream in (sys.stdout, sys.stderr):  # a path outside the console code page must not crash the run
        try:
            stream.reconfigure(errors='replace')
        except (AttributeError, ValueError):
            pass
    NOTES.clear()
    INCOMPLETE.clear()
    CURRENT.update(strict=args.strict, no_trusted_exclude=args.no_trusted_exclude)
    SENSITIVE_PATHS.clear()
    try:
        if args.worktree_config and args.all is None:
            raise GateError('--worktree-config is for --all only: a change is always judged by trusted lists')
        if args.whole_history and not args.range:
            raise GateError('--whole-history is for --range only')
        git = Git()
        try:
            root = git.toplevel()
        except GateError:
            root = None  # --text may run outside a checkout
        load_names([os.environ.get('OPENWILLOW_LEAK_NAMES')] +
                   ([os.path.join(r, os.path.basename(NAMES_FILE)) for r in local_roots(git, root)] if root else []))
        if args.text is not None:
            if args.text == '-':
                data = sys.stdin.buffer.read()
            else:
                with open(args.text, 'rb') as f:
                    data = f.read()
            config = trusted_config(git, args.config_from) if root else Config('', None, 'none')
            CURRENT['config'] = config
            label = 'stdin' if args.text == '-' else 'text'
            if args.label:
                label = args.label if re.fullmatch(r'[\w.-]{1,40}', args.label) else 'text'
            decoded = decode(data)
            findings = decoded.findings(label)
            if not decoded.binary:
                findings.extend(scan_message(label, decoded.text, config))
            return report(findings, 0, 1, 'text(s)', args.show_paths)
        if root is None:
            raise GateError('not inside a git checkout')
        if args.install_hook:
            install_hook(git, root)
            return 0
        if args.all is not None:
            if args.worktree_config:
                NOTES.append('lists read from the working tree (--worktree-config)')
                config = config_from_files(root)
            else:
                config = config_from_commit(git, git.resolve(args.config_from or args.all))
                if config is None:
                    raise GateError(f'the commit holds no {ALLOW_FILE} (use --worktree-config)')
            CURRENT['config'] = config
            findings, scanned = run_all(git, args.all, config)
            if args.write_baseline:
                public, private = write_baseline(findings, args.write_baseline, f'--all {args.all}')
                print(f'leak_gate: wrote {public} entries to the baseline'
                      + (f' and {private} private-name entries next to the names list' if private else ''))
                return 0
            kept, baselined = apply_baseline(findings, config, report_stale=True)
            return report(kept, baselined, scanned, show_paths=args.show_paths)
        config = trusted_config(git, args.config_from)
        CURRENT['config'] = config
        if args.pre_push:
            scans, scanned = run_pre_push(git, root, config, args.pre_push,
                                          sys.stdin.buffer.read().decode('utf-8', 'replace'),
                                          check_local=not args.no_local)
            findings = [f for s in scans for f in s.findings]
            return report(findings, sum(s.baselined for s in scans), scanned, 'commit(s)', args.show_paths)
        scan = Scan(git, root, config)
        if args.range:
            if '..' not in args.range or '...' in args.range:
                raise GateError('--range wants BASE..HEAD')
            base, head = args.range.split('..', 1)
            scan.whole_history = args.whole_history
            scanned, unit = scan_range(scan, base or None, head or 'HEAD', check_local=not args.no_local), 'commit(s)'
        else:
            run_staged(scan)
            scanned, unit = len(scan.paths), 'file(s)'
        scan.gate_change()
        return report(scan.findings, scan.baselined, scanned, unit, args.show_paths)
    except GateError as e:
        print(f'leak_gate: error: {e}', file=sys.stderr)
        return 2
    except Exception as e:  # an internal error must never read as clean, and never quotes input
        print(f'leak_gate: internal error ({type(e).__name__}); nothing was judged clean', file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
