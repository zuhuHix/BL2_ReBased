"""List every native UFunction of the installed script packages with its iNative number.

Writes tab-separated lines "Package.Class.Function<TAB>iNative<TAB>FunctionFlags(hex)" to the output file. The
output is game-derived (names and numbers read from the player's packages): write it outside the repository or
under the ignored local/ folder. OwNativeTables.java joins it with the executable's registration tables to map
script native_<n> to an address (gnatives.tsv).

UFunction tail (research/native_count.py): [iNative u16][OperPrecedence u8][FunctionFlags u32]
[RepOffset u16 if FUNC_Net][FriendlyName FName].

Usage: python tools/ghidra/script_natives.py <out.tsv> [--cooked <CookedPCConsole>]
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "research"))
import native_count as nc  # noqa: E402

PACKAGES = ["Core.upk", "Engine.upk", "GameFramework.upk", "GearboxFramework.upk", "WillowGame.upk", "GFxUI.upk",
            "IpDrv.upk", "OnlineSubsystemSteamworks.upk", "AkAudio.upk"]
FUNC_NET, FUNC_NATIVE = 0x40, 0x400


def natives(cooked, package):
    data = nc.unwrap_fully_compressed(os.path.join(cooked, package))
    _, _, names, imports, exports = nc.parse_package(data)
    function_class = next((-(i + 1) for i, im in enumerate(imports) if im["name"] == "Function" and im["class"] == "Class"), None)
    for export in exports:
        if export["class"] != function_class:
            continue
        flags = nc.read_function_flags(data, export, names)
        if flags is None or not flags & FUNC_NATIVE:
            continue
        end = export["off"] + export["size"]
        has_rep = bool(flags & FUNC_NET) and struct.unpack_from("<I", data, end - 14)[0] == flags
        inative = struct.unpack_from("<H", data, end - (17 if has_rep else 15))[0]
        owner = nc.objname(export["outer"], imports, exports)
        yield f"{package[:-4]}.{owner}.{export['name']}", inative, flags


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("out")
    default = os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole")
    parser.add_argument("--cooked", default=default)
    args = parser.parse_args()
    repo = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))
    target = os.path.realpath(args.out)
    if target.startswith(repo + os.sep) and not target.startswith(os.path.join(repo, "local") + os.sep):
        sys.exit("refusing to write game-derived output inside the repository outside local/: " + target)
    count = numbered = 0
    with open(target, "w", encoding="utf-8") as out:
        for package in PACKAGES:
            path = os.path.join(args.cooked, package)
            if not os.path.exists(path):
                print("missing", path, file=sys.stderr)
                continue
            for name, inative, flags in natives(args.cooked, package):
                out.write(f"{name}\t{inative}\t{flags:x}\n")
                count += 1
                numbered += inative != 0
    print(f"{count} native functions, {numbered} with an iNative -> {target}")


if __name__ == "__main__":
    main()
