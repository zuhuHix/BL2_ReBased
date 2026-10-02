"""Synthetic checks for research/particle_system.py. No game files: every byte string and value is invented here.

Run: python tests/particle_system_test.py
"""
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "research"))
import particle_system as ps  # noqa: E402

I32 = lambda v: struct.pack("<i", v)
F32 = lambda v: struct.pack("<f", v)


class Names:
    """Interns strings into a name table and writes FName pairs (index, number)."""

    def __init__(self):
        self.table = ["None"]

    def __call__(self, text):
        if text not in self.table:
            self.table.append(text)
        return I32(self.table.index(text)) + I32(0)


def tag(n, name, kind, body, detail=None, index=0, boolean=None):
    out = n(name) + n(kind) + I32(len(body)) + I32(index)
    if detail is not None:
        out += n(detail)
    if boolean is not None:
        out += bytes([boolean])
    return out + body


def end(n):
    return n("None")


def raw_float(n, table, time_scale=None, op=None, elements=None, chunk=None, dist_ref=0):
    body = tag(n, "Distribution", "ObjectProperty", I32(dist_ref))
    if op is not None:
        body += tag(n, "Op", "ByteProperty", bytes([op]), detail="None")
    if elements is not None:
        body += tag(n, "LookupTableNumElements", "ByteProperty", bytes([elements]), detail="None")
    if chunk is not None:
        body += tag(n, "LookupTableChunkSize", "ByteProperty", bytes([chunk]), detail="None")
    body += tag(n, "LookupTable", "ArrayProperty", I32(len(table)) + b"".join(F32(v) for v in table))
    if time_scale is not None:
        body += tag(n, "LookupTableTimeScale", "FloatProperty", F32(time_scale))
    return body + end(n)


def ctx_of(n):
    return ps.Context(n.table, lambda ref: "obj%d" % ref)


class TaggedStream(unittest.TestCase):
    def test_scalars_structs_and_arrays_consume_exactly(self):
        n = Names()
        burst = (tag(n, "Count", "IntProperty", I32(7)) + tag(n, "CountLow", "IntProperty", I32(-1))
                 + tag(n, "Time", "FloatProperty", F32(0.5)) + end(n))
        stream = (tag(n, "Lives", "IntProperty", I32(3))
                  + tag(n, "Scale", "FloatProperty", F32(1.5))
                  + tag(n, "bOn", "BoolProperty", b"", boolean=1)
                  + tag(n, "Mode", "ByteProperty", n("MODE_Two"), detail="EMode")
                  + tag(n, "Raw", "ByteProperty", b"\x09", detail="None")
                  + tag(n, "Label", "NameProperty", n("Hello"))
                  + tag(n, "Target", "ObjectProperty", I32(12))
                  + tag(n, "Offset", "StructProperty", F32(1) + F32(2) + F32(3), detail="Vector")
                  + tag(n, "Tint", "StructProperty", bytes([10, 20, 30, 40]), detail="Color")
                  + tag(n, "Rate", "StructProperty", raw_float(n, [2.0, 2.0, 2.0, 2.0]), detail="RawDistributionFloat")
                  + tag(n, "BurstList", "ArrayProperty", I32(1) + burst)
                  + tag(n, "Lives", "IntProperty", I32(4), index=1)
                  + end(n))
        data = b"\0" * 4 + stream
        sizes = []
        props, after = ps.walk(ctx_of(n), data, 4, len(data), sizes=sizes)
        self.assertEqual(after, len(data))
        self.assertEqual(props["Lives"], 3)
        self.assertEqual(props["Lives[1]"], 4)
        self.assertAlmostEqual(props["Scale"], 1.5)
        self.assertIs(props["bOn"], True)
        self.assertEqual(props["Mode"], "MODE_Two")
        self.assertEqual(props["Raw"], 9)
        self.assertEqual(props["Label"], "Hello")
        self.assertEqual(props["Target"], "obj12")
        self.assertEqual(props["Offset"], {"X": 1.0, "Y": 2.0, "Z": 3.0})
        self.assertEqual(props["Tint"], {"B": 10, "G": 20, "R": 30, "A": 40})
        self.assertEqual(props["Rate"]["_type"], "RawDistributionFloat")
        self.assertEqual(props["Rate"]["LookupTable"], [2.0] * 4)
        self.assertIsNone(props["Rate"]["Distribution"])
        self.assertEqual(props["BurstList"][0]["Count"], 7)
        self.assertEqual(len(sizes), 12)

    def test_declared_size_must_match_the_value(self):
        n = Names()
        bad = n("Lives") + n("IntProperty") + I32(8) + I32(0) + I32(1) + I32(2) + end(n)
        with self.assertRaises(ps.DecodeError):
            ps.walk(ctx_of(n), bad, 0, len(bad))

    def test_float_array_size_must_be_four_bytes_per_element(self):
        n = Names()
        body = I32(3) + F32(1) + F32(2)  # claims three floats, carries two
        bad = tag(n, "LookupTable", "ArrayProperty", body) + end(n)
        with self.assertRaises(ps.DecodeError):
            ps.walk(ctx_of(n), bad, 0, len(bad))

    def test_unknown_arrays_are_kept_raw(self):
        n = Names()
        data = tag(n, "Mystery", "ArrayProperty", I32(2) + b"\x01\x02\x03") + end(n)
        props, _ = ps.walk(ctx_of(n), data, 0, len(data))
        self.assertEqual(props["Mystery"], {"_raw_array": "010203", "_count": 2})


