#!/usr/bin/env python3
"""Is a slice_watch recording usable? The check a capture session runs before the game is closed (lane capture-prep, 2026-11-11).

    python tools/real_game/slice_watch_check.py local/realgame/<set>/watch.jsonl --spec <spec.json> [--json]
    python tools/real_game/slice_watch_check.py --validate-spec <spec.json>

The recording is the JSON-lines file tools/real_game/scripts/slice_watch.py writes. The spec is the file the capture passed to
-WatchSpec, and its "checks" object says what a usable recording must hold. Every check compares shape and plausibility only: a hook
fired, two events came in an order, a gap lies in a wide sane band, a read answered. None forces an expected value; a value the session
is meant to measure stays UNVERIFIED until a person reads it. The output is counts and pass/fail only, never a recorded value, so it
may be pasted into a report. Exit 0 when every check passes, 1 otherwise, 2 when the recording or the spec cannot be read or the spec
is refused.

Always checked: the recording ended (its end line is there: a crash or a stopped recorder fails this), at most max_errors hook or read
errors (default 0), and nothing dropped by a cap (see max_dropped). Optional checks:
  max_dropped:       {"row"|"frame"|"snap"|"pawns"|"labelled"|"bytes"|"per_func": N}: how many lines a cap may drop, per kind
                     ("per_func" counts the rows dropped by per_func_cap, all functions together). Every kind not named allows 0.
  no_missing:        true: every name in funcs resolved.
  max_auto_failures: the most automatic snapshots or pawn reads with a failed part (default: no limit).
  require:           [{"func": "<substring of a hook path>", "tag": "pre"|"post", "min": N, "max": M}]
  order:             [["<first>", "<then>"]]: some row matching <then> comes after the first row matching <first> (game time).
  gap:               [{"from": "<substring>", "to": "<substring>", "min": s, "max": s}]: game seconds from the first <from> row to the
                     first <to> row after it lie in [min, max].
  edges:             [{"func": "<substring>", "arg": "<argument>", "values": [v, ...]}]: rows of that hook carry each value.
  snaps:             ["<label>", ...]: labelled snapshots with these labels exist.
  snap_content:      {"<part>": "present"|"dict"|"nonempty"|"number"}: in every labelled snapshot the part read without failing and
                     holds that kind of value (present: read, may be null; dict: a non-empty object; nonempty: a non-empty list;
                     number: a number).
  mods_must_be_off:  ["<substring>", ...]: the start snapshot read the mod list, and no enabled mod's name holds one (case-insensitive).
  frames:            {"min": N, "dt_min": s, "dt_max": s, "share": 0.95}: at least N frame rows, and that share of the frames in which no
                     automatic read or flush ran have a positive tick DeltaTime inside [dt_min, dt_max].
  rows_have_dt:      true: hook rows carry the frame delta (dt).
  pawns:             {"min_reads": N, "min_pawns": K, "fields": [...]}: pawn reads without a failed part, each with K+ pawns that all
                     carry those fields with a value.
"""
import argparse
import importlib.util
import json
import sys
from pathlib import Path

CHECK_KEYS = {"no_missing", "max_errors", "max_dropped", "max_auto_failures", "require", "order", "gap", "edges", "snaps", "snap_content",
              "mods_must_be_off", "frames", "rows_have_dt", "pawns"}
CONTENT_KINDS = {"present", "dict", "nonempty", "number"}


def load_recording(path):
    """The JSON-lines recording as one dict: meta fields, rows, frames, snaps, pawns, and the end line (None after a crash). A last line
    cut short by a crash is skipped."""
    out = {"rows": [], "frames": [], "snaps": [], "pawns": [], "missing": [], "end": None}
    lines = Path(path).read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        if not line.strip():
            continue
        try:
            rec = json.loads(line)
        except ValueError:
            if i == len(lines) - 1:
                continue
            raise
        kind = rec.pop("k", None)
        if kind == "meta":
            out.update(missing=rec.get("missing", []), resolved=rec.get("resolved", []), spec=rec.get("spec"))
        elif kind in ("row", "frame", "snap"):
            out[kind + "s"].append(rec)
        elif kind == "pawns":
            out["pawns"].append(rec)
        elif kind == "end":
            out["end"] = rec
    return out


