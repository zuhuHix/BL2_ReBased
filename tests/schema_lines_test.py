"""Every array schema a tool hands to ow-package must name each property once.

AI-assisted. Synthetic: imports the tools and inspects their schema lists; no game data.

ow-package --array-schema rejects a schema that repeats a key ("duplicate array schema
key", src/cli.cpp). Tools build their schema by concatenating another tool's list
(e.g. slice_values = weapon_recipe + its own lines), so a line added to the shared list
can silently collide with a copy downstream. That broke tools/prepare_slice_world.py
(via slice_values) and tools/loot_pools.py once weapon_recipe gained
ValueResolverChain. This test applies the reader's rule to every module-level
SCHEMA_LINES / SCHEMA in tools/.
"""
from collections import Counter
import importlib
from pathlib import Path
import re
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1] / 'tools'
sys.path.insert(0, str(TOOLS))


def schema_modules():
    """(module name, attribute) for each tool that defines a top-level schema list or string."""
    found = []
    for path in sorted(TOOLS.glob('*.py')):
        text = path.read_text(encoding='utf-8', errors='replace')
        for attribute in ('SCHEMA_LINES', 'SCHEMA'):
            if re.search(rf'^{attribute} = ', text, re.M):
                found.append((path.stem, attribute))
    return found


def schema_keys(value):
    """Keys of a schema the way the reader parses it (blank and '#' lines skipped)."""
    lines = value.splitlines() if isinstance(value, str) else list(value)
    keys = []
    for line in lines:
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        key, equal, kind = line.partition('=')
        if not key or not equal or not kind:
            raise ValueError(f'invalid array schema line: {line!r}')
        keys.append(key)
    return keys


class SchemaLinesTest(unittest.TestCase):
    def test_rule_matches_reader(self):
        self.assertEqual(schema_keys(['A=ObjectProperty', '# note', '', 'B=StructProperty:S']), ['A', 'B'])
        with self.assertRaises(ValueError):
            schema_keys(['A='])

    def test_known_composed_schemas_are_found(self):
        names = {name for name, _ in schema_modules()}
        for expected in ('weapon_recipe', 'slice_values', 'loot_pools', 'prepare_slice_world'):
            self.assertIn(expected, names)

    def test_no_tool_schema_repeats_a_key(self):
        checked = 0
        for name, attribute in schema_modules():
            value = getattr(importlib.import_module(name), attribute, None)
            if not isinstance(value, (str, list, tuple)):
                continue
            with self.subTest(module=name, attribute=attribute):
                repeated = sorted(k for k, n in Counter(schema_keys(value)).items() if n > 1)
                self.assertEqual(repeated, [], f'{name}.{attribute} repeats {repeated}')
                checked += 1
        self.assertGreater(checked, 5)

    def test_slice_values_writes_a_valid_schema(self):
        """slice_values.build composes its schema at run time; capture what it hands the reader."""
        import slice_values
        import weapon_recipe
        written = []

        class Stop(Exception):
            pass

        def fake_package(reader, package, schema):
            written.append(Path(schema).read_text())
            raise Stop

        real = weapon_recipe.Package
        weapon_recipe.Package = fake_package
        try:
            with self.assertRaises(Stop):
                slice_values.build('ow-package', 'game')
        finally:
            weapon_recipe.Package = real
        keys = schema_keys(written[0])
        self.assertIn('ValueResolverChain', keys)
        self.assertEqual(sorted(k for k, n in Counter(keys).items() if n > 1), [])


if __name__ == '__main__':
    unittest.main()