class Merge(unittest.TestCase):
    def test_struct_fields_merge_and_arrays_replace(self):
        default = {"A": 1, "S": {"Op": 1, "LookupTable": [1.0, 1.0, 1.0, 1.0], "LookupTableTimeScale": 0.0}}
        own = {"S": {"LookupTable": [5.0, 5.0, 5.0, 5.0]}, "B": 2}
        m = ps.merge(default, own)
        self.assertEqual(m["A"], 1)
        self.assertEqual(m["B"], 2)
        self.assertEqual(m["S"]["Op"], 1)
        self.assertEqual(m["S"]["LookupTable"], [5.0] * 4)
        self.assertEqual(default["S"]["LookupTable"], [1.0] * 4)  # defaults untouched


def table(values, width=1, elements=1, time_scale=0.0, start=0.0, op=1):
    return {"type": 0, "op": op, "elements": elements, "chunk": elements * width, "time_scale": time_scale,
            "start_time": start, "width": width, "values": values}


class BakedTables(unittest.TestCase):
    def test_constant_float_is_two_equal_entries(self):
        t = table([4.0, 4.0, 4.0, 4.0])
        self.assertEqual(ps.check_table(t), [])
        self.assertEqual(ps.summarize(t), {"kind": "constant", "value": 4.0})

    def test_vector_pair(self):
        lo, hi = [1.0, 2.0, 3.0], [4.0, 5.0, 6.0]
        t = table([1.0, 6.0] + lo + hi + lo + hi, width=3, elements=2, op=2)
        self.assertEqual(ps.check_table(t), [])
        self.assertEqual(ps.summarize(t), {"kind": "pair", "value": {"low": lo, "high": hi}})

    def test_curve_times_follow_time_scale_and_start(self):
        t = table([0.0, 3.0, 0.0, 1.0, 3.0], time_scale=4.0, start=0.5)
        self.assertEqual(ps.check_table(t), [])
        self.assertEqual([round(x, 6) for x, _ in ps.entries(t)], [0.5, 0.75, 1.0])
        self.assertAlmostEqual(ps.sample(t, 0.625)[0], 0.5)
        self.assertAlmostEqual(ps.sample(t, 2.0)[0], 3.0)   # clamped at the end
        self.assertAlmostEqual(ps.sample(t, 0.0)[0], 0.0)   # clamped at the start

    def test_range_header_may_exceed_the_samples(self):
        # A curve key between samples can lift the stored range above every sample.
        t = table([0.0, 1.0, 0.0, 0.9, 0.2], time_scale=2.0)
        self.assertEqual(ps.check_table(t), [])

    def test_layout_failures_are_reported(self):
        self.assertTrue(ps.check_table(table([1.0, 1.0, 1.0], width=3)))           # not 2 + n x 3
        self.assertTrue(ps.check_table(table([0.0, 1.0, 5.0, 5.0])))               # range does not bound
        self.assertTrue(ps.check_table(table([1.0, 1.0, 1.0, 1.0, 1.0])))          # 3 entries, no time scale
        bad = table([0.0, 1.0, 0.0, 1.0, 0.0, 1.0])
        bad["chunk"] = 2  # elements 1 x width 1 != 2
        self.assertTrue(ps.check_table(bad))

    def test_pair_header_is_min_low_and_max_high(self):
        self.assertEqual(ps.check_table(table([3.0, 1.0, 3.0, 1.0, 3.0, 1.0], elements=2, op=2)), [])
        self.assertTrue(ps.check_table(table([9.0, 9.0, 3.0, 1.0, 3.0, 1.0], elements=2, op=2)))


