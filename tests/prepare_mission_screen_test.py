"""Synthetic tests for tools/prepare_mission_screen.py's string step (invented strings, no game files). Run: python tests/prepare_mission_screen_test.py"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_mission_screen as tool  # noqa: E402

table = {key: 'text' for key in tool.STRINGS.values()}
table[tool.STRINGS['accept']] = '<StringAliasMap:Yes> Take'
table[tool.STRINGS['complete']] = '<StringAliasMap:Yes> Done     <StringAliasMap:No> Leave'

strings = tool.strings_from(table, {'Yes': '[Y]', 'No': '[N]'})
assert strings['accept'] == '[Y] Take', strings
assert strings['complete'] == '[Y] Done     [N] Leave', strings
assert set(strings) == set(tool.STRINGS)

# A prompt with a hole in it is an error, not a page that shows the token.
for broken, aliases in ((dict(table), {'Yes': '[Y]'}), ({k: v for k, v in table.items() if k != tool.STRINGS['accept']}, {'Yes': '[Y]', 'No': '[N]'})):
    try:
        tool.strings_from(broken, aliases)
    except KeyError:
        continue
    raise AssertionError('a missing string or key label must fail')
print('ok')
