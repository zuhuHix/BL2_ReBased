"""Trace Sanctuary ice, transition and HLS placements using existing readers.

Writes game-derived evidence only to a caller-selected local output. Class
counts establish omitted terrain/BSP objects, not their spatial coverage.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
from prepare_level import Scene, props


def audit(scene, reader):
    groups = {'ice': [], 'transition': [], 'hls': []}
    for actor in scene['actors']:
        mesh = scene['meshes'][actor['mesh']]
        for section in mesh['sections']:
            slot = section['slot']
            overrides = actor['materials']
            name = overrides[slot] if slot < len(overrides) and overrides[slot] else section['material']
            material = scene['materials'].get(name, {})
            source = material.get('source', '')
            group = ('ice' if mesh['source'].endswith(':Prop_Glacier.Meshes.IcePlate') else
                     'transition' if source.endswith(':Common_Materials.Environment.WorldTransition') else
                     'hls' if source.endswith(':Prop_SancBuildings.Optimization.Mati_SancBuild4a') else None)
            if group:
                groups[group].append({'level': actor['level'], 'component': actor['source'],
                                      'mesh': mesh['source'], 'slot': slot,
                                      'transform': actor['transform'],
                                      'hidden_visual': actor.get('hidden_visual', False),
                                      'collision_enabled': actor['collision_enabled'],
                                      'collision_status': mesh.get('collision', {}).get('status'),
                                      'material': material})
    omitted = {}
    evidence = {}
    for level in scene['levels']:
        records = reader.load(level)
        counts = Counter(row['class'].rsplit('.', 1)[-1] for row in records.values())
        omitted[level] = {kind: counts[kind] for kind in
                          ('Terrain', 'TerrainComponent', 'Model', 'ModelComponent')}
        for row in records.values():
            if row['path'] in {
                'Prop_Glacier.Meshes.IcePlate', 'Prop_Glacier.Materials.Mat_FrozenLake',
                'Common_Materials.Environment.WorldTransition',
                'Prop_SancBuildings.Optimization.Mati_SancBuild4a',
                'Prop_SancBuildings.Optimization.Sanc_HLS_Master',
                'Prop_SancBuildings.Material.Mati_SancBuild4a'}:
                evidence[level + ':' + row['path']] = {
                    'index': row['index'], 'class': row['class'], 'properties': props(row)}
    return {'map': scene['map'], 'counts': {k: len(v) for k, v in groups.items()},
            'placements': groups, 'source_evidence': evidence,
            'unimported_class_counts': omitted,
            'limitations': ['Terrain/BSP spatial coverage is unverified; volume Models are not necessarily floors.',
                            'Frozen sublevel activation is not native Kismet behavior.',
                            'Texture fallbacks do not reconstruct native material graphs.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', required=True, type=Path)
    parser.add_argument('--game', required=True, type=Path)
    parser.add_argument('--scene', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    scene = json.loads((args.scene / 'scene.json').read_text(encoding='utf-8'))
    if scene.get('schema') != 1 or scene.get('map') != 'Sanctuary_P':
        parser.error('Expected a schema 1 Sanctuary_P scene')
    reader = Scene(args.reader, args.game, args.output.parent)
    result = audit(scene, reader)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'counts': result['counts'], 'unimported_class_counts': result['unimported_class_counts']}))


if __name__ == '__main__':
    main()
