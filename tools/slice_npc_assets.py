#!/usr/bin/env python3
"""Host-side half of the slice NPC/weapon asset pipeline (Marcus, target dummy, stock Maliwan pistol).

AI-assisted. Clean-room: no game data lives in this file. Everything derived from the installed game is
written under the ignored repository ``local/`` directory (``local/slice`` and
``local/external/umodel/slice-npc``); nothing is copied into the repository.

Steps (each is a sub-command; ``all`` runs identity, extract, anims, pistol and editor-job in order):

* ``identity``  resolve what the stock data references, with our own reader (``ow-package --object-dump``
                over the export caches of ``tools/mission_closure.py``'s path index): pawn -> mesh component ->
                SkeletalMesh / AnimSets / AnimTree -> animation names. Object identities (package, export
                index, class, size) come from the reader, not from UModel.
* ``extract``   run UModel (external binary, never added to the repository) one bounded object at a time and
                inventory command line, elapsed time, exit code, warnings, duplicates and output files.
* ``crosscheck`` compare UModel's textures with our reader's own ``--texture`` decode (dimensions and mean
                absolute difference), and UModel mesh/anim internals (bone sets, vertex/triangle counts).
* ``anims``     convert the UModel MD5 clips that the stock data names into UE bone tracks (JSON). The MD5-to-UE
                mapping is the one fitted by ``tools/prepare_character_anims.py`` for Maya, extended to clips
                that animate only a subset of the mesh's bones (the rest keep the reference pose).
* ``pistol``    roll the stock Maliwan pistol balances with ``tools/weapon_recipe.py`` (its rules are
                UNVERIFIED), list every candidate part's gestalt fragment, and write (a) a rolled sample glTF
                and (b) one glTF whose primitives are the individual candidate fragments.
* ``editor-job`` write ``local/slice/editor_job.json`` for ``tools/slice_npc_editor.py`` (UE Python).
* ``manifest``  merge identity, extraction, editor reports and checks into ``local/slice/npc_assets.json``.

UModel materials are heuristic (``.mat``/``.props.txt`` + textures it guessed); this tool reports them as
"textures bound by UModel's guess", never as verified material graphs.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import time

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from prepare_character_anims import MIRROR, matrix, to_quat, ue_matrix  # noqa: E402
from prepare_character_pose import read_joints  # noqa: E402
from prepare_sanctuary_pillar import block, numbers  # noqa: E402
import filter_gestalt_gltf  # noqa: E402
import weapon_recipe  # noqa: E402

SLICE = ROOT / 'local/slice'
UMODEL_OUT = ROOT / 'local/external/umodel/slice-npc'
INDEX_DB = ROOT / 'local/census/path_index.sqlite'
TOOL = 'tools/slice_npc_assets.py'
MAX_FIT_ERROR = 1e-3

# What the stock data was found to use is resolved at run time; only the starting points are named here.
NPC_START = {
    'Marcus': {
        'package': 'Sanctuary_Dynamic',
        'pawn': 'GD_Marcus.Character.Pawn_Marcus',
        'dest': '/Game/OpenWillow/Characters/Marcus',
        # (role, AnimSet group/name as UModel -groups lays it out, clip) chosen from the clips the AnimTree names (Idle, Walk_F, Run_F) plus
        # two conversational gestures from the second AnimSet the pawn lists. Gestures are a cheap extra, not
        # something the AnimTree plays by itself (they come from special moves/dialog, which are not decoded).
        'roles': [('idle', 'Anim_Generic_NPC/Anim_Generic_NPC', 'Idle'), ('walk', 'Anim_Generic_NPC/Anim_Generic_NPC', 'Walk_F'),
                  ('run', 'Anim_Generic_NPC/Anim_Generic_NPC', 'Run_F'),
                  ('gesture_casual', 'Anim_Generic_NPC/Gestures', 'Casual_Var1'),
                  ('gesture_emphatic', 'Anim_Generic_NPC/Gestures', 'Emphatic_var1')],
    },
    'TargetDummy': {
        'package': 'Sanctuary_Dynamic',
        'pawn': 'GD_TargetDummy.Character.Pawn_TargetDummy',
        'dest': '/Game/OpenWillow/Characters/TargetDummy',
        # Clip names come from GD_Z1_RockPaperGenocideData.Anims.SpecialMove_TargetDummyIdle and the three
        # GD_TargetDummy.Anims.Anim_TargetDummy_Death* definitions; checked in identity().
        'roles': [('idle', 'Anim_Sanctuary/Anim_Fink', 'Shield_Struggle_2'),
                  ('death_fire', 'Anim_Sanctuary/Anim_Fink', 'Death_Fire_var1'),
                  ('death_corrosive', 'Anim_Sanctuary/Anim_Fink', 'Death_Corrosive_var3'),
                  ('death_shock', 'Anim_Sanctuary/Anim_Fink', 'Death_Shock_var1')],
        'extra_objects': [
            ('GD_Z1_RockPaperGenocideData.Anims.SpecialMove_TargetDummyIdle', 'animname', 'Shield_Struggle_2'),
            ('GD_TargetDummy.Anims.Anim_TargetDummy_DeathFire', 'animname', 'Death_Fire_var1'),
            ('GD_TargetDummy.Anims.Anim_TargetDummy_DeathCorrosive', 'animname', 'Death_Corrosive_var3'),
            ('GD_TargetDummy.Anims.Anim_TargetDummy_DeathShock', 'animname', 'Death_Shock_var1'),
        ],
    },
}
PISTOL_BALANCES = ['GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire',
                   'GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire']
PISTOL_DEST = '/Game/OpenWillow/Weapons/MaliwanPistol'


def now():
    return datetime.datetime.now().astimezone().isoformat(timespec='seconds')


def settings(args):
    game = args.game or os.environ.get('OPENWILLOW_BL2')
    if not game or not Path(game).is_dir():
        sys.exit('Pass --game <Borderlands 2 folder> or set OPENWILLOW_BL2')
    cooked = Path(game) / 'WillowGame/CookedPCConsole'
    reader = Path(args.reader).resolve()
    if not reader.is_file():
        sys.exit(f'ow-package not found at {reader}')
    return game, cooked, reader


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=1), encoding='utf-8')


def read_json(path, default=None):
    if not path.is_file():
        return default
    return json.loads(path.read_text(encoding='utf-8'))


# --------------------------------------------------------------------------------------------- identity


class Reader:
    """Thin wrapper over ow-package plus the path index built by tools/mission_closure.py (ignored local/)."""

    def __init__(self, reader, cooked):
        if not INDEX_DB.is_file():
            sys.exit('local/census/path_index.sqlite is missing: run tools/export_index.py and tools/mission_closure.py first')
        self.reader, self.cooked = str(reader), Path(cooked)
        self.db = sqlite3.connect(INDEX_DB)
        self.dumps = {}

    def rows(self, path):
        return self.db.execute('select pkg, idx, class, size from ex where path=? order by pkg', (path,)).fetchall()

    def identity(self, path, package=None):
        rows = self.rows(path)
        if not rows:
            return {'path': path, 'status': 'not found in the export index'}
        pick = next((r for r in rows if r[0] == package), rows[0])
        return {'path': path, 'class': pick[2], 'package': pick[0], 'export_index': pick[1], 'size': pick[3],
                'also_in': len(rows) - 1, 'source': 'ow-package --exports (cached)'}

    def dump(self, path, package):
        """Instantiate one export (class defaults + tagged overrides) with the reader's VM, trying the
        three established property prefixes in turn. Prefix semantics are UNVERIFIED (see DECISIONS.md)."""
        key = (package, path)
        if key in self.dumps:
            return self.dumps[key]
        ident = self.identity(path, package)
        if 'export_index' not in ident or ident['package'] != package:
            raise RuntimeError(f'{path} is not an export of {package}')
        last = ''
        for prefix in (4, 8):
            run = subprocess.run([self.reader, str(self.cooked / f'{package}.upk'), '--object-dump',
                                  str(ident['export_index']), str(prefix), '--cooked', str(self.cooked)],
                                 capture_output=True, encoding='utf-8')
            if run.returncode == 0:
                data = json.loads(run.stdout)
                data.pop('log', None)
                self.dumps[key] = data['properties']
                return self.dumps[key]
            last = run.stderr.strip()
        raise RuntimeError(f'{path}: object-dump failed ({last})')


def leaf(path):
    return path.rsplit('.', 1)[-1]


def identity_step(args):
    game, cooked, reader_path = settings(args)
    reader = Reader(reader_path, cooked)
    result = {'tool': TOOL, 'generated': now(), 'npcs': {}}
    for name, spec in NPC_START.items():
        package = spec['package']
        pawn = reader.dump(spec['pawn'], package)
        component_path = pawn['mesh']
        component = reader.dump(component_path, package)
        entry = {'dest': spec['dest'], 'package': package,
                 'pawn': reader.identity(spec['pawn'], package),
                 'pawn_properties': {k: pawn[k] for k in ('bodyclass', 'aiclass', 'mesh', 'allegiance') if k in pawn},
                 'mesh_component': reader.identity(component_path, package),
                 'mesh_component_properties': {k: v for k, v in component.items()
                                               if k in ('skeletalmesh', 'animsets', 'animtreetemplate', 'materials',
                                                        'physicsasset', 'translation', 'scale')}}
        mesh_path = component['skeletalmesh']
        entry['skeletal_mesh'] = reader.identity(mesh_path, package)
        entry['anim_sets'] = [reader.identity(p, package) for p in component.get('animsets', [])]
        entry['materials_from_component'] = [reader.identity(p, package) for p in component.get('materials', [])]
        # The mesh's own material slots are not decoded by our reader (SkeletalMesh serialization has no public
        # spec); UModel reports them while exporting. They are filled in from the extraction log.
        entry['anim_tree'] = reader.identity(component['animtreetemplate'], package) if 'animtreetemplate' in component else None
        entry['body_class'] = reader.identity(pawn.get('bodyclass', ''), package) if pawn.get('bodyclass') else None
        entry['physics_asset'] = reader.identity(component['physicsasset'], package) if component.get('physicsasset') else None
        # Animation names the data refers to, with the reader's decoded property that names each.
        names = []
        if component.get('animtreetemplate'):
            tree = reader.dump(component['animtreetemplate'], package)
            for node_path in tree.get('animtickarray', []):
                try:
                    node = reader.dump(node_path, package)
                except RuntimeError:
                    continue
                if 'animseqname' in node and node['animseqname'] != 'None':
                    names.append({'node': node_path, 'class': reader.identity(node_path, package).get('class'),
                                  'animseqname': node['animseqname']})
        entry['anim_tree_sequence_names'] = names
        checks = []
        for path, prop, expected in spec.get('extra_objects', []):
            value = reader.dump(path, package).get(prop)
            checks.append({'object': reader.identity(path, package), 'property': prop, 'value': value,
                           'expected': expected, 'ok': value == expected})
        entry['named_by_special_moves_and_anim_definitions'] = checks
        if any(not c['ok'] for c in checks):
            sys.exit(f'{name}: a clip name differs from the expected one: {checks}')
        if name == 'Marcus':
            wanted = {'Idle', 'Walk_F', 'Run_F'}
            found = {n['animseqname'] for n in names}
            if not wanted <= found:
                sys.exit(f'Marcus AnimTree does not name {sorted(wanted - found)}; has {sorted(found)}')
        result['npcs'][name] = entry
    # The weapon identities.
    pistol = {}
    for balance in PISTOL_BALANCES:
        pistol[balance] = reader.identity(balance, 'Startup')
    pistol['gestalt_definition'] = reader.identity('Weap_Pistol.GestaltDef_Pistol', 'Startup')
    pistol['gestalt_mesh'] = reader.identity('Weap_Pistol.GestaltDef_Pistol_GestaltSkeletalMesh', 'Startup')
    result['pistol'] = pistol
    write_json(SLICE / 'npc_identity.json', result)
    print(f'identity: wrote {SLICE / "npc_identity.json"}')
    return result


# --------------------------------------------------------------------------------------------- extraction


UMODEL_CLASS = {'SkeletalMesh': 'SkeletalMesh', 'AnimSet': 'AnimSet', 'Texture2D': 'Texture2D',
                'MaterialInstanceConstant': 'MaterialInstanceConstant'}


def umodel_path(args):
    value = args.umodel or os.environ.get('OPENWILLOW_UMODEL')
    if not value or not Path(value).is_file() or Path(value).name.lower() not in ('umodel.exe', 'umodel'):
        sys.exit('Pass --umodel <umodel.exe> or set OPENWILLOW_UMODEL (UModel build 1590; never inside the repository)')
    return Path(value).resolve()


def run_umodel(umodel, cooked, job_id, group, package, obj, cls, formats, want_group=None):
    """One bounded UModel export. Returns an inventory record; never raises on UModel failure."""
    out = UMODEL_OUT / group
    out.mkdir(parents=True, exist_ok=True)
    log_dir = SLICE / 'logs'
    log_dir.mkdir(parents=True, exist_ok=True)
    # AnimSet names are not unique inside a package (three 'Gestures' sets in Sanctuary_Dynamic), so those
    # exports use -groups, which puts every set under its own group folder instead of merging them.
    extra = ['-groups'] if cls == 'AnimSet' else []
    arguments = [f'-path={cooked}', '-game=border', '-export'] + extra + [f'-{f}' for f in formats] + \
        [f'-out={out}', package, obj, cls]
    started = time.perf_counter()
    run = subprocess.run([str(umodel)] + arguments, capture_output=True, encoding='utf-8', errors='replace')
    elapsed = time.perf_counter() - started
    text = run.stdout + (('\n' + run.stderr) if run.stderr.strip() else '')
    (log_dir / f'{job_id}.log').write_text(text, encoding='utf-8')
    found = re.search(r'Found (\d+) object\(s\)', text)
    announced = re.findall(r'Exporting (\w+) (\S+) to (.+)', text)
    outputs, other = [], []
    for kind, name, target in announced:
        target = Path(target.strip())
        if kind == 'AnimSet':
            # UModel announces only the first clip; every clip of the set lands in the same folder.
            siblings = sorted(target.parent.glob('*')) if target.parent.is_dir() else []
        else:
            siblings = sorted(target.parent.glob(target.stem + '.*')) if target.parent.is_dir() else []
        for item in siblings or [target]:
            if not item.is_file() or any(o['path'] == str(item) for o in outputs + other):
                continue
            record = {'path': str(item), 'class': kind, 'object': name, 'bytes': item.stat().st_size}
            # With -groups the layout is <out>/<package>/<group>/<set>/: keep only the wanted group's set.
            if want_group and item.parent.parent.name != want_group:
                other.append(record)
            else:
                outputs.append(record)
    warnings = sorted({line.strip() for line in text.splitlines()
                       if re.search(r'WARNING|unknown|dropping|ERROR|Failed|not found|unsupported', line, re.I)})
    # UModel prints the exact object name it exported; no match means no export happened.
    ok = run.returncode == 0 and bool(outputs)
    sanitized = ['$UMODEL'] + [a.replace(str(cooked), '$COOKED').replace(str(out), f'$OUT/{group}') for a in arguments]
    return {'id': job_id, 'group': group, 'package': package, 'object': obj, 'class': cls, 'formats': formats,
            'command': ' '.join(sanitized), 'exit_code': run.returncode, 'elapsed_seconds': round(elapsed, 2),
            'objects_found': int(found.group(1)) if found else 0, 'success': ok, 'warnings': warnings,
            'outputs': outputs, 'output_bytes': sum(o['bytes'] for o in outputs),
            'duplicate_name_outputs_not_used': {'files': len(other), 'bytes': sum(o['bytes'] for o in other),
                                                'groups': sorted({Path(o['path']).parent.parent.name for o in other})},
            'log': f'local/slice/logs/{job_id}.log'}


def mic_textures(props_path):
    """(parameter, object path) pairs from UModel's .props.txt for a MaterialInstanceConstant."""
    text = Path(props_path).read_text(encoding='utf-8', errors='replace')
    pairs = re.findall(r"ParameterValue = Texture2D'([^']+)'\s+ParameterName = (\S+)", text)
    parent = re.search(r"Parent = MaterialInstanceConstant'([^']+)'", text)
    scalars = re.findall(r'ParameterValue = ([-0-9.e]+)\s+ParameterName = (\S+)', text)
    vectors = re.findall(r'ParameterValue = \{ R=([-0-9.e]+), G=([-0-9.e]+), B=([-0-9.e]+), A=([-0-9.e]+) \}\s+ParameterName = (\S+)', text)
    return {'textures': [{'parameter': p, 'object': o} for o, p in pairs],
            'parent': parent.group(1) if parent else None,
            'scalars': {n: float(v) for v, n in scalars},
            'vectors': {n: [float(r), float(g), float(b), float(a)] for r, g, b, a, n in vectors}}


