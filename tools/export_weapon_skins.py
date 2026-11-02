#!/usr/bin/env python3
"""Export weapon skin material instances (MaterialInstanceConstant) with UE Viewer (UModel).

For every skin in the cooked census (`visual_census.json`, made by tools/weapon_part_census.py)
this runs UModel on the package that exports the MIC into a private root folder, exports any parent
MIC UModel did not write, copies each chain texture into the leaf's Texture2D folder, checks the
root with tools/render_weapon_previews.py `resolve_material`, and writes `inventory.json`.

Each skin gets its own root (`skins/<leaf>-<hash>/<Package>/...`). The resolver finds props by leaf
name only, and leaf names are not unique across the census (several skins share one), so a shared
tree could hand one skin another skin's material.

The output is game-derived. It must stay under the ignored `local/` tree; other locations are refused.
A chain that ends in a Material (not a MIC) is recorded as `base_parent`; that Material's defaults are
not exported, so the chain is only as complete as UModel's output.

Phases:
  1. Index: which census package exports each skin MIC (`ow-package.exe --exports`).
  2. Export: UModel per skin, in parallel, straight into the skin's root.
  3. Chain and verify (serial): export parent MICs the root lacks, copy chain textures into the
     leaf's Texture2D folder, check with `resolve_material`.

Example:
  python tools/export_weapon_skins.py --umodel "C:\\path\\to\\umodel.exe" --only Mati_MaliwanRaid_Infection
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))
from render_weapon_previews import resolve_material  # noqa: E402

MIC_CLASS = "Engine.MaterialInstanceConstant"
MAX_PARENT_DEPTH = 8
MESSAGE_RE = re.compile(r"unsupported|warning|ignoring|unknown|not found|error", re.I)
PARENT_RE = re.compile(r"^Parent = (\w+)'([^']+)'", re.M)
DEFAULT_GAME = r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2"
DEFAULT_READER = REPO / "build" / "Release" / "ow-package.exe"
DEFAULT_CENSUS = REPO / "local" / "wpa" / "census" / "visual_census.json"
DEFAULT_OUTPUT = REPO / "local" / "external" / "umodel" / "weapon-skins"


class ChainError(Exception):
    pass


def check_output(path: Path) -> Path:
    out = path.resolve()
    local = (REPO / "local").resolve()
    if out != local and not out.is_relative_to(local):
        raise SystemExit(f"refusing output outside {local}: {out}")
    return out


def sha1(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def package_file(game: str, name: str) -> Path | None:
    base = Path(game)
    direct = base / "WillowGame" / "CookedPCConsole" / name
    if direct.exists():
        return direct
    return next(iter(sorted((base / "DLC").glob(f"*/Compat/Content/{name}"))), None)


def census_packages(parts: dict) -> list[str]:
    names = set()
    for part in parts.values():
        names.update(part.get("packages") or [])
    return sorted(names)


def export_index(reader: Path, game: str, names: list[str]):
    """Map MIC object path -> census packages that export it."""
    index, errors = {}, {}
    for name in names:
        path = package_file(game, name)
        if path is None:
            errors[name] = "package file not found"
            continue
        proc = subprocess.run([str(reader), str(path), "--exports"], capture_output=True, text=True)
        if proc.returncode:
            errors[name] = proc.stderr.strip()[:200]
            continue
        for entry in json.loads(proc.stdout):
            if entry.get("class") == MIC_CLASS:
                index.setdefault(entry["path"], []).append(name)
    return index, errors


def choose_package(mic: str, used_by_parts: list[str], parts: dict, index: dict) -> str | None:
    """Prefer the material part's own package, then Startup.upk, then any exporter."""
    owners = index.get(mic, [])
    for part in used_by_parts:
        for name in (parts.get(part) or {}).get("packages") or []:
            if name in owners:
                return name
    if "Startup.upk" in owners:
        return "Startup.upk"
    return owners[0] if owners else None


