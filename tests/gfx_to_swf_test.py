"""Synthetic tests for tools/gfx_to_swf.py: script localization and font-library imports.

Builds small AVM1 action streams by hand (SWF spec action layouts; no game
data) and checks that translating a $File.Section.Key string re-measures
every branch, function, With and Try distance that spans it.
Run: python tests/gfx_to_swf_test.py
"""
import struct
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import gfx_to_swf as g  # noqa: E402

TABLE = {'file.section.key': 'LONGER TRANSLATED TEXT'}   # 22 bytes for a 17-byte token
TOKEN = b'$File.Section.Key'
GROWTH = len(TABLE['file.section.key']) - len(TOKEN)


def action(op, payload=None):
    return bytes([op]) if payload is None else bytes([op]) + struct.pack('<H', len(payload)) + payload


def push_string(s):
    return action(0x96, b'\0' + s + b'\0')


def jump(offset, op=0x99):
    return action(op, struct.pack('<h', offset))


def parse(code):
    """[(position, op, payload)] up to ActionEnd."""
    out, pos = [], 0
    while code[pos]:
        op = code[pos]
        size = struct.unpack_from('<H', code, pos + 1)[0] if op >= 0x80 else 0
        out.append((pos, op, code[pos + 3:pos + 3 + size] if op >= 0x80 else None))
        pos += 3 + size if op >= 0x80 else 1
    return out, pos


def localize(code):
    counts = Counter()
    return g.localize_actions(code, TABLE, counts), counts


def test_forward_jump_over_token():
    pop = action(0x17)
    tail = action(0x07)                                   # Stop, the jump target
    body = push_string(TOKEN) + pop
    code = jump(len(body)) + body + tail + b'\0'
    new, counts = localize(code)
    assert counts['translated'] == 1, counts
    actions, _ = parse(new)
    offset = struct.unpack('<h', actions[0][2])[0]
    assert offset == len(body) + GROWTH, offset
    assert actions[0][0] + 5 + offset == actions[-1][0]    # still lands on Stop


def test_backward_branch_over_token():
    head = action(0x07)                                   # loop target at 0
    middle = push_string(TOKEN) + action(0x17)
    branch_end = len(head) + len(middle) + 5
    code = head + middle + jump(-branch_end, 0x9D) + b'\0'
    new, _ = localize(code)
    actions, _ = parse(new)
    branch_pos, _, data = actions[-1]
    assert branch_pos + 5 + struct.unpack('<h', data)[0] == 0


def test_jump_not_spanning_token_is_unchanged():
    code = push_string(TOKEN) + jump(1) + action(0x17) + action(0x07) + b'\0'
    new, _ = localize(code)
    actions, _ = parse(new)
    assert struct.unpack('<h', actions[1][2])[0] == 1


def test_function_body_size_grows():
    body = push_string(TOKEN) + action(0x3E)             # return "..."
    header = b'f\0' + struct.pack('<H', 0) + struct.pack('<H', len(body))
    code = action(0x9B, header) + body + action(0x07) + b'\0'
    new, _ = localize(code)
    actions, _ = parse(new)
    size = struct.unpack_from('<H', actions[0][2], len(actions[0][2]) - 2)[0]
    assert size == len(body) + GROWTH, size


def test_try_block_sizes():
    try_block = push_string(TOKEN) + action(0x17)
    catch_block = action(0x17)
    header = bytes([0]) + struct.pack('<3H', len(try_block), len(catch_block), 0) + b'e\0'
    code = action(0x8F, header) + try_block + catch_block + action(0x07) + b'\0'
    new, _ = localize(code)
    actions, _ = parse(new)
    sizes = struct.unpack_from('<3H', actions[0][2], 1)
    assert sizes == (len(try_block) + GROWTH, len(catch_block), 0), sizes


def test_misaligned_branch_leaves_stream_alone():
    code = push_string(TOKEN) + jump(-4) + action(0x07) + b'\0'   # lands mid-action
    new, counts = localize(code)
    assert new == code
    assert counts == Counter({'script_skipped': 1}), counts


def test_no_tokens_is_identity():
    code = push_string(b'$NotAToken') + jump(1) + action(0x17) + b'\0'
    new, counts = localize(code)
    assert new == code and not counts