def find_output(group, name, extensions):
    for ext in extensions:
        hits = sorted((UMODEL_OUT / group).rglob(f'{name}.{ext}'))
        if hits:
            return hits[0]
    return None


def mesh_materials_from_log(text):
    return [(name, package.removesuffix('.upk'))
            for name, package in re.findall(r'Loading MaterialInstanceConstant (\S+) from package (\S+)', text)]


def extract_step(args):
    game, cooked, reader_path = settings(args)
    umodel = umodel_path(args)
    identity = read_json(SLICE / 'npc_identity.json') or identity_step(args)
    records = []
    # Start from empty export folders so the inventory describes this run only (UModel changes what it writes
    # when earlier outputs exist, e.g. it adds textures to a glTF export).
    import shutil
    for group in list(identity['npcs']) + ['Pistol']:
        shutil.rmtree(UMODEL_OUT / group, ignore_errors=True)

    def go(job_id, group, package, obj, cls, formats, want_group=None):
        record = run_umodel(umodel, cooked, job_id, group, package, obj, cls, formats, want_group)
        state = 'ok' if record['success'] else 'FAILED'
        print(f'  {job_id}: {state} exit={record["exit_code"]} {record["elapsed_seconds"]}s '
              f'files={len(record["outputs"])} bytes={record["output_bytes"]} found={record["objects_found"]}')
        records.append(record)
        return record

    for name, npc in identity['npcs'].items():
        package = npc['package']
        mesh = leaf(npc['skeletal_mesh']['path'])
        npc_mesh_log = ''
        print(f'{name}:')
        r = go(f'{name}.mesh.gltf', name, package, mesh, 'SkeletalMesh', ['gltf'])
        npc_mesh_log = (ROOT / r['log']).read_text(encoding='utf-8') if r['log'] else ''
        go(f'{name}.mesh.md5', name, package, mesh, 'SkeletalMesh', ['md5'])
        # The mesh's material slots are discovered by UModel; export each MIC (props + the textures UModel binds).
        slots = []
        overrides = npc.get('materials_from_component', [])
        for index, (default_mic, default_package) in enumerate(mesh_materials_from_log(npc_mesh_log)):
            # The pawn's mesh component may override slot i (SkeletalMeshComponent.Materials[i]); the stock pawn
            # then shows the override, not the mesh's own default that UModel reports for the mesh.
            override = overrides[index] if index < len(overrides) and 'path' in overrides[index] else None
            mic = leaf(override['path']) if override else default_mic
            mic_package = override['package'] if override else default_package
            if mic in [s['material'] for s in slots]:
                continue
            mr = go(f'{name}.material.{mic}', name, mic_package, mic, 'MaterialInstanceConstant', ['png'])
            props = find_output(name, mic, ['props.txt'])
            info = mic_textures(props) if props else {'textures': []}
            slots.append({'name': default_mic, 'material': mic, 'package': mic_package,
                          'overrides_mesh_default': default_mic if override else None,
                          'props': str(props) if props else None, **info, 'export_record': mr['id'],
                          'identity': reader_identity(args, mic, mic_package)})
        npc['mesh_material_slots'] = slots
        for anim_set in npc['anim_sets']:
            group_name = anim_set['path'].split('.')[0]
            go(f'{name}.animset.{anim_set["path"]}', name, anim_set['package'], leaf(anim_set['path']),
               'AnimSet', ['md5'], want_group=group_name)
    # Pistol: gestalt mesh and the Maliwan material.
    print('Pistol:')
    gestalt = identity['pistol']['gestalt_mesh']
    r = go('Pistol.gestalt.gltf', 'Pistol', gestalt['package'], leaf(gestalt['path']), 'SkeletalMesh', ['gltf'])
    material = pistol_material_name(args)
    if material:
        go(f'Pistol.material.{material}', 'Pistol', 'Startup', material, 'MaterialInstanceConstant', ['png'])
    inventory = {'tool': TOOL, 'generated': now(), 'umodel': str(umodel.name),
                 'umodel_build': 'build 1590 (from docs/verification/EXTERNAL_TOOL_BENCHMARK.md)',
                 'records': records}
    write_json(SLICE / 'npc_extract.json', inventory)
    write_json(SLICE / 'npc_identity.json', identity)
    failed = [r['id'] for r in records if not r['success']]
    print(f'extract: {len(records)} jobs, {len(failed)} failed {failed}')
    return inventory


