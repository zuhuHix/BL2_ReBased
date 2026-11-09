"""What the NPC editor steps will import, checked before they change anything.

AI-assisted (Claude), 2026-11-09. Plain Python (no ``unreal``): tests/editor_inputs_test.py runs it, and the editor scripts
(tools/slice_npc_editor.py ``npcs`` mode, tools/phaselock_target_editor.py, tools/phaselock_target_attach_editor.py) call it first.

Why: Unreal's importer asserts (editor exit 3) on a missing source instead of failing cleanly, and a job written without the UModel
exports holds the string "None" for every glTF (2026-11-09, runner-z36). Each check covers only the files its step actually imports, so
a slot the zone step keeps plain on purpose (Marcus: no p_Masks) is not an error.
"""
from pathlib import Path


def file_problem(path):
    """Why a source cannot be imported, or None: no path in the job (None, "None", ""), not a file, or an empty file."""
    if path is None or str(path).strip() in ('', 'None'):
        return 'no file in the job (the UModel exports were missing when the job was written)'
    file = Path(path)
    if not file.is_file():
        return f'{path} is missing'
    if file.stat().st_size == 0:
        return f'{path} is empty'
    return None


def producer(job):
    """The command that writes this job, for the failure message."""
    if 'phaselock' in str(job.get('tool', '')).lower():
        return 'powershell -File tools/import_phaselock_target.ps1 -Steps prepare'
    return 'powershell -File tools/seed_slice_npc_assets.ps1 -Steps extract,import'


def _report(job, step, problems):
    if not problems:
        return None
    return (f'{step}: inputs missing, nothing changed: ' + '; '.join(problems)
            + f' (rewrite the job after the UModel extract: {producer(job)})')


def npcs_problem(job):
    """slice_npc_editor.py npcs mode: every NPC's skeletal-mesh glTF and the p_Diffuse / p_Normal textures it imports."""
    problems = []
    for npc in job.get('npcs', []):
        if file_problem(npc.get('gltf')):
            problems.append(f'{npc["id"]} mesh: {file_problem(npc.get("gltf"))}')
        for slot in npc.get('slots', []):
            for parameter in ('p_Diffuse', 'p_Normal'):
                if parameter in slot.get('textures', {}) and file_problem(slot['textures'][parameter]):
                    problems.append(f'{npc["id"]}/{slot["slot"]} {parameter}: {file_problem(slot["textures"][parameter])}')
    return _report(job, 'npcs', problems)


ZONE_TEXTURES = ('p_Diffuse', 'p_Masks', 'p_Normal')


def zone_problem(job):
    """phaselock_target_editor.py: the slots it zones are those with p_Masks and p_Diffuse; it imports those two and p_Normal if listed.
    A slot whose job says p_Masks or p_Diffuse was expected but not exported (missing_textures, written by the phaselock job) would be
    skipped silently, so it is reported too. Slots with neither (kept plain) are fine."""
    problems = []
    for npc in job.get('npcs', []):
        for slot in npc.get('slots', []):
            where = f'{npc["id"]}/{slot["slot"]}'
            textures = slot.get('textures', {})
            lost = [p for p in slot.get('missing_textures', []) if p in ('p_Masks', 'p_Diffuse')]
            if lost:
                problems.append(f'{where}: {", ".join(lost)} not in the UModel output')
            if 'p_Masks' in textures and 'p_Diffuse' in textures:
                for parameter in ZONE_TEXTURES:
                    if parameter in textures and file_problem(textures[parameter]):
                        problems.append(f'{where} {parameter}: {file_problem(textures[parameter])}')
    return _report(job, 'zone', problems)


def attach_problem(job):
    """phaselock_target_attach_editor.py: every attachment's static-mesh glTF."""
    problems = [f'{npc["id"]}/{piece["id"]}: {file_problem(piece.get("gltf"))}'
                for npc in job.get('npcs', []) for piece in npc.get('attachment_meshes', []) if file_problem(piece.get('gltf'))]
    return _report(job, 'attach', problems)
