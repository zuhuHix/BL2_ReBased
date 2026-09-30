"""Synthetic return-record contract checks; no game or SDK dependency."""
import ast
from pathlib import Path
from types import SimpleNamespace
import unittest


def callback(records):
    source = Path(__file__).resolve().parents[1] / 'tools/sdk_trace/openwillow_uitrace/__init__.py'
    tree = ast.parse(source.read_text())
    function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'on_return')
    namespace = {'state': {'last_detailed': {}}, 'plain': lambda value: value,
                 'write': records.append}
    exec(compile(ast.Module(body=[function], type_ignores=[]), str(source), 'exec'), namespace)
    return namespace['on_return'], namespace['state']


def function(path):
    return SimpleNamespace(func=SimpleNamespace(_path_name=lambda: path))


class ReturnRecords(unittest.TestCase):
    def test_display_getter_keeps_completed_output_and_identity(self):
        records = []
        record, _ = callback(records)
        native_output = {'X': 120., 'Z': -300., 'YRotation': 18., 'hasZ': True}
        record('synthetic-clip', SimpleNamespace(D=native_output), None,
               function('GFxUI.GFxObject:GetDisplayInfo'))
        self.assertEqual(records[0]['out']['D'], native_output)
        self.assertEqual(records[0]['obj'], 'synthetic-clip')
        self.assertIsNone(records[0]['ret'])

    def test_other_returns_do_not_require_display_arguments(self):
        records = []
        record, _ = callback(records)
        record('synthetic-movie', None, 7, function('Synthetic.Movie:ReadCount'))
        self.assertEqual(records[0]['ret'], 7)
        self.assertNotIn('out', records[0])

    def test_budget_and_error_isolation(self):
        records = []
        record, state = callback(records)
        path = 'GFxUI.GFxObject:GetDisplayInfo'
        state['last_detailed'][path] = False
        record(None, None, None, function(path))
        self.assertEqual(records, [])
        state['last_detailed'][path] = True
        record(None, None, None, function(path))
        self.assertEqual(records[0]['phase'], 'error')


if __name__ == '__main__':
    unittest.main()
