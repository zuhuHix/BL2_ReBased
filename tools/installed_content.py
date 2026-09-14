"""Discover installed base-game and optional DLC content without parsing it."""
PACKAGE_SUFFIXES = {'.upk', '.umap', '.u'}


def content_files(game, include_dlc=False):
    roots = [game / 'WillowGame/CookedPCConsole']
    if include_dlc:
        roots.append(game / 'DLC')
    return sorted(p for root in roots for p in root.rglob('*')
                  if p.is_file() and p.suffix.lower() in PACKAGE_SUFFIXES | {'.tfc'})


def cache_directory(paths, package, fallback):
    """Use a package-local cache when duplicated; otherwise require uniqueness."""
    local = [p for p in paths if p.parent == package.parent]
    if len(local) == 1:
        return local[0].parent
    if len(paths) == 1:
        return paths[0].parent
    if not paths:
        # Inline mips do not need a cache. Leave missing-stream validation to
        # the reader instead of rejecting every texture with a stale name.
        return fallback
    raise ValueError('Ambiguous texture cache: ' + ', '.join(str(p) for p in paths))
