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
#    Then a lethal hit kills the pawn: its death roll (COMBAT_LOOT.md) reports every pool of its item pool list with the chance
#    the VM evaluated; the host rolls those chances itself (loot_by_level) and turns a weapon pool into ow.RollGun's roll.
# 2. The imported enemy assets: local/phaselock/target/host_target.json (tools/import_phaselock_target.ps1, the bullymong).
#    Extra clips (walk, run, hit, melee) are picked by name from the imported AnimSequence files next to the idle clip; the
#    choice is a name match (UNVERIFIED that it is the clip the game plays) and the manifest says which file was taken.
# 3. Spot candidates for the range: the standable terrain points of local/sanctuary/terrain-runtime.json (the terrain probes the
#    level import wrote), with Sanctuary_P Terrain_2 first: the terrain the gate's floor lies in (SANCTUARY_ARTIFACT_PASS.md,
#    "the void inside the gate lies within Sanctuary_P Terrain_2"), then the other terrains by distance from the plaza. The host
#    tries them in order (a textured floor where the whole formation fits) before its ring scout. The manifest also names the
#    default candidate by its index (default_spot_candidate): an index reveals no coordinates, and the host's rules find the same
#    point from it, so no map coordinate is ever written to a tracked file.
# 4. The scout centre: the plaza's centre from the level import's inspection views (local/sanctuary/inspection-views.json).
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


def kind_entry(label, target, health, source, listing=None, arrival=None, ratio=None, loot=None):
    entry = {"label": label, "mesh": "", "idle": "", "death": "", "walk": "", "run": "", "hit": "", "melee": "",
             "mesh_offset_z": 0.0, "mesh_scale": 1.0, "collision_radius": 0.0, "collision_height": 0.0,
             "head_bone": "", "head_radius": 18.0, "walk_speed": 150.0, "patrol_radius": 500.0,
             "shield": 0.0, "shield_delay": 0.0, "shield_rate": 0.0,
             "run_speed": 400.0,
             "health_by_level": {str(k): v for k, v in health.items()},
             "melee_arrival_by_level": {str(k): v for k, v in (arrival or {}).items()},
             "melee_ratio_by_level": {str(k): v for k, v in (ratio or {}).items()},
             "loot_by_level": {str(k): v for k, v in sorted((loot or {}).items())}, "source": source}
    if target:
        for key in ("mesh", "idle", "death"):
            entry[key] = target.get(key, "")
        for key in ("mesh_offset_z", "mesh_scale", "collision_radius", "collision_height"):
            if key in target:
                entry[key] = float(target[key])
        if entry["idle"]:
            entry.update(find_clips(entry["idle"], listing))
    return entry


def local_output(path, repo=REPO):
    """The output file, resolved; refuses anything not strictly below the repository's ignored local/ folder, because the
    manifest holds game-derived names, numbers and coordinates that must never land in a tracked file (the pattern of
    tools/prepare_skill_effects.py)."""
    resolved = os.path.realpath(path)
    local = os.path.realpath(os.path.join(repo, "local"))
    if os.path.commonpath([resolved, local]) != local or resolved == local:
        raise SystemExit("refusing --out %s: it must be inside %s (ignored game-derived output)" % (path, local))
    return resolved
LETHAL = "GD_Impact.DamageType.DmgType_Normal:Shotgun"   # the kill shot of COMBAT_LOOT's runs (damage type and source)
# The item stage carries a random offset for some kinds (the adult's Random_Minus02-Plus01, COMBAT_LOOT): it is sampled over
# SAMPLE_KILLS kills of fresh pawns at SAMPLE_LEVEL, and the offset's range is applied to every level.
SAMPLE_LEVEL = 20
SAMPLE_KILLS = 24
# What the host does with a pool of the death roll, by the pool's name (host choice): a weapon pool goes to the weapon
# generator (ow.RollGun's roll), money and ammo pools to the range's own pickups; anything else is logged as not hosted.
POOL_KINDS = (("GunsAndGear", "weapon"), ("Money", "money"), ("Ammo", "ammo"))


def pool_kind(pool):
    """By the pool's own name (the last path part): the package part (e.g. AmmoAndResourcePools) names a whole group."""
    name = pool.rsplit(".", 1)[-1]
    return next((kind for key, kind in POOL_KINDS if key in name), "other")