def run_export(umodel: str, game: str, root: Path, package: str, obj: str, key: str, timeout: float,
               out: Path) -> dict:
    """Run UModel for one MIC; its output lands in `root`. Writes only under root and out/logs."""
    cmd = [umodel, f"-path={Path(game).as_posix()}", "-game=border", "-export", "-png",
           f"-out={root.as_posix()}", Path(package).stem, obj, "MaterialInstanceConstant"]
    started = time.monotonic()
    timed_out, code = False, None
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=timeout)
        code, output = proc.returncode, proc.stdout + proc.stderr
    except subprocess.TimeoutExpired as exc:  # kills only the UModel child this call started
        timed_out, output = True, (exc.stdout or b"") + (exc.stderr or b"")
    elapsed = round(time.monotonic() - started, 2)
    text = output.decode("utf-8", errors="replace")
    log = out / "logs" / f"{key}.log"
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(f"{subprocess.list2cmdline(cmd)}\n\n{text}", encoding="utf-8")
    print(f"  export {key}: exit={code} timeout={timed_out} {elapsed}s", flush=True)
    return {"command": subprocess.list2cmdline(cmd), "exit_code": code, "timed_out": timed_out,
            "elapsed_s": elapsed, "log": log.relative_to(out).as_posix(),
            "messages": [line.strip() for line in text.splitlines() if MESSAGE_RE.search(line)]}


def find_props_all(root: Path, leaf: str) -> list[Path]:
    return sorted(root.glob(f"**/MaterialInstanceConstant/{leaf}.props.txt"))


def parent_ref(props: Path) -> tuple[str | None, str | None]:
    match = PARENT_RE.search(props.read_text(encoding="utf-8", errors="replace"))
    return (match.group(1), match.group(2)) if match else (None, None)


def ensure_parents(props: Path, rec: dict, index: dict, ctx: dict) -> str | None:
    """Export parent MICs the root lacks. Returns the non-MIC base (e.g. a Material), if any."""
    path = props
    for _ in range(MAX_PARENT_DEPTH):
        cls, ref = parent_ref(path)
        if ref is None:
            return None
        if cls != "MaterialInstanceConstant":
            return f"{cls}'{ref}'"
        leaf = ref.split(".")[-1]
        hits = find_props_all(rec["root"], leaf)
        if not hits:
            owners = index.get(ref) or []
            if not owners:
                raise ChainError(f"parent {ref} is not exported by any census package")
            pkg = "Startup.upk" if "Startup.upk" in owners else owners[0]
            key = f"{rec['key']}-parent-{leaf}"
            res = run_export(ctx["umodel"], ctx["game"], rec["root"], pkg, leaf, key, ctx["timeout"], ctx["out"])
            rec["parent_exports"].append({"mic": ref, "package_used": pkg, "exit_code": res["exit_code"],
                                          "elapsed_s": res["elapsed_s"]})
            hits = find_props_all(rec["root"], leaf)
            if not hits:
                raise ChainError(f"parent {ref} not written by UModel (exit {res['exit_code']})")
        path = hits[0]
    raise ChainError(f"parent chain deeper than {MAX_PARENT_DEPTH}")


def finish(rec: dict, index: dict, ctx: dict) -> None:
    """Chain, texture copy and verification for one skin root."""
    root, leaf = rec["root"], rec["leaf"]
    own = root / rec["package_stem"] / "MaterialInstanceConstant" / f"{leaf}.props.txt"
    if not own.exists():
        rec["error"] = f"UModel wrote no {leaf}.props.txt under {rec['package_stem']}/ (exit {rec['exit_code']})"
        return
    if len(find_props_all(root, leaf)) > 1:
        rec["error"] = f"{len(find_props_all(root, leaf))} props files share the leaf name {leaf}"
        return
    try:
        rec["base_parent"] = ensure_parents(own, rec, index, ctx)
    except ChainError as exc:
        rec["error"] = str(exc)
        return
    resolved = resolve_material(leaf, [root])
    if resolved is None:
        rec["error"] = "resolve_material found no props"
        return
    merged, texture_dir, chain = resolved
    rec["chain"] = chain
    ambiguous = [name for name in chain if len(find_props_all(root, name)) > 1]
    if ambiguous:
        rec["error"] = f"chain names share a leaf with another props file: {ambiguous}"
        return
    for param, tex in sorted(merged["texture"].items()):
        if not tex:
            continue
        dst = texture_dir / f"{tex}.png"
        if dst.exists():
            rec["textures"][param] = True
            continue
        sources = sorted(root.glob(f"**/Texture2D/{tex}.png"))
        if not sources:
            tag = " (engine_stub)" if "stub" in tex.lower() else ""
            rec["missing"].append(f"{param}={tex}{tag}")
            rec["textures"][param] = False
            continue
        digests = {sha1(src) for src in sources}
        if len(digests) > 1:
            rec["conflicts"].append(f"{param}={tex}: {len(sources)} differing copies in root")
            rec["textures"][param] = False
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(sources[0], dst)
        rec["copied"].append(tex)
        rec["textures"][param] = True
    verify = resolve_material(leaf, [root])
    present = verify is not None and all((texture_dir / f"{t}.png").exists()
                                         for t in verify[0]["texture"].values() if t)
    problems = rec["missing"] + [f"conflict {c}" for c in rec["conflicts"]]
    rec["resolve_ok"] = bool(present and not problems)
    if problems:
        rec["error"] = "; ".join(problems[:6])


