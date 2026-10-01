"""Diagnostic UI trace replay on fresh class defaults, not a live-state parity test.

Consumes sdk_trace call/return JSONL. Only scalar inputs and scalar returns are
compared. Object/struct/array inputs, out results, native functions and executions
reaching stubs are explicitly skipped or blocked. All output stays local.
"""
import argparse
from collections import Counter
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile


def pairs(records):
    """Match nested calls by function and object; never pair across a damaged stack."""
    stack, result = [], []
    rejected = Counter()
    for record in records:
        phase = record.get("phase")
        if phase == "call":
            stack.append(record)
        elif phase == "return":
            key = (record.get("func"), record.get("obj"))
            if not stack or (stack[-1].get("func"), stack[-1].get("obj")) != key:
                rejected["unpaired_return"] += 1
                rejected["unpaired_call"] += len(stack)
                stack.clear()
                continue
            result.append((stack.pop(), record))
        elif phase in ("start", "stop", "error"):
            rejected["unpaired_call"] += len(stack)
            stack.clear()
    rejected["unpaired_call"] += len(stack)
    return sorted(result, key=lambda pair: pair[0].get("seq", 0)), rejected


def encode(value):
    if value is None:
        return "o:"
    if isinstance(value, bool):
        return "b:" + str(int(value))
    if isinstance(value, int):
        return "d:" + str(value)
    if isinstance(value, float) and math.isfinite(value):
        return "d:" + repr(value)
    if isinstance(value, str):
        # UObject and depth-limited repr strings are lossy, not text arguments.
        if "'" in value and value.endswith("'") or value.startswith("<"):
            raise ValueError("object_or_repr_input")
        return "t:" + value.encode("utf-8").hex()
    raise ValueError("aggregate_or_nonfinite_input")


def compare(expected, actual):
    kind, value = actual["type"], actual["value"]
    if expected is None:
        return kind == "None" or kind == "Object" and value == "None"
    if isinstance(expected, bool):
        return kind == "Bool" and value == str(int(expected))
    if isinstance(expected, int):
        return kind in ("Int", "Byte") and int(value) == expected
    if isinstance(expected, float):
        return kind == "Float" and math.isclose(float(value), expected, rel_tol=1e-6, abs_tol=1e-6)
    if isinstance(expected, str):
        return kind in ("Name", "String") and (value.casefold() == expected.casefold() if kind == "Name" else value == expected)
    return False


def replay(reader, cooked, trace, limit=0, function_filter=""):
    records = [json.loads(line) for line in trace.read_text(encoding="utf-8").splitlines() if line.strip()]
    matched, rejected = pairs(records)
    classes = {}
    # Reflected class identity, rather than assuming the receiver is the declaring class.
    packages = {pair[0]["func"].split(".", 1)[0] for pair in matched} | {"Core", "Engine", "GFxUI", "WillowGame"}
    for package in sorted(packages):
        path = cooked / (package + ".upk")
        if not path.is_file():
            continue
        output = subprocess.run([str(reader), str(path), "--exports"], check=True, capture_output=True, text=True, encoding="utf-8")
        for export in json.loads(output.stdout):
            if export["class"] == "Class":
                classes.setdefault(export["name"], set()).add(package + "." + export["path"])
    rows, cases = [], []
    for call, ret in matched:
        if function_filter and function_filter not in call["func"]:
            continue
        if limit and len(rows) >= limit:
            break
        row = {"seq": call.get("seq"), "function": call["func"], "status": "skipped"}
        rows.append(row)
        try:
            if "ret" not in ret or "out" in ret:
                raise ValueError("missing_return_or_out_results")
            expected = ret["ret"]
            if isinstance(expected, (dict, list)) or isinstance(expected, float) and not math.isfinite(expected):
                raise ValueError("unsupported_return")
            if isinstance(expected, str):
                encode(expected)
            obj = call.get("obj", "")
            class_name = obj.split("'", 1)[0]
            candidates = classes.get(class_name, set())
            if len(candidates) != 1:
                raise ValueError("unresolved_or_ambiguous_receiver_class")
            args = call.get("args", {})
            if not isinstance(args, dict) or any(any(c in key for c in "\t\r\n=:") for key in args):
                raise ValueError("invalid_arguments")
            fields = [call["func"].replace(":", "."), next(iter(candidates))]
            fields.extend(name + "=" + encode(value) for name, value in args.items())
            row["expected"] = expected
            cases.append((row, "\t".join(fields)))
        except ValueError as error:
            row["reason"] = str(error)
    if cases:
        with tempfile.TemporaryDirectory(prefix="ow-replay-") as folder:
            batch = Path(folder) / "cases.tsv"
            batch.write_text("\n".join(case for _, case in cases) + "\n", encoding="utf-8")
            output = subprocess.run([str(reader), str(cooked / "Core.upk"), "--run-batch", str(batch), "--cooked", str(cooked)],
                                    check=True, capture_output=True, text=True, encoding="utf-8")
        results = [json.loads(line) for line in output.stdout.splitlines()]
        if len(results) != len(cases):
            raise RuntimeError("batch result count mismatch")
        for index, ((row, _), actual) in enumerate(zip(cases, results)):
            if actual["case"] != index:
                raise RuntimeError("batch result order mismatch")
            row["actual"] = actual
            if actual["error"]:
                row.update(status="blocked", reason="runtime_or_input_error")
            elif actual["native"] or actual["unimplemented"]:
                row.update(status="blocked", reason="native_or_stub_execution")
            elif actual["type"] not in ("None", "Int", "Byte", "Bool", "Float", "String", "Name", "Object"):
                row.update(status="skipped", reason="unsupported_return")
            else:
                row["status"] = "return_match_unverified" if compare(row["expected"], actual) else "return_mismatch_unverified"
    return {"mode": "fresh defaults; live receiver state and side effects UNVERIFIED",
            "paired_calls": len(matched), "pairing_rejections": dict(rejected),
            "counts": dict(Counter(row["status"] for row in rows)), "cases": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--cooked", type=Path, default=Path(os.environ.get("OPENWILLOW_BL2", ".")) / "WillowGame/CookedPCConsole")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--function", default="")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / "local"
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error("game-derived replay reports must be written under ignored local/")
    if args.limit < 0:
        parser.error("--limit must be nonnegative")
    report = replay(args.reader.resolve(), args.cooked.resolve(), args.trace, args.limit, args.function)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in report.items() if key != "cases"}))


if __name__ == "__main__":
    main()