def stage_offsets(run_json):
    """{row class: (lowest, highest)} of item stage minus pawn level over every kill of the run (the sampled offset range)."""
    pawns = run_json.get("pawns", [])
    out = {}
    for step in run_json.get("steps", []):
        loot = (step.get("result") or {}).get("loot") if step.get("step", "").startswith("hit:") else None
        index = int(step["step"].split(":")[1]) if loot else -1
        if not loot or index >= len(pawns):
            continue
        leaf = pawns[index].get("archetype", "").rsplit(".", 1)[-1]
        row_class = leaf[5:] if leaf.startswith("Pawn_") else leaf
        offset = int(loot.get("item_stage", 0)) - int(pawns[index].get("level", 0))
        low, high = out.get(row_class, (offset, offset))
        out[row_class] = (min(low, offset), max(high, offset))
    return out


def loot_tables(run_json):
    """{row class: {level: {"item_stage", "item_stage_min", "item_stage_max", "pools": [{"pool", "chance", "kind"}]}}} from the
    death rolls of the kill steps; the stage range is the level plus the sampled offset range (at least 1)."""
    pawns = run_json.get("pawns", [])
    offsets = stage_offsets(run_json)
    out = {}
    for step in run_json.get("steps", []):
        if not step.get("step", "").startswith("hit:"):
            continue
        loot = (step.get("result") or {}).get("loot")
        index = int(step["step"].split(":")[1])
        if not loot or index >= len(pawns):
            continue
        archetype = pawns[index].get("archetype", "")
        row_class = archetype.rsplit(".", 1)[-1]
        row_class = row_class[5:] if row_class.startswith("Pawn_") else row_class
        level = int(pawns[index].get("level", 0))
        if level in out.get(row_class, {}):
            continue   # the first kill of a level is its row (the sample kills only widen the offset range)
        low, high = offsets.get(row_class, (0, 0))
        out.setdefault(row_class, {})[level] = {
            "item_stage": loot.get("item_stage", 0),
            "item_stage_min": max(1, level + low), "item_stage_max": max(1, level + high),
            "pools": [{"pool": p["pool"], "chance": p["chance"], "kind": pool_kind(p["pool"])} for p in loot.get("pools", [])]}
    return out


# The range's default spot, as an index into spot_candidates: the candidate the hub picked from the batch 6 survey frames (open snowy
# ground, no buildings). --spot-candidate overrides it.
DEFAULT_SPOT_CANDIDATE = 1


def scout_centre(views_json):
    """The plaza's centre from the inspection views named plaza_*: a view looking along Y (yaw near 90 or -90) stands on the plaza's
    X axis, one looking along X (yaw near 0 or 180) on its Y axis; the centre is where those axes cross. None without both kinds."""
    xs, ys = [], []
    for view in views_json.get("views", []):
        if not view.get("name", "").startswith("plaza"):
            continue
        location, rotation = view.get("location"), view.get("rotation")
        if not location or not rotation or len(location) < 2 or len(rotation) < 2:
            continue
        yaw = abs(float(rotation[1])) % 180.0
        if abs(yaw - 90.0) < 1.0:
            xs.append(float(location[0]))
        elif yaw < 1.0 or yaw > 179.0:
            ys.append(float(location[1]))
    if not xs or not ys:
        return None
    return {"x": sum(xs) / len(xs), "y": sum(ys) / len(ys)}


GATE_TERRAIN = "Sanctuary_P:TheWorld.PersistentLevel.Terrain_2"


def spot_candidates(terrain_json, centre):
    """Standable terrain points as range spot candidates: the gate's terrain first, then the rest by distance from the plaza."""
    out = []
    for probe in terrain_json.get("probes", []):
        source = probe.get("source", "")
        for stand in probe.get("stands", []):
            point = stand.get("point")
            if not point or len(point) != 3:
                continue
            gate = source == GATE_TERRAIN
            distance = ((point[0] - centre["x"]) ** 2 + (point[1] - centre["y"]) ** 2) ** 0.5
            out.append({"x": point[0], "y": point[1], "z": point[2], "source": source, "gate_terrain": gate,
                        "plaza_distance": round(distance, 1)})
    out.sort(key=lambda c: (not c["gate_terrain"], c["plaza_distance"]))
    return out


