"""vm_census.py's merge of the sweep rows, on invented rows (review r2 of #519)."""
import collections
import json
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from vm_census import add_demand, demand_key


class VmCensusTest(unittest.TestCase):
    def test_off_rows_with_a_package_stay_apart_and_fold_case(self):
        rows = json.loads('[{"functions_reaching":2,"native":"Widget.Ping()","package":"PackA"},'
                          '{"functions_reaching":1,"native":"Widget.Ping()","package":"PackB"}]')
        demand = collections.Counter()
        add_demand(demand, rows)
        add_demand(demand, [{"functions_reaching": 3, "native": "Widget.Ping()", "package": "PACKA"}])
        self.assertEqual(demand, {"Widget.Ping() [packa]": 5, "Widget.Ping() [packb]": 1})

    def test_reference_rows_keep_the_bare_key(self):
        demand = collections.Counter()
        add_demand(demand, json.loads('[{"functions_reaching":2,"native":"Widget.Ping()"}]'))
        self.assertEqual(demand, {"Widget.Ping()": 2})
        self.assertEqual(demand_key({"native": "function Foo.Bar", "package": ""}), "function Foo.Bar")


if __name__ == '__main__':
    unittest.main()
