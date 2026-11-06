"""Texture address modes for the weapon library import: no unreal import, so the rule can be tested without the editor.

Textures are shared by name between skins and between roles, and one import run asks for the same texture many times, some with
the installed modes (a pattern, a decal, a reflection) and some without (a mask). So a mode given anywhere in the run wins, in any
order, and a texture nobody gave a mode for is set back to wrap: the installed default, and it undoes an earlier run's clamp.
"""
WRAP = ('TA_Wrap', 'TA_Wrap')


def apply_address(asset, address, enum):
    """Set AddressX and AddressY on asset; no modes means wrap. Returns whether anything changed."""
    changed = False
    for prop, mode in zip(('address_x', 'address_y'), address or WRAP):
        wanted = getattr(enum, mode.upper())
        if asset.get_editor_property(prop) != wanted:
            asset.set_editor_property(prop, wanted)
            changed = True
    return changed


class TextureCache:
    """The import script's texture requests: one load per source, and the modes applied on every request, cache hits included."""

    def __init__(self, load, enum, save):
        self.load, self.save = load, save
        self.addresses = RunAddresses(enum)
        self.assets = {}

    def get(self, source, address=None):
        asset = self.assets.get(str(source))
        if asset is None:
            asset = self.load(source)
            self.assets[str(source)] = asset
        if self.addresses.apply(asset.get_path_name(), asset, address):
            self.save(asset)
        return asset


class RunAddresses:
    """Applies modes for one import run, keyed by the texture asset's path."""

    def __init__(self, enum):
        self.enum = enum
        self.given = set()

    def apply(self, key, asset, address):
        if address:
            self.given.add(key)
        elif key in self.given:
            return False  # a role without modes must not undo what another role gave in this run
        return apply_address(asset, address, self.enum)
