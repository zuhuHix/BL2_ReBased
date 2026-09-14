"""Synthetic DXT/ARGB, PNG, bulk/TFC and static-mesh serialization tests."""
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
         'EPixelFormat', 'PF_DXT1', 'PF_DXT5', 'TextureFileCacheName', 'NameProperty', 'TestCache', 'PF_A8R8G8B8', 'PF_G8']
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
def texture(block, five=False, flags=0, declared=None, fmt=None, width=4, height=4):
    props = tag('Format', 'ByteProperty', fname(fmt or ('PF_DXT5' if five else 'PF_DXT1')), fname('EPixelFormat'))
    props += tag('TextureFileCacheName', 'NameProperty', fname('TestCache'))
    stored = literal(block) if flags & 16 else block
    native = bytes(16) + words(1, flags, len(block) if declared is None else declared, len(stored), 17)
    if not flags & 1: native += stored
    return words(0) + props + fname('None') + native + words(width, height)
def texture_mips(blocks, fmt='PF_DXT1', dimensions=None):
    props = tag('Format', 'ByteProperty', fname(fmt), fname('EPixelFormat'))
    props += tag('TextureFileCacheName', 'NameProperty', fname('TestCache'))
    native = bytes(16) + words(len(blocks))
    for i, block in enumerate(blocks):
        native += words(0, len(block), len(block), 0) + block + words(*(dimensions[i] if dimensions else (4, 4)))
    return words(0) + props + fname('None') + native
def png_pixels(path, width=4, height=4):
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
        if kind == b'IHDR': assert struct.unpack('>IIBBBBB', chunk) == (width, height, 8, 6, 0, 0, 0)
        if kind == b'IDAT': compressed += chunk
        at += n+12
    rows = zlib.decompress(compressed)
    stride = width * 4 + 1
    assert len(rows) == height * stride and all(rows[y*stride] == 0 for y in range(height))
    return b''.join(rows[y*stride+1:(y+1)*stride] for y in range(height))
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
    # Distinct channels and alpha, odd row width: catches swizzle, premultiplication,
    # row inversion and accidental DXT block rounding.
    bgra = bytes([3, 17, 241, 0, 23, 91, 7, 64, 255, 0, 1, 255,
                  8, 9, 10, 128, 33, 44, 55, 1, 77, 88, 99, 254])
    rgba = bytes([241, 17, 3, 0, 7, 91, 23, 64, 1, 0, 255, 255,
                  10, 9, 8, 128, 55, 44, 33, 1, 99, 88, 77, 254])
    def argb(block=bgra, **kwargs):
        return texture(block, fmt='PF_A8R8G8B8', width=kwargs.pop('width', 3),
                       height=kwargs.pop('height', 2), **kwargs)
    for flags in [0, 16, 1, 17]:
        (root/'TestCache.tfc').write_bytes(bytes(17)+(literal(bgra) if flags & 16 else bgra))
        result = run('Texture2D', argb(flags=flags))
        assert result.returncode == 0, result.stderr
        assert json.loads(result.stdout)['format'] == 'PF_A8R8G8B8'
        assert png_pixels(output, 3, 2) == rgba
    for payload, error in [
        (argb(bgra[:-1]), 'A8R8G8B8 mip byte count mismatch'),
        (argb(bgra+b'X'), 'A8R8G8B8 mip byte count mismatch'),
        (argb(declared=25), 'texture bulk decoded size mismatch'),
        (argb(width=0), 'invalid texture mip dimensions'),
        (argb(width=16385), 'invalid texture dimensions'),
        (argb(width=16384, height=16384), 'invalid texture dimensions'),
        (argb(width=0xffffffff, height=0xffffffff), 'invalid texture dimensions'),
        (argb(flags=2), 'unsupported texture bulk codec/flags'),
        (texture(bgra, fmt='PF_G8'), 'texture importer supports')]:
        result = run('Texture2D', payload)
        assert result.returncode != 0 and error in result.stderr, result.stderr
    assert run('Texture2D', argb()[:-1]).returncode != 0
    (root/'TestCache.tfc').write_bytes(bytes(18))
    assert run('Texture2D', argb(flags=1)).returncode != 0
    result = run('Texture2D', texture_mips([bgra, bgra[:4]], 'PF_A8R8G8B8', [(3,2),(1,1)]),
                 extra=('--mip','1','--all-mips',str(root/'argb_mips')))
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)['resident_mips'] == 2
    assert png_pixels(output, 1, 1) == rgba[:4]
    assert png_pixels(root/'argb_mips/mip_00.png', 3, 2) == rgba
    assert png_pixels(root/'argb_mips/mip_01.png', 1, 1) == rgba[:4]
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
print('A8R8G8B8 and DXT1/5 pixels, PNG CRC/zlib, streamed/compressed bulk, mesh buffers and corrupt inputs passed.')