def pick_skins(skins: dict, only: list[str] | None) -> dict:
    if not only:
        return skins
    wanted = set(only)
    return {mic: info for mic, info in skins.items() if mic in wanted or mic.split(".")[-1] in wanted}


def summarize(records: list[dict], out: Path, index_info: dict) -> dict:
    by_class = collections.defaultdict(lambda: {"attempted": 0, "resolve_ok": 0, "failed": 0})
    for rec in records:
        bucket = by_class[rec["weapon_class"] or "unknown"]
        bucket["attempted"] += 1
        bucket["resolve_ok" if rec["resolve_ok"] else "failed"] += 1
    pngs = sorted((out / "skins").glob("**/Texture2D/*.png"))
    by_hash, names = {}, collections.defaultdict(set)
    within = {"names_in_several_folders": 0, "different_content": 0, "examples": []}
    for png in pngs:
        digest = sha1(png)
        by_hash.setdefault(digest, png.stat().st_size)
        names[png.name].add(digest)
    for rec in records:
        folders = collections.defaultdict(set)
        for png in sorted(rec["root"].glob("**/Texture2D/*.png")):
            folders[png.name].add((png.parent.parent.name, sha1(png)))
        for name, entries in folders.items():
            if len({pkg for pkg, _ in entries}) > 1:
                within["names_in_several_folders"] += 1
                if len({digest for _, digest in entries}) > 1:
                    within["different_content"] += 1
                    if len(within["examples"]) < 10:
                        within["examples"].append(f"{rec['leaf']}: {name}")
    messages = collections.Counter(m for rec in records for m in rec["messages"])
    bases = collections.Counter(rec["base_parent"] or "none (chain ends at a MIC)" for rec in records)
    leaves = collections.Counter(rec["leaf"] for rec in records)
    tree_bytes = sum(p.stat().st_size for p in (out / "skins").rglob("*") if p.is_file())
    return {
        "attempted": len(records),
        "resolve_ok": sum(rec["resolve_ok"] for rec in records),
        "failed": sum(not rec["resolve_ok"] for rec in records),
        "by_weapon_class": dict(sorted(by_class.items())),
        "leaf_name_collisions": {"leaves_shared_by_several_skins": sum(1 for n in leaves.values() if n > 1),
                                 "skins_sharing_a_leaf": sum(n for n in leaves.values() if n > 1)},
        "png": {"files_in_roots": len(pngs), "distinct_content": len(by_hash),
                "distinct_bytes": sum(by_hash.values()), "root_tree_bytes": tree_bytes},
        "textures_with_several_contents_across_skins": sum(1 for d in names.values() if len(d) > 1),
        "same_name_in_several_folders_within_a_skin": within,
        "umodel_message_lines": sum(messages.values()),
        "umodel_top_messages": messages.most_common(30),
        "base_parents": bases.most_common(),
        "parent_mic_exports": sum(len(rec["parent_exports"]) for rec in records),
        "index": index_info,
    }


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--census", type=Path, default=DEFAULT_CENSUS)
    parser.add_argument("--umodel", default=os.environ.get("UMODEL_EXE"),
                        help="UE Viewer umodel.exe (default: $UMODEL_EXE)")
    parser.add_argument("--game", default=DEFAULT_GAME, help="Borderlands 2 install root (-path)")
    parser.add_argument("--reader", type=Path, default=DEFAULT_READER, help="ow-package.exe")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT, help="must be under local/")
    parser.add_argument("--only", nargs="+", help="leaf names or full MIC paths")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--timeout", type=float, default=300.0, help="seconds per UModel run")
    args = parser.parse_args(argv)
    if not args.umodel:
        parser.error("--umodel (or $UMODEL_EXE) is required")
    return args


