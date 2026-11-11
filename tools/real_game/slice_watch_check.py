#!/usr/bin/env python3
"""Is a slice_watch dump usable? The check a capture session runs before the game is closed (lane capture-prep, 2026-11-11).

    python tools/real_game/slice_watch_check.py local/realgame/<set>/watch.json --spec <spec.json> [--json]

The spec is the same JSON file the capture passed to -WatchSpec (tools/real_game/scripts/slice_watch.py); its "checks" object says
what a usable dump must hold. Every check compares shape and plausibility only: that a hook fired, that two events came in an
order, that a gap lies in a wide sane band. None forces an expected value; a value the session is meant to measure stays
UNVERIFIED until a person reads it. The output is counts and pass/fail only, never a recorded value, so it may be pasted into a
report. Exit 0 when every check passes, 1 otherwise, 2 when the dump or the spec cannot be read.

Checks (all optional):
  no_missing:        true: every name in funcs resolved to at least one hook.
  max_errors:        the most hook or read errors allowed (default 0).
  require:           [{"func": "<substring of a hook path>", "tag": "pre"|"post", "min": N, "max": M}]
  order:             [["<first>", "<then>"]]: some row matching <then> comes after the first row matching <first> (game time).
  gap:               [{"from": "<substring>", "to": "<substring>", "min": s, "max": s}]: game seconds from the first <from> row to the
                     first <to> row after it lie in [min, max].
  edges:             [{"func": "<substring>", "arg": "<argument>", "values": [v, ...]}]: rows of that hook carry each value (for example
                     an input event's press and release as separate rows).
  snaps:             ["<label>", ...]: snapshots with these labels exist.
  snap_fields:       ["player", "pools", "cash", "checkpoint", "stations", ...]: present in every labelled snapshot (the
                     capture's own, not the automatic once-a-second ones).
  mods_allowed:      ["<mod name>", ...]: at the start snapshot, every enabled SDK mod is one of these.
  mods_must_be_off:  ["<substring>", ...]: at the start snapshot the mod list was read, and no enabled mod's name holds one of these
                     (case-insensitive).
  frames:            {"min": N, "dt_min": s, "dt_max": s, "share": 0.95}: at least N frame rows, and that share of them have a
                     positive tick DeltaTime inside [dt_min, dt_max].
  rows_have_dt:      true: hook rows carry the frame delta (dt) beside them.
  pawns:             {"min_reads": N, "min_pawns": K, "fields": ["cylinder", "GroundSpeed"]}: pawn reads with those fields.
"""
import argparse
import json
import sys
from pathlib import Path


def _match(rows, needle, tag=None):
    return [r for r in rows if needle in str(r.get("func", "")) and (tag is None or r.get("tag") == tag)]


def _g(row):
    g = row.get("g")
    return g if isinstance(g, (int, float)) else None


def check(dump, checks):
    """A list of (name, ok, detail) tuples. Details hold counts and pass/fail only."""
    rows = dump.get("rows", [])
    out = []
    if checks.get("no_missing"):
        missing = dump.get("missing", [])
        out.append(("no_missing", not missing, f"{len(missing)} names did not resolve"))
    errors = len(dump.get("errors", []))
    limit = checks.get("max_errors", 0)
    out.append(("max_errors", errors <= limit, f"{errors} errors (limit {limit})"))
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
    snaps = dump.get("snaps", [])
    labels = {s.get("label") for s in snaps}
    for label in checks.get("snaps", []):
        out.append((f"snap {label}", label in labels, "present" if label in labels else "absent"))
    fields = checks.get("snap_fields", [])
    if fields:
        named = [s for s in snaps if s.get("label") != "auto"]
        bad = [s.get("label") for s in named if any(f not in s for f in fields)]
        out.append(("snap_fields", bool(named) and not bad, f"{len(named)} labelled snapshots, {len(bad)} lack a field"))
    if "mods_allowed" in checks:
        start = next((s for s in snaps if s.get("label") == "start"), None)
        mods = (start or {}).get("mods") or []
        unread = any("error" in m for m in mods)
        extra = [m.get("name") for m in mods if m.get("enabled") and m.get("name") not in checks["mods_allowed"]]
        ok = start is not None and bool(mods) and not unread and not extra
        out.append(("mods_allowed", ok, f"{len(mods)} mods listed, {len(extra)} enabled outside the allowed list"
                    + (" (mod list unreadable)" if unread else "")))
    if "mods_must_be_off" in checks:
        start = next((s for s in snaps if s.get("label") == "start"), None)
        mods = (start or {}).get("mods") or []
        unread = not mods or any("error" in m for m in mods)
        on = [m for m in mods if m.get("enabled") and any(w.lower() in str(m.get("name", "")).lower() for w in checks["mods_must_be_off"])]
        enabled = sum(1 for m in mods if m.get("enabled"))
        out.append(("mods_must_be_off", not unread and not on,
                    f"{len(mods)} mods listed, {enabled} enabled, {len(on)} of the named ones still on" + (" (mod list unread)" if unread else "")))
    frames_check = checks.get("frames")
    if frames_check:
        frames = dump.get("frames", [])
        lo, hi = frames_check.get("dt_min", 0.0), frames_check.get("dt_max", 1.0)
        good = [f for f in frames if isinstance(f.get("dt_arg"), (int, float)) and lo <= f["dt_arg"] <= hi and f["dt_arg"] > 0]
        share = len(good) / len(frames) if frames else 0.0
        ok = len(frames) >= frames_check.get("min", 1) and share >= frames_check.get("share", 0.95)
        out.append(("frames", ok, f"{len(frames)} frame rows, {share:.0%} with a delta in [{lo}, {hi}] s"))
    if checks.get("rows_have_dt"):
        with_dt = [r for r in rows if isinstance(r.get("dt"), (int, float))]
        out.append(("rows_have_dt", bool(rows) and len(with_dt) == len(rows), f"{len(with_dt)} of {len(rows)} rows carry dt"))
    pawns = checks.get("pawns")
    if pawns:
        reads = dump.get("pawns", [])
        need = pawns.get("fields", [])
        full = [r for r in reads if len(r.get("pawns", [])) >= pawns.get("min_pawns", 1)
                and all(all(f in p and p[f] is not None for f in need) for p in r["pawns"])]
        ok = len(full) >= pawns.get("min_reads", 1)
        out.append(("pawns", ok, f"{len(full)} of {len(reads)} reads with {pawns.get('min_pawns', 1)}+ pawns and every field"))
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("dump")
    ap.add_argument("--spec", required=True)
    ap.add_argument("--json", action="store_true", help="print the results as JSON")
    a = ap.parse_args(argv)
    try:
        dump = json.loads(Path(a.dump).read_text(encoding="utf-8"))
        checks = json.loads(Path(a.spec).read_text(encoding="utf-8")).get("checks", {})
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
