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
  labelled reads:    always checked. A labelled snapshot or pawn read recorded as not taken (asked for before the map settled) fails,
                     unless its label is in expect_not_taken; a label in expect_not_taken that was taken fails too. A labelled read
                     taken late (it carries asked_t) fails unless it is 'start' or 'arrival', the only ones the watch may defer. A
                     read kind the watch switched off fails unless it is in allow_switched_off. A deferred read never taken fails.
  expect_not_taken:  ["<label>", ...]: labels that must be recorded as not taken (S0's deliberate early read); every instance of such
                     a label must be not taken.
  allow_not_taken:   ["<label>", ...]: labels that may be recorded as not taken without failing (a down first seen with no pawn);
                     such a line counts as present for snaps.
  max_defer_seconds: how late a deferred start or arrival read may land after it was asked for (default 60).
  allow_switched_off: ["<kind>", ...]: read kinds that may be switched off without failing the recording.
  max_dropped:       {"row"|"frame"|"snap"|"pawns"|"labelled"|"bytes"|"per_func": N}: how many lines a cap may drop, per kind
                     ("per_func" counts the rows dropped by per_func_cap, all functions together). Every kind not named allows 0.
  no_missing:        true: every name resolved: the start line's, every group's 'installed' line's and the end line's missing lists
                     together are empty.
  max_auto_failures: the most automatic snapshots or pawn reads with a failed part (default: no limit).
  require:           [{"func": "<substring of a hook path>", "tag": "pre"|"post", "min": N, "max": M}]
  order:             [["<first>", "<then>"]]: some row matching <then> comes after the first row matching <first>, on the
                     recording's own clock (t, real seconds since the watch started; the map clock restarts at every travel).
  gap:               [{"from": "<substring>", "to": "<substring>", "min": s, "max": s}]: seconds on that clock from the first <from> row
                     to the first <to> row after it lie in [min, max].
  must_fire:         ["<substring>", ...]: a hooked function whose path holds it fired at least once (the end line's per-function
                     record). Every recording also gets an 'exercised' line: how many hooked functions fired, and the ones installed
                     but never exercised (a long item that relies on one of those makes its first real run).
  groups:            always checked when the spec has groups: every group was installed (its 'installed' line is in the recording)
                     with nothing missing.
  args_nonempty:     [{"func": "<substring>", "tag": "pre"|"post", "min": N}]: at least N rows of that hook, and every one of them carries
                     non-empty arguments and no args_error (a function known to have parameters: an empty or partial read there means
                     the reader refused or failed it).
  edges:             [{"func": "<substring>", "arg": "<argument>", "values": [v, ...]}]: rows of that hook carry each value.
  snaps:             ["<label>" | ["<label>", "<alternative>", ...], ...]: labelled snapshots with these labels exist (for a list,
                     any one of them).
  snap_content:      {"<part>": "present"|"dict"|"nonempty"|"number"}: in every labelled snapshot the part read without failing and
                     holds that kind of value (present: read, may be null; dict: a non-empty object; nonempty: a non-empty list;
                     number: a number).
  mods_must_be_off:  ["<substring>", ...]: the start snapshot read the mod list, and no enabled mod's name holds one (case-insensitive).
  frames:            {"min": N, "dt_min": s, "dt_max": s, "share": 0.95}: at least N frame rows, and that share of the frames in which no
                     automatic read or flush ran have a positive tick DeltaTime inside [dt_min, dt_max].
  rows_have_dt:      true: hook rows carry the frame delta (dt).
  pawns:             {"min_reads": N, "min_pawns": K, "fields": [...]}: pawn reads without a failed part, each with K+ pawns that
                     carry every one of those fields with a value; when "health_pool" is a field, a pawn whose health pool is flagged
                     stale (no pool, GUID differs, shut down, not in its slot) does not count.
"""
import argparse
import importlib.util
import json
import sys
from pathlib import Path

DEFERRABLE = ("start", "arrival")
CHECK_KEYS = {"must_fire", "args_nonempty", "no_missing", "max_errors", "max_dropped", "expect_not_taken", "allow_switched_off", "allow_not_taken", "max_defer_seconds", "max_auto_failures", "require", "order", "gap", "edges", "snaps", "snap_content",
              "mods_must_be_off", "frames", "rows_have_dt", "pawns"}
CONTENT_KINDS = {"present", "dict", "nonempty", "number"}


def load_recording(path):
    """The JSON-lines recording as one dict: meta fields, rows, frames, snaps, pawns, and the end line (None after a crash). A last line
    cut short by a crash is skipped."""
    out = {"rows": [], "frames": [], "snaps": [], "pawns": [], "missing": [], "end": None, "groups": [], "marks": []}
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
            out.update(missing=rec.get("missing", []), resolved=rec.get("resolved", []), spec=rec.get("spec"), params_misfit=rec.get("params_misfit", {}))
        elif kind in ("row", "frame", "snap"):
            out[kind + "s"].append(rec)
        elif kind == "pawns":
            out["pawns"].append(rec)
        elif kind == "group":
            out["groups"].append(rec)
            if rec.get("event") == "installed":
                out["missing"] = list(out["missing"]) + [m for m in rec.get("missing", []) if m not in out["missing"]]
        elif kind == "mark":
            out["marks"].append(rec)
        elif kind == "end":
            out["end"] = rec
            out["missing"] = list(out["missing"]) + [m for m in rec.get("missing", []) or [] if m not in out["missing"]]
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
    snaps = checks.get("snaps", [])
    if not isinstance(snaps, list) or not all(isinstance(x, str) or (isinstance(x, list) and x and all(isinstance(y, str) for y in x)) for x in snaps):
        raise ValueError("snaps must be a list of labels or of lists of alternative labels")
    for key in ("mods_must_be_off", "expect_not_taken", "allow_switched_off", "allow_not_taken", "must_fire"):
        if key in checks and not (isinstance(checks[key], list) and all(isinstance(x, str) for x in checks[key])):
            raise ValueError(f"{key} must be a list of strings")
    return spec


def _match(rows, needle, tag=None):
    return [r for r in rows if needle in str(r.get("func", "")) and (tag is None or r.get("tag") == tag)]


def _g(row):
    """A row's time on the recording's own clock (t): it never restarts, unlike the map clock (g), which a travel resets."""
    t = row.get("t")
    return t if isinstance(t, (int, float)) and not isinstance(t, bool) else None


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
    fn = (end or {}).get("fn") or {}
    if fn:
        fired = [p for p, r in fn.items() if (r or {}).get("n", 0) > 0]
        never = sorted(p for p, r in fn.items() if not (r or {}).get("n", 0))
        out.append(("exercised", True, f"{len(fired)} of {len(fn)} hooked functions fired; installed, never exercised: {', '.join(never) or 'none'}"))
    for need in checks.get("must_fire", []):
        hit = [p for p, r in fn.items() if need in p and (r or {}).get("n", 0) > 0]
        out.append((f"must_fire {need}", end is not None and bool(hit), f"{len(hit)} matching functions fired"))
    spec_groups = [g.get("name") for g in ((dump.get("spec") or {}).get("groups") or [])]
    if spec_groups:
        lines = {g.get("name"): g for g in dump.get("groups", []) if g.get("event") == "installed"}
        not_installed = [g for g in spec_groups if g not in lines]
        partial = [g for g in spec_groups if g in lines and lines[g].get("missing")]
        out.append(("groups", not not_installed and not partial,
                    f"{len(spec_groups) - len(not_installed) - len(partial)} of {len(spec_groups)} groups installed whole; "
                    f"not installed: {', '.join(not_installed) or 'none'}; installed with names missing: {', '.join(partial) or 'none'}"))
    for req in checks.get("args_nonempty", []):
        matched = _match(rows, req["func"], req.get("tag"))
        empty = [r for r in matched if not {k: v for k, v in (r.get("args") or {}).items() if k != "args_error"}]
        broken = [r for r in matched if "args_error" in (r.get("args") or {})]
        ok = len(matched) >= req.get("min", 1) and not empty and not broken
        out.append((f"args_nonempty {req['func']} {req.get('tag', '')}".strip(), ok,
                    f"{len(matched)} rows, {len(empty)} with no arguments, {len(broken)} with a failed argument read"))
    misfit = (end or {}).get("params_misfit") or dump.get("params_misfit") or {}
    if misfit:
        out.append(("params_misfit", True, f"{len(misfit)} hooked functions whose parameters were refused (listed in the recording; counted as errors)"))
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
    every_labelled = [s for s in dump.get("snaps", []) + dump.get("pawns", []) if s.get("label") != "auto"]
    expected = set(checks.get("expect_not_taken", []))
    allowed_nt = set(checks.get("allow_not_taken", []))
    not_taken = {s.get("label") for s in every_labelled if s.get("not_taken")}
    taken = {s.get("label") for s in every_labelled if not s.get("not_taken")}
    unexpected = sorted(not_taken - expected - allowed_nt)
    missing_expected = sorted(expected - not_taken)
    also_taken = sorted(expected & taken)
    out.append(("not_taken", not unexpected and not missing_expected and not also_taken,
                f"{len(not_taken)} labelled reads not taken; unexpected: {', '.join(unexpected) or 'none'}; "
                f"expected but absent: {', '.join(missing_expected) or 'none'}; expected but also taken: {', '.join(also_taken) or 'none'}"))
    late = sorted({s.get("label") for s in every_labelled if "asked_t" in s and not s.get("not_taken") and s.get("label") not in DEFERRABLE})
    out.append(("taken_late", not late, f"labelled reads taken late other than start/arrival: {', '.join(late) or 'none'}"))
    bound = checks.get("max_defer_seconds", 60)
    slow = sorted({s.get("label") for s in every_labelled if "asked_t" in s and not s.get("not_taken")
                   and isinstance(s.get("t"), (int, float)) and isinstance(s.get("asked_t"), (int, float)) and s["t"] - s["asked_t"] > bound})
    out.append(("defer_bound", not slow, f"deferred reads later than {bound} s: {', '.join(slow) or 'none'}"))
    switched = sorted(set((end or {}).get("switched_off", {}) or {}) - set(checks.get("allow_switched_off", [])))
    out.append(("switched_off", end is not None and not switched, f"read kinds switched off: {', '.join(switched) or 'none'}"))
    never = (end or {}).get("never_taken", []) or []
    out.append(("never_taken", end is not None and not never, f"{len(never)} deferred reads never taken"))
    labelled = [s for s in dump.get("snaps", []) if s.get("label") != "auto" and not s.get("not_taken")]
    labels = {s.get("label") for s in labelled}
    present = labels | {s.get("label") for s in every_labelled if s.get("not_taken") and s.get("label") in allowed_nt}
    for label in checks.get("snaps", []):
        options = label if isinstance(label, list) else [label]
        name = "/".join(options)
        ok = any(o in present for o in options)
        out.append((f"snap {name}", ok, "present" if ok else "absent"))
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

        def healthy(pool):
            """A health pool counts only when every condition was shown: a record, no stale reason, numeric values."""
            if not isinstance(pool, dict) or "stale" not in pool or pool.get("stale") is not None:
                return False
            return all(isinstance(pool.get(k), (int, float)) and not isinstance(pool.get(k), bool) for k in ("CurrentValue", "MaxValue"))

        def good(p):
            if not all(p.get(f) is not None for f in need):
                return False
            return not ("health_pool" in need and not healthy(p.get("health_pool")))
        full = [r for r in reads if not r.get("failed") and not r.get("not_taken")
                and sum(1 for p in r.get("pawns", []) if good(p)) >= pawns.get("min_pawns", 1)]
        stale = sum(1 for r in reads for p in r.get("pawns", []) if not healthy(p.get("health_pool")))
        ok = len(full) >= pawns.get("min_reads", 1)
        out.append(("pawns", ok, f"{len(full)} of {len(reads)} reads clean, with {pawns.get('min_pawns', 1)}+ pawns carrying every field; "
                                 f"{stale} pawn rows without a healthy health pool"))
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
