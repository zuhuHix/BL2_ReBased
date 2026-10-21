"""Blocking volumes (UE3 brush volumes) for the host walker.

A BlockingVolume keeps its collision in its BrushComponent's tagged BrushAggGeom
(a KAggregateGeom, the same layout as RB_BodySetup.AggGeom). This module reads it
with the existing property decoder and places it at the actor's Location. Whether
a volume blocks the player comes from the actor's bCollideActors and bBlockActors,
with unset values taken from the class defaults in the installed script package
(Default__<Class>, following its archetype chain). Nothing here decodes the cooked
PhysX data that follows the component's properties.

Observed for Sanctuary (2026-10-21): every volume carries only Location (no
Rotation, DrawScale, DrawScale3D or PrePivot). Those transform fields are refused
rather than guessed, because their composition order for brushes is UNVERIFIED.
Kismet toggles of DynamicBlockingVolume are not interpreted: the record holds the
serialized initial state.
"""
import json
from pathlib import Path

from collision_geometry import fields, point, shapes

SCHEMA = Path(__file__).with_name('collision-arrays.schema')
# Observed object prefixes before the tagged properties: placed actors 26 bytes,
# their components 8, class default objects 4 (the same values ow-package's
# --scene-records uses for actors and components). Not a general UObject layout.
ACTOR_PREFIX, COMPONENT_PREFIX, DEFAULTS_PREFIX = 26, 8, 4
TRANSFORM_FIELDS = ('Rotation', 'DrawScale', 'DrawScale3D', 'PrePivot')
FLAG_FIELDS = ('bCollideActors', 'bBlockActors', 'bBlockCamera', 'bEnabled')


def is_blocking_family(cls):
    return cls.rsplit('.', 1)[-1].endswith('BlockingVolume')


def volume_record(cls, actor, component, defaults):
    """Pure decision for one placed volume from decoded property dicts.

    `actor` and `component` map property names to decoded values; `defaults` holds
    the class-default flags. Raises ValueError for anything not understood.
    """
    for name in TRANSFORM_FIELDS:
        if name in actor:
            raise ValueError('Unsupported volume transform field: ' + name)
    if 'Location' not in actor:
        raise ValueError('Volume without Location')
    flags = {name: actor.get(name, defaults.get(name)) for name in FLAG_FIELDS}
    blocks = bool(flags['bCollideActors']) and bool(flags['bBlockActors'])
    record = {'class': cls, 'location': point(actor['Location']), 'hulls': [], 'blocks_players': blocks,
              'dynamic': cls.rsplit('.', 1)[-1] == 'DynamicBlockingVolume', 'flags': flags}
    if not blocks:
        return record  # e.g. PathBlockingVolume: cooked without a brush component
    geometry = shapes(component.get('_tags', []), 'BrushAggGeom')
    if geometry['spheres'] or geometry['capsules'] or not geometry['hulls']:
        raise ValueError('Volume aggregate is not convex-only')
    record['hulls'] = geometry['hulls']
    return record


class Defaults:
    """Class-default flags read from Default__<Class> in the class's script package."""

    def __init__(self, scene):
        self.scene, self.cache, self.exports = scene, {}, {}

    def _exports(self, package):
        if package not in self.exports:
            self.exports[package] = {e['index']: e for e in self.scene.call(package, '--exports')}
        return self.exports[package]

    def flags(self, cls):
        if cls in self.cache:
            return self.cache[cls]
        package, name = cls.split('.', 1)
        exports = self._exports(package)
        index = next((e['index'] for e in exports.values() if e['path'] == 'Default__' + name), None)
        if index is None:
            raise ValueError('No class defaults for ' + cls)
        chain = []
        while index:
            chain.append(index)
            index = exports[index].get('archetype_index', 0)
            if index < 0 or len(chain) > 32:
                break  # archetypes outside this package (Core.Object) carry none of these flags
        result = {}
        for index in reversed(chain):
            data = self.scene.call(package, '--properties', index, '--property-offset', DEFAULTS_PREFIX)
            result.update({k: v for k, v in fields(data['properties']).items() if k in FLAG_FIELDS})
        self.cache[cls] = result
        return result


