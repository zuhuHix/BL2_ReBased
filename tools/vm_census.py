"""VM sweep census: run every script function of the BL2 code packages on the VM and rank what is missing.

For each package, `ow-package <package> --vm-sweep` instantiates each class with its default object and calls
every decodable script function once with zero-valued arguments (natives without an implementation are logged
stubs). This script merges the per-package reports into:

  * how many script functions execute end to end, and what stops the rest;
  * the natives that scripts reach but no implementation exists for, ranked by the number of script functions
    that reach them. That ranking is the practical work list for ROADMAP Phases 3 and 4.

Needs a Borderlands 2 install (OPENWILLOW_BL2 or --game). Writes nothing unless --json is given; keep any
output under ignored local/ (it names game functions).

Usage: python tools/vm_census.py --reader build/Release/ow-package.exe [--top 40] [--json local/vm_census.json]
"""
import argparse
import collections
import json
import os
import subprocess
import sys

PACKAGES = ["Core", "Engine", "GameFramework", "GearboxFramework", "WillowGame", "GFxUI", "IpDrv",
            "OnlineSubsystemSteamworks"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reader", required=True)
    parser.add_argument("--game", default=None)
    parser.add_argument("--top", type=int, default=40)
    parser.add_argument("--steps", type=int, default=200000)
    parser.add_argument("--json", default=None)
    args = parser.parse_args()
    game = args.game or os.path.join(os.environ["OPENWILLOW_BL2"], "WillowGame", "CookedPCConsole")
    demand = collections.Counter()
    failures = collections.Counter()
    examples = {}
    totals = collections.Counter()
    print(f"{'package':<28}{'functions':>10}{'run ok':>9}{'failed':>8}{'undecodable':>13}{'seconds':>9}")
    for name in PACKAGES:
        out = subprocess.run([args.reader, os.path.join(game, name + ".upk"), "--vm-sweep", "--cooked", game,
                              "--top", "100000", "--steps", str(args.steps)],
                             capture_output=True, text=True, encoding="utf-8")
        if out.returncode != 0:
            print(f"{name}: sweep failed: {out.stderr.strip()}")
            return 1
        data = json.loads(out.stdout)
        totals["attempted"] += data["attempted"]
        totals["succeeded"] += data["succeeded"]
        totals["undecodable"] += data["undecodable_skipped"]
        print(f"{name:<28}{data['attempted']:>10}{data['succeeded']:>9}{data['failed']:>8}"
              f"{data['undecodable_skipped']:>13}{data['seconds']:>9.2f}")
        for item in data["unimplemented_natives"]:
            demand[item["native"]] += item["functions_reaching"]
        for item in data["failures"]:
            key = item["example"].split(":", 1)[1].strip()[:70] if ":" in item["example"] else item["example"]
            failures[key] += item["count"]
            examples.setdefault(key, item["example"])
    print(f"{'TOTAL':<28}{totals['attempted']:>10}{totals['succeeded']:>9}{totals['attempted'] - totals['succeeded']:>8}"
          f"{totals['undecodable']:>13}")
    print(f"\n{len(demand)} distinct unimplemented natives reached. Top {args.top} by script functions reaching them:")
    for native, count in demand.most_common(args.top):
        print(f"  {count:>6}  {native}")
    print("\nWhat stops the functions that do not finish:")
    for key, count in failures.most_common(10):
        print(f"  {count:>4}  {examples[key][:150]}")
    if args.json:
        with open(args.json, "w", encoding="utf-8") as handle:
            json.dump({"totals": totals, "demand": demand.most_common(), "failures": failures.most_common()}, handle,
                      indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