def validate_spec(spec_path):
    """The recorder's own spec check (scripts/slice_watch.py sw_load_spec), plus the checks' keys and content kinds. Raises ValueError."""
    loader = importlib.util.spec_from_file_location("slice_watch", Path(__file__).resolve().parent / "scripts/slice_watch.py")
    module = importlib.util.module_from_spec(loader)
    loader.loader.exec_module(module)
    spec = json.loads(Path(spec_path).read_text(encoding="utf-8"))
    module.sw_load_spec(spec)
    checks = spec.get("checks", {})
    unknown = sorted(set(checks) - CHECK_KEYS)
    if unknown:
        raise ValueError(f"unknown checks: {unknown}")
    bad = {k: v for k, v in checks.get("snap_content", {}).items() if v not in CONTENT_KINDS}
    if bad:
        raise ValueError(f"snap_content kinds must be one of {sorted(CONTENT_KINDS)}: {bad}")
    md = checks.get("max_dropped", {})
    if not isinstance(md, dict) or not all(isinstance(v, int) and not isinstance(v, bool) and v >= 0 for v in md.values())             or set(md) - {"row", "frame", "snap", "pawns", "labelled", "bytes", "per_func"}:
        raise ValueError("max_dropped must map row, frame, snap, pawns, labelled, bytes or per_func to a whole number")
    for key in ("snaps", "mods_must_be_off"):
        if key in checks and not (isinstance(checks[key], list) and all(isinstance(x, str) for x in checks[key])):
            raise ValueError(f"{key} must be a list of strings")
    return spec


def _match(rows, needle, tag=None):
    return [r for r in rows if needle in str(r.get("func", "")) and (tag is None or r.get("tag") == tag)]


def _g(row):
    g = row.get("g")
    return g if isinstance(g, (int, float)) and not isinstance(g, bool) else None