_reader_cache = {}


def reader_identity(args, path, package):
    game, cooked, reader_path = settings(args)
    if 'r' not in _reader_cache:
        _reader_cache['r'] = Reader(reader_path, cooked)
    short = _reader_cache['r'].db.execute('select path from ex where name=? and pkg=? and class like ?',
                                           (path, package, '%MaterialInstanceConstant')).fetchall()
    if not short:
        return {'path': path, 'status': 'not found in the export index'}
    full = short[0][0]
    ident = _reader_cache['r'].identity(full, package)
    ident['also_paths'] = len(short) - 1
    return ident


def pistol_material_name(args):
    recipe = read_json(SLICE / 'pistol/recipe_Pistol_Maliwan_2_Fire.json')
    if not recipe:
        pistol_step(args, rolls_only=True)
        recipe = read_json(SLICE / 'pistol/recipe_Pistol_Maliwan_2_Fire.json')
    return leaf(recipe['material']) if recipe and recipe.get('material') else None


# ---------------------------------------------------------------------------------- checks vs reader


def read_png_rgba(path):
    from PIL import Image
    with Image.open(path) as image:
        return np.asarray(image.convert('RGBA'), dtype=np.int16)


def crosscheck_step(args):
    """Texture decode comparison (our reader vs UModel) and UModel-internal mesh/animation consistency."""
    game, cooked, reader_path = settings(args)
    reader = Reader(reader_path, cooked)
    identity = read_json(SLICE / 'npc_identity.json')
    inventory = read_json(SLICE / 'npc_extract.json')
    work = SLICE / 'xcheck'
    work.mkdir(parents=True, exist_ok=True)
    report = {'tool': TOOL, 'generated': now(), 'textures': [], 'meshes': [], 'anims': []}
    seen = set()
    groups = [(n, v['package'], v.get('mesh_material_slots', [])) for n, v in identity['npcs'].items()]
    material = pistol_material_name(args)
    props = find_output('Pistol', material, ['props.txt']) if material else None
    if props:
        groups.append(('Pistol', 'Startup', [mic_textures(props)]))
    for group, package, slots in groups:
        for slot in slots:
            for tex in slot['textures']:
                key = (group, tex['object'])
                if key in seen:
                    continue
                seen.add(key)
                name = leaf(tex['object'])
                ident = reader.identity(tex['object'], package)
                if ident.get('package') != package:
                    # Texture lives elsewhere (UModel resolved it by name); try the package that holds it.
                    rows = reader.rows(tex['object'])
                    ident = reader.identity(tex['object'], rows[0][0]) if rows else ident
                item = {'group': group, 'parameter': tex['parameter'], 'object': tex['object'], 'identity': ident}
                shot = find_output(group, name, ['png', 'tga'])
                item['umodel_file'] = str(shot.name) if shot else None
                if ident.get('class') != 'Engine.Texture2D' or not shot:
                    item['result'] = 'not compared (no UModel file or identity)'
                    report['textures'].append(item)
                    continue
                decoded = work / f'{group}_{name}.png'
                done = subprocess.run([str(reader_path), str(cooked / f'{ident["package"]}.upk'), '--texture',
                                       str(ident['export_index']), '--property-offset', '4', '--output', str(decoded),
                                       '--tfc', str(cooked)], capture_output=True, encoding='utf-8')
                if done.returncode:
                    item['result'] = f'our decode failed: {done.stderr.strip()[:160]}'
                    report['textures'].append(item)
                    continue
                meta = json.loads(done.stdout)
                ours, theirs = read_png_rgba(decoded), read_png_rgba(shot)
                item['reader'] = {'format': meta.get('format'), 'width': meta.get('width'), 'height': meta.get('height')}
                item['umodel_size'] = [theirs.shape[1], theirs.shape[0]]
                if ours.shape != theirs.shape:
                    item['result'] = 'DIMENSIONS DIFFER'
                else:
                    diff = np.abs(ours - theirs)
                    item['mean_abs_diff_rgb'] = round(float(diff[..., :3].mean()), 4)
                    item['mean_abs_diff_alpha'] = round(float(diff[..., 3].mean()), 4)
                    item['result'] = 'match' if diff[..., :3].mean() < 1.0 else 'DIFFERS'
                report['textures'].append(item)
                decoded.unlink(missing_ok=True)
    for name in identity['npcs']:
        mesh = leaf(identity['npcs'][name]['skeletal_mesh']['path'])
        md5 = find_output(name, mesh, ['md5mesh'])
        gltf = find_output(name, mesh, ['gltf'])
        item = {'group': name, 'mesh': mesh, 'reader': 'identity only (SkeletalMesh serialization is not decoded by our reader)'}
        if md5 and gltf:
            text = md5.read_text(encoding='utf-8')
            doc = json.loads(gltf.read_text(encoding='utf-8'))
            verts = [int(v) for v in re.findall(r'numverts (\d+)', text)]
            tris = [int(v) for v in re.findall(r'numtris (\d+)', text)]
            joints = read_joints(text)
            skin = doc['skins'][0]['joints'] if doc.get('skins') else []
            gv = [doc['accessors'][p['attributes']['POSITION']]['count'] for m in doc['meshes'] for p in m['primitives']]
            gt = [doc['accessors'][p['indices']]['count'] // 3 for m in doc['meshes'] for p in m['primitives']]
            item.update({'md5_bones': len(joints), 'gltf_skin_joints': len(skin), 'md5_vertices_per_section': verts,
                         'gltf_vertices_per_section': gv, 'md5_triangles_per_section': tris,
                         'gltf_triangles_per_section': gt,
                         'umodel_formats_agree': (len(joints) == len(skin) and verts == gv and tris == gt),
                         'gltf_materials': [m.get('name') for m in doc['materials']]})
            names = [n.get('name') for n in doc['nodes']]
            item['gltf_bone_names_equal_md5'] = all(j[0] in names for j in joints)
        report['meshes'].append(item)
    write_json(SLICE / 'npc_crosscheck.json', report)
    bad = [t for t in report['textures'] if t['result'] not in ('match',)]
    print(f'crosscheck: {len(report["textures"])} textures compared, {len(bad)} not a plain match; '
          f'{len(report["meshes"])} meshes; wrote {SLICE / "npc_crosscheck.json"}')
    return report


# ---------------------------------------------------------------------------------------- animations


def read_md5anim(path):
    text = Path(path).read_text(encoding='utf-8')
    rows = [line.split('"') for line in block(text, 'hierarchy').splitlines() if line.strip()]
    names = [r[1] for r in rows]
    flags = [int(r[2].split()[1]) for r in rows]
    if any(f != 63 for f in flags):
        raise ValueError(f'{path}: expects all six components animated per bone (flags 63), got {set(flags)}')
    count = int(re.search(r'\bnumFrames\s+(\d+)', text).group(1))
    rate = float(re.search(r'\bframeRate\s+(\S+)', text).group(1))
    frames = []
    for index in range(count):
        values = [numbers(line) for line in block(text, f'frame {index}').splitlines() if line.strip()]
        if len(values) != len(names):
            raise ValueError(f'{path}: frame {index} has {len(values)} bones, hierarchy has {len(names)}')
        frames.append(values)
    return names, rate, frames


def convert_subset(mesh_md5, reference_json, clips):
    """MD5 clips (possibly animating only some bones) -> {clip: {rate, frames, tracks}} in UE bone space.

    Same fit as tools/prepare_character_anims.py: T_i(t) = C W_i(t) D_i with C the Y mirror and
    D_i = W_i(bind)^-1 C^-1 T_i(bind). The parent used for every bone is the MESH hierarchy (UModel's
    md5anim hierarchy block lists every bone under Root, its frames are local to the mesh parents; this
    matches how prepare_character_anims.py already treats Maya's clips). Bones the clip does not animate get
    no track and keep the reference pose in UE. UNVERIFIED beyond the structural checks below.
    """
    joints = read_joints(Path(mesh_md5).read_text(encoding='utf-8'))
    reference = {b['name']: b for b in json.loads(Path(reference_json).read_text(encoding='utf-8'))}
    names = [j[0] for j in joints]
    if set(names) != set(reference):
        raise ValueError(f'UE skeleton and MD5 mesh bones differ: {sorted(set(names) ^ set(reference))[:10]}')
    bind_md5 = [matrix(position, q) for _, _, position, q in joints]
    bind_ue = [ue_matrix(reference[n]['loc'], reference[n]['quat']) for n in names]
    source = np.array([m[:3, 3] for m in bind_md5])
    target = np.array([m[:3, 3] for m in bind_ue])
    fitted, *_ = np.linalg.lstsq(np.c_[source, np.ones(len(source))], target, rcond=None)
    linear, offset = fitted[:3].T, fitted[3]
    fit_error = float(np.abs(linear - MIRROR[:3, :3]).max())
    if fit_error > MAX_FIT_ERROR or np.abs(offset).max() > 0.05:
        raise ValueError(f'MD5 to UE map is not the expected Y mirror: {linear} {offset}')
    c = MIRROR
    inverse_c = np.linalg.inv(c)
    fix = [np.linalg.inv(w) @ inverse_c @ t for w, t in zip(bind_md5, bind_ue)]
    parents = [j[1] for j in joints]
    index = {n: i for i, n in enumerate(names)}
    output, report = {}, {'fit_error': fit_error}
    for label, path in clips.items():
        anim_names, rate, frames = read_md5anim(path)
        missing = [n for n in anim_names if n not in index]
        if missing:
            raise ValueError(f'{label}: clip bones not in the mesh skeleton: {missing[:8]}')
        animated = sorted(index[n] for n in anim_names)
        for i in animated:
            if parents[i] >= 0 and parents[i] not in animated:
                raise ValueError(f'{label}: bone {names[i]} animated but its parent {names[parents[i]]} is not')
        order = {n: k for k, n in enumerate(anim_names)}
        tracks = {names[i]: {'pos': [], 'rot': []} for i in animated}
        for values in frames:
            world = {}
            for i in animated:
                local = values[order[names[i]]]
                position, orientation = local[:3], quaternion_of(local[3:])
                if parents[i] >= 0:
                    parent_position, parent_rotation = world[parents[i]]
                    position = tuple(a + b for a, b in zip(parent_position, rotate_of(parent_rotation, position)))
                    orientation = multiply_of(parent_rotation, orientation)
                world[i] = (position, orientation)
            ue = {i: c @ matrix(*world[i]) @ fix[i] for i in animated}
            for i in animated:
                local = ue[i] if parents[i] < 0 else np.linalg.inv(ue[parents[i]]) @ ue[i]
                tracks[names[i]]['pos'].append([float(v) for v in local[:3, 3]])
                tracks[names[i]]['rot'].append(to_quat(local[:3, :3]))
        output[label] = {'rate': rate, 'frames': len(frames), 'tracks': tracks}
        report[label] = {'bones_animated': len(animated), 'mesh_bones': len(names), 'frames': len(frames), 'rate': rate}
    return output, report


def quaternion_of(xyz):
    from prepare_sanctuary_pillar import quaternion
    return quaternion(xyz)


def rotate_of(q, value):
    from prepare_sanctuary_pillar import rotate
    return rotate(q, value)


def multiply_of(a, b):
    from prepare_sanctuary_pillar import multiply
    return multiply(a, b)


def anims_step(args):
    identity = read_json(SLICE / 'npc_identity.json')
    report = {'tool': TOOL, 'generated': now(), 'npcs': {}}
    for name, spec in NPC_START.items():
        reference = SLICE / f'ref_pose_{name}.json'
        if not reference.is_file():
            print(f'anims: {name}: reference pose {reference.name} missing (run the editor refpose step first)')
            continue
        mesh = leaf(identity['npcs'][name]['skeletal_mesh']['path'])
        mesh_md5 = find_output(name, mesh, ['md5mesh'])
        clips, roles = {}, {}
        for role, anim_set, clip in spec['roles']:
            hits = sorted((UMODEL_OUT / name).rglob(f'{clip}.md5anim'))
            hit = next((h for h in hits if f'{h.parent.parent.name}/{h.parent.name}' == anim_set), None)
            if hit is None:
                print(f'anims: {name}: role {role}: {anim_set}/{clip} was not exported')
                roles[role] = {'anim_set': anim_set, 'clip': clip, 'status': 'not exported'}
                continue
            clips[role] = hit
            roles[role] = {'anim_set': anim_set, 'clip': clip}
        tracks, details = convert_subset(mesh_md5, reference, clips)
        write_json(SLICE / f'anim_tracks_{name}.json', tracks)
        for role in clips:
            roles[role].update(details[role])
        report['npcs'][name] = {'fit_error': details['fit_error'], 'roles': roles,
                                'tracks_json': f'local/slice/anim_tracks_{name}.json'}
        print(f'anims: {name}: {len(clips)} clips converted (fit error {details["fit_error"]:.2e})')
    write_json(SLICE / 'npc_anims.json', report)
    return report


# ------------------------------------------------------------------------------------------- pistol


def pistol_step(args, rolls_only=False):
    game, cooked, reader_path = settings(args)
    out = SLICE / 'pistol'
    out.mkdir(parents=True, exist_ok=True)
    startup = cooked / 'Startup.upk'
    schema = out / 'weapon_recipe.schema'
    schema.write_text('\n'.join(weapon_recipe.SCHEMA_LINES) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(str(reader_path), startup, str(schema.resolve()))
    summary = {'tool': TOOL, 'generated': now(), 'rules_status': 'UNVERIFIED (tools/weapon_recipe.py rules)',
               'balances': {}}
    for balance in PISTOL_BALANCES:
        recipe = weapon_recipe.roll(package, balance, 1, 30)
        (out / f'recipe_{leaf(balance)}.json').write_text(json.dumps(recipe, indent=1), encoding='utf-8')
        slots = {}
        for slot, chosen in recipe['parts'].items():
            candidates = []
            for cand in chosen['candidates']:
                props = package.props(cand['part'])
                candidates.append({'part': cand['part'], 'weight': cand['weight'],
                                   'fragment': props.get('GestaltModeSkeletalMeshName'),
                                   'material': props.get('Material'),
                                   'identity': {'package': 'Startup',
                                                'export_index': package.index.get(cand['part'])}})
            slots[slot] = {'rolled_part': chosen['part'], 'candidates': candidates}
        summary['balances'][balance] = {'seed': 1, 'stage': 30, 'material': recipe['material'], 'gestalt': recipe['gestalt'],
                                        'name': recipe['name'], 'notes': recipe['notes'],
                                        'merge': [m['balance'] for m in recipe['merge']], 'slots': slots,
                                        'rolled_fragments': recipe['gestalt_fragments']}
    write_json(out / 'candidates.json', summary)
    if rolls_only:
        return summary
    gestalt_dir = UMODEL_OUT / 'Pistol'
    gltf = next(iter(sorted(gestalt_dir.rglob('GestaltDef_Pistol_GestaltSkeletalMesh.gltf'))), None)
    if not gltf:
        sys.exit('pistol: run extract first (the gestalt glTF is missing)')
    gestalt_json = out / 'GestaltDef_Pistol.json'
    index = package.index['Weap_Pistol.GestaltDef_Pistol']
    schema_file = ROOT / 'tools/gestalt-arrays.schema'  # tracked; was the local/infinity/gestalt.schema hand-made copy
    done = subprocess.run([str(reader_path), str(startup), '--properties', str(index), '--property-offset', '4',
                           '--array-schema', str(schema_file)], capture_output=True, encoding='utf-8')
    if done.returncode:
        sys.exit(f'pistol: gestalt decode failed: {done.stderr.strip()}')
    gestalt_json.write_text(done.stdout, encoding='utf-8')
    parts = filter_gestalt_gltf.gestalt_parts(gestalt_json)
    known = {p['SkeletalMeshFragmentName'] for p in parts}
    # Rolled sample (same method as tools/seed_inventory_demo.py): the chosen parts' fragments only.
    fire = summary['balances'][PISTOL_BALANCES[0]]
    sample_fragments = sorted(set(fire['rolled_fragments']) & known)
    sample_missing = sorted(set(fire['rolled_fragments']) - known)
    sample_recipe = out / 'sample_fragments.json'
    sample_recipe.write_text(json.dumps({'gestalt_fragments': sample_fragments}), encoding='utf-8')
    run_filter = [sys.executable, str(ROOT / 'tools/filter_gestalt_gltf.py'), '--gltf', str(gltf), '--gestalt',
                  str(gestalt_json), '--recipe', str(sample_recipe), '--output', str(out / 'Pistol_Maliwan_2_Fire_seed1.gltf')]
    done = subprocess.run(run_filter, capture_output=True, encoding='utf-8')
    if done.returncode:
        sys.exit(f'pistol: sample filter failed: {done.stderr.strip()}')
    # Candidate fragments: every fragment any slot may select, each as its own primitive.
    wanted = {}
    for balance in summary['balances'].values():
        for slot, info in balance['slots'].items():
            for cand in info['candidates']:
                if cand['fragment']:
                    wanted.setdefault(cand['fragment'], set()).add(f'{slot}:{leaf(cand["part"])}')
    unresolved = sorted(f for f in wanted if f not in known)
    sections = candidate_sections_gltf(gltf, parts, sorted(f for f in wanted if f in known),
                                       out / 'Pistol_Maliwan_Candidates.gltf')
    summary['sample'] = {'gltf': 'local/slice/pistol/Pistol_Maliwan_2_Fire_seed1.gltf', 'fragments': sample_fragments,
                         'unresolved_fragments': sample_missing}
    summary['candidate_sections'] = {'gltf': 'local/slice/pistol/Pistol_Maliwan_Candidates.gltf',
                                     'fragments': sections,
                                     'parts_by_fragment': {k: sorted(v) for k, v in sorted(wanted.items())},
                                     'unresolved_fragments': unresolved}
    summary['gestalt_decode'] = {'fragments_in_gestalt': len(known)}
    write_json(out / 'candidates.json', summary)
    print(f'pistol: sample {len(sample_fragments)} fragments (unresolved {sample_missing}); candidate sections '
          f'{len(sections)} (unresolved {unresolved})')
    return summary


def candidate_sections_gltf(source_gltf, parts, fragments, output):
    """One primitive per fragment (each with its own named glTF material) in a single skeletal mesh.

    A fragment name with several ranges in the part table keeps all of them in its one primitive (as
    tools/filter_gestalt_gltf.py does); it used to keep only the last range (RTSE_GUN_DATA_CROSSCHECK.md section 6).

    Same section-order assumption as tools/filter_gestalt_gltf.py (checked there for the same file): one
    primitive per LOD section in material order. The new primitives share the original vertex buffers.
    """
    import struct
    gltf = json.loads(Path(source_gltf).read_text(encoding='utf-8'))
    blob = bytearray((Path(source_gltf).parent / gltf['buffers'][0]['uri']).read_bytes())
    primitives = gltf['meshes'][0]['primitives']
    starts, first = [], 0
    for material, primitive in enumerate(primitives):
        count = gltf['accessors'][primitive['indices']]['count']
        expected = 3 * sum(p['NumPrimitives'] for p in parts if p['MaterialIndex'] == material)
        if primitive.get('material') != material or count != expected:
            raise RuntimeError(f'section {material}: {count} indices, gestalt ranges give {expected}')
        starts.append(first)
        first += count
    original_materials = gltf['materials']
    new_primitives, new_materials, report = [], [], []
    ranges = {}
    for part in parts:  # a fragment name can own several ranges (launcher accessory, sniper body): keep every one
        ranges.setdefault(part['SkeletalMeshFragmentName'], []).append(part)
    for fragment in fragments:
        owned = ranges[fragment]
        materials = {part['MaterialIndex'] for part in owned}
        if len(materials) != 1:
            raise RuntimeError(f'{fragment}: its ranges use several materials {sorted(materials)}')
        material = materials.pop()
        primitive = primitives[material]
        accessor = gltf['accessors'][primitive['indices']]
        view = gltf['bufferViews'][accessor['bufferView']]
        base = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
        indices = []
        for part in owned:
            local = part['FirstIndex'] - starts[material]
            indices += struct.unpack_from(f'<{3 * part["NumPrimitives"]}H', blob, base + 2 * local)
        while len(blob) % 4:
            blob.append(0)
        gltf['bufferViews'].append({'buffer': 0, 'byteOffset': len(blob), 'byteLength': 2 * len(indices), 'target': 34963})
        blob += struct.pack(f'<{len(indices)}H', *indices)
        gltf['accessors'].append({'bufferView': len(gltf['bufferViews']) - 1, 'componentType': 5123,
                                  'count': len(indices), 'type': 'SCALAR'})
        kept = dict(primitive)
        kept['indices'] = len(gltf['accessors']) - 1
        kept['material'] = len(new_materials)
        new_materials.append({**original_materials[material], 'name': f'Frag_{fragment}'})
        new_primitives.append(kept)
        report.append({'fragment': fragment, 'section_index': len(new_primitives) - 1,
                       'triangles': sum(part['NumPrimitives'] for part in owned), 'ranges': len(owned),
                       'gestalt_material_index': material, 'ue_material_slot': f'Frag_{fragment}'})
    binary = Path(output).with_suffix('.bin')
    binary.write_bytes(blob)
    gltf['buffers'][0] = {'uri': binary.name, 'byteLength': len(blob)}
    gltf['meshes'][0]['primitives'] = new_primitives
    gltf['materials'] = new_materials
    Path(output).write_text(json.dumps(gltf), encoding='utf-8')
    return report


# ----------------------------------------------------------------------------------- editor job file


def editor_job_step(args):
    identity = read_json(SLICE / 'npc_identity.json')
    inventory = read_json(SLICE / 'npc_extract.json')
    job = {'tool': TOOL, 'generated': now(), 'npcs': [], 'slice_dir': str(SLICE)}
    for name, spec in NPC_START.items():
        npc = identity['npcs'][name]
        mesh = leaf(npc['skeletal_mesh']['path'])
        gltf = find_output(name, mesh, ['gltf'])
        slots = []
        for slot in npc.get('mesh_material_slots', []):
            textures = {}
            for tex in slot['textures']:
                file = find_output(name, leaf(tex['object']), ['png', 'tga'])
                if file:
                    textures[tex['parameter']] = str(file)
            slots.append({'slot': slot['name'], 'material': slot['material'], 'textures': textures, 'scalars': slot.get('scalars', {}),
                          'vectors': slot.get('vectors', {})})
        job['npcs'].append({'id': name, 'dest': spec['dest'], 'mesh_name': mesh, 'gltf': str(gltf), 'slots': slots,
                            'tracks': str(SLICE / f'anim_tracks_{name}.json'),
                            'roles': [r for r, _, _ in spec['roles']],
                            'reference_out': str(SLICE / f'ref_pose_{name}.json')})
    pistol = read_json(SLICE / 'pistol/candidates.json')
    material = pistol_material_name(args)
    textures = {}
    if material:
        props = find_output('Pistol', material, ['props.txt'])
        info = mic_textures(props) if props else {'textures': []}
        for tex in info['textures']:
            file = find_output('Pistol', leaf(tex['object']), ['png', 'tga'])
            if file:
                textures[tex['parameter']] = str(file)
        job['pistol'] = {'dest': PISTOL_DEST, 'material': material, 'textures': textures,
                         'scalars': info.get('scalars', {}), 'vectors': info.get('vectors', {}),
                         'sample_gltf': str(SLICE / 'pistol/Pistol_Maliwan_2_Fire_seed1.gltf'),
                         'candidates_gltf': str(SLICE / 'pistol/Pistol_Maliwan_Candidates.gltf'),
                         'sample_fragments': pistol['sample']['fragments'] if 'sample' in pistol else [],
                         'candidate_fragments': [s['fragment'] for s in pistol['candidate_sections']['fragments']]
                         if 'candidate_sections' in pistol else []}
    write_json(SLICE / 'editor_job.json', job)
    print(f'editor-job: wrote {SLICE / "editor_job.json"}')
    return job


# ---------------------------------------------------------------------------------------- manifest


def manifest_step(args):
    identity = read_json(SLICE / 'npc_identity.json')
    inventory = read_json(SLICE / 'npc_extract.json')
    check = read_json(SLICE / 'npc_crosscheck.json', {})
    anims = read_json(SLICE / 'npc_anims.json', {})
    pistol = read_json(SLICE / 'pistol/candidates.json', {})
    editor = {m: read_json(SLICE / f'editor_report_{m}.json', {}) for m in ('npcs', 'pistol', 'anims', 'preview')}
    manifest = {'schema': 'openwillow.slice_npc_assets/1', 'generated': now(), 'tool': TOOL,
                'status_note': ('UE paths below exist only if the matching import_status is "imported". Materials are '
                                "textures bound by UModel's guess, not verified material graphs. UNVERIFIED: MD5-to-UE "
                                'animation mapping beyond structural checks, weapon part rules (tools/weapon_recipe.py).'),
                'npcs': {}, 'pistol': {}}
    for name, npc in identity['npcs'].items():
        rec = editor['npcs'].get(name, {})
        ar = editor['anims'].get(name, {})
        roles = (anims.get('npcs', {}).get(name, {}) or {}).get('roles', {})
        entry = {
            'source_identity': {'pawn': npc['pawn'], 'skeletal_mesh': npc['skeletal_mesh'],
                                'anim_sets': npc['anim_sets'], 'anim_tree': npc['anim_tree'],
                                'mesh_material_slots': [{'name': s['name'], 'material_used': s['material'], 'identity': s.get('identity'),
                                                         'textures_bound_by_umodel_guess': s['textures'],
                                                         'umodel_parent_material': s.get('parent')}
                                                        for s in npc.get('mesh_material_slots', [])]},
            'import_status': rec.get('status', 'not imported'),
            'ue': {'skeletal_mesh': rec.get('skeletal_mesh'), 'skeleton': rec.get('skeleton'),
                   'physics_asset': rec.get('physics_asset'), 'materials': rec.get('materials', []),
                   'textures': rec.get('textures', []),
                   'preview_level': (editor['preview'].get(name) or {}).get('level'),
                   'bounds_extent_cm': rec.get('bounds_extent'), 'bone_count': rec.get('bone_count'),
                   'material_slots': rec.get('material_slots')},
            'anims': {role: {**info, 'ue_asset': (ar.get('assets') or {}).get(role),
                             'import_status': 'imported' if (ar.get('assets') or {}).get(role) else 'not imported'}
                      for role, info in roles.items()},
        }
        manifest['npcs'][name] = entry
    prec = (editor['pistol'] or {}).get('pistol', {})
    preview_pistol = (editor['preview'] or {}).get('pistol', {})
    manifest['pistol'] = {
        'source_identity': {k: v for k, v in identity['pistol'].items()},
        'candidates': {b: {'material': v['material'], 'slots': v['slots'], 'rules_status': pistol.get('rules_status')}
                       for b, v in pistol.get('balances', {}).items()},
        'sample': pistol.get('sample'), 'candidate_sections': pistol.get('candidate_sections'),
        'import_status': prec.get('status', 'not imported'),
        'ue': prec,
        'fresh_session_check': preview_pistol,
    }
    # Flat list of the UE paths the integration side needs.
    use = {}
    for name, entry in manifest['npcs'].items():
        ue = entry['ue']
        use[name] = {'skeletal_mesh': ue['skeletal_mesh'], 'skeleton': ue['skeleton'], 'materials': ue['materials'],
                     'anims': {role: a['ue_asset'] for role, a in entry['anims'].items() if a['ue_asset']},
                     'preview_level': ue['preview_level'], 'import_status': entry['import_status']}
    meshes = prec.get('meshes', {})
    use['MaliwanPistol'] = {
        'rolled_sample_mesh': (meshes.get('sample') or {}).get('skeletal_mesh'),
        'candidate_sections_mesh': (meshes.get('candidates') or {}).get('skeletal_mesh'),
        'material': prec.get('material'),
        'candidate_section_slot_by_fragment': {s['fragment']: s['ue_material_slot'] for s in
                                               (pistol.get('candidate_sections') or {}).get('fragments', [])},
        'parts_by_fragment': (pistol.get('candidate_sections') or {}).get('parts_by_fragment'),
        'import_status': prec.get('status', 'not imported')}
    manifest['use'] = use
    write_json(SLICE / 'npc_assets.json', manifest)
    print(f'manifest: wrote {SLICE / "npc_assets.json"}')


def all_step(args):
    identity_step(args)
    extract_step(args)
    pistol_step(args)
    crosscheck_step(args)
    editor_job_step(args)


STEPS = {'identity': identity_step, 'extract': extract_step, 'crosscheck': crosscheck_step, 'anims': anims_step,
         'pistol': pistol_step, 'editor-job': editor_job_step, 'manifest': manifest_step, 'all': all_step}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('step', choices=sorted(STEPS))
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', help='Borderlands 2 folder (default $OPENWILLOW_BL2)')
    parser.add_argument('--umodel', help='umodel.exe (default $OPENWILLOW_UMODEL)')
    args = parser.parse_args()
    STEPS[args.step](args)


if __name__ == '__main__':
    main()
