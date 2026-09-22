"""Report material and placement gaps without modifying a scene.

The report is deliberately manifest-only. It prioritizes visible placed
sections, keeps partial approximations separate from missing diffuse, and
groups importer issues so the next fix can target materials, components,
collision, or vertex data independently.
"""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path


def issue_category(issue):
    """Return a stable repair bucket for a recorded scene issue."""
    error = str(issue.get('error', ''))
    context = str(issue.get('object', ''))
    lowered = error.casefold()
    if error == 'Unsupported component owner':
        return 'unsupported_component_owner'
    if 'color stream' in lowered:
        return 'invalid_color_stream'
    if ':collision' in context.casefold() or 'collision' in lowered:
        return 'collision'
    if error.startswith('Approximation:'):
        return 'approximation'
    if 'neutral fallback' in lowered:
        return 'material_fallback'
    return 'other'


def gap_status(material):
    """Explain what kind of follow-up a diffuse gap needs."""
    channels = material.get('channels', {})
    if material.get('surface_approximation'):
        return 'partial_surface_approximation'
    resource = material.get('cooked_texture_resource')
    if isinstance(resource, dict) and resource.get('textures'):
        return 'cooked_resource_candidates'
    if any(value for channel, value in channels.items() if channel != 'diffuse'):
        return 'partial_channels'
    if material.get('blend_mode', 'BLEND_Opaque') != 'BLEND_Opaque':
        return 'non_opaque'
    if not any(channels.values()):
        return 'no_supported_channels'
    return 'missing_diffuse'


def section_policy(section):
    """Return a validated section disposition, if the manifest records one."""
    policy = section.get('material_policy')
    if not isinstance(policy, dict):
        return None
    if policy.get('version') != 'sanctuary_unassigned_sections_v1':
        raise ValueError('Unsupported section material policy version')
    method = policy.get('method')
    if method not in {
            'observed_companion_material_v1',
            'same_mesh_observed_material_v1',
            'host_neutral_fallback_v1',
            'helper_hidden_collision_only_v1'}:
        raise ValueError('Unsupported section material policy method')
    return policy


