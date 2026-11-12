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
# 4. The scout centre: the plaza's centre from the views named plaza_* in local/sanctuary/inspection-views.json. No tool generates
#    those: the file is written by hand as the input of tools/test_ue_viewer.ps1 -Inspect (docs/TOOLING.md, "Repeatable inspection
#    viewpoints"), and other inspection work replaces its views, so a worktree's copy may not have them. The main checkout's copy does;
#    pass it with --views.
#
# Usage: python tools/prepare_range.py [--reader build/Release/ow-package.exe] [--levels 1-50] [--no-vm]
# A relative path given to --reader, --target, --out, --terrain or --views is taken from the repository root, not the current folder.
import argparse
import json
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Range kind -> (combat-run kind or package/archetype path, the VM's row class). The marauder and the nomad shoot, which the range's
# brain does not do; they are not here.
KINDS = {
    "bullymong": ("regular", "PrimalBeast"),
    "bullymong_adult": ("adult", "PrimalBeastAdult"),
    "bullymong_baby": ("baby", "PrimalBeastBaby"),
    "bullymong_slinger": ("ranged", "PrimalBeast_Ranged"),
    "psycho": ("Ice_Dynamic/GD_Psycho.Character.Pawn_Psycho", "Psycho"),
}
# What each swings (combat-run swing:<pawn>:<kind or melee definition path>): the bullymongs' named blows, the slinger's rock, the
# psycho's melee definition.
SWINGS = {"regular": "melee", "adult": "melee", "baby": "melee", "ranged": "slinger",
          "Ice_Dynamic/GD_Psycho.Character.Pawn_Psycho": "GD_PsychoShared.Melee.MeleeDef_PsychoShared_Melee"}
# The pawn balance of an archetype given by path (the named kinds carry theirs in the VM).
BALANCES = {"Ice_Dynamic/GD_Psycho.Character.Pawn_Psycho": "GD_Population_Psycho.Balance.PawnBalance_Psycho"}
# How far a kind's melee numbers are checked. Only the bullymongs have live rows (COMBAT_MELEE.md); others are the VM's answer alone.
MELEE_LIVE_KINDS = {"bullymong", "bullymong_adult", "bullymong_baby", "bullymong_slinger"}
# Where a kind's stock attack ranges are read (the swing-start rule, melee_start_range): its AI definition's melee actions.
AI_PACKAGE = "Ice_Dynamic"
AI_DEFINITIONS = {"bullymong": "GD_PrimalBeast.Character.AIDef_PrimalBeast", "bullymong_adult": "GD_PrimalBeastAdult.Character.AIDef_PrimalBeastAdult",
                  "bullymong_baby": "GD_PrimalBeastBaby.Character.AIDef_PrimalBeastBaby",
                  "bullymong_slinger": "GD_PrimalBeast_Ranged.Character.AIDef_PrimalBeast_Ranged", "psycho": "GD_Psycho.Character.AIDef_Psycho"}
# Which imported asset (local/phaselock/target/host_target.json) a kind uses. One imported bullymong serves every bullymong kind (host
# stand-in: the adult, baby and slinger have their own stock meshes, which are not imported). A kind without its asset stays on the host's
# engine shapes.
KIND_ASSETS = {"bullymong": "Bullymong", "bullymong_adult": "Bullymong", "bullymong_baby": "Bullymong", "bullymong_slinger": "Bullymong",
               "psycho": "Psycho"}
# A kind's own collision cylinder (radius, half height) where it borrows another kind's asset, so the asset's cylinder is not its own. As
# in tools/prepare_phaselock_target.py, a live read wins over the asset. The baby: CharClass_PrimalBeastBaby's cylinder, radius 64 and
# height 48 on every baby pawn read in session 2 of 2026-11-12 (Three Horns Divide, Maya level 8, six den recordings); its 189 swing calls
# all came at 106.0 uu or more from Maya, which is 64 plus her radius 42 (standing). The mesh stays the borrowed adult's (host stand-in).
OBSERVED_COLLISION = {"bullymong_baby": (64.0, 48.0)}
# Chase run and patrol walk per kind (uu/s): GroundSpeed and GroundSpeed x WalkingPct (0.25) of the kind's pawn, read live in every den
# recording of session 2 of 2026-11-12 (GroundSpeed 500 for the adult and the slinger, 300 for the baby). The slinger's moving medians were
# 500 in 4 of 4 trials, the adult's in 6 of 11 (leaps and knockback in the rest), the baby's 300 in 4 of 8. The regular bullymong is not
# here: its field also reads 500, but its measured chase median was 375 in 4 of 6 trials, unexplained (UNVERIFIED), so it keeps the
# stand-ins of kind_entry (run 400, walk 150), as does the psycho (no read).
OBSERVED_SPEED = {"bullymong_adult": (500.0, 125.0), "bullymong_slinger": (500.0, 125.0), "bullymong_baby": (300.0, 75.0)}
# The documented step that writes the worn pieces' placements into host_target.json (docs/verification/COMBAT_RANGE.md). An earlier editor
# job can rewrite that file without them; the range must then not be built on enemies that lost their worn parts (2026-11-12).
ATTACH_STEP = ("powershell -File tools/import_phaselock_target.ps1 -Steps attach (its last step is python tools/prepare_phaselock_target.py "
               "attachments), then python tools/prepare_range.py")
# Extra spawns of kinds that join the range only once their own mesh is imported (an engine-shape psycho would prove nothing).
SPAWNS_WITH_ASSET = [{"kind": "psycho", "count": 1}]
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
        # What the pawn wears on its bones (the import writes them once the pieces are imported; the host attaches them as it does for a
        # test pawn).
        if target.get("attachments"):
            entry["attachments"] = target["attachments"]
        for key in ("mesh_offset_z", "mesh_scale", "collision_radius", "collision_height"):
            if key in target:
                entry[key] = float(target[key])
        if entry["idle"]:
            entry.update(find_clips(entry["idle"], listing))
    return entry


def worn_counts(targets, identity, target_path, identity_path):
    """Asset -> how many worn pieces its imported pawn carries, from the import's own read of the pawn's body composition (identity.json,
    targets.<asset>.attachments, which tools/prepare_phaselock_target.py checks against its expected set). The truth comes from the data,
    not from a count kept here: an asset whose pawn wears nothing gets 0 and is fine. Stops (SystemExit) when host_target.json carries a
    different number of placements than the identity has pieces, or the identity has no entry for an imported asset, because the manifest
    would then hold enemies without their worn parts and say nothing."""
    counts, problems = {}, []
    for asset, target in sorted(targets.items()):
        if not isinstance(target, dict) or not target.get("mesh"):
            continue
        entry = (identity.get("targets") or {}).get(asset)
        if not isinstance(entry, dict):
            problems.append("%s: no entry in %s" % (asset, identity_path))
            continue
        # The import's identity step always writes the key; one without it (an identity.json from before 2026-11-06) knows nothing about
        # worn pieces, and reading that as "wears nothing" would let a target without placements through.
        if not isinstance(entry.get("attachments"), list):
            problems.append("%s: no attachments list in %s (written before the import read worn pieces)" % (asset, identity_path))
            continue
        counts[asset] = len(entry["attachments"])
        placed = len(target.get("attachments") or [])
        if placed != counts[asset]:
            problems.append("%s: %d attachment placements in %s, %d worn pieces in %s" % (asset, placed, target_path, counts[asset], identity_path))
    if problems:
        raise SystemExit("the imported enemies' worn attachments do not match the import (%s): the placements were lost (for example by "
                         "an editor job that rewrote host_target.json) or never written, or the identity is older than the import's read of worn "
                         "pieces. No manifest was written. Re-run %s"
                         % ("; ".join(problems), ATTACH_STEP))
    return counts


def apply_observed(name, entry):
    """A kind's own live-read numbers over what its (possibly borrowed) asset gave."""
    if name in OBSERVED_COLLISION:
        entry["collision_radius"], entry["collision_height"] = OBSERVED_COLLISION[name]
        entry["collision_source"] = "live read 2026-11-12 (own cylinder; the asset is borrowed)"
    if name in OBSERVED_SPEED:
        entry["run_speed"], entry["walk_speed"] = OBSERVED_SPEED[name]
        entry["speed_source"] = "live read 2026-11-12 (GroundSpeed, WalkingPct 0.25)"
    return entry


def from_repo(path):
    """A command-line path as given when absolute; a relative one from the repository root, so the tool works from any folder
    (the usage line's build/Release/ow-package.exe is not a path from the folder the tool was run in)."""
    return path if os.path.isabs(path) else os.path.join(REPO, path)


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
COMMAND_LIMIT = 30000   # a Windows command line holds 32,767 characters; the psycho's spawn steps name long paths
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


def spawns_for(kinds):
    """The range's population: the default spawns, then each kind in SPAWNS_WITH_ASSET whose asset is imported. New ones go last, so the
    enemies the range test's steps use by index stay the same."""
    return SPAWNS + [s for s in SPAWNS_WITH_ASSET if kinds.get(s["kind"], {}).get("mesh")]


def melee_status(kind):
    """How far a kind's melee numbers are checked (written to the manifest with the numbers)."""
    if kind in MELEE_LIVE_KINDS:
        return "VM rows reproduce live rows (COMBAT_MELEE.md)"
    return "VM only, UNVERIFIED (no live row)"


def spawn_step(vm_kind, level):
    """The combat-run spawn step: a named kind, or an archetype path with its stage and pawn balance."""
    balance = BALANCES.get(vm_kind)
    return "spawn:%s:%d:%d:%s" % (vm_kind, level, level, balance) if balance else "spawn:%s:%d" % (vm_kind, level)


def kind_steps(vm_kind, levels, first_pawn):
    """The steps of one kind. Per level: a Maya of that level (refilled), the pawn, its swing at her, the kill; then the stage-offset
    sample, more kills of fresh pawns at one level. Pawn indices count up from first_pawn."""
    steps, pawn = [], first_pawn
    for level in levels:
        steps += ["maya:%d" % level, spawn_step(vm_kind, level), "swing:%d:%s" % (pawn, SWINGS[vm_kind]),
                  "hit:%d:999999:%s" % (pawn, LETHAL)]
        pawn += 1
    steps.append("maya:%d" % SAMPLE_LEVEL)
    for _ in range(SAMPLE_KILLS):
        steps += [spawn_step(vm_kind, SAMPLE_LEVEL), "hit:%d:999999:%s" % (pawn, LETHAL)]
        pawn += 1
    return steps


def step_batches(levels, base_length, limit=COMMAND_LIMIT):
    """The kinds grouped into VM runs that fit one Windows command line (each run loads the packages, so as few as fit)."""
    batches, current, length = [], [], base_length
    for vm_kind, _ in KINDS.values():
        size = sum(len(step) + 3 for step in kind_steps(vm_kind, levels, 0)) + 200   # + room for longer pawn indices
        if current and length + size > limit:
            batches.append(current)
            current, length = [], base_length
        current.append(vm_kind)
        length += size
    if current:
        batches.append(current)
    return batches


def merge_runs(runs):
    """One result from several VM runs: rows together, pawns in order, and each hit step's pawn index moved to match."""
    merged = {"rows": {}, "pawns": [], "steps": []}
    for run in runs:
        offset = len(merged["pawns"])
        merged["rows"].update(run.get("rows", {}))
        merged["pawns"] += run.get("pawns", [])
        for step in run.get("steps", []):
            parts = step.get("step", "").split(":")
            if parts[0] == "hit":
                parts[1] = str(int(parts[1]) + offset)
            merged["steps"].append(dict(step, step=":".join(parts)))
    return merged


def melee_start_range(actions):
    """The farthest a kind's plain melee actions start from, given (animation name, range maximum) per attack action. By the animation's
    name (a host choice, UNVERIFIED as the tree's own choice): element variants and the throws are not the swing the range plays."""
    ranges = [high for anim, high in actions if high and "melee" in anim.lower() and "slag" not in anim.lower()]
    return max(ranges) if ranges else 0.0


def read_attack_actions(ai_definition, dump):
    """(animation name, range maximum) of each attack action of an AI definition; dump(path) gives an object's properties."""
    actions, seen = [], set()
    for node in dump(ai_definition).get("nodelist") or []:
        action = node.get("Action")
        if not action or action in seen or "AnimAttack" not in action.rsplit(".", 1)[-1]:
            continue
        seen.add(action)
        props = dump(action)
        high = 0.0
        for limit in props.get("limits") or []:
            if "range" in limit.rsplit(".", 1)[-1].lower():
                high = float((dump(limit).get("range") or {}).get("MaxVal") or 0.0)
        actions.append((str(props.get("attackanim") or "").rsplit(".", 1)[-1], high))
    return actions


def object_reader(reader, package_path, cooked):
    """dump(path): an object's properties by path, through ow-package (its export list, then --object-dump with the established prefixes)."""
    listing = subprocess.run([reader, package_path, "--exports"], capture_output=True, text=True, encoding="utf-8", check=True)
    index = {row["path"]: row["index"] for row in json.loads(listing.stdout)}

    def dump(path):
        if path not in index:
            raise KeyError(path)
        problem = ""
        for prefix in ("4", "8"):
            run = subprocess.run([reader, package_path, "--object-dump", str(index[path]), prefix, "--cooked", cooked],
                                 capture_output=True, text=True, encoding="utf-8")
            if run.returncode == 0:
                return json.loads(run.stdout)["properties"]
            problem = run.stderr.strip()
        raise RuntimeError("%s: %s" % (path, problem))
    return dump


def start_ranges(reader):
    """Kind -> melee_start_range. A kind that cannot be read is left out and named: tools/check_range.py then stops a test run on it."""
    cooked = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    out = {}
    try:
        dump = object_reader(reader, os.path.join(cooked, AI_PACKAGE + ".upk"), cooked)
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print("stock attack ranges not read (%s)" % error)
        return out
    for kind, definition in AI_DEFINITIONS.items():
        try:
            found = melee_start_range(read_attack_actions(definition, dump))
        except (KeyError, RuntimeError, OSError, ValueError) as error:
            print("%s: stock attack range not read (%s)" % (kind, error))
            continue
        if found:
            out[kind] = found
        else:
            print("%s: no melee attack action with a range in %s" % (kind, definition))
    return out


def run_vm(reader, levels):
    cooked = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    if not os.path.isdir(cooked):
        raise SystemExit("set OPENWILLOW_BL2 to the installed Borderlands 2 folder")
    base = [reader, os.path.join(cooked, "Sanctuary_Dynamic.upk"), "--combat-run", "--cooked", cooked]
    runs = []
    for vm_kinds in step_batches(levels, len(" ".join(base))):
        steps = []
        for vm_kind in vm_kinds:
            steps += kind_steps(vm_kind, levels, sum(1 for s in steps if s.startswith("spawn:")))
        out = subprocess.run(base + steps, capture_output=True, text=True, check=True)
        runs.append(json.loads(out.stdout))
    return merge_runs(runs)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", default=os.path.join(REPO, "build", "Release", "ow-package.exe"))
    parser.add_argument("--levels", default="1-50")
    parser.add_argument("--target", default=os.path.join(REPO, "local", "phaselock", "target", "host_target.json"))
    parser.add_argument("--identity", help="the import's identity.json (worn pieces per asset); default: next to --target")
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
    args.identity = args.identity or os.path.join(os.path.dirname(from_repo(args.target)), "identity.json")
    for name in ("reader", "target", "identity", "out", "terrain", "views"):
        setattr(args, name, from_repo(getattr(args, name)))
    args.out = local_output(args.out)
    run = {} if args.no_vm else run_vm(args.reader, parse_levels(args.levels))
    targets = {}
    if os.path.isfile(args.target):
        with open(args.target, encoding="utf-8") as f:
            targets = json.load(f)
    identity = {}
    if targets and os.path.isfile(args.identity):
        try:
            with open(args.identity, encoding="utf-8") as f:
                identity = json.load(f)
        except (OSError, ValueError) as error:
            raise SystemExit("cannot read the import's identity %s (%s): no manifest was written. Re-run %s" % (args.identity, error, ATTACH_STEP))
        if not isinstance(identity, dict):
            raise SystemExit("the import's identity %s is not a JSON object: no manifest was written. Re-run %s" % (args.identity, ATTACH_STEP))
    worn = worn_counts(targets, identity, args.target, args.identity)
    kinds = {}
    ranges = {} if args.no_vm else start_ranges(args.reader)
    loot = loot_tables(run)
    for name, (vm_kind, row_class) in KINDS.items():
        health = health_table(run, row_class)
        source = "combat-run spawn:%s (VM spawn health, UNVERIFIED)" % vm_kind if health else "no VM run"
        kinds[name] = kind_entry(name.replace("_", " ").title(), targets.get(KIND_ASSETS.get(name)), health, source,
                                 arrival=health_table(run, row_class, "arrival"), ratio=health_table(run, row_class, "ratio"),
                                 loot=loot.get(row_class))
        apply_observed(name, kinds[name])
        # How many worn pieces the kind's enemies must carry (tools/check_range.py's preflight holds the manifest to it); engine shapes wear
        # none.
        kinds[name]["attachments_expected"] = worn.get(KIND_ASSETS.get(name), 0) if kinds[name]["mesh"] else 0
        kinds[name]["melee_status"] = melee_status(name)
        if name in ranges:
            kinds[name]["melee_start_range"] = ranges[name]
    # The spot comes only from ignored local data: both files are needed for the default spot to exist.
    candidates, centre, problem = [], None, ""
    if not os.path.isfile(args.views):
        problem = "no views file at %s" % args.views
    else:
        with open(args.views, encoding="utf-8") as f:
            centre = scout_centre(json.load(f))
        if not centre:
            problem = "%s has no usable plaza_* views (at least one looking along X and one along Y)" % args.views
    if centre:
        if os.path.isfile(args.terrain):
            with open(args.terrain, encoding="utf-8") as f:
                candidates = spot_candidates(json.load(f), centre)
        if not candidates:
            problem = "no terrain stand points in %s (tools/prepare_terrain.py writes it during the level import)" % args.terrain
    if problem and not args.allow_missing_spot_data:
        raise SystemExit("no spot data: " + problem + ". The plaza_* views are not generated by any tool: inspection-views.json is written "
                         "by hand for tools/test_ue_viewer.ps1 -Inspect and other inspection work replaces its views. Pass --views with "
                         "a copy that has them (the main checkout's local/sanctuary/inspection-views.json), and --terrain if needed.")
    if candidates and not 0 <= args.spot_candidate < len(candidates):
        raise SystemExit("--spot-candidate %d is outside the %d candidates" % (args.spot_candidate, len(candidates)))
    manifest = {"schema": "ow-range-v1", "respawn_seconds": 8, "spawns": spawns_for(kinds), "kinds": kinds, "spot_candidates": candidates,
                "scout_centre": centre, "default_spot_candidate": args.spot_candidate if candidates else -1,
                "assets": "imported" if targets else "missing (run tools/import_phaselock_target.ps1)"}
    for spawn in SPAWNS_WITH_ASSET:
        if spawn not in manifest["spawns"]:
            print("NOTE: no %s on the range: %s has no imported %s (tools/import_phaselock_target.ps1 stops at its first failed step: read its output)"
                  % (spawn["kind"], args.target, KIND_ASSETS[spawn["kind"]]))
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=1)
    for name, kind in kinds.items():
        levels = kind["health_by_level"]
        print("%s: %d health levels, %d melee levels, %d loot levels, assets %s, walk %s, hit %s, melee %s, worn %d of %d" % (name, len(levels), len(kind["melee_arrival_by_level"]), len(kind["loot_by_level"]), "yes" if kind["mesh"] else "no",
              kind["walk"] or "-", kind["hit"] or "-", kind["melee"] or "-", len(kind.get("attachments") or []), kind["attachments_expected"]))
    print("spot candidates: %d (%d on the gate terrain), default candidate index %d" % (len(candidates),
          sum(c["gate_terrain"] for c in candidates), args.spot_candidate if candidates else -1))
    print("wrote " + args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
