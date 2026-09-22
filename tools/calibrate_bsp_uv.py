"""Sweep BSP texel scale and V orientation from matched local captures.

The package contains the surface Base, TextureU and TextureV axes, but it does
not contain the world-units-per-texture-repeat divisor. UModel/UE Viewer does
not export BSP surfaces either. This tool therefore accepts human-measured
matched original-game and UE5 captures, checks their shared world/screen
anchors, and keeps scale/orientation UNVERIFIED unless one candidate is
supported by two independent measurements or surface identities.

The input and output are local evidence only. They must stay under ignored
``local/`` or UE ``Saved/`` directories; this source file never reads or emits
game bytes.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path


SCHEMA = 1
ORIENTATIONS = ('same', 'flipped')
DEFAULT_SCALES = (16.0, 32.0, 48.0, 64.0, 96.0, 128.0, 192.0,
                  256.0, 384.0, 512.0, 1024.0)
DEFAULT_REPEAT_TOLERANCE = 0.15
MIN_INDEPENDENT = 2


def _finite_number(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError('%s must be a number' % label)
    if not math.isfinite(float(value)):
        raise ValueError('%s must be finite' % label)
    return float(value)


def _vector(value, label, length=3):
    if not isinstance(value, list) or len(value) != length:
        raise ValueError('%s must have %d numbers' % (label, length))
    return tuple(_finite_number(v, '%s[%d]' % (label, i)) for i, v in enumerate(value))


def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def _capture_path(value, base, label):
    if not isinstance(value, str) or not value.strip():
        raise ValueError('%s capture path is required' % label)
    path = Path(value)
    if not path.is_absolute():
        path = base / path
    return str(path), path.is_file()


def _surface_key(surface):
    required = ('level', 'model', 'component', 'node', 'material')
    if not isinstance(surface, dict):
        raise ValueError('surface identity is required')
    missing = [k for k in required if k not in surface]
    if missing:
        raise ValueError('surface identity missing: ' + ', '.join(missing))
    return '|'.join(str(surface[k]) for k in required)


def _world_anchors(sample):
    """The two anchors are the same physical points in both views."""
    anchors = sample.get('anchors')
    if not isinstance(anchors, dict):
        raise ValueError('anchors are required')
    world = []
    for label in ('a', 'b'):
        anchor = anchors.get(label)
        if not isinstance(anchor, dict):
            raise ValueError('anchor %s is required' % label)
        world.append(_vector(anchor.get('world_cm'), 'anchors.%s.world_cm' % label))
    delta = tuple(world[1][i] - world[0][i] for i in range(3))
    if sum(v * v for v in delta) <= 1e-8:
        raise ValueError('world anchors must differ')
    return world[0], world[1]


def _pixel_delta(sample, pixel_key, capture_name):
    """Where those anchors landed in one specific capture."""
    anchors = sample['anchors']
    pixels = []
    for label in ('a', 'b'):
        anchor = anchors[label]
        if pixel_key not in anchor:
            raise ValueError('anchors.%s.%s is required; the %s view must be '
                             'measured, not assumed from a capture path'
                             % (label, pixel_key, capture_name))
        pixels.append(_vector(anchor.get(pixel_key),
                              'anchors.%s.%s' % (label, pixel_key), 2))
    delta = tuple(pixels[1][i] - pixels[0][i] for i in range(2))
    if sum(v * v for v in delta) <= 1e-8:
        raise ValueError('%s screen anchors must differ' % capture_name)
    return delta


def _scene_inventory(scene_path, runtime_path=None):
    """Return section identities from an already-prepared ignored scene."""
    if not scene_path:
        return []
    scene_path = Path(scene_path)
    scene = json.loads(scene_path.read_text(encoding='utf-8'))
    materials = scene.get('materials', {})
    result = []
    for mesh_name, mesh in scene.get('meshes', {}).items():
        if not mesh.get('bsp'):
            continue
        bsp = mesh.get('bsp', {})
        for section in mesh.get('sections', []):
            material_key = section.get('material')
            material = materials.get(material_key, {})
            result.append({
                'mesh': mesh_name,
                'source': mesh.get('source'),
                'model': bsp.get('model'),
                'slot': section.get('slot'),
                'material': material_key,
                'material_source': material.get('source'),
                'material_channels': sorted(material.get('channels', {})),
                'nodes': section.get('bsp_nodes', []),
                'identity_status': 'observed scene manifest identity',
            })
    if runtime_path:
        runtime = json.loads(Path(runtime_path).read_text(encoding='utf-8'))
        for model in runtime.get('models', []):
            for probe in model.get('candidates', []):
                level = probe.get('level')
                component = probe.get('source')
                match = next((entry for entry in result
                              if entry.get('source') == '%s:%s' % (level, component)
                              and probe.get('node') in entry.get('nodes', [])), None)
                if match is not None:
                    match['host_probe'] = {
                        'node': probe.get('node'),
                        'start_cm': probe.get('start'),
                        'end_cm': probe.get('end'),
                        'clearance_cm': probe.get('clearance'),
                        'status': 'host collision candidate; not original-game evidence',
                    }
    return result


def _tiled_candidates(inventory):
    """Select source-named tile candidates without asserting visual tiling."""
    candidates = []
    for entry in inventory:
        source = entry.get('material_source') or ''
        if 'TilingMaterials' not in source:
            continue
        candidates.append({**entry,
                           'selection_basis': 'source path contains TilingMaterials',
                           'selection_status': 'candidate; repeat density remains UNVERIFIED'})
    return candidates


def _validate_sample(raw, base, index):
    if not isinstance(raw, dict):
        raise ValueError('measurement %d must be an object' % index)
    sample_id = raw.get('id')
    if not isinstance(sample_id, str) or not sample_id.strip():
        raise ValueError('measurement %d id is required' % index)
    surface = raw.get('surface')
    surface_key = _surface_key(surface)
    axis = raw.get('axis')
    if axis not in ('u', 'v'):
        raise ValueError('%s axis must be u or v' % sample_id)
    axis_vector = _vector(raw.get('axis_vector'), '%s axis_vector' % sample_id)
    if _dot(axis_vector, axis_vector) <= 1e-12:
        raise ValueError('%s axis_vector must be nonzero' % sample_id)
    captures = raw.get('captures')
    if not isinstance(captures, dict):
        raise ValueError('%s captures object is required' % sample_id)
    original_capture, original_exists = _capture_path(
        captures.get('original_game'), base,
        '%s original-game' % sample_id)
    ue5_capture, ue5_exists = _capture_path(
        captures.get('ue5'), base, '%s UE5' % sample_id)
    # The world anchors are shared by both views, so they are read once. Each
    # view then carries its own pixel anchors: that is what distinguishes a
    # measured matched view from a capture path that merely exists.
    anchor_a, anchor_b = _world_anchors(raw)
    original_pixels = _pixel_delta(raw, 'pixel_original_game', 'original-game')
    ue5_pixels = _pixel_delta(raw, 'pixel_ue5', 'UE5')
    world_delta = tuple(anchor_b[i] - anchor_a[i] for i in range(3))
    projected_distance = abs(_dot(world_delta, axis_vector))
    if projected_distance <= 1e-8:
        raise ValueError('%s anchors have no projected distance on %s axis' % (sample_id, axis))
    group = raw.get('independence_group', surface_key)
    if not isinstance(group, str) or not group.strip():
        raise ValueError('%s independence_group is required' % sample_id)
    valid = original_exists and ue5_exists
    issues = []
    if not original_exists:
        issues.append('original-game capture is missing: ' + original_capture)
    if not ue5_exists:
        issues.append('UE5 capture is missing: ' + ue5_capture)
    sample = {
        'id': sample_id,
        'surface': surface,
        'surface_key': surface_key,
        'independence_group': group,
        'axis': axis,
        'axis_vector': list(axis_vector),
        'projected_axis_distance_cm': projected_distance,
        'captures': {'original_game': original_capture, 'ue5': ue5_capture},
        'capture_status': 'available' if valid else 'missing',
        'anchors': {
            'world_cm': [list(anchor_a), list(anchor_b)],
            'original_game_pixel_delta': list(original_pixels),
            'ue5_pixel_delta': list(ue5_pixels),
        },
        'issues': issues,
    }
    if 'original_repeat_count' in raw:
        repeats = _finite_number(raw['original_repeat_count'], '%s original_repeat_count' % sample_id)
        if repeats <= 0:
            raise ValueError('%s original_repeat_count must be positive' % sample_id)
        sample['original_repeat_count'] = repeats
    orientation_valid = valid
    if 'original_progress_sign' in raw:
        sign = _finite_number(raw['original_progress_sign'], '%s original_progress_sign' % sample_id)
        if sign not in (-1.0, 1.0):
            raise ValueError('%s original_progress_sign must be -1 or 1' % sample_id)
        sample['original_progress_sign'] = int(sign)
        host_signs = raw.get('host_progress_sign_by_orientation')
        if not isinstance(host_signs, dict):
            orientation_valid = False
            issues.append('%s host_progress_sign_by_orientation is required' % sample_id)
            host_signs = {}
        sample['host_progress_sign_by_orientation'] = {}
        for orientation in ORIENTATIONS:
            try:
                sign = _finite_number(host_signs.get(orientation),
                                      '%s host %s progress sign' % (sample_id, orientation))
                if sign not in (-1.0, 1.0):
                    raise ValueError('must be -1 or 1')
                sample['host_progress_sign_by_orientation'][orientation] = int(sign)
            except ValueError as error:
                orientation_valid = False
                issues.append('%s host %s progress sign %s' % (sample_id, orientation, error))
    sample['valid_for_calibration'] = valid
    sample['orientation_valid'] = orientation_valid and 'original_progress_sign' in sample
    return sample


def _candidate_scales(samples, scales, tolerance):
    density = [s for s in samples if s.get('valid_for_calibration') and
               'original_repeat_count' in s]
    candidates = []
    for scale in scales:
        matches = []
        for sample in density:
            predicted = sample['projected_axis_distance_cm'] / scale
            error = abs(predicted - sample['original_repeat_count'])
            if error <= tolerance:
                matches.append({'id': sample['id'], 'surface_key': sample['surface_key'],
                                'independence_group': sample['independence_group'],
                                'predicted_repeats': predicted,
                                'original_repeats': sample['original_repeat_count'],
                                'absolute_error': error})
        groups = sorted({m['independence_group'] for m in matches})
        candidates.append({'texel_scale': scale, 'matches': matches,
                           'independent_groups': groups,
                           'independent_count': len(groups),
                           'accepted': len(groups) >= MIN_INDEPENDENT})
    accepted = [c for c in candidates if c['accepted']]
    return density, candidates, accepted


def _candidate_orientations(samples):
    orientation_samples = [s for s in samples if s.get('orientation_valid') and
                           s.get('axis') == 'v' and 'original_progress_sign' in s]
    candidates = []
    for orientation in ORIENTATIONS:
        matches = []
        for sample in orientation_samples:
            expected = sample['original_progress_sign']
            observed = sample['host_progress_sign_by_orientation'][orientation]
            if observed == expected:
                matches.append({'id': sample['id'], 'surface_key': sample['surface_key'],
                                'independence_group': sample['independence_group'],
                                'original_progress_sign': expected,
                                'host_progress_sign': observed})
        groups = sorted({m['independence_group'] for m in matches})
        candidates.append({'v_orientation': orientation, 'matches': matches,
                           'independent_groups': groups,
                           'independent_count': len(groups),
                           'accepted': len(groups) >= MIN_INDEPENDENT})
    accepted = [c for c in candidates if c['accepted']]
    return orientation_samples, candidates, accepted


def analyze(data, base_dir='.', scales=DEFAULT_SCALES,
            repeat_tolerance=DEFAULT_REPEAT_TOLERANCE, scene_path=None,
            runtime_path=None):
    """Return a calibration report without mutating a scene or package."""
    if not isinstance(data, dict):
        raise ValueError('calibration input must be an object')
    if data.get('schema', SCHEMA) != SCHEMA:
        raise ValueError('unsupported calibration schema')
    base = Path(base_dir)
    raw_measurements = data.get('measurements', [])
    if not isinstance(raw_measurements, list):
        raise ValueError('measurements must be an array')
    samples, invalid = [], []
    for index, raw in enumerate(raw_measurements):
        try:
            samples.append(_validate_sample(raw, base, index))
        except ValueError as error:
            invalid.append({'index': index, 'id': raw.get('id') if isinstance(raw, dict) else None,
                            'error': str(error)})
    normalized_scales = sorted({_finite_number(s, 'texel scale') for s in scales if _finite_number(s, 'texel scale') > 0})
    if not normalized_scales:
        raise ValueError('at least one positive texel scale candidate is required')
    density, scale_candidates, accepted_scales = _candidate_scales(
        samples, normalized_scales, repeat_tolerance)
    orientations, orientation_candidates, accepted_orientations = _candidate_orientations(samples)
    missing = []
    if not density:
        missing.append('Two independent matched captures with original repeat counts are required for texel scale.')
    if density and not accepted_scales:
        missing.append('No texel-scale candidate is supported by two independent measurements or surfaces.')
    if len(accepted_scales) > 1:
        missing.append('More than one texel-scale candidate meets the acceptance tolerance.')
    if not orientations:
        missing.append('Two independent matched V-orientation measurements are required.')
    if orientations and not accepted_orientations:
        missing.append('Neither V orientation candidate is supported by two independent measurements or surfaces.')
    if len(accepted_orientations) > 1:
        missing.append('Both V orientation candidates meet the acceptance gate; evidence is ambiguous.')
    if invalid:
        missing.append('One or more measurements are invalid or lack capture files; inspect invalid_measurements.')
    verified_scale = accepted_scales[0]['texel_scale'] if len(accepted_scales) == 1 else None
    verified_orientation = (accepted_orientations[0]['v_orientation']
                            if len(accepted_orientations) == 1 else None)
    inventory = _scene_inventory(scene_path, runtime_path)
    if not inventory:
        inventory = [{'surface': s['surface'], 'surface_key': s['surface_key'],
                      'identity_status': 'measurement supplied identity'} for s in samples]
    report = {
        'schema': SCHEMA,
        'status': 'VERIFIED' if verified_scale is not None and verified_orientation is not None else 'UNVERIFIED',
        'evidence_policy': {
            'minimum_independent_measurements_or_surfaces': MIN_INDEPENDENT,
            'repeat_tolerance': repeat_tolerance,
            'capture_requirement': 'original-game and UE5 files with shared world anchors and nonzero pixel anchor deltas',
        },
        'provenance': data.get('provenance', {}),
        'scene': str(scene_path) if scene_path else None,
        'runtime': str(runtime_path) if runtime_path else None,
        'surface_inventory': inventory,
        'tiled_candidates': _tiled_candidates(inventory),
        'scale': {
            'status': 'VERIFIED' if verified_scale is not None else 'UNVERIFIED',
            'texel_scale': verified_scale,
            'measurements_considered': [s['id'] for s in density],
            'candidates': scale_candidates,
        },
        'v_orientation': {
            'status': 'VERIFIED' if verified_orientation is not None else 'UNVERIFIED',
            'v_orientation': verified_orientation,
            'measurements_considered': [s['id'] for s in orientations],
            'candidates': orientation_candidates,
        },
        'measurements': samples,
        'invalid_measurements': invalid,
        'missing_evidence': sorted(set(missing)),
        'external_oracle': {
            'tool': 'UModel / UE Viewer',
            'status': 'UNVERIFIED for BSP surfaces; tool does not export BSP ModelComponents',
            'provenance_required': True,
        },
    }
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--measurements', type=Path,
                        help='JSON file with matched capture measurements; omitted means no evidence')
    parser.add_argument('--scene', type=Path,
                        help='prepared ignored scene.json for BSP section identity inventory')
    parser.add_argument('--runtime', type=Path,
                        help='prepared ignored bsp-runtime.json for host candidate anchors')
    parser.add_argument('--ue5-capture', action='append', default=[],
                        help='existing UE5 baseline capture to record as provenance (repeatable)')
    parser.add_argument('--output', type=Path, required=True,
                        help='ignored local report path')
    parser.add_argument('--scales', nargs='+', type=float, default=DEFAULT_SCALES,
                        help='independent texel-scale candidates in world units per repeat')
    parser.add_argument('--repeat-tolerance', type=float, default=DEFAULT_REPEAT_TOLERANCE,
                        help='absolute repeat-count tolerance for each scale candidate')
    parser.add_argument('--require-verified', action='store_true',
                        help='return nonzero unless both scale and orientation verify')
    args = parser.parse_args()
    if args.repeat_tolerance < 0 or not math.isfinite(args.repeat_tolerance):
        parser.error('--repeat-tolerance must be finite and nonnegative')
    if args.measurements:
        data = json.loads(args.measurements.read_text(encoding='utf-8'))
        base = args.measurements.parent
    else:
        data = {'schema': SCHEMA, 'measurements': [],
                'provenance': {'status': 'UNVERIFIED',
                               'note': 'No matched original-game and UE5 captures were supplied.'}}
        base = Path('.')
    if args.ue5_capture:
        provenance = dict(data.get('provenance', {}))
        provenance['ue5_host_captures'] = [
            {'path': str(Path(path)), 'exists': Path(path).is_file(),
             'status': 'UE5 host evidence only; no original-game match'}
            for path in args.ue5_capture]
        data = {**data, 'provenance': provenance}
    report = analyze(data, base, args.scales, args.repeat_tolerance, args.scene, args.runtime)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'status': report['status'],
                      'scale': report['scale']['status'],
                      'v_orientation': report['v_orientation']['status'],
                      'measurements': len(report['measurements']),
                      'invalid_measurements': len(report['invalid_measurements']),
                      'missing_evidence': report['missing_evidence']}, indent=2))
    return 0 if report['status'] == 'VERIFIED' or not args.require_verified else 2


if __name__ == '__main__':
    raise SystemExit(main())
