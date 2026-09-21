"""Add corroborated Sanctuary root BSP polygons to an existing frozen scene.

Materials retain native assignments. UVs project each point onto the surface's
own texture axes from its base point (policy `surface_axes`), divided by a
texel scale that no available oracle can confirm and that stays UNVERIFIED;
`--uv planar` keeps the earlier 128 cm world-planar placeholder. Lighting and
collision flags remain UNVERIFIED. Collision is an opt-in host triangle
approximation on the recovered surfaces.
"""
import argparse
import json
import math
from pathlib import Path

from bsp_decode import (decode_model, decode_component, validate_coverage,
                        require_root, dot, sub, cross)
from prepare_level import Scene, transform

UV_POLICIES = ('surface_axes', 'planar')
# World units per texture repeat for unit-length axes. Every oracle we have
# (umodel, the game's object dumps) is silent on this constant; only a matched
# in-game view can confirm it. See DECISIONS.md, 2026-09-18 texture axes.
TEXEL_SCALE = 128.0


def surface_uv(p, point, texel_scale=TEXEL_SCALE):
    """((P - Base) . TextureU, (P - Base) . TextureV) / texel scale."""
    d = sub(point, p['base'])
    return dot(d, p['texture_u']) / texel_scale, dot(d, p['texture_v']) / texel_scale


def planar_uv(p, point):
    """The earlier placeholder: the two non-dominant world axes over 128 cm."""
    normal = p['plane'][:3]
    dominant = max(range(3), key=lambda i: abs(normal[i]))
    axes = [i for i in range(3) if i != dominant]
    return point[axes[0]] / 128, point[axes[1]] / 128


def section_obj(polygons, uv='surface_axes', texel_scale=TEXEL_SCALE):
    if uv not in UV_POLICIES:
        raise ValueError('Unknown BSP UV policy')
    lines, faces, offset = [], [], 0
    for p in polygons:
        normal = p['plane'][:3]
        for v in p['points']:
            lines.append('v ' + ' '.join(format(x, '.9g') for x in v))
        for v in p['points']:
            s, t = surface_uv(p, v, texel_scale) if uv == 'surface_axes' else planar_uv(p, v)
            # Same V convention as the static-mesh OBJ writer in src/assets.cpp.
            lines.append(f'vt {s:.9g} {1-t:.9g}')
        for _ in p['points']:
            lines.append('vn ' + ' '.join(format(x, '.9g') for x in normal))
        # Match host_obj's left-handed face convention. Polygon ordering above
        # is independently checked against both native plane records.
        for a, b, c in p['triangles']:
            faces.append('f ' + ' '.join(f'{offset+i+1}/{offset+i+1}/{offset+i+1}' for i in (a,c,b)))
        offset += len(p['points'])
    return '\n'.join(lines + ['g bsp'] + faces) + '\n', len(faces)


