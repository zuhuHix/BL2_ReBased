#!/usr/bin/env python3
# AI-assisted (Claude). Writes the ignored range manifest local/range/range_manifest.json (schema ow-range-v1) that the
# UE host's gun range reads (OpenWillowRange.cpp, ow.Range).
#
# Two inputs, both local and game-derived (nothing here is written to a tracked file):
# 1. Stock enemy health per level: one `ow-package --combat-run` with a spawn:<kind>:<level> step per kind and level. The VM
#    runs the pawn's installed spawn script and health pool (COMBAT_HIT_CHAIN.md, COMBAT_DEATH.md); the rows
#    max_health.<class>.L<level> are its answer. That the VM's spawn health equals the game's is UNVERIFIED (no live row of
#    a fresh pawn's maximum was recorded).
#    The same run swings each kind's melee (or the slinger's rock) at a Maya of the same level: the rows
#    arrival.<class>.L<level> (the evaluated damage) and ratio.<class>.L<level> (what the damage pipeline passed on) reproduce
#    the live melee rows (COMBAT_MELEE.md); the host multiplies them for an enemy at Maya's level.
# 2. The imported enemy assets: local/phaselock/target/host_target.json (tools/import_phaselock_target.ps1, the bullymong).
#    Extra clips (walk, run, hit, melee) are picked by name from the imported AnimSequence files next to the idle clip; the
#    choice is a name match (UNVERIFIED that it is the clip the game plays) and the manifest says which file was taken.
# 3. Spot candidates for the range: the standable terrain points of local/sanctuary/terrain-runtime.json (the terrain probes the
#    level import wrote), with Sanctuary_P Terrain_2 first: the terrain the gate's floor lies in (SANCTUARY_ARTIFACT_PASS.md,
#    "the void inside the gate lies within Sanctuary_P Terrain_2"), then the other terrains by distance from the plaza. The host
#    tries them in order (a textured floor where the whole formation fits) before its ring scout.
#
# Usage: python tools/prepare_range.py [--reader build/Release/ow-package.exe] [--levels 1-50] [--no-vm]
import argparse
import json
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Range kind -> (combat-run kind, the VM's row class). The bandit and psycho are not among the VM's spawnable kinds yet.
KINDS = {
    "bullymong": ("regular", "PrimalBeast"),
    "bullymong_adult": ("adult", "PrimalBeastAdult"),
    "bullymong_baby": ("baby", "PrimalBeastBaby"),
    "bullymong_slinger": ("ranged", "PrimalBeast_Ranged"),
}
# The swing kind of each (combat-run swing:<pawn>:<kind>): the melee blow, the slinger's rock.
SWINGS = {"regular": "melee", "adult": "melee", "baby": "melee", "ranged": "slinger"}
# The range's default population (host choice).
SPAWNS = [{"kind": "bullymong", "count": 3}, {"kind": "bullymong_adult", "count": 1}]
# Clip roles -> name patterns tried in order against the imported AnimSequence file names (host choice, UNVERIFIED).
CLIP_PATTERNS = {
    "walk": [r"walk_?f(wd)?\b", r"walk"],
    "run": [r"run_?f(wd)?\b", r"run"],
    "hit": [r"flinch", r"hit_?react", r"hit"],
    "melee": [r"melee", r"attack", r"swipe"],
}


def parse_levels(text):
    first, _, last = text.partition("-")
    first = int(first)
    last = int(last) if last else first
    if first < 1 or last < first:
        raise ValueError("levels must be N or N-M with 1 <= N <= M")
    return list(range(first, last + 1))


def health_table(run_json, row_class, row="max_health"):
    """{level: value} from the combat-run rows <row>.<row_class>.L<level> (max_health, arrival, ratio)."""
    table = {}
    prefix = row + "." + row_class + ".L"
    for key, value in run_json.get("rows", {}).items():
        if key.startswith(prefix) and key[len(prefix):].isdigit():
            table[int(key[len(prefix):])] = float(value)
    return dict(sorted(table.items()))


def pick_clip(names, role):
    """The first imported clip name matching the role's patterns (case-insensitive), else ""."""
    for pattern in CLIP_PATTERNS[role]:
        for name in sorted(names):
            if re.search(pattern, name, re.IGNORECASE):
                return name
    return ""


def object_dir(object_path):
    """/Game/A/B/Clip.Clip -> /Game/A/B"""
    return object_path.rsplit("/", 1)[0]


def content_folder(object_path):
    """/Game/A/B/Clip.Clip -> <repo>/host/ue5/OpenWillow/Content/A/B"""
    rel = object_dir(object_path)[len("/Game/"):] if object_path.startswith("/Game/") else ""
    return os.path.join(REPO, "host", "ue5", "OpenWillow", "Content", *rel.split("/"))


def find_clips(idle_path, listing=None):
    """Role -> object path, from the .uasset names in the idle clip's folder (listing overrides the folder, for tests)."""
    if listing is None:
        folder = content_folder(idle_path)
        listing = [f[:-7] for f in os.listdir(folder) if f.endswith(".uasset")] if os.path.isdir(folder) else []
    idle_name = idle_path.rsplit(".", 1)[-1]
    names = [n for n in listing if n != idle_name and "phaselock" not in n.lower() and "death" not in n.lower()]
    base = object_dir(idle_path)
    return {role: (base + "/" + n + "." + n if n else "") for role, n in ((r, pick_clip(names, r)) for r in CLIP_PATTERNS)}


def kind_entry(label, target, health, source, listing=None, arrival=None, ratio=None):
    entry = {"label": label, "mesh": "", "idle": "", "death": "", "walk": "", "run": "", "hit": "", "melee": "",
             "mesh_offset_z": 0.0, "mesh_scale": 1.0, "collision_radius": 0.0, "collision_height": 0.0,
             "head_bone": "", "head_radius": 18.0, "walk_speed": 150.0, "patrol_radius": 500.0,
             "shield": 0.0, "shield_delay": 0.0, "shield_rate": 0.0,
             "run_speed": 400.0,
             "health_by_level": {str(k): v for k, v in health.items()},
             "melee_arrival_by_level": {str(k): v for k, v in (arrival or {}).items()},
             "melee_ratio_by_level": {str(k): v for k, v in (ratio or {}).items()}, "source": source}
    if target:
        for key in ("mesh", "idle", "death"):
            entry[key] = target.get(key, "")
        for key in ("mesh_offset_z", "mesh_scale", "collision_radius", "collision_height"):
            if key in target:
                entry[key] = float(target[key])
        if entry["idle"]:
            entry.update(find_clips(entry["idle"], listing))
    return entry


PLAZA = (0.0, 0.0)             # the scout centre; from ignored local data later in this stack
GATE_TERRAIN = "Sanctuary_P:TheWorld.PersistentLevel.Terrain_2"


def spot_candidates(terrain_json):
    """Standable terrain points as range spot candidates: the gate's terrain first, then the rest by distance from the plaza."""
    out = []
    for probe in terrain_json.get("probes", []):
        source = probe.get("source", "")
        for stand in probe.get("stands", []):
            point = stand.get("point")
            if not point or len(point) != 3:
                continue
            gate = source == GATE_TERRAIN
            distance = ((point[0] - PLAZA[0]) ** 2 + (point[1] - PLAZA[1]) ** 2) ** 0.5
            out.append({"x": point[0], "y": point[1], "z": point[2], "source": source, "gate_terrain": gate,
                        "plaza_distance": round(distance, 1)})
    out.sort(key=lambda c: (not c["gate_terrain"], c["plaza_distance"]))
    return out


def run_vm(reader, levels):
    cooked = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    if not os.path.isdir(cooked):
        raise SystemExit("set OPENWILLOW_BL2 to the installed Borderlands 2 folder")
    # Per kind and level: a Maya of that level (refilled), the pawn, its swing at her. Pawn indices count up from 0.
    steps = []
    for vm_kind, _ in KINDS.values():
        for level in levels:
            steps += ["maya:%d" % level, "spawn:%s:%d" % (vm_kind, level), "swing:%d:%s" % (len(steps) // 3, SWINGS[vm_kind])]
    command = [reader, os.path.join(cooked, "Sanctuary_Dynamic.upk"), "--combat-run", "--cooked", cooked] + steps
    out = subprocess.run(command, capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", default=os.path.join(REPO, "build", "Release", "ow-package.exe"))
    parser.add_argument("--levels", default="1-50")
    parser.add_argument("--target", default=os.path.join(REPO, "local", "phaselock", "target", "host_target.json"))
    parser.add_argument("--out", default=os.path.join(REPO, "local", "range", "range_manifest.json"))
    parser.add_argument("--terrain", default=os.path.join(REPO, "local", "sanctuary", "terrain-runtime.json"),
                        help="terrain probes of the level import (spot candidates); missing: the host scouts rings only")
    parser.add_argument("--no-vm", action="store_true", help="skip the VM run (empty health tables: the host uses its stand-in)")
    args = parser.parse_args(argv)
    run = {} if args.no_vm else run_vm(args.reader, parse_levels(args.levels))
    targets = {}
    if os.path.isfile(args.target):
        with open(args.target, encoding="utf-8") as f:
            targets = json.load(f)
    # One imported bullymong asset serves every bullymong kind (host stand-in: the adult, baby and slinger have their own
    # stock meshes, which are not imported yet).
    target = next(iter(targets.values()), None)
    kinds = {}
    for name, (vm_kind, row_class) in KINDS.items():
        health = health_table(run, row_class)
        source = "combat-run spawn:%s (VM spawn health, UNVERIFIED)" % vm_kind if health else "no VM run"
        kinds[name] = kind_entry(name.replace("_", " ").title(), target, health, source,
                                 arrival=health_table(run, row_class, "arrival"), ratio=health_table(run, row_class, "ratio"))
    candidates = []
    if os.path.isfile(args.terrain):
        with open(args.terrain, encoding="utf-8") as f:
            candidates = spot_candidates(json.load(f))
    manifest = {"schema": "ow-range-v1", "respawn_seconds": 8, "spawns": SPAWNS, "kinds": kinds, "spot_candidates": candidates,
                "assets": "imported" if target else "missing (run tools/import_phaselock_target.ps1)"}
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=1)
    for name, kind in kinds.items():
        levels = kind["health_by_level"]
        print("%s: %d health levels, %d melee levels, assets %s, walk %s, hit %s, melee %s" % (name, len(levels), len(kind["melee_arrival_by_level"]), "yes" if kind["mesh"] else "no",
              kind["walk"] or "-", kind["hit"] or "-", kind["melee"] or "-"))
    print("spot candidates: %d (%d on the gate terrain)" % (len(candidates), sum(c["gate_terrain"] for c in candidates)))
    print("wrote " + args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
