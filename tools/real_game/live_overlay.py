"""weapon_recipe.Package whose weapon parts, types and name parts carry the values read from the running game.

Our own code. The input is the file tools/real_game/scripts/weapon_dump.py writes (game data: it stays under
ignored local/, normally local/realgame/cards/live_weapon_data.json). Same idea as tools/weapon_card_audit.py's
RuntimeOverlayPackage, but the source is the live game instead of OpenBLCMM's static dump. Differences from
the cooked packages are the game's own values at run time, whatever their origin (not explained yet).

Shape handling: the dump lists every property, including defaults the cooked packages omit, so properties
the live game reports as empty or zero are dropped from structs (the evaluator treats missing as default),
object references (written as Class'path') become plain paths, and the one enum in these keys,
EModifierType, is turned back into its name.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import weapon_recipe  # noqa: E402

ENUM_MODIFIER_TYPE = {0: 'MT_Scale', 1: 'MT_PreAdd', 2: 'MT_PostAdd'}  # observed order, same as the cooked decode


def clean(value, key=None):
    if isinstance(value, dict):
        out = {k: clean(v, k) for k, v in value.items()}
        out = {k: v for k, v in out.items() if v not in (None, 'None', '', [], {})}
        if out.get('BaseValueScaleConstant') == 1.0 and 'BaseValueAttribute' not in out:
            out.pop('BaseValueScaleConstant')  # the default, the cooked structs omit it
        return out
    if isinstance(value, list):
        return [clean(v, key) for v in value]
    if key == 'ModifierType' and isinstance(value, int):
        return ENUM_MODIFIER_TYPE[value]
    if isinstance(value, str) and "'" in value:
        return value.split("'")[1]
    return value


class LivePackage(weapon_recipe.Package):
    def __init__(self, reader, path, schema, live_data):
        super().__init__(reader, path, schema)
        self.live = json.loads(Path(live_data).read_text(encoding='utf-8'))
        self.overlaid = {}
        self.replaced = {}

    def props(self, path):
        if path in self.overlaid:
            return self.overlaid[path]
        props = super().props(path)
        row = self.live.get(path)
        if row:
            props = dict(props)
            for key, value in row['properties'].items():
                live = clean(value, key)
                if live in (None, [], {}) and key not in props:
                    continue
                if live != props.get(key):
                    self.replaced.setdefault(path, []).append(key)
                props[key] = live
        self.overlaid[path] = props
        return props