def test_import_url_backslashes():
    body = b'..\\Shared\\Lib.swf\0' + struct.pack('<HH', 1, 0) + struct.pack('<H', 5) + b'name\0'
    assert g.normalize_import_url(body).startswith(b'../Shared/Lib.swf\0')
    assert g.normalize_import_url(body)[len(b'../Shared/Lib.swf\0'):] == body[len(b'..\\Shared\\Lib.swf\0'):]


def font_library_swf():
    """A minimal FWS movie: one stand-in DefineFont3 (id 7) exported as $Body (no game data)."""
    rect = bytes([0x00])                      # nbits 0: an empty frame rect in one byte
    header = rect + struct.pack('<HH', 0x1800, 1)
    tags = g.write_tag(g.DEFINE_FONT3, struct.pack('<H', 7) + b'FONTBODY')
    tags += g.write_tag(g.EXPORT_ASSETS, struct.pack('<HH', 1, 7) + b'$Body\0') + g.write_tag(0, b'')
    body = header + tags
    return b'FWS' + bytes([9]) + struct.pack('<I', 8 + len(body)) + body


def font_import(url):
    # ImportAssets2: url, two reserved bytes, count, (id, name)
    return url.encode() + b'\0' + struct.pack('<BBHH', 1, 0, 1, 23) + b'$Body\0'


def test_plain_font_import_copies_library_from_run_root():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        run = Path(tmp)
        (run / 'gfxfontlib.swf').write_bytes(font_library_swf())
        movie = run / 'UI_Movie'
        movie.mkdir()
        conv = g.Converter(run, font_base=movie)
        out = conv.convert([(g.IMPORT_ASSETS2, font_import('gfxfontlib.swf'))])
        assert (movie / 'gfxfontlib.swf').read_bytes() == (run / 'gfxfontlib.swf').read_bytes()
        assert conv.copied_libraries and conv.inlined_fonts == [{'id': 23, 'name': '$Body', 'from': 'gfxfontlib.swf'}]
        assert list(g.read_tags(out, 0, len(out)))[0][0] == g.DEFINE_FONT3
        # the ../ form already resolves; nothing more is copied, and an existing copy is not overwritten
        (movie / 'gfxfontlib.swf').write_bytes(font_library_swf() + b'')
        conv2 = g.Converter(run, font_base=movie)
        conv2.convert([(g.IMPORT_ASSETS2, font_import('../gfxfontlib.swf')), (g.IMPORT_ASSETS2, font_import('gfxfontlib.swf'))])
        assert conv2.copied_libraries == [] and len(conv2.inlined_fonts) == 2


def test_traversal_or_other_library_url_is_not_copied():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        run = base / 'run'
        movie = run / 'UI_Movie'
        movie.mkdir(parents=True)
        (run / 'gfxfontlib.swf').write_bytes(font_library_swf())
        (run / 'escape.swf').write_bytes(font_library_swf())
        conv = g.Converter(run, font_base=movie)
        conv.convert([(g.IMPORT_ASSETS2, font_import('../../escape.swf')),
                      (g.IMPORT_ASSETS2, font_import('../../gfxfontlib.swf')),
                      (g.IMPORT_ASSETS2, font_import('sub/../../../gfxfontlib.swf')),
                      (g.IMPORT_ASSETS2, font_import('escape.swf'))])
        assert not (base / 'escape.swf').exists() and not (base / 'gfxfontlib.swf').exists()
        assert not (movie / 'escape.swf').exists()
        assert conv.copied_libraries == [] and len(conv.skipped_libraries) == 2


def test_missing_library_keeps_the_import():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        movie = Path(tmp) / 'UI_Movie'
        movie.mkdir()
        conv = g.Converter(tmp, font_base=movie)
        out = conv.convert([(g.IMPORT_ASSETS2, font_import('gfxfontlib.swf'))])
        assert conv.copied_libraries == [] and conv.inlined_fonts == []
        assert list(g.read_tags(out, 0, len(out)))[0][0] == g.IMPORT_ASSETS2


if __name__ == '__main__':
    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith('test_')]
    for name, fn in tests:
        fn()
        print('ok', name)
    print(f'{len(tests)} tests passed')