def run_vm(reader, levels):
    cooked = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    if not os.path.isdir(cooked):
        raise SystemExit("set OPENWILLOW_BL2 to the installed Borderlands 2 folder")
    # Per kind and level: a Maya of that level (refilled), the pawn, its swing at her. Pawn indices count up from 0.
    steps, pawn = [], 0
    for vm_kind, _ in KINDS.values():
        for level in levels:
            steps += ["maya:%d" % level, "spawn:%s:%d" % (vm_kind, level), "swing:%d:%s" % (pawn, SWINGS[vm_kind]),
                      "hit:%d:999999:%s" % (pawn, LETHAL)]
            pawn += 1
        # The stage-offset sample: more kills of fresh pawns of this kind at one level.
        steps.append("maya:%d" % SAMPLE_LEVEL)
        for _ in range(SAMPLE_KILLS):
            steps += ["spawn:%s:%d" % (vm_kind, SAMPLE_LEVEL), "hit:%d:999999:%s" % (pawn, LETHAL)]
            pawn += 1
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
    parser.add_argument("--views", default=os.path.join(REPO, "local", "sanctuary", "inspection-views.json"),
                        help="inspection views of the level import (the scout centre)")
    parser.add_argument("--spot-candidate", type=int, default=DEFAULT_SPOT_CANDIDATE,
                        help="index into spot_candidates of the range's default spot")
    parser.add_argument("--allow-missing-spot-data", action="store_true",
                        help="write the manifest without candidates or a scout centre (the range then needs -owrangeat or range.json)")
    parser.add_argument("--no-vm", action="store_true", help="skip the VM run (empty health tables: the host uses its stand-in)")
    args = parser.parse_args(argv)
    args.out = local_output(args.out)
    run = {} if args.no_vm else run_vm(args.reader, parse_levels(args.levels))
    targets = {}
    if os.path.isfile(args.target):
        with open(args.target, encoding="utf-8") as f:
            targets = json.load(f)
    # One imported bullymong asset serves every bullymong kind (host stand-in: the adult, baby and slinger have their own
    # stock meshes, which are not imported yet).
    target = next(iter(targets.values()), None)
    kinds = {}
    loot = loot_tables(run)
    for name, (vm_kind, row_class) in KINDS.items():
        health = health_table(run, row_class)
        source = "combat-run spawn:%s (VM spawn health, UNVERIFIED)" % vm_kind if health else "no VM run"
        kinds[name] = kind_entry(name.replace("_", " ").title(), target, health, source,
                                 arrival=health_table(run, row_class, "arrival"), ratio=health_table(run, row_class, "ratio"),
                                 loot=loot.get(row_class))
    # The spot comes only from ignored local data: both files are needed for the default spot to exist.
    candidates, centre = [], None
    if os.path.isfile(args.views):
        with open(args.views, encoding="utf-8") as f:
            centre = scout_centre(json.load(f))
    if centre and os.path.isfile(args.terrain):
        with open(args.terrain, encoding="utf-8") as f:
            candidates = spot_candidates(json.load(f), centre)
    if not candidates and not args.allow_missing_spot_data:
        raise SystemExit("no spot data: need %s and %s (the level import writes them; copy local/sanctuary from a prepared checkout, "
                         "or pass --terrain and --views)" % (args.terrain, args.views))
    if candidates and not 0 <= args.spot_candidate < len(candidates):
        raise SystemExit("--spot-candidate %d is outside the %d candidates" % (args.spot_candidate, len(candidates)))
    manifest = {"schema": "ow-range-v1", "respawn_seconds": 8, "spawns": SPAWNS, "kinds": kinds, "spot_candidates": candidates,
                "scout_centre": centre, "default_spot_candidate": args.spot_candidate if candidates else -1,
                "assets": "imported" if target else "missing (run tools/import_phaselock_target.ps1)"}
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=1)
    for name, kind in kinds.items():
        levels = kind["health_by_level"]
        print("%s: %d health levels, %d melee levels, %d loot levels, assets %s, walk %s, hit %s, melee %s" % (name, len(levels), len(kind["melee_arrival_by_level"]), len(kind["loot_by_level"]), "yes" if kind["mesh"] else "no",
              kind["walk"] or "-", kind["hit"] or "-", kind["melee"] or "-"))
    print("spot candidates: %d (%d on the gate terrain), default candidate index %d" % (len(candidates),
          sum(c["gate_terrain"] for c in candidates), args.spot_candidate if candidates else -1))
    print("wrote " + args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
