"""Coordinate adapter for UE5's right-handed OBJ input convention."""


def actor_label(level, source, slot):
    import hashlib
    identity = hashlib.sha256(source.encode('utf-8')).hexdigest()[:16]
    return f'OpenWillow_{level}_{source.split(".")[-1]}_{identity}_s{slot}'


def host_obj(text):
    # Our source OBJ stores UE left-handed centimeters. Reflect into the OBJ
    # importer's right-handed convention; reflect normals and reverse winding
    # together so the host conversion returns the source orientation.
    result = []
    for line in text.splitlines():
        fields = line.split()
        if fields and fields[0] in ('v', 'vn'):
            fields[2] = format(-float(fields[2]), '.9g')
            line = ' '.join(fields)
        elif fields and fields[0] == 'f':
            line = 'f ' + ' '.join(reversed(fields[1:]))
        result.append(line)
    return '\n'.join(result) + '\n'
