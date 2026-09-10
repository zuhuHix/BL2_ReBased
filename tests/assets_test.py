"""Synthetic DXT, PNG, bulk/TFC and static-mesh serialization tests."""
import binascii
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib

reader = str(Path(sys.argv[1]).resolve())
names = ['None', 'Texture2D', 'StaticMesh', 'Class', 'Probe', 'Format', 'ByteProperty',
         'EPixelFormat', 'PF_DXT1', 'PF_DXT5', 'TextureFileCacheName', 'NameProperty', 'TestCache']
def words(*v): return struct.pack('<' + 'I' * len(v), *v)
def fname(s): return words(names.index(s), 0)
def tag(name, kind, value, extra=b''):
    return fname(name) + fname(kind) + words(len(value), 0) + extra + value
def package(cls, payload):
    table = b''.join(words(len(n)+1) + n.encode() + b'\0' + bytes(8) for n in names)
    io = 48 + len(table)
    imp = fname('None') + fname('Class') + words(0) + fname(cls)
    eo = io + len(imp)
    exp = words(0xffffffff, 0, 0) + fname('Probe') + words(0, 0, 0, len(payload), eo+68, 0, 0) + bytes(20)
    return words(0x9e2a83c1, 832 | (46 << 16), 48, 0, 0, len(names), 48, 1, eo, 1, io, 0) + table + imp + exp + payload
def literal(data):
    assert 4 <= len(data) <= 238
    stream = bytes([17+len(data)]) + data + b'\x11\0\0'
    return words(0x9e2a83c1, 131072, len(stream), len(data), len(stream), len(data)) + stream
def texture(block, five=False, flags=0, declared=None):
    props = tag('Format', 'ByteProperty', fname('PF_DXT5' if five else 'PF_DXT1'), fname('EPixelFormat'))
    props += tag('TextureFileCacheName', 'NameProperty', fname('TestCache'))
    stored = literal(block) if flags & 16 else block
    native = bytes(16) + words(1, flags, len(block) if declared is None else declared, len(stored), 17)
    if not flags & 1: native += stored
    return words(0) + props + fname('None') + native + words(4, 4)
def texture_mips(blocks):
    props = tag('Format', 'ByteProperty', fname('PF_DXT1'), fname('EPixelFormat'))
    props += tag('TextureFileCacheName', 'NameProperty', fname('TestCache'))
    native = bytes(16) + words(len(blocks))
    for block in blocks:
        native += words(0, len(block), len(block), 0) + block + words(4, 4)
    return words(0) + props + fname('None') + native
def png_pixels(path):
    data = path.read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    at = 8
    compressed = b''
    while at < len(data):
        n, = struct.unpack_from('>I', data, at)
        kind = data[at+4:at+8]
        chunk = data[at+8:at+8+n]
        crc, = struct.unpack_from('>I', data, at+8+n)
        assert crc == binascii.crc32(kind+chunk)
        if kind == b'IDAT': compressed += chunk
        at += n+12
    rows = zlib.decompress(compressed)
    assert len(rows) == 68 and all(rows[y*17] == 0 for y in range(4))
    return b''.join(rows[y*17+1:(y+1)*17] for y in range(4))
def mesh(bad_index=False, full=False):
    native = bytes(28+4+24) + words(6,0,8,0,18,0,0,0,1)
    native += words(0,0,0,0,1)  # empty source bulk, one section
    native += words(0,0,0,1,0,1,0,2,0,0) + b'\0'
    native += words(12,3,12,3) + struct.pack('<9f',0,0,0,100,0,0,0,100,0)
    stride = 16 if full else 12
    native += words(1,stride,3,int(full),stride,3)
    for u,v in [(0,0),(1,0),(0,1)]:
        native += bytes([255,128,128,255,128,128,255,255]) + struct.pack('<2f' if full else '<2e',u,v)
    native += words(0,0,3,2,3) + struct.pack('<3H',0,1,9 if bad_index else 2)
    return words(0) + fname('None') + native