class Curves(unittest.TestCase):
    def test_linear_constant_and_hermite_segments(self):
        pts = [{"InVal": 0.0, "OutVal": 0.0, "InterpMode": "CIM_Linear"},
               {"InVal": 1.0, "OutVal": 2.0, "InterpMode": "CIM_Constant"},
               {"InVal": 2.0, "OutVal": 6.0, "InterpMode": "CIM_CurveUser", "ArriveTangent": 0.0, "LeaveTangent": 0.0},
               {"InVal": 3.0, "OutVal": 0.0, "ArriveTangent": 0.0}]
        self.assertAlmostEqual(ps.eval_curve(pts, 0.5, 1)[0], 1.0)
        self.assertAlmostEqual(ps.eval_curve(pts, 1.5, 1)[0], 2.0)
        self.assertAlmostEqual(ps.eval_curve(pts, 2.5, 1)[0], 3.0)  # flat tangents: midpoint of 6 and 0
        self.assertAlmostEqual(ps.eval_curve(pts, -1.0, 1)[0], 0.0)
        self.assertAlmostEqual(ps.eval_curve(pts, 9.0, 1)[0], 0.0)

    def test_distribution_objects_as_baked_entries(self):
        self.assertEqual(ps.eval_distribution("DistributionFloatUniform", {"Min": 1.0, "Max": 3.0}, 0, 1), [1.0, 3.0])
        v = ps.eval_distribution("DistributionVectorConstant", {"Constant": {"X": 1.0, "Y": 2.0, "Z": 3.0}}, 0, 3)
        self.assertEqual(v, [1.0, 2.0, 3.0])
        curve = {"ConstantCurve": {"Points": [{"InVal": 0.0, "OutVal": 1.0}, {"InVal": 1.0, "OutVal": 3.0}]}}
        self.assertAlmostEqual(ps.eval_distribution("DistributionFloatConstantCurve", curve, 0.25, 1)[0], 1.5)
        self.assertIsNone(ps.eval_distribution("DistributionFloatParticleParameter", {}, 0, 1))


class FakePkg:
    """Just enough of behavior_census.Pkg for Package.own and Resolver."""

    def __init__(self, name, names, objects):
        # objects: list of (path, class, archetype, outer, payload)
        self.name, self.names = name, names
        self.data = b""
        self.exports, self.paths, self.classes = [], [], []
        for path, cls, _arch, outer, payload in objects:
            self.exports.append({"off": len(self.data), "size": len(payload), "outer": outer,
                                 "name": path.split(".")[-1]})
            self.paths.append(path)
            self.classes.append(cls)
            self.data += payload

    def path(self, ref):
        return self.paths[ref - 1] if ref > 0 else "Engine.Imported"

    def class_name(self, ref):
        return self.classes[ref - 1]


def fake_package(name, names, objects):
    P = ps.Package.__new__(ps.Package)
    P.pkg = FakePkg(name, names.table, objects)
    P.name = name
    P.ctx = ps.Context(names.table, P.pkg.path)
    P.by_path = {p.lower(): i for i, p in enumerate(P.pkg.paths, 1)}
    P.archetypes = [o[2] for o in objects]
    P.cache = {}
    return P


class PackageAndResolver(unittest.TestCase):
    def test_prefix_is_chosen_by_exact_consumption(self):
        n = Names()
        body = tag(n, "Lives", "IntProperty", I32(5)) + end(n)
        P = fake_package("Pk", n, [("A", "Thing", 0, 0, b"\0" * 4 + body),
                                   ("B", "Thing", 0, 0, b"\0" * 8 + body)])
        self.assertEqual((P.own(1)["Lives"], P.own(1)["_prefix"], P.own(1)["_trailing"]), (5, 4, 0))
        self.assertEqual((P.own(2)["Lives"], P.own(2)["_prefix"], P.own(2)["_trailing"]), (5, 8, 0))

    def test_effective_values_follow_the_class_default_chain(self):
        n = Names()
        cdo = tag(n, "Rate", "StructProperty",
                  raw_float(n, [1.0, 1.0, 1.0, 1.0], op=1, elements=1, chunk=1), detail="RawDistributionFloat")
        cdo += tag(n, "Flag", "IntProperty", I32(9)) + end(n)
        engine = fake_package("Engine", n, [("Default__ParticleModuleThing", "ParticleModuleThing", 0, 0,
                                             b"\0" * 4 + cdo)])
        own = tag(n, "Rate", "StructProperty", raw_float(n, [3.0, 3.0, 3.0, 3.0]), detail="RawDistributionFloat") + end(n)
        P = fake_package("Pk", n, [("Sys.Mod", "ParticleModuleThing", 0, 0, b"\0" * 4 + own)])
        R = ps.Resolver.__new__(ps.Resolver)
        R.packages, R.engine, R.memo = {"Engine": engine, "Pk": P}, engine, {}
        m = R.effective(P, 1)
        self.assertEqual(m["Flag"], 9)                       # only in the CDO
        self.assertEqual(m["Rate"]["Op"], 1)                 # header bytes inherited field by field
        self.assertEqual(m["Rate"]["LookupTableChunkSize"], 1)
        self.assertEqual(m["Rate"]["LookupTable"], [3.0] * 4)  # own table replaces the default's
        t = ps.raw_table(m["Rate"], ps.FLOAT_WIDTH[m["Rate"]["_type"]])
        self.assertEqual(ps.summarize(t), {"kind": "constant", "value": 3.0})


class Compact(unittest.TestCase):
    def test_compact_prefers_parameter_then_table(self):
        rendered = {"Lifetime": {"header": {}, "parameter": {"name": "P"}},
                    "Size": {"header": {}, "table": {"kind": "constant", "value": 2.0}},
                    "Other": {"header": {}}, "Plain": 3}
        self.assertEqual(ps.compact(rendered), {"Lifetime": {"parameter": {"name": "P"}},
                                                "Size": {"kind": "constant", "value": 2.0},
                                                "Other": None, "Plain": 3})


if __name__ == "__main__":
    unittest.main()
