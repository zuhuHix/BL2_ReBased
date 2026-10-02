"""Compute the in-memory (32-bit) field layout of a script class or struct from the installed packages.

Native code reads object fields by byte offset; this prints the offset of every script-declared property so a
decompiled read such as "+0x44" can be named. Prints to stdout (game-derived names: keep any saved output under
ignored local/ or outside the repository).

How the layout is computed (RECOVERED FROM THE PACKAGES, rules UNVERIFIED beyond the oracle below):
  - a struct's fields form a linked list: each field export stores Next right after its NetIndex and the None
    that ends its tagged properties (functions carry the Gearbox u16-array prefix first, see
    research/script_disasm.py). The head is the child that no other child points at. Cooked packages list the
    fields in reverse declaration order and offsets are assigned in that list order;
  - a property export continues with ArrayDim i32, PropertyFlags u64, Category FName, ArrayEnum i32,
    RepOffset u16 when CPF_Net (0x20), then its type reference (Struct, Class, Enum, Inner...);
  - sizes/alignments on 32-bit MSVC: Byte 1, Int/Float/Object/Class/Component 4, Bool 4 with consecutive bools
    sharing one 32-bit word (bit 0 first), Name 8, Interface 8, Str/Array 12, Delegate 12, structs their computed
    size aligned to their largest member alignment; a class starts at its superclass size.
Oracle: Core.Object must come out at 0x3C bytes, which matches the decompiled engine (Outer at 0x28).

Usage: python tools/ghidra/class_layout.py WillowGame.MissionObjectiveDefinition [Core.Object ...] [--cooked <dir>]
       A name may be Package.Class, Class, or Package.Class.Struct for script structs.
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "research"))
import native_count as nc  # noqa: E402

PACKAGES = ["Core", "Engine", "GameFramework", "GearboxFramework", "WillowGame", "GFxUI", "IpDrv",
            "OnlineSubsystemSteamworks", "AkAudio"]
CPF_NET = 0x20
SIMPLE = {"ByteProperty": (1, 1), "IntProperty": (4, 4), "FloatProperty": (4, 4), "ObjectProperty": (4, 4),
          "ClassProperty": (4, 4), "ComponentProperty": (4, 4), "NameProperty": (8, 4), "InterfaceProperty": (8, 4),
          "StrProperty": (12, 4), "ArrayProperty": (12, 4), "DelegateProperty": (12, 4), "BoolProperty": (4, 4)}


class Packages:
    def __init__(self, cooked):
        self.cooked = cooked
        self.loaded = {}
        self.by_path = {}      # "Package.Outer.Name" (lower) -> (package, export index 1-based)

    def load(self, name):
        if name in self.loaded:
            return self.loaded[name]
        data = nc.unwrap_fully_compressed(os.path.join(self.cooked, name + ".upk"))
        _, _, names, imports, exports = nc.parse_package(data)
        pkg = {"name": name, "data": data, "names": names, "imports": imports, "exports": exports}
        self.loaded[name] = pkg
        for i in range(1, len(exports) + 1):
            self.by_path[(name + "." + self.export_path(pkg, i)).lower()] = (pkg, i)
        return pkg

    def export_path(self, pkg, index):
        parts = []
        while index > 0:
            e = pkg["exports"][index - 1]
            parts.append(e["name"])
            index = e["outer"]
        return ".".join(reversed(parts))

    def import_path(self, pkg, index):
        parts = []
        while index < 0:
            im = pkg["imports"][-index - 1]
            parts.append(im["name"])
            index = im["outer"]
        return ".".join(reversed(parts))

    def resolve(self, pkg, ref):
        """Object reference -> (package, export index), loading the package an import lives in."""
        if ref > 0:
            return pkg, ref
        if ref == 0:
            return None
        path = self.import_path(pkg, ref)
        package = path.split(".", 1)[0]
        if package in PACKAGES:
            self.load(package)
        return self.by_path.get(path.lower())

    def find(self, name):
        for p in PACKAGES:
            self.load(p)
        if name.lower() in self.by_path:
            return self.by_path[name.lower()]
        hits = [v for k, v in self.by_path.items() if k.endswith("." + name.lower()) and
                self.class_name(*v) in ("Class", "ScriptStruct")]
        if len(hits) != 1:
            raise SystemExit(f"{name}: {len(hits)} matches; use Package.Class")
        return hits[0]

    def class_name(self, pkg, index):
        return nc.objname(pkg["exports"][index - 1]["class"], pkg["imports"], pkg["exports"]) or "Class"


def i32(data, at):
    return struct.unpack_from("<i", data, at)[0]


class Layout:
    def __init__(self, packages):
        self.p = packages
        self.sizes = {}        # (package name, index) -> (size, alignment)

    def next_of(self, pkg, index):
        e = pkg["exports"][index - 1]
        at = e["off"]
        if self.p.class_name(pkg, index) == "Function":
            count = struct.unpack_from("<H", pkg["data"], at)[0]
            at += 2 + 2 * count
        return i32(pkg["data"], at + 12)

    def chain(self, pkg, index):
        children = [i for i, e in enumerate(pkg["exports"], 1) if e["outer"] == index]
        nexts = {c: self.next_of(pkg, c) for c in children}
        pointed = set(nexts.values())
        heads = [c for c in children if c not in pointed]
        if len(heads) > 1:
            raise SystemExit(f"{self.p.export_path(pkg, index)}: {len(heads)} field-list heads")
        order, cursor, seen = [], heads[0] if heads else 0, set()
        while cursor > 0 and cursor not in seen:
            seen.add(cursor)
            order.append(cursor)
            cursor = nexts.get(cursor, 0)
        return order

    def property(self, pkg, index):
        """(type, array dim, flags, type reference) of a property export."""
        e = pkg["exports"][index - 1]
        data, at = pkg["data"], e["off"] + 16
        dim = i32(data, at)
        flags = struct.unpack_from("<Q", data, at + 4)[0]
        at += 4 + 8 + 8 + 4          # ArrayDim, PropertyFlags, Category, ArrayEnum
        if flags & CPF_NET:
            at += 2
        ref = i32(data, at) if at + 4 <= e["off"] + e["size"] else 0
        return self.p.class_name(pkg, index), dim, flags, ref

    def size_of(self, pkg, index):
        key = (pkg["name"], index)
        if key not in self.sizes:
            fields = self.fields(pkg, index)
            self.sizes[key] = fields[-1] if fields else (0, 1)
        return self.sizes[key]

    def fields(self, pkg, index):
        """[(offset, size, type, name, bit or None, detail)...] then (total size, alignment) as the last item."""
        e = pkg["exports"][index - 1]
        offset, align, rows = 0, 4 if self.p.class_name(pkg, index) == "Class" else 1, []
        if e["super"]:
            target = self.p.resolve(pkg, e["super"])
            if not target:
                raise SystemExit(f"unresolved super of {self.p.export_path(pkg, index)}")
            offset, sup_align = self.size_of(*target)
            align = max(align, sup_align)
        bool_word, bool_bit = None, 0
        for child in self.chain(pkg, index):
            kind = self.p.class_name(pkg, child)
            if not kind.endswith("Property"):
                continue
            ptype, dim, flags, ref = self.property(pkg, child)
            name = pkg["exports"][child - 1]["name"]
            detail = ""
            if ptype == "BoolProperty":
                if bool_word is not None and bool_bit < 31:
                    bool_bit += 1
                    rows.append((bool_word, 4, ptype, name, bool_bit, ""))
                    continue
                offset = (offset + 3) & ~3
                bool_word, bool_bit = offset, 0
                rows.append((offset, 4, ptype, name, 0, ""))
                offset += 4
                align = max(align, 4)
                continue
            bool_word = None
            if ptype == "StructProperty":
                target = self.p.resolve(pkg, ref)
                if not target:
                    raise SystemExit(f"unresolved struct of {name}")
                size, a = self.size_of(*target)
                detail = self.p.export_path(*target)
            elif ptype in SIMPLE:
                size, a = SIMPLE[ptype]
                if ptype == "ArrayProperty" and ref > 0:
                    # The inner property is an export of this package; name its element type.
                    itype, _, _, iref = self.property(pkg, ref)
                    target = self.p.resolve(pkg, iref) if iref else None
                    detail = "of " + itype + (" " + self.p.export_path(*target) if target else "")
                if ref and ptype in ("ObjectProperty", "ClassProperty", "ComponentProperty", "ByteProperty",
                                     "InterfaceProperty"):
                    target = self.p.resolve(pkg, ref)
                    detail = self.p.export_path(*target) if target else ""
            else:
                raise SystemExit(f"{name}: unsupported property type {ptype}")
            offset = (offset + a - 1) & ~(a - 1)
            rows.append((offset, size * dim, ptype, name + (f"[{dim}]" if dim > 1 else ""), None, detail))
            offset += size * dim
            align = max(align, a)
        total = (offset + align - 1) & ~(align - 1)
        rows.append((total, align))
        return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("names", nargs="+")
    parser.add_argument("--cooked", default=os.path.join(os.environ.get("OPENWILLOW_BL2", ""), "WillowGame", "CookedPCConsole"))
    args = parser.parse_args()
    packages = Packages(args.cooked)
    layout = Layout(packages)
    for name in args.names:
        pkg, index = packages.find(name)
        rows = layout.fields(pkg, index)
        total, align = rows[-1]
        e = pkg["exports"][index - 1]
        sup = packages.resolve(pkg, e["super"]) if e["super"] else None
        print(f"{pkg['name']}.{packages.export_path(pkg, index)}: size 0x{total:x}, align {align}"
              + (f", extends {packages.export_path(*sup)}" if sup else ""))
        for offset, size, ptype, field, bit, detail in rows[:-1]:
            where = f"0x{offset:04x}" + (f".bit{bit}" if bit is not None else "")
            print(f"  {where:<12} {size:>4}  {ptype:<18} {field}" + (f"  ({detail})" if detail else ""))


if __name__ == "__main__":
    main()