def probe_candidates(model, components, package):
    candidates = []
    owners = {i: c for c in components for i in c['nodes']}
    for p in model['polygons']:
        if p['plane'][2] < .99:
            continue
        points = p['points']
        center = [sum(v[k] for v in points)/len(points) for k in range(3)]
        edges = [sub(points[(j+1)%len(points)], v) for j,v in enumerate(points)]
        longest = max(edges, key=lambda e: dot(e,e))
        length = math.sqrt(dot(longest,longest))
        direction = [v/length for v in longest]
        start = [center[k]-direction[k]*100 for k in range(3)]
        end = [center[k]+direction[k]*100 for k in range(3)]
        def clearance(v):
            return min(dot(cross(edge, sub(v,a)), p['plane'][:3])/math.sqrt(dot(edge,edge))
                       for a,edge in zip(points,edges))
        margin = min(clearance(start),clearance(end))
        if margin < 45:
            continue
        c = owners[p['node']]
        candidates.append({'source': c['path'], 'level': package, 'node': p['node'],
                           'start': start, 'end': end, 'clearance': margin})
    return sorted(candidates, key=lambda p: -p['clearance'])[:6]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--collision', action='store_true')
    parser.add_argument('--uv', choices=UV_POLICIES, default='surface_axes',
                        help='surface_axes: native base point and texture axes; planar: 128 cm world placeholder')
    parser.add_argument('--texel-scale', type=float, default=TEXEL_SCALE,
                        help='world units per texture repeat for unit axes (UNVERIFIED default %(default)s)')
    args = parser.parse_args()
    if not args.texel_scale > 0:
        parser.error('--texel-scale must be positive')
    uv_policy = ({'policy': 'bsp_surface_axes_v1', 'texel_scale': args.texel_scale,
                  'texel_scale_status': 'UNVERIFIED', 'axes_status': 'corroborated by Polys cross-check'}
                 if args.uv == 'surface_axes' else
                 {'policy': 'planar_world_128cm_approximation'})
    if not (args.game/'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    filename = args.scene/'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    if manifest['schema'] != 1 or manifest['dynamic_policy'] != 'frozen' or manifest['map'] != 'Sanctuary_P':
        parser.error('This observed BSP slice is scoped to frozen Sanctuary')
    scene = Scene(args.reader, args.game, args.scene,
                  include_dlc=manifest.get('package_scope') == 'base_and_dlc')
    scene.materials = {name: dict(m) for name,m in manifest['materials'].items()}
    for m in scene.materials.values(): m.pop('_channel_priority',None)
    original_texture = scene.texture
    def cached_texture(key, channel):
        if key is not None:
            name = scene.filename(key, '_'+channel+'.png')
            if (scene.output/name).is_file(): return name
        return original_texture(key,channel)
    scene.texture = cached_texture
    models = []
    # Validate every selected Model and component before changing scene outputs.
    for level in ('Sanctuary_P','Sanctuary_Land'):
        if level not in manifest['levels']:
            raise ValueError('Required Sanctuary sublevel is absent')
        records = scene.call(level,'--terrain-records',Path(__file__).with_name('terrain-arrays.schema'))
        lookup = {r['index']:r for r in records}
        selected = []
        for r in records:
            if r['class'] not in ('Engine.Model','Engine.ModelComponent'): continue
            try: require_root(r,lookup,r['class'])
            except ValueError: continue  # volume-owned Models are excluded structurally
            selected.append(r)
        roots = [r for r in selected if r['class']=='Engine.Model']
        if len(roots)!=1: raise ValueError('Expected one persistent-level root Model')
        payloads = {r['index']:bytes(r['payload']) for r in scene.call(level,'--payloads',*[r['index'] for r in selected])}
        root = roots[0]
        model = decode_model(payloads[root['index']],root,lookup)
        components = [decode_component(payloads[r['index']],r,lookup,model)
                      for r in selected if r['class']=='Engine.ModelComponent']
        validate_coverage(model,components)
        models.append((level,model,components))
    actors = [a for a in manifest['actors'] if 'bsp' not in a]
    meshes = {k:v for k,v in manifest['meshes'].items() if 'bsp' not in v}
    issues = [i for i in manifest['issues'] if not i.get('bsp')]
    summary = {'models':len(models),'components':0,'polygons':0,'triangles':0,'sections':0}
    probes = []
    for level,model,components in models:
        scene.load(level)
        probes.append({'model':level+':'+model['path'], 'candidates':probe_candidates(model,components,level)})
        for comp in components:
            sections = []
            name = scene.filename((level,comp['index']),'_bsp')
            identity = level+':'+comp['path']
            for slot,element in enumerate(comp['elements']):
                key = scene.resolve(level,element['material'])
                material = scene.material(key)
                if material is None: raise ValueError('BSP material reference did not resolve')
                polygons = [model['polygons'][i] for i in element['nodes']]
                obj,n = section_obj(polygons, args.uv, args.texel_scale)
                file = name+f'_s{slot}.obj'
                (args.scene/file).write_text(obj,encoding='utf-8')
                sections.append({'slot':slot,'file':file,'material':material,
                                 'bsp_nodes':element['nodes'],'triangles':n})
                summary['triangles']+=n
                summary['sections']+=1
            meshes[name] = {'source':identity,'sections':sections,
                            'collision':{'status':'triangle_mesh' if args.collision else 'absent',
                                         'hulls':[], 'source':identity,
                                         'policy':'bsp_polygon_triangle_host_approximation_v1'},
                            'bsp':{'model':level+':'+model['path'],'nodes':comp['nodes'],
                                   'topology':'node_vertex_pool_planes_and_component_membership_v1',
                                   'uv_policy':uv_policy,
                                   'lighting':'UNVERIFIED; host inspection lighting',
                                   'native_collision_flags':'UNVERIFIED',
                                   'opaque_model_remainder':model['opaque_remainder'],
                                   'elements':comp['elements']}}
            actors.append({'source':comp['path'],'level':level,'mesh':name,
                           'transform':{'actor':transform({}),'component':transform({},True)},
                           'materials':[],'static':True,'collision_enabled':bool(args.collision),
                           'native_skybox':False,'hidden_visual':False,'bsp':level+':'+model['path']})
            summary['components']+=1
            summary['polygons']+=len(comp['nodes'])
        issues.append({'object':level+':'+model['path'],'bsp':True,
                       'error':('Approximation: planar 128cm UVs, ' if args.uv == 'planar' else
                                'Native texture axes with an UNVERIFIED texel scale, ')
                               + 'host lighting and opt-in triangle collision; lightmaps/collision flags UNVERIFIED'})
    manifest.update(actors=actors,meshes=meshes,materials=scene.materials,
                    issues=issues+[dict(i,bsp=True) for i in scene.issues],
                    bsp_policy={**summary,'collision':bool(args.collision),'scope':'Sanctuary root Models'},
                    visual_validation='pending')
    temporary = filename.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    temporary.replace(filename)
    (args.scene/'bsp-runtime.json').write_text(json.dumps({'collision':bool(args.collision),'models':probes},indent=2)+'\n')
    print(json.dumps(summary))


if __name__=='__main__':
    main()
