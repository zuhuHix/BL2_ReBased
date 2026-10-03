"""Collect the numbers the host needs to draw Phaselock's stock presentation (hand orb, tattoo glow, bubble, light).

AI-assisted (Claude). Reads installed data through `ow-package --properties` and writes the ignored manifest
`local/phaselock/fx_manifest.json` (format `openwillow.phaselock_fx/1`). Nothing game-derived is tracked. The particle
templates themselves are decoded separately by `research/particle_system.py` (default output
`local/phaselock/emitters/`); this manifest only names them.

What is read, and from where (record: docs/verification/PHASELOCK_STOCK_DATA.md, "Host presentation pass"):
- `LiftActionSkill` settings: the archetype's own value, else the `Default__LiftActionSkill` class default
  (first-person hand-effect socket, translation and scale; bubble draw-scale divisor, intro and outro times,
  collapse duration and maximum; parameter names; the four particle templates).
- The light: the archetype's `PhaselockLight` component carries only a lightmap GUID, so radius, brightness, colour,
  falloff and shadow flags come from the class default component it is based on.
- The arms socket named by `FirstPersonAttachmentName` on the first-person arms mesh (bone, location, rotation).
- The first-person cast clips' notifies (`Phase_Lock_Lift`, `Phase_Lock_Fail` in the arms AnimSet): time and the
  custom event they fire.
- `Skill_Phaselock`'s coordinated effect (from the action-skill manifest): its material scalar curve, kept in stored
  key order, and its duration; and the arms material's vector parameters whose names contain the curve's
  parameter stem (`PowerEmissive`), which the host reads as the glow colour (a name pairing, UNVERIFIED).
- The screen particle shown on `OnSelectedTarget` and hidden by `SPA_Hide` (action-skill manifest behaviors).

Usage:
    python tools/prepare_phaselock_fx.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import prepare_skill_tree  # noqa: E402

FORMAT = 'openwillow.phaselock_fx/1'
SCHEMA_LINES = [
    'Notifies=StructProperty:AnimNotifyEvent',
    'Behaviors=ObjectProperty',
    'MaterialScalarParameters=StructProperty:CoordinatedScalarParameter',
    'MaterialVectorParameters=StructProperty:CoordinatedVectorParameter',
    'Points=StructProperty:InterpCurvePointFloat',
    'ScalarParameterValues=StructProperty:ScalarParameterValue',
    'VectorParameterValues=StructProperty:VectorParameterValue',
    'TextureParameterValues=StructProperty:TextureParameterValue',
    'Sequences=ObjectProperty',
]
# UE3 rotators are stored in 65536ths of a turn.
ROTATOR_UNITS_PER_TURN = 65536.0
SETTINGS = ('FirstPersonTranslation', 'FirstPersonScale', 'FirstPersonAttachmentName', 'FirstPersonParticleSystem',
            'FirstPersonParticleSystem_Fizzled', 'BubbleFXScale', 'BubbleFXIntroTime', 'BubbleFXOutroOverlapTime',
            'CollapseDuration', 'MaxCollapseValue', 'PhaselockLifeTimeParamName', 'SphereCollapseParamName',
            'BubbleFXParticleSystem_FadeIn', 'BubbleFXParticleSystem', 'BubbleFXParticleSystem_FadeOut',
            'LiftDuration', 'LockFadeOutTime')
LIGHT_FIELDS = ('Radius', 'FalloffExponent', 'Brightness', 'LightColor', 'CastShadows', 'CastDynamicShadows')


def rotator_degrees(rotator):
    """UE3 rotator (pitch/yaw/roll integers) -> degrees."""
    return {axis.lower(): round((rotator or {}).get(axis, 0) * 360.0 / ROTATOR_UNITS_PER_TURN, 4)
            for axis in ('Pitch', 'Yaw', 'Roll')}


def vector(value):
    return [float((value or {}).get(axis, 0.0)) for axis in ('X', 'Y', 'Z')]


def short(path):
    """FX_CHAR_Siren.Particles.Part_X -> Part_X (the emitter JSON file stem of research/particle_system.py)."""
    return path.rsplit('.', 1)[-1] if path else None


def props_any_offset(package, path):
    """Tagged properties of an export whose payload prefix is not 4 bytes (component subobjects use 16 here).

    The reader fails unless the tag stream ends exactly at the export end, so the first offset it accepts is the one
    whose stream is structurally complete."""
    for offset in (4, 8, 12, 16, 20):
        try:
            data = json.loads(package.run('--properties', package.index[path], '--property-offset', offset,
                                          '--array-schema', package.schema))
        except RuntimeError:
            continue
        return {p['name']: prepare_skill_tree.plain(p.get('value')) for p in data['properties']
                if p.get('status') == 'decoded'}, offset
    raise RuntimeError(f'no property offset decodes {path}')


def socket(package, mesh, name):
    for path in package.index:
        if path.startswith(mesh + '.') and package.classes.get(path) == 'Engine.SkeletalMeshSocket':
            props = package.props(path)
            if props.get('SocketName') == name:
                return {'name': name, 'bone': props.get('BoneName'), 'location': vector(props.get('RelativeLocation')),
                        'rotationDegrees': rotator_degrees(props.get('RelativeRotation')), 'object': path}
    raise KeyError(f'socket {name} not found on {mesh}')


def clip_notifies(package, anim_set, clips):
    out = {}
    for path in package.index:
        if not path.startswith(anim_set + '.') or package.classes.get(path) != 'Engine.AnimSequence':
            continue
        props = package.props(path)
        name = props.get('SequenceName')
        if name not in clips:
            continue
        notifies = []
        for notify in props.get('Notifies') or []:
            row = {'time': notify.get('Time'), 'notify': notify.get('Notify'), 'events': []}
            if row['notify'] in package.index:
                row['class'] = package.classes.get(row['notify'])
                for behavior in package.props(row['notify']).get('Behaviors') or []:
                    if behavior in package.index:
                        event = package.props(behavior).get('CustomEventName')
                        if event:
                            row['events'].append(event)
            notifies.append(row)
        out[name] = {'sequence': path, 'length': props.get('SequenceLength'), 'notifies': notifies}
    missing = [c for c in clips if c not in out]
    if missing:
        raise KeyError(f'clips {missing} not found in {anim_set}')
    return out


def provider_behaviors(manifest):
    """Every behavior of the action-skill manifest's providers, with the events that link to it directly."""
    providers = [manifest.get('behaviorProvider'), manifest['actionSkill'].get('behaviorProvider'),
                 manifest['skill'].get('behaviorProvider')]
    for provider in providers:
        for sequence in (provider or {}).get('sequences') or []:
            linked = {}
            for event in sequence['events']:
                for link in event['links']:
                    linked.setdefault(link['behavior'], []).append(event['name'])
            for behavior in sequence['behaviors']:
                yield behavior, linked.get(behavior['index'], []), provider['id']


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--package', default='GD_Siren_Streaming_SF.upk')
    parser.add_argument('--archetype', default='GD_Siren_Skills.Phaselock.ActionSkill_Phaselock')
    parser.add_argument('--arms-mesh', default='Char_Siren.Hands_Siren')
    parser.add_argument('--arms-material', default='Char_Siren.Mati_Siren_Hands')
    parser.add_argument('--arms-animset', default='Anim_Siren.Siren_1st')
    parser.add_argument('--clips', nargs='+', default=['Phase_Lock_Lift', 'Phase_Lock_Fail'])
    parser.add_argument('--action-skill', default='local/character/action_skill_siren.json',
                        help='manifest of tools/prepare_action_skill.py (behaviors of the skill providers)')
    parser.add_argument('--emitters', default='local/phaselock/emitters', help='research/particle_system.py output')
    parser.add_argument('--output', default='local/phaselock/fx_manifest.json')
    args = parser.parse_args()

    cooked = Path(args.game) / 'WillowGame' / 'CookedPCConsole'
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    schema = output.parent / 'phaselock_fx.schema'
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = str(Path(args.reader).resolve())
    package = prepare_skill_tree.Package(reader, cooked / args.package, str(schema.resolve()))
    willow = prepare_skill_tree.Package(reader, cooked / 'WillowGame.upk', str(schema.resolve()))

    archetype = package.props(args.archetype)
    defaults = willow.props('Default__LiftActionSkill')
    settings = {name: archetype.get(name, defaults.get(name)) for name in SETTINGS}
    sources = {name: 'archetype' if name in archetype else 'class default' for name in SETTINGS}

    # Light: archetype component first, then the class default component it is based on.
    own_light, own_offset = props_any_offset(package, archetype['PhaselockLight']) \
        if archetype.get('PhaselockLight') in package.index else ({}, None)
    default_light, default_offset = props_any_offset(willow, defaults['PhaselockLight'])
    light = {field: own_light.get(field, default_light.get(field)) for field in LIGHT_FIELDS}
    light_sources = {field: 'archetype component' if field in own_light else 'class default component'
                     for field in LIGHT_FIELDS}

    hand_socket = socket(package, args.arms_mesh, settings['FirstPersonAttachmentName'])
    notifies = clip_notifies(package, args.arms_animset, args.clips)

    manifest = json.loads(Path(args.action_skill).read_text(encoding='utf-8'))
    glow = None
    screen = {'show': [], 'hide': []}
    for behavior, events, provider in provider_behaviors(manifest):
        props = behavior.get('properties') or {}
        if behavior['class'] == 'WillowGame.Behavior_CoordinatedEffect' and props.get('Status') == 'CHANGE_Enable':
            glow = {'effect': props.get('Effect'), 'provider': provider, 'directEvents': events}
        if behavior['class'] == 'WillowGame.Behavior_ScreenParticle':
            template = (props.get('Parameters') or {}).get('Template')
            kind = 'hide' if props.get('Action') == 'SPA_Hide' else 'show'
            screen[kind].append({'template': short(template), 'object': template, 'directEvents': events,
                                 'behavior': behavior['id']})
    if not glow or glow['effect'] not in package.index:
        raise KeyError('no enabled coordinated effect found on the skill providers')
    effect = package.props(glow['effect'])
    scalars = []
    for parameter in effect.get('MaterialScalarParameters') or []:
        curve = parameter.get('ParamValueOverTime') or {}
        scalars.append({'parameter': parameter.get('ParamName'),
                        'points': [{'time': p.get('InVal'), 'value': p.get('OutVal'), 'mode': p.get('InterpMode')}
                                   for p in curve.get('Points') or []],
                        'interpMethod': curve.get('InterpMethod')})
    material = package.props(args.arms_material)
    stems = {s['parameter'].replace('p_Enable', '') for s in scalars if s['parameter']}
    colours = {v.get('ParameterName'): v.get('ParameterValue') for v in material.get('VectorParameterValues') or []
               if any(stem in (v.get('ParameterName') or '') for stem in stems)}
    scalar_defaults = {v.get('ParameterName'): v.get('ParameterValue') for v in material.get('ScalarParameterValues') or []
                       if v.get('ParameterName') in {s['parameter'] for s in scalars}}

    templates = [settings[k] for k in ('FirstPersonParticleSystem', 'FirstPersonParticleSystem_Fizzled',
                                        'BubbleFXParticleSystem_FadeIn', 'BubbleFXParticleSystem',
                                        'BubbleFXParticleSystem_FadeOut')] + [s['object'] for s in screen['show']]
    emitter_dir = Path(args.emitters)
    missing = [short(t) for t in templates if not (emitter_dir / f'{short(t)}.json').exists()]

    out = {
        'format': FORMAT,
        'source': {'package': args.package, 'archetype': args.archetype, 'classDefaults': 'WillowGame.Default__LiftActionSkill',
                   'settingSources': sources},
        'emitterDir': str(emitter_dir).replace('\\', '/'),
        'missingTemplates': missing,
        'handFx': {
            'socket': hand_socket,
            'translation': vector(settings['FirstPersonTranslation']),
            'scale': settings['FirstPersonScale'],
            'hitTemplate': short(settings['FirstPersonParticleSystem']),
            'missTemplate': short(settings['FirstPersonParticleSystem_Fizzled']),
            'clips': notifies,
        },
        'bubble': {
            'drawScaleDivisor': settings['BubbleFXScale'],
            'introTime': settings['BubbleFXIntroTime'],
            'outroOverlapTime': settings['BubbleFXOutroOverlapTime'],
            'collapseDuration': settings['CollapseDuration'],
            'maxCollapseValue': settings['MaxCollapseValue'],
            'lifeTimeParam': settings['PhaselockLifeTimeParamName'],
            'collapseParam': settings['SphereCollapseParamName'],
            'fadeInTemplate': short(settings['BubbleFXParticleSystem_FadeIn']),
            'loopTemplate': short(settings['BubbleFXParticleSystem']),
            'fadeOutTemplate': short(settings['BubbleFXParticleSystem_FadeOut']),
        },
        'light': {**light, 'sources': light_sources, 'propertyOffsets': [own_offset, default_offset]},
        'tattooGlow': {**glow, 'duration': effect.get('EffectDuration'), 'scalars': scalars,
                       'material': args.arms_material, 'materialColours': colours, 'materialScalarDefaults': scalar_defaults},
        'screenEffect': screen,
    }
    output.write_text(json.dumps(out, indent=1) + '\n', encoding='utf-8')
    print(f'wrote {output}: hand socket {hand_socket["bone"]}, notifies '
          + ', '.join(f'{k} {[n["time"] for n in v["notifies"]]}' for k, v in notifies.items())
          + f'; glow {len(scalars)} curve(s); screen {[s["template"] for s in screen["show"]]}; missing templates {missing}')
    return 0 if not missing else 1


if __name__ == '__main__':
    sys.exit(main())