def audit(scene):
    if scene['schema'] != 1 or scene['dynamic_policy'] != 'frozen':
        raise ValueError('Unsupported scene schema/policy')
    uses = defaultdict(list)
    section_audit = []
    section_policy_counts = Counter()
    unassigned_placements = []
    native_null_sections = 0
    for actor in scene['actors']:
        mesh = scene['meshes'][actor['mesh']]
        for section in mesh['sections']:
            slot = section['slot']
            overrides = actor['materials']
            override = overrides[slot] if slot < len(overrides) and overrides[slot] else None
            material = override or section['material']
            policy = section_policy(section)
            method = policy.get('method') if policy else None
            if (policy and policy.get('native_material_index') == 0) or section['material'] is None:
                native_null_sections += 1
            if method in ('observed_companion_material_v1',
                          'same_mesh_observed_material_v1'):
                if override is None:
                    section_policy_counts[method] += 1
            elif method in ('host_neutral_fallback_v1',
                            'helper_hidden_collision_only_v1') and material is None and override is None:
                section_policy_counts[method] += 1
            uses[material].append({'level': actor['level'], 'component': actor['source'],
                                   'mesh': mesh['source'], 'slot': slot})
            definition = scene['materials'].get(material)
            scope = 'terrain' if actor.get('terrain') else 'bsp' if actor.get('bsp') else 'building_or_prop'
            if material is not None and definition is None:
                cause, disposition = 'unresolved_material_binding', 'repair_required'
            elif (material is None and method == 'helper_hidden_collision_only_v1'
                  and actor.get('hidden_visual', False)):
                cause, disposition = 'intentional_helper_hidden', 'hide_visual_keep_observed_collision'
            elif (material is None and method == 'host_neutral_fallback_v1'
                  and override is None):
                cause, disposition = 'explicit_neutral_material_fallback', 'explicit_importer_neutral_fallback_rgb_0.5'
            elif material is None:
                cause, disposition = 'unassigned_material', 'repair_required'
                unassigned_placements.append({'level': actor['level'], 'component': actor['source'],
                                              'mesh': mesh['source'], 'slot': slot})
            elif (method in ('observed_companion_material_v1',
                             'same_mesh_observed_material_v1')
                  and override is None):
                cause, disposition = 'explicit_material_fallback', 'explicit_manifest_material_fallback_partial_unverified'
            elif definition.get('channels', {}).get('diffuse'):
                cause, disposition = 'recovered_diffuse', 'inspect_native_graph_parity'
            elif 'constant_diffuse' in definition:
                cause, disposition = 'constant_diffuse', 'explicit_manifest_color'
            else:
                cause, disposition = gap_status(definition), 'explicit_importer_neutral_fallback_rgb_0.5'
            section_audit.append({
                'level': actor['level'], 'component': actor['source'],
                'mesh': mesh['source'], 'slot': slot, 'scope': scope,
                'material_id': material, 'material': (definition or {}).get('source'),
                'cause': cause, 'disposition': disposition,
                'classification': ('unresolved material binding' if cause == 'unresolved_material_binding'
                                   else 'intentional hidden collision helper' if cause == 'intentional_helper_hidden'
                                   else 'present geometry with explicit host fallback' if cause == 'explicit_neutral_material_fallback'
                                   else 'present geometry with explicit material fallback' if cause == 'explicit_material_fallback'
                                   else 'separate building/prop material issue' if scope == 'building_or_prop' and cause != 'recovered_diffuse'
                                   else 'present geometry with incomplete material' if cause != 'recovered_diffuse'
                                   else 'present geometry with recovered diffuse'),
                'geometry_file': section.get('file'),
                'material_policy': policy,
                'policy_applied': bool(policy and override is None),
                'native_material_index': (policy or {}).get('native_material_index'),
                'hidden_visual': actor.get('hidden_visual', False),
                'evidence': 'scene manifest binding; rendered appearance UNVERIFIED',
                'visual_status': 'UNVERIFIED'})
    gaps = []
    partial_surfaces = []
    counts = Counter()
    gap_status_counts = Counter()
    for name, material in scene['materials'].items():
        if 'surface_approximation' in material:
            partial_surfaces.append({'source': material['source'],
                                     'placed_sections': len(uses[name]),
                                     'approximation': material['surface_approximation']})
            counts['partial_surface_approximations'] += 1
            counts['partial_surface_approximation_placed_sections'] += len(uses[name])
            gap_status_counts['partial_surface_approximation'] += 1
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
        status = gap_status(material)
        gap_status_counts[status] += 1
        gaps.append({'id': name, 'source': source, 'blend_mode': blend,
                     'status': status,
                     'channels': material['channels'], 'no_supported_channels': neutral,
                     'placed_sections': len(uses[name]), 'placements': uses[name],
                     'cooked_resource': resource,
                     'issues': [issue for issue in scene['issues']
                                if issue['object'] == source or issue['object'].startswith(source + ':')]})
    gaps.sort(key=lambda gap: (-gap['placed_sections'], gap['source']))
    issue_counts = Counter()
    issue_examples = defaultdict(list)
    for issue in scene.get('issues', []):
        category = issue_category(issue)
        issue_counts[category] += 1
        if len(issue_examples[category]) < 8:
            issue_examples[category].append(issue)
    return {'schema': 1, 'map': scene['map'], 'materials': len(scene['materials']),
            'placed_sections': sum(map(len, uses.values())), 'counts': dict(counts),
            'gap_status_counts': dict(sorted(gap_status_counts.items())),
            'issue_counts': dict(sorted(issue_counts.items())),
            'issue_examples': dict(sorted(issue_examples.items())),
            'section_policy_counts': dict(sorted(section_policy_counts.items())),
            'resolved_policy_sections': sum(section_policy_counts.values()),
            'native_null_sections': native_null_sections,
            'unassigned_placed_sections': len(unassigned_placements),
            'unassigned_placements': unassigned_placements, 'gaps': gaps,
            'partial_surfaces': partial_surfaces,
            'section_audit': section_audit,
            'note': 'Counts describe manifest coverage, not visual fidelity or shader reconstruction.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--output', type=Path, help='Optional local JSON report')
    parser.add_argument('--all-gaps', action='store_true',
                        help='Print non-opaque diffuse gaps as well as opaque gaps')
    args = parser.parse_args()
    report = audit(json.loads((args.scene / 'scene.json').read_text(encoding='utf-8')))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items()
                      if k not in ('gaps', 'unassigned_placements', 'partial_surfaces',
                                   'issue_examples', 'section_audit')}, indent=2))
    if report['unassigned_placed_sections']:
        print(f"Unassigned placed sections: {report['unassigned_placed_sections']}")
    for category, count in report['issue_counts'].items():
        print(f"Issues | {category}: {count}")
    for gap in report['gaps']:
        if gap['blend_mode'] == 'BLEND_Opaque' or args.all_gaps:
            print(f"{gap['placed_sections']:4} sections | {gap['status']} | {gap['source']} | channels={','.join(gap['channels']) or 'none'}")


if __name__ == '__main__':
    main()
