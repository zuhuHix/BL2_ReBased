"""Coordinate adapter for UE5's right-handed OBJ input convention."""


def actor_label(level, source, slot):
    import hashlib
    identity = hashlib.sha256(source.encode('utf-8')).hexdigest()[:16]
    return f'OpenWillow_{level}_{source.split(".")[-1]}_{identity}_s{slot}'


def host_obj(text):
    # Our source OBJ stores UE left-handed centimeters and original UE index
    # order (face cross products oppose the stored outward normals). Reflect
    # positions/normals into OBJ space, retaining that index order. UE's OBJ
    # importer performs the remaining winding conversion. Reversing here too
    # exposes back faces: signs read mirrored and outward surfaces disappear.
    result = []
    for line in text.splitlines():
        fields = line.split()
        if fields and fields[0] in ('v', 'vn'):
            fields[2] = format(-float(fields[2]), '.9g')
            line = ' '.join(fields)
        result.append(line)
    return '\n'.join(result) + '\n'
