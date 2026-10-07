"""OpenWillow real-game driver: a local command channel into the running game.

A Library mod for the community mod SDK (pyunrealsdk with mods_base, installed separately by the
player; recorded in THIRD_PARTY.md). It changes nothing by itself. Every few frames it looks for
`*.py` files in a command folder (absolute path in `cmd_dir.txt` next to this file, normally the
repository's ignored `local/realgame/cmd`), runs each one once on the game thread and writes what
it printed plus any traceback to `<name>.out`, then renames the script to `<name>.py.done`.

Scripts share one namespace across calls, so state (hooks, lists) persists. Helpers in scope:
`unrealsdk`, `find` (find_object), `find_all`, `get_pc`, `plain`, `mark(label, **data)` which
appends `{"t": time.perf_counter(), ...}` to `events.jsonl` in the command folder. On Windows
`time.perf_counter()` is QueryPerformanceCounter, the same clock as .NET Stopwatch.GetTimestamp,
so in-game marks can be lined up with screenshots taken by tools/real_game/realgame.ps1.

Scripts run inside a frame: keep them short (no sleeps). Local developer use only; whatever the
scripts write is game data and stays under local/.
"""
import contextlib
import io
import json
import time
import traceback
from pathlib import Path

import unrealsdk
from mods_base import Library, build_mod, get_pc
from unrealsdk.hooks import Block, Type, add_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

HOOK_ID = "openwillow_realgame_poll"
TICK = "WillowGame.WillowGameViewportClient:Tick"
state = {"tick": 0}


def cmd_dir():
    configured = Path(__file__).with_name("cmd_dir.txt")
    if configured.is_file():
        return Path(configured.read_text(encoding="utf-8").strip())
    return Path(__file__).with_name("cmd")


def plain(value, depth=0):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if depth >= 3:
        return repr(value)[:200]
    if isinstance(value, UObject):
        return f"{value.Class.Name}'{value._path_name()}'"
    if isinstance(value, WrappedStruct):
        return {p.Name: plain(getattr(value, p.Name), depth + 1) for p in value._type._properties()}
    if isinstance(value, WrappedArray):
        return [plain(v, depth + 1) for v in list(value)[:128]]
    return repr(value)[:200]


def mark(label, **data):
    folder = cmd_dir()
    with (folder / "events.jsonl").open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({"t": time.perf_counter(), "label": label, **data}) + "\n")


SAVE_FUNCTIONS = ("SaveGame", "Save", "SaveRawData", "SaveGraveyard")


def _block_save(_obj, _args, _ret, func):
    try:
        mark("save_blocked", func=func.func.Name)
    except Exception:
        pass
    return Block


def block_saves():
    """Refuse calls to WillowSaveGameManager's save functions for the rest of this game session.

    Call it at the main menu before loading a character. On 2026-10-02 it held for character loads,
    a map travel, disconnects and quitting; game start and character selection still write
    profile.bin and a .bak copy. Back the saves up first and remove anything spawned before quitting.
    """
    for name in SAVE_FUNCTIONS:
        add_hook("WillowGame.WillowSaveGameManager:" + name, Type.PRE, "openwillow_block_save", _block_save)
    return len(SAVE_FUNCTIONS)


# Command scripts must look objects up again in every command: a reference kept from an earlier
# command can point at an object the game has since moved or destroyed (an inventory slot swap
# through such a reference crashed the game on 2026-10-02).
scope = {"unrealsdk": unrealsdk, "find": unrealsdk.find_object, "find_all": unrealsdk.find_all,
         "get_pc": get_pc, "plain": plain, "mark": mark, "Type": Type, "add_hook": add_hook,
         "block_saves": block_saves}


def run_pending():
    folder = cmd_dir()
    if not folder.is_dir():
        return
    for script in sorted(folder.glob("*.py")):
        code = script.read_text(encoding="utf-8")
        script.replace(script.with_suffix(".py.done"))
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            try:
                exec(compile(code, str(script), "exec"), scope)
            except Exception:
                print(traceback.format_exc()[-3000:])
        script.with_suffix(".out").write_text(out.getvalue(), encoding="utf-8")


def on_tick(*_args):
    state["tick"] += 1
    if state["tick"] % 3 == 0:
        try:
            run_pending()
        except Exception:
            pass  # the channel must never break the game


add_hook(TICK, Type.POST_UNCONDITIONAL, HOOK_ID, on_tick)

# Install-Driver -BlockSavesAtStart writes this flag: the game then refuses save calls from the first frame, which is
# needed when a startup mod loads a character before any command can run (the main-menu call is too late).
if Path(__file__).with_name("autoblock.flag").is_file():
    block_saves()

mod = build_mod(cls=Library, name="OpenWillow Real Game Driver", author="OpenWillow",
                description="Local command channel for OpenWillow ground-truth captures.",
                version="0.1.0", inject_version_from_pyproject=False)