def live_observations(dump):
    """{'<level>:TheWorld.PersistentLevel.<name>': state} from walking.py's walk_dump volumes.

    The running game names a placed actor '<level>.TheWorld:PersistentLevel.<name>'.
    """
    result = {}
    for row in dump['volumes']:
        level, _, rest = row['path'].partition('.TheWorld:')
        if not rest:
            continue
        component = row.get('component') or {}
        result[level + ':TheWorld.' + rest] = {
            'collide': bool(row.get('bCollideActors')) and bool(component.get('CollideActors', True)),
            'block': bool(row.get('bBlockActors')) and bool(component.get('BlockActors', True)),
            'block_zero_extent': bool(component.get('BlockZeroExtent')),
            'block_nonzero_extent': bool(component.get('BlockNonZeroExtent', True))}
    return result


def apply_live_state(volumes, observed):
    """Attach live observations ({'<level>:<path>': state}) to the decoded volumes.

    A dynamic volume takes its blocking state from the observation; one the capture does
    not mention stays dynamic_unobserved. A static volume keeps its decoded state; if the
    live state disagrees the volume is marked 'live_mismatch' and left out of the host.
    """
    for volume in volumes:
        state = observed.get(volume['level'] + ':' + volume['source'])
        if state is None or volume['status'] == 'unsupported':
            continue
        volume['live_state'] = state
        on = bool(state['collide']) and bool(state['block']) and bool(state.get('block_nonzero_extent', True))
        if volume['status'] == 'dynamic_unobserved':
            volume['status'] = 'blocking' if on else 'non_blocking'
        elif on != (volume['status'] == 'blocking'):
            volume['status'] = 'live_mismatch'
    return volumes


def collect(scene, levels):
    """All blocking-family volumes in the given levels: (records, counts)."""
    defaults, records = Defaults(scene), []
    counts = {'blocking': 0, 'dynamic_unobserved': 0, 'non_blocking': 0, 'live_mismatch': 0, 'unsupported': 0, 'hulls': 0}
    for level in levels:
        for record in scene.load(level).values():
            if not is_blocking_family(record['class']) or not record['path'].startswith('TheWorld.PersistentLevel.'):
                continue
            entry = {'source': record['path'], 'level': level, 'class': record['class']}
            try:
                actor_data = scene.call(level, '--properties', record['index'], '--property-offset', ACTOR_PREFIX,
                                        '--array-schema', SCHEMA)
                if actor_data['trailing_bytes']:
                    raise ValueError('Volume actor has trailing bytes')
                actor = fields(actor_data['properties'])
                ref = actor.get('CollisionComponent') or actor.get('BrushComponent')
                component = {'_tags': []}
                if ref and ref.get('index', 0) > 0:
                    component_data = scene.call(level, '--properties', ref['index'], '--property-offset',
                                                COMPONENT_PREFIX, '--array-schema', SCHEMA)
                    component = dict(fields(component_data['properties']), _tags=component_data['properties'])
                entry.update(volume_record(record['class'], actor, component, defaults.flags(record['class'])))
                # Kismet may toggle a DynamicBlockingVolume before the player sees it; keep it
                # out of the host until its state is observed in the running game (live_state).
                entry['status'] = ('non_blocking' if not entry['blocks_players'] else
                                   'dynamic_unobserved' if entry['dynamic'] else 'blocking')
            except (ValueError, KeyError) as error:
                entry.update(status='unsupported', reason=str(error), hulls=[])
            counts[entry['status']] += 1
            counts['hulls'] += len(entry['hulls'])
            records.append(entry)
    return records, counts


if __name__ == '__main__':
    import argparse
    from prepare_level import Scene
    parser = argparse.ArgumentParser(description='List blocking-family volumes of a map (JSON to stdout).')
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', default='Sanctuary_P')
    args = parser.parse_args()
    scene = Scene(args.reader, args.game, Path('local/collision/scratch'))
    volumes, counts = collect(scene, scene.levels(args.map))
    print(json.dumps({'counts': counts, 'volumes': volumes}, indent=1))