def main(argv=None) -> int:
    args = parse_args(argv)
    out = check_output(args.output)
    census = json.loads(args.census.read_text(encoding="utf-8"))
    parts = census.get("parts", {})
    selected = pick_skins(census["skins"], args.only)
    names = census_packages(parts)
    print(f"index: {len(names)} census packages", flush=True)
    index, index_errors = export_index(args.reader, args.game, names)
    print(f"index: {len(index)} MIC paths; {len(index_errors)} package errors", flush=True)

    # Only the trees this tool owns are cleared; anything else under out/ is left alone.
    shutil.rmtree(out / "skins", ignore_errors=True)
    shutil.rmtree(out / "logs", ignore_errors=True)
    records = []
    for mic, info in sorted(selected.items()):
        leaf = mic.split(".")[-1]
        package = choose_package(mic, info.get("used_by_parts") or [], parts, index)
        key = f"{leaf}-{hashlib.sha1(mic.encode()).hexdigest()[:8]}"
        records.append({
            "mic": mic, "leaf": leaf, "key": key, "root": out / "skins" / key,
            "weapon_class": info.get("weapon_class"), "used_by_parts": info.get("used_by_parts") or [],
            "package_used": package, "package_stem": Path(package).stem if package else None,
            "command": None, "exit_code": None, "timed_out": False, "elapsed_s": None, "log": None,
            "files": [], "parent_exports": [], "chain": [], "base_parent": None, "textures": {},
            "missing": [], "copied": [], "conflicts": [], "resolve_ok": False, "messages": [],
            "error": None if package else "no census package exports this MIC",
        })
    jobs = [rec for rec in records if rec["package_used"]]

    print(f"export: {len(jobs)} skins, {args.workers} workers", flush=True)

    def export(rec):
        shutil.rmtree(rec["root"], ignore_errors=True)
        rec["root"].mkdir(parents=True)
        res = run_export(args.umodel, args.game, rec["root"], rec["package_used"], rec["leaf"], rec["key"],
                         args.timeout, out)
        rec.update(command=res["command"], exit_code=res["exit_code"], timed_out=res["timed_out"],
                   elapsed_s=res["elapsed_s"], log=res["log"], messages=res["messages"])
        if res["timed_out"]:
            rec["error"] = f"timed out after {args.timeout:.0f}s"
        elif res["exit_code"]:
            rec["error"] = f"UModel exit {res['exit_code']}"
        return rec

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        list(pool.map(export, jobs))

    ctx = {"umodel": args.umodel, "game": args.game, "timeout": args.timeout, "out": out}
    for rec in records:
        if rec["error"] is None:
            try:
                finish(rec, index, ctx)
            except Exception as exc:  # keep going; the inventory records the failure
                rec["error"] = f"{type(exc).__name__}: {exc}"
        if rec["resolve_ok"]:
            print(f"  ok   {rec['leaf']}", flush=True)
        else:
            print(f"  FAIL {rec['leaf']}: {rec['error']}", flush=True)

    summary = summarize(records, out, {"packages": len(names), "mic_paths": len(index),
                                       "package_errors": index_errors})
    for rec in records:
        rec["files"] = sorted(p.relative_to(rec["root"]).as_posix() for p in rec["root"].rglob("*") if p.is_file()) \
            if rec["root"].exists() else []
        rec["root"] = rec["root"].relative_to(out).as_posix()
        rec.pop("messages", None)
    (out / "inventory.json").write_text(json.dumps({"totals": summary, "skins": records}, indent=1),
                                        encoding="utf-8")

    print("totals:", json.dumps({k: summary[k] for k in ("attempted", "resolve_ok", "failed")}), flush=True)
    print("by weapon class:", json.dumps(summary["by_weapon_class"]))
    print("png:", json.dumps(summary["png"]))
    print("leaf collisions:", json.dumps(summary["leaf_name_collisions"]))
    print("same name in several folders within a skin:",
          json.dumps({k: v for k, v in summary["same_name_in_several_folders_within_a_skin"].items()
                      if k != "examples"}))
    print("base parents:", summary["base_parents"][:5])
    print(f"inventory: {out / 'inventory.json'}")
    return 0 if summary["failed"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
