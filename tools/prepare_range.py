#!/usr/bin/env python3
# AI-assisted (Claude). Writes the ignored range manifest local/range/range_manifest.json (schema ow-range-v1) that the
# UE host's gun range reads (OpenWillowRange.cpp, ow.Range).
#
# Two inputs, both local and game-derived (nothing here is written to a tracked file):
# 1. Stock enemy health per level: one `ow-package --combat-run` with a spawn:<kind>:<level> step per kind and level. The VM
#    runs the pawn's installed spawn script and health pool (COMBAT_HIT_CHAIN.md, COMBAT_DEATH.md); the rows
#    max_health.<class>.L<level> are its answer. That the VM's spawn health equals the game's is UNVERIFIED (no live row of
#    a fresh pawn's maximum was recorded).
# 2. The imported enemy assets: local/phaselock/target/host_target.json (tools/import_phaselock_target.ps1, the bullymong).
#    Extra clips (walk, run, hit, melee) are picked by name from the imported AnimSequence files next to the idle clip; the
#    choice is a name match (UNVERIFIED that it is the clip the game plays) and the manifest says which file was taken.
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


def health_table(run_json, row_class):
    """{level: max health} from the combat-run rows max_health.<row_class>.L<level>."""
    table = {}
    prefix = "max_health." + row_class + ".L"
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


def kind_entry(label, target, health, source, listing=None):
    entry = {"label": label, "mesh": "", "idle": "", "death": "", "walk": "", "run": "", "hit": "", "melee": "",
             "mesh_offset_z": 0.0, "mesh_scale": 1.0, "collision_radius": 0.0, "collision_height": 0.0,
             "head_bone": "", "head_radius": 18.0, "walk_speed": 150.0, "patrol_radius": 500.0,
             "shield": 0.0, "shield_delay": 0.0, "shield_rate": 0.0,
             "health_by_level": {str(k): v for k, v in health.items()}, "source": source}
    if target:
        for key in ("mesh", "idle", "death"):
            entry[key] = target.get(key, "")
        for key in ("mesh_offset_z", "mesh_scale", "collision_radius", "collision_height"):
            if key in target:
                entry[key] = float(target[key])
        if entry["idle"]:
            entry.update(find_clips(entry["idle"], listing))
    return entry


def run_vm(reader, levels):
    cooked = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    if not os.path.isdir(cooked):
        raise SystemExit("set OPENWILLOW_BL2 to the installed Borderlands 2 folder")
    steps = ["spawn:%s:%d" % (vm_kind, level) for vm_kind, _ in KINDS.values() for level in levels]
    command = [reader, os.path.join(cooked, "Sanctuary_Dynamic.upk"), "--combat-run", "--cooked", cooked] + steps
    out = subprocess.run(command, capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", default=os.path.join(REPO, "build", "Release", "ow-package.exe"))
    parser.add_argument("--levels", default="1-50")
    parser.add_argument("--target", default=os.path.join(REPO, "local", "phaselock", "target", "host_target.json"))
    parser.add_argument("--out", default=os.path.join(REPO, "local", "range", "range_manifest.json"))
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
        kinds[name] = kind_entry(name.replace("_", " ").title(), target, health, source)
    manifest = {"schema": "ow-range-v1", "respawn_seconds": 8, "spawns": SPAWNS, "kinds": kinds,
                "assets": "imported" if target else "missing (run tools/import_phaselock_target.ps1)"}
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=1)
    for name, kind in kinds.items():
        levels = kind["health_by_level"]
        print("%s: %d levels, assets %s, walk %s, hit %s, melee %s" % (name, len(levels), "yes" if kind["mesh"] else "no",
              kind["walk"] or "-", kind["hit"] or "-", kind["melee"] or "-"))
    print("wrote " + args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
