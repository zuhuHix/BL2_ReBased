"""Synthetic table and malformed-input tests; contains no game data."""
import json
import pathlib
import struct
import subprocess
import sys
import tempfile

sys.argv[1] = str(pathlib.Path(sys.argv[1]).resolve())

def pack(*values):
    return struct.pack('<' + 'I' * len(values), *values)

name = pack(5) + b'None\0' + bytes(8)
header = pack(0x9E2A83C1, 832 | (46 << 16), 48, 0, 0, 1, 48, 1, 93, 1, 65, 0)
imp = pack(0, 0, 0, 0, 0, 0, 0)
exp = pack(0xFFFFFFFF, 0, 0, 0, 0, 0, 0, 0, 0, 161, 0, 0, 0, 0, 0, 0, 0)
valid = header + name + imp + exp
compressed = '--compressed' in sys.argv
def container(data, block=131072):
    # Literal-only LZO1X streams with end markers, generated without game data.
    streams = []
    sizes = []
    for offset in range(0, len(data), block):
        chunk = data[offset:offset + block]
        assert len(chunk) >= 4
        if len(chunk) <= 238:
            prefix = bytes([17 + len(chunk)])
        else:
            zeros, final = divmod(len(chunk) - 19, 255)
            prefix = bytes(1 + zeros) + bytes([final + 1])
        stream = prefix + chunk + b'\x11\x00\x00'
        streams.append(stream)
        sizes.extend((len(stream), len(chunk)))
    payload = b''.join(streams)
    return pack(0x9E2A83C1, block, len(payload), len(data), *sizes) + payload

with tempfile.TemporaryDirectory() as folder:
    path = pathlib.Path(folder) / 'synthetic.bin'
    def run(data):
        path.write_bytes(data)
        return subprocess.run([sys.argv[1], str(path)], capture_output=True, text=True)
    fixture = container(valid) if compressed else valid
    result = run(fixture)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == dict(version=832, licensee=46, names=1, imports=1, exports=1)
    for end in range(len(fixture)):
        assert run(fixture[:end]).returncode != 0, end
    for offset, value in [(0, 0), (4, 0), (20, 0xFFFFFFFF), (24, 999999), (65, 9), (93, 2), (125, 999999), (137, 0xFFFFFFFF)]:
        bad = bytearray(valid)
        struct.pack_into('<I', bad, offset, value)
        assert run(container(bad) if compressed else bad).returncode != 0, offset
    if compressed:
        # Partial package compression: logical offsets differ from disk offsets.
        partial_header = bytearray(pack(0x9E2A83C1, 832 | (46 << 16), 128, 0, 0x02000000,
                                       1, 128, 1, 173, 1, 145))
        partial_header += bytes(20 + 16) + pack(1, 1, 1, 0, 0, 0, 2, 1)
        assert len(partial_header) == 112
        partial_payload = name + imp + exp
        partial_payload = bytearray(partial_payload)
        struct.pack_into('<I', partial_payload, 45 + 36, 241)
        stream = container(partial_payload)
        partial_header += pack(128, len(partial_payload), 144, len(stream)) + bytes(16)
        partial = partial_header + stream
        assert run(partial).returncode == 0, run(partial).stderr
        for offset, value in [(104, 1), (108, 999), (112, 0), (116, 999), (120, 4), (124, 999)]:
            bad = bytearray(partial); struct.pack_into('<I', bad, offset, value)
            assert run(bad).returncode != 0, offset
        assert run(partial + b'extra').returncode != 0
        for padding in [4096 - len(valid), 8192 - len(valid), 9000]:
            result = run(container(valid + bytes(padding), block=4096))
            assert result.returncode == 0, result.stderr
        for offset, value in [(4, 0), (8, 1), (12, 0), (12, 0xFFFFFFFF), (16, 0), (20, 1)]:
            bad = bytearray(fixture)
            struct.pack_into('<I', bad, offset, value)
            assert run(bad).returncode != 0, offset
        assert run(fixture + b'garbage').returncode != 0
        bad = bytearray(fixture)
        bad[-3:] = b'\x00\x00\x00'
        assert run(bad).returncode != 0
        # Invalid backward reference before any output and an oversized literal run.
        for stream in [b'\x11\x04\x00', bytes([255]) + valid + b'\x11\x00\x00']:
            assert run(pack(0x9E2A83C1, 131072, len(stream), len(valid), len(stream), len(valid)) + stream).returncode != 0
        reference = pathlib.Path(folder) / 'reference.bin'
        reference.write_bytes(valid)
        path.write_bytes(fixture)
        assert subprocess.run([sys.argv[1], str(path), '--verify-decoded', str(reference)], capture_output=True).returncode == 0
        reference.write_bytes(valid + b'wrong')
        assert subprocess.run([sys.argv[1], str(path), '--verify-decoded', str(reference)], capture_output=True).returncode != 0
print('Synthetic package, all truncations, and corrupt fields passed.')