with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    source, output = root/'asset.upk', root/'out.png'
    def run(cls, payload, mode='--texture', extra=()):
        source.write_bytes(package(cls,payload))
        args = [reader,str(source),mode,'1','--property-offset','4','--output',str(output)]
        if mode == '--texture': args += ['--tfc',str(root)]
        args += list(extra)
        return subprocess.run(args,capture_output=True,text=True)
    red = struct.pack('<HHI',0xf800,0,0)
    for flags in [0,16,1,17]:
        (root/'TestCache.tfc').write_bytes(bytes(17)+(literal(red) if flags & 16 else red))
        result = run('Texture2D',texture(red,flags=flags))
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout)['streamed'] == bool(flags & 1)
        assert png_pixels(output) == bytes([255,0,0,255])*16
    transparent = struct.pack('<HHI',0,0xffff,0xffffffff)
    assert run('Texture2D',texture(transparent)).returncode == 0
    assert png_pixels(output) == bytes(64)
    five = bytes([64,0])+bytes(6)+red
    assert run('Texture2D',texture(five,True)).returncode == 0
    assert png_pixels(output) == bytes([255,0,0,64])*16
    for invalid in [texture(red,declared=9),texture(red,flags=2),texture(red)[:-1]]:
        assert run('Texture2D',invalid).returncode != 0
    (root/'TestCache.tfc').write_bytes(bytes(18))
    assert run('Texture2D',texture(red,flags=1)).returncode != 0
    green = struct.pack('<HHI',0x07e0,0,0)
    result = run('Texture2D',texture_mips([red, green]))
    assert result.returncode == 0, result.stderr
    report = json.loads(result.stdout)
    assert report['serialized_mips'] == 2 and report['resident_mips'] == 2
    result = run('Texture2D',texture_mips([red, green]),extra=('--mip','1','--all-mips',str(root/'mips')))
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)['mip'] == 1
    assert (root/'mips/mip_00.png').is_file() and (root/'mips/mip_01.png').is_file()
    assert png_pixels(output) == bytes([0,255,0,255])*16
    for full in [False,True]:
        result = run('StaticMesh',mesh(full=full),'--mesh')
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout)['vertices'] == 3
        obj = output.read_text()
        assert 'v 100 0 0\n' in obj and 'vt 0 0\n' in obj and 'f 1/1/1 2/2/2 3/3/3\n' in obj
    assert run('StaticMesh',mesh(True),'--mesh').returncode != 0
    for end in range(0,len(mesh()),7):
        assert run('StaticMesh',mesh()[:end],'--mesh').returncode != 0
    def mesh_variant(index_width=4, lod_count=2):
        native = bytes(28+4+24) + words(6,0,8,0,18,0,0,0,lod_count)
        for lod in range(lod_count):
            native += words(0,0,0,0,1)
            native += words(0,0,0,1,0,1,0,2,0,0) + b'\0'
            native += words(12,3,12,3) + struct.pack('<9f',0,0,0,100+lod,0,0,0,100+lod,0)
            native += words(1,12,3,0,12,3)
            for u,v in [(0,0),(1,0),(0,1)]:
                native += bytes([255,128,128,255,128,128,255,255]) + struct.pack('<2e',u,v)
            native += words(0,0,3,index_width,3)
            native += struct.pack('<3I',0,1,2) if index_width == 4 else struct.pack('<3H',0,1,2)
        return words(0) + fname('None') + native
    result = run('StaticMesh',mesh_variant(),'--mesh',extra=('--lod','1'))
    assert result.returncode == 0, result.stderr
    report = json.loads(result.stdout)
    assert report['lods'] == 2 and report['selected_lod'] == 1
    assert report['lod_summaries'][1]['index_width'] == 4
    assert 'v 101 0 0\n' in output.read_text()
print('DXT1/5 pixels, PNG CRC/zlib, streamed/compressed bulk, mesh buffers and corrupt inputs passed.')
