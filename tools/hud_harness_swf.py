"""Write a small wrapper SWF that loads a UI movie and lets JavaScript drive it.

The wrapper is our own code, assembled here as ActionScript 2 (AVM1) bytecode
from Adobe's public SWF specification; it contains no game data. It loads the
movie into _level1 with loadMovieNum and registers one ExternalInterface
callback, `ow(target, op, a, b)`:
  op "set": target[a] = b          op "get": return target[a]
  otherwise: return target[op](a, b)
`target` is a dot path resolved with eval (e.g. "_level1.p1.health"), so a
harness page can move clips to frame labels, set text, or read state the way
the host game does through Scaleform. Output belongs under ignored local/.
"""
import argparse
import struct
import zlib
from pathlib import Path


class Asm:
    """Minimal AVM1 assembler with forward labels for If/Jump."""

    def __init__(self):
        self.code = bytearray()
        self.fixups = []
        self.labels = {}

    def op(self, code, payload=b''):
        self.code.append(code)
        if code >= 0x80:
            self.code += struct.pack('<H', len(payload)) + payload

    def push(self, *values):
        payload = bytearray()
        for v in values:
            if v is None:
                payload.append(2)
            elif isinstance(v, bool):
                payload += bytes((5, 1 if v else 0))
            elif isinstance(v, int):
                payload += bytes((7,)) + struct.pack('<i', v)
            elif isinstance(v, tuple) and v[0] == 'reg':
                payload += bytes((4, v[1]))
            else:
                payload += bytes((0,)) + v.encode('latin-1') + b'\0'
        self.op(0x96, bytes(payload))

    def branch(self, code, label):
        self.code.append(code)
        self.code += struct.pack('<H', 2)
        self.fixups.append((len(self.code), label))
        self.code += b'\0\0'

    def label(self, name):
        self.labels[name] = len(self.code)

    def bytes(self):
        for end, label in self.fixups:
            struct.pack_into('<h', self.code, end, self.labels[label] - (end + 2))
        return bytes(self.code)


GET_VAR, SET_VAR, GET_MEMBER, SET_MEMBER = 0x1C, 0x1D, 0x4E, 0x4F
CALL_METHOD, EQUALS2, NOT, POP, RETURN, STORE_REG = 0x52, 0x49, 0x12, 0x17, 0x3E, 0x87
IF, JUMP, DEFINE_FUNCTION, GET_URL2, STOP = 0x9D, 0x99, 0x9B, 0x9A, 0x07


def callback_body():
    a = Asm()
    # r1 = eval(target)
    a.push('target'); a.op(GET_VAR); a.op(GET_VAR); a.op(STORE_REG, b'\x01'); a.op(POP)
    # if (op == "set") { r1[a] = b; return true; }
    a.push('op'); a.op(GET_VAR); a.push('set'); a.op(EQUALS2); a.op(NOT); a.branch(IF, 'not_set')
    a.push(('reg', 1)); a.push('a'); a.op(GET_VAR); a.push('b'); a.op(GET_VAR); a.op(SET_MEMBER)
    a.push(True); a.op(RETURN)
    a.label('not_set')
    # if (op == "get") return r1[a];
    a.push('op'); a.op(GET_VAR); a.push('get'); a.op(EQUALS2); a.op(NOT); a.branch(IF, 'call')
    a.push(('reg', 1)); a.push('a'); a.op(GET_VAR); a.op(GET_MEMBER); a.op(RETURN)
    a.label('call')
    # return r1[op](a, b);
    a.push('b'); a.op(GET_VAR); a.push('a'); a.op(GET_VAR); a.push(2)
    a.push(('reg', 1)); a.push('op'); a.op(GET_VAR); a.op(CALL_METHOD); a.op(RETURN)
    return a.bytes()


def frame_script(movie):
    a = Asm()
    a.push(movie, '_level1'); a.op(GET_URL2, b'\x00')          # loadMovieNum(movie, 1)
    body = callback_body()
    params = b''.join(p.encode() + b'\0' for p in ('target', 'op', 'a', 'b'))
    a.op(DEFINE_FUNCTION, b'\0' + struct.pack('<H', 4) + params + struct.pack('<H', len(body)))
    a.code += body                                              # anonymous function object
    a.push(None, 'ow', 3)                                       # args: (name, instance, fn) reversed
    # flash.external.ExternalInterface.addCallback("ow", null, fn)
    a.push('flash'); a.op(GET_VAR); a.push('external'); a.op(GET_MEMBER)
    a.push('ExternalInterface'); a.op(GET_MEMBER); a.push('addCallback'); a.op(CALL_METHOD); a.op(POP)
    a.op(STOP)
    return a.bytes() + b'\0'


def tag(code, body):
    return struct.pack('<HI', (code << 6) | 0x3F, len(body)) + body


def rect_1280x720():
    # nbits 16: xmin 0, xmax 25600, ymin 0, ymax 14400 twips.
    bits = '{:05b}'.format(16) + '{:016b}'.format(0) + '{:016b}'.format(25600) + '{:016b}'.format(0) + '{:016b}'.format(14400)
    bits += '0' * (-len(bits) % 8)
    return int(bits, 2).to_bytes(len(bits) // 8, 'big')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--movie', required=True, help='URL of the UI movie relative to the wrapper')
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    body = rect_1280x720() + struct.pack('<BBH', 0, 24, 1)
    body += tag(69, struct.pack('<I', 0))                      # FileAttributes: AVM1, no network flag
    body += tag(9, bytes((0x1B, 0x23, 0x30)))                   # SetBackgroundColor
    body += tag(12, frame_script(args.movie))                   # DoAction
    body += tag(1, b'') + tag(0, b'')
    Path(args.output).write_bytes(b'FWS' + bytes([9]) + struct.pack('<I', 8 + len(body)) + body)
    print(f'wrote {args.output} ({8 + len(body)} bytes)')


if __name__ == '__main__':
    main()
