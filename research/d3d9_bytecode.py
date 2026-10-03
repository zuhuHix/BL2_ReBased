"""Small reader for Direct3D 9 shader model 3 token streams (vs_3_0 / ps_3_0).

Written from Microsoft's public description of the D3D9 shader bytecode (version,
instruction, destination/source parameter and comment tokens in d3d9types.h, and
the D3DXSHADER_CONSTANTTABLE layout of the 'CTAB' comment). It is not a full
disassembler: it prints one line per instruction in a plain register notation so
that a material's arithmetic can be read, and it checks itself structurally (every
instruction length must land on the next token, the stream must end with the end
token exactly at the given size).

Input is a compiled shader from a game shader cache, so the listing it prints is
game-derived and must stay under ignored local/.
"""
import struct

OPCODES = {
    0: 'nop', 1: 'mov', 2: 'add', 3: 'sub', 4: 'mad', 5: 'mul', 6: 'rcp', 7: 'rsq', 8: 'dp3', 9: 'dp4',
    10: 'min', 11: 'max', 12: 'slt', 13: 'sge', 14: 'exp', 15: 'log', 16: 'lit', 17: 'dst', 18: 'lrp',
    19: 'frc', 20: 'm4x4', 21: 'm4x3', 22: 'm3x4', 23: 'm3x3', 24: 'm3x2', 25: 'call', 26: 'callnz',
    27: 'loop', 28: 'ret', 29: 'endloop', 30: 'label', 31: 'dcl', 32: 'pow', 33: 'crs', 34: 'sgn',
    35: 'abs', 36: 'nrm', 37: 'sincos', 38: 'rep', 39: 'endrep', 40: 'if', 41: 'ifc', 42: 'else',
    43: 'endif', 44: 'break', 45: 'breakc', 46: 'mova', 47: 'defb', 48: 'defi', 64: 'texcoord',
    65: 'texkill', 66: 'texld', 78: 'expp', 79: 'logp', 80: 'cnd', 81: 'def', 88: 'cmp', 89: 'bem',
    90: 'dp2add', 91: 'dsx', 92: 'dsy', 93: 'texldd', 94: 'setp', 95: 'texldl', 96: 'breakp',
}
REGISTERS = {0: 'r', 1: 'v', 2: 'c', 3: 'a', 4: 'oRast', 5: 'oAttr', 6: 'o', 7: 'i', 8: 'oC', 9: 'oDepth',
             10: 's', 14: 'b', 15: 'aL', 16: 'half', 17: 'misc', 18: 'l', 19: 'p'}
SOURCE_MODIFIERS = {0: '{}', 1: '-{}', 2: '({}_bias)', 3: '-({}_bias)', 4: '({}_bx2)', 5: '-({}_bx2)',
                    6: '(1-{})', 7: '({}_x2)', 8: '-({}_x2)', 9: '({}_dz)', 10: '({}_dw)', 11: 'abs({})',
                    12: '-abs({})', 13: '!{}'}
COMPARISONS = {1: 'gt', 2: 'eq', 3: 'ge', 4: 'lt', 5: 'ne', 6: 'le'}


def register(token):
    kind = ((token >> 28) & 7) | ((token >> 8) & 0x18)
    return REGISTERS.get(kind, f'reg{kind}_'), token & 0x7FF


def destination(token):
    name, number = register(token)
    mask = (token >> 16) & 0xF
    text = f'{name}{number}' + ('' if mask == 0xF else '.' + ''.join('xyzw'[i] for i in range(4) if mask >> i & 1))
    modifier = (token >> 20) & 0xF
    return ('_sat ' if modifier & 1 else ' ') + text, mask


def source(token, relative=None):
    name, number = register(token)
    swizzle = (token >> 16) & 0xFF
    components = ''.join('xyzw'[(swizzle >> (2 * i)) & 3] for i in range(4))
    if components == 'xyzw':
        components = ''
    elif len(set(components)) == 1:
        components = components[0]
    text = f'{name}{number}' + (f'[{relative}]' if relative else '') + ('.' + components if components else '')
    return SOURCE_MODIFIERS.get((token >> 24) & 0xF, '?{}').format(text)


