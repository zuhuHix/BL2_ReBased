"""Cross-check the C++ bytecode loader against the Python oracle.

Runs `ow-package <package> --script-check --failures` for every code package and
`research/script_disasm.py`'s checker on the same package, then requires the same number of
functions, natives, decoded functions and the same set of failing functions. Needs a Borderlands 2
install (OPENWILLOW_BL2 or --game); nothing is written.

Usage: python tools/verify_scripts.py --reader build/Release/ow-package.exe
"""
import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "research"))
import native_count as nc  # noqa: E402
import script_disasm as sd  # noqa: E402

PACKAGES = sd.ALL_PACKAGES


def python_result(path):
    pkg = sd.Package(path)
    natives = pkg.native_table()
    total = native = decoded = 0
    failures = set()
    for _, e, flags in pkg.functions():
        total += 1
        if flags & nc.FUNC_NATIVE:
            native += 1
            continue
        try:
            sd.decode_function(pkg, e, flags, natives)
            decoded += 1
        except Exception:
            failures.add(f"{pkg.qualified(e['outer'])}.{e['name']}")
    return total, native, decoded, failures


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reader", required=True)
    parser.add_argument("--game", default=None)
    args = parser.parse_args()
    game = args.game or os.path.join(os.environ["OPENWILLOW_BL2"], "WillowGame", "CookedPCConsole")
    bad = 0
    for name in PACKAGES:
        path = os.path.join(game, name)
        out = subprocess.run([args.reader, path, "--script-check", "--failures"], capture_output=True, text=True,
                             encoding="utf-8")
        if out.returncode != 0:
            print(f"{name}: reader failed: {out.stderr.strip()}")
            bad += 1
            continue
        cpp = json.loads(out.stdout)
        cpp_failures = {item.split(":", 1)[0] for item in cpp["failures"]}
        total, native, decoded, failures = python_result(path)
        ok = (cpp["functions"], cpp["native"], cpp["decoded"]) == (total, native, decoded)
        same_set = failures == cpp_failures
        status = "match" if ok and same_set else "MISMATCH"
        print(f"{name:<32} functions {cpp['functions']:>6} native {cpp['native']:>5} decoded {cpp['decoded']:>6} "
              f"failed {cpp['failed']:>3}   python {decoded:>6}/{len(failures):<3}  {status}")
        if status != "match":
            bad += 1
            print("   only C++:", sorted(cpp_failures - failures)[:5], " only Python:", sorted(failures - cpp_failures)[:5])
    print("PASS" if not bad else "FAIL")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