def _content_ok(snap, part, kind):
    if part in (snap.get("failed") or []) or part not in snap:
        return False
    value = snap[part]
    if kind == "present":
        return True
    if kind == "dict":
        return isinstance(value, dict) and bool(value)
    if kind == "nonempty":
        return isinstance(value, list) and bool(value)
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def check(dump, checks):
    """A list of (name, ok, detail) tuples. Details hold counts and pass/fail only."""
    rows = dump.get("rows", [])
    end = dump.get("end")
    out = [("ended", end is not None and not end.get("stopped"), "end line present" if end else "no end line (crashed or never dumped)")]
    if checks.get("no_missing"):
        missing = dump.get("missing", [])
        out.append(("no_missing", not missing, f"{len(missing)} names did not resolve"))
    errors = len((end or {}).get("errors", []))
    limit = checks.get("max_errors", 0)
    out.append(("max_errors", end is not None and errors <= limit, f"{errors} errors (limit {limit})"))
    allowed = checks.get("max_dropped", {})
    dropped = (end or {}).get("dropped", {}) or {}
    kinds = {}
    for key, n in dropped.items():
        kind = "per_func" if ":" in key else key
        kinds[kind] = kinds.get(kind, 0) + int(n)
    over = sorted(k for k, n in kinds.items() if n > allowed.get(k, 0))
    out.append(("max_dropped", end is not None and not over,
                f"{sum(kinds.values())} lines dropped by caps; over the allowance: {', '.join(over) if over else 'none'}"))
    if "max_auto_failures" in checks:
        n = (end or {}).get("auto_failures", 0)
        out.append(("max_auto_failures", end is not None and n <= checks["max_auto_failures"],
                    f"{n} automatic reads with a failed part (limit {checks['max_auto_failures']})"))
    for req in checks.get("require", []):
        n = len(_match(rows, req["func"], req.get("tag")))
        lo, hi = req.get("min", 1), req.get("max")
        ok = n >= lo and (hi is None or n <= hi)
        out.append((f"require {req['func']} {req.get('tag', '')}".strip(), ok, f"{n} rows (want {lo}..{hi if hi is not None else 'any'})"))
    for first, then in checks.get("order", []):
        a = [_g(r) for r in _match(rows, first) if _g(r) is not None]
        b = [_g(r) for r in _match(rows, then) if _g(r) is not None]
        ok = bool(a) and any(x >= min(a) for x in b)
        out.append((f"order {first} -> {then}", ok, f"{len(a)} and {len(b)} rows"))
    for gap in checks.get("gap", []):
        a = sorted(_g(r) for r in _match(rows, gap["from"]) if _g(r) is not None)
        b = sorted(_g(r) for r in _match(rows, gap["to"]) if _g(r) is not None)
        after = [x for x in b if a and x >= a[0]]
        ok = bool(after) and gap.get("min", 0) <= after[0] - a[0] <= gap.get("max", float("inf"))
        within = "inside" if ok else "outside or missing"
        out.append((f"gap {gap['from']} -> {gap['to']}", ok, f"{within} the band [{gap.get('min', 0)}, {gap.get('max', 'inf')}] s"))
    for edge in checks.get("edges", []):
        seen = {r.get("args", {}).get(edge["arg"]) for r in _match(rows, edge["func"])}
        lacking = [v for v in edge["values"] if v not in seen]
        out.append((f"edges {edge['func']}.{edge['arg']}", not lacking, f"{len(edge['values']) - len(lacking)} of {len(edge['values'])} values seen"))
    labelled = [s for s in dump.get("snaps", []) if s.get("label") != "auto"]
    labels = {s.get("label") for s in labelled}
    for label in checks.get("snaps", []):
        out.append((f"snap {label}", label in labels, "present" if label in labels else "absent"))
    content = checks.get("snap_content", {})
    if content:
        bad = sum(1 for s in labelled for part, kind in content.items() if not _content_ok(s, part, kind))
        out.append(("snap_content", bool(labelled) and not bad,
                    f"{len(labelled)} labelled snapshots, {bad} failed or empty reads of {len(content)} required parts"))
    if "mods_must_be_off" in checks:
        start = next((s for s in labelled if s.get("label") == "start"), None)
        mods = (start or {}).get("mods") if start and "mods" not in (start.get("failed") or []) else None
        on = [m for m in (mods or []) if m.get("enabled") and any(w.lower() in str(m.get("name", "")).lower() for w in checks["mods_must_be_off"])]
        enabled = sum(1 for m in (mods or []) if m.get("enabled"))
        out.append(("mods_must_be_off", bool(mods) and not on,
                    f"{len(mods or [])} mods listed, {enabled} enabled, {len(on)} of the named ones still on" + ("" if mods else " (mod list not read)")))
    frames_check = checks.get("frames")
    if frames_check:
        frames = dump.get("frames", [])
        quiet = [f for f in frames if not f.get("busy")]
        lo, hi = frames_check.get("dt_min", 0.0), frames_check.get("dt_max", 1.0)
        good = [f for f in quiet if isinstance(f.get("dt_arg"), (int, float)) and lo <= f["dt_arg"] <= hi and f["dt_arg"] > 0]
        share = len(good) / len(quiet) if quiet else 0.0
        ok = len(frames) >= frames_check.get("min", 1) and share >= frames_check.get("share", 0.95)
        out.append(("frames", ok, f"{len(frames)} frame rows ({len(frames) - len(quiet)} with a read or flush), {share:.0%} of the rest in [{lo}, {hi}] s"))
    if checks.get("rows_have_dt"):
        with_dt = [r for r in rows if isinstance(r.get("dt"), (int, float))]
        out.append(("rows_have_dt", bool(rows) and len(with_dt) == len(rows), f"{len(with_dt)} of {len(rows)} rows carry dt"))
    pawns = checks.get("pawns")
    if pawns:
        reads = dump.get("pawns", [])
        need = pawns.get("fields", [])
        full = [r for r in reads if not r.get("failed") and len(r.get("pawns", [])) >= pawns.get("min_pawns", 1)
                and all(all(p.get(f) is not None for f in need) for p in r["pawns"])]
        ok = len(full) >= pawns.get("min_reads", 1)
        out.append(("pawns", ok, f"{len(full)} of {len(reads)} reads clean, with {pawns.get('min_pawns', 1)}+ pawns and every field"))
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("recording", nargs="?")
    ap.add_argument("--spec")
    ap.add_argument("--validate-spec", help="check a spec file and exit (0 good, 2 refused)")
    ap.add_argument("--json", action="store_true", help="print the results as JSON")
    a = ap.parse_args(argv)
    if a.validate_spec:
        try:
            validate_spec(a.validate_spec)
        except (OSError, ValueError) as e:
            print(f"SPEC REFUSED: {e}")
            return 2
        print("SPEC OK")
        return 0
    if not a.recording or not a.spec:
        ap.error("a recording and --spec are needed")
    try:
        checks = validate_spec(a.spec).get("checks", {})
        dump = load_recording(a.recording)
    except (OSError, ValueError) as e:
        print(f"UNREADABLE: {e}")
        return 2
    results = check(dump, checks)
    if a.json:
        print(json.dumps([{"check": n, "ok": ok, "detail": d} for n, ok, d in results], indent=1))
    else:
        for name, ok, detail in results:
            print(f"{'OK  ' if ok else 'FAIL'} {name}: {detail}")
    failed = sum(1 for _, ok, _ in results if not ok)
    print(f"{'USABLE' if not failed else 'NOT USABLE'}: {len(results) - failed} of {len(results)} checks passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
