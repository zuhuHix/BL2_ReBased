"""Report diffuse gaps and their placed sections without modifying a scene."""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path


def audit(scene):
    if scene['schema'] != 1 or scene['dynamic_policy'] != 'frozen':
        raise ValueError('Unsupported scene schema/policy')
    uses = defaultdict(list)
    for actor in scene['actors']:
        mesh = scene['meshes'][actor['mesh']]
        for section in mesh['sections']:
            slot = section['slot']
            overrides = actor['materials']
            material = overrides[slot] if slot < len(overrides) and overrides[slot] else section['material']
            uses[material].append({'level': actor['level'], 'component': actor['source'],
                                   'mesh': mesh['source'], 'slot': slot})
    gaps = []
    counts = Counter()
    for name, material in scene['materials'].items():
        if material['channels'].get('diffuse'):
            counts['textured_diffuse'] += 1
            continue
        if 'constant_diffuse' in material:
            counts['constant_diffuse'] += 1
            continue
        blend = material.get('blend_mode', 'BLEND_Opaque')
        neutral = not any(material['channels'].values())
        counts['missing_diffuse'] += 1
        if neutral:
            counts['no_channels'] += 1
        if blend == 'BLEND_Opaque':
            counts['opaque_missing_diffuse'] += 1
            counts['opaque_missing_diffuse_placed_sections'] += len(uses[name])
            if neutral:
                counts['opaque_no_channels'] += 1
                counts['opaque_no_channels_placed_sections'] += len(uses[name])
        resource = material.get('cooked_texture_resource', {})
        source = material['source']
        gaps.append({'id': name, 'source': source, 'blend_mode': blend,
                     'channels': material['channels'], 'no_supported_channels': neutral,
                     'placed_sections': len(uses[name]), 'placements': uses[name],
                     'cooked_resource': resource,
                     'issues': [issue for issue in scene['issues']
                                if issue['object'] == source or issue['object'].startswith(source + ':')]})
    gaps.sort(key=lambda gap: (-gap['placed_sections'], gap['source']))
    return {'schema': 1, 'map': scene['map'], 'materials': len(scene['materials']),
            'placed_sections': sum(map(len, uses.values())), 'counts': dict(counts),
            'unassigned_placed_sections': len(uses[None]),
            'unassigned_placements': uses[None], 'gaps': gaps,
            'note': 'Counts describe manifest coverage, not visual fidelity or shader reconstruction.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--output', type=Path, help='Optional local JSON report')
    args = parser.parse_args()
    report = audit(json.loads((args.scene / 'scene.json').read_text(encoding='utf-8')))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items()
                      if k not in ('gaps', 'unassigned_placements')}, indent=2))
    for gap in report['gaps']:
        if gap['blend_mode'] == 'BLEND_Opaque':
            print(f"{gap['placed_sections']:4} sections | {gap['source']} | channels={','.join(gap['channels']) or 'none'}")


if __name__ == '__main__':
    main()