def constant_table(blob):
    """CTAB comment payload (after the fourcc) -> list of (name, register set, index, count)."""
    size, creator, version, count, info = struct.unpack_from('<5I', blob, 0)
    if size != 28 or info + count * 20 > len(blob):
        raise ValueError('unexpected constant table header')
    sets = {0: 'b', 1: 'i', 2: 'c', 3: 's'}
    out = []
    for n in range(count):
        name, regset, index, regcount = struct.unpack_from('<IHHH', blob, info + n * 20)
        end = blob.index(b'\0', name)
        out.append((blob[name:end].decode('ascii'), sets.get(regset, regset), index, regcount))
    return out


def disassemble(code):
    """-> (version text, constants, [instruction lines]). Raises on any structural mismatch."""
    if len(code) % 4:
        raise ValueError('code size is not a whole number of tokens')
    tokens = struct.unpack(f'<{len(code) // 4}I', code)
    version = tokens[0]
    if version >> 16 not in (0xFFFF, 0xFFFE):
        raise ValueError('not a vertex or pixel shader version token')
    shader = ('ps' if version >> 16 == 0xFFFF else 'vs') + f'_{version >> 8 & 0xFF}_{version & 0xFF}'
    constants, lines, pos = [], [], 1
    while pos < len(tokens):
        token = tokens[pos]
        opcode = token & 0xFFFF
        if opcode == 0xFFFF:
            if pos != len(tokens) - 1:
                raise ValueError(f'end token at {pos} of {len(tokens)}')
            return shader, constants, lines
        if opcode == 0xFFFE:
            length = (token >> 16) & 0x7FFF
            blob = code[(pos + 1) * 4:(pos + 1 + length) * 4]
            if blob[:4] == b'CTAB':
                constants = constant_table(blob[4:])
            pos += 1 + length
            continue
        length = (token >> 24) & 0xF
        args = tokens[pos + 1:pos + 1 + length]
        if len(args) != length:
            raise ValueError(f'instruction at token {pos} runs past the end')
        name = OPCODES.get(opcode, f'op{opcode}')
        control = (token >> 16) & 0xFF
        if name in ('ifc', 'breakc', 'setp') and control in COMPARISONS:
            name += '_' + COMPARISONS[control]
        if name == 'dcl':
            usage = args[0]
            dest, _ = destination(args[1])
            lines.append(f'dcl_{usage & 0x1F}_{(usage >> 16) & 0xF}{dest}' if register(args[1])[0] != 's'
                         else f'dcl_sampler{(usage >> 27) & 0xF}{dest}')
        elif name == 'def':
            dest, _ = destination(args[0])
            values = struct.unpack('<4f', struct.pack('<4I', *args[1:5]))
            lines.append(f'def{dest}, ' + ', '.join(f'{v:.6g}' for v in values))
        elif name in ('defi', 'defb'):
            dest, _ = destination(args[0])
            lines.append(f'{name}{dest}, ' + ', '.join(str(v) for v in args[1:]))
        else:
            parts, index = [], 0
            has_dest = name not in ('if', 'ifc_gt', 'ifc_eq', 'ifc_ge', 'ifc_lt', 'ifc_ne', 'ifc_le', 'rep',
                                    'loop', 'call', 'callnz', 'label', 'texkill', 'breakc', 'breakp', 'else',
                                    'endif', 'endrep', 'endloop', 'ret', 'break') or name.startswith('setp')
            if name == 'texkill':
                has_dest = True
            text = name
            while index < len(args):
                token_value = args[index]
                relative = None
                if (token_value >> 13) & 1 and index + 1 < len(args) and not (has_dest and index == 0):
                    relative = source(args[index + 1])
                    index += 1
                if has_dest and index == 0:
                    dest, _ = destination(token_value)
                    text += dest
                else:
                    parts.append(source(token_value, relative))
                index += 1
            lines.append(text + (', ' if has_dest and parts else ' ') + ', '.join(parts))
        pos += 1 + length
    raise ValueError('no end token')
