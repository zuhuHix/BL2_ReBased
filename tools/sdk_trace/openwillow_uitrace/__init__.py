"""OpenWillow UI trace: log what Borderlands 2's UI code does, for golden files.

A mod for the community mod SDK (unrealsdk / pyunrealsdk with mods_base,
installed separately by the player; recorded in THIRD_PARTY.md). It changes
nothing in game. While enabled it hooks every function declared on classes
that inherit GFxUI.GFxMoviePlayer or GFxUI.GFxObject, which covers Gearbox's
menu controllers (StatusMenuExGFxMovie, ...), the Scaleform bridge they call
(SetString, Invoke, GotoAndStop, ...) and the ext* callbacks movies call back.
Each call is written as one JSON line: sequence number, time, phase
("call"/"return"), function path, the object it ran on, and its arguments or
return value. After MAX_DETAILED calls to one function only a count is kept,
so per-frame HUD updates do not flood the file. Bridge calls that name a movie
member (Invoke, ActionScript*, GetObject, Set*) are counted per member, so a
busy one such as SetInfo cannot use up the budget of the rest.

This records observed behaviour (docs/LEGAL.md, clean-room rule 3). Traces
contain game data and belong under the repository's ignored local/ folder;
the destination is read from trace_dir.txt next to this file.
"""
import json
import time
from collections import Counter
from pathlib import Path

import unrealsdk
from mods_base import Game, build_mod
from unrealsdk import logging
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

__version__ = "0.2.1"
__version_info__ = (0, 2, 1)

HOOK_ID = "openwillow_uitrace"
BASES = ("GFxMoviePlayer", "GFxObject")
MAX_DETAILED = 200          # full records per function; later calls only counted
MAX_DEPTH = 3               # nesting kept when writing structs and arrays
MEMBER_ARGS = ("Member", "Method", "Path")  # bridge arguments naming a movie member

state = {"file": None, "seq": 0, "start": 0.0, "hooked": [], "calls": Counter(), "last_detailed": {}}


def trace_dir():
    configured = Path(__file__).with_name("trace_dir.txt")
    if configured.is_file():
        return Path(configured.read_text(encoding="utf-8").strip())
    return Path(__file__).with_name("traces")


def plain(value, depth=0):
    """A JSON-safe rendering of an unrealsdk value."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if depth >= MAX_DEPTH:
        return repr(value)[:200]
    if isinstance(value, UObject):
        return f"{value.Class.Name}'{value._path_name()}'"
    if isinstance(value, WrappedStruct):
        return {p.Name: plain(getattr(value, p.Name), depth + 1) for p in value._type._properties()}
    if isinstance(value, WrappedArray):
        return [plain(v, depth + 1) for v in list(value)[:64]]
    return repr(value)[:200]


def write(record):
    if state["file"] is None:
        return
    state["seq"] += 1
    record = {"seq": state["seq"], "t": round(time.perf_counter() - state["start"], 4), **record}
    state["file"].write(json.dumps(record, ensure_ascii=False) + "\n")


def budget_key(path, args):
    """The function, plus the movie member a bridge call names, if any."""
    names = {p.Name for p in args._type._properties()}
    for name in MEMBER_ARGS:
        if name in names:
            member = getattr(args, name)
            if isinstance(member, str) and member:
                return f"{path}#{member}"
    return path


def on_call(obj, args, _ret, func):
    try:
        path = func.func._path_name()
        key = budget_key(path, args)
        state["calls"][key] += 1
        # Returns have no arguments, so they follow their call's decision.
        state["last_detailed"][path] = state["calls"][key] <= MAX_DETAILED
        if state["last_detailed"][path]:
            write({"phase": "call", "func": path, "obj": plain(obj),
                   "args": {p.Name: plain(getattr(args, p.Name))
                            for p in args._type._properties() if p.Name != "ReturnValue"}})
    except Exception as error:  # a trace must never break the game
        write({"phase": "error", "error": repr(error)[:300]})


def on_return(obj, args, ret, func):
    try:
        path = func.func._path_name()
        if state["last_detailed"].get(path, True):
            record = {"phase": "return", "func": path, "obj": plain(obj), "ret": plain(ret)}
            # D is an out parameter: its pre-call value is often an empty
            # struct. Preserve the native getter's completed output, without
            # invoking another getter or changing any game property.
            if path == "GFxUI.GFxObject:GetDisplayInfo":
                record["out"] = {"D": plain(args.D)}
            write(record)
    except Exception as error:
        write({"phase": "error", "error": repr(error)[:300]})


def ui_functions():
    """Paths of every function declared on a GFxMoviePlayer/GFxObject class."""
    bases = [unrealsdk.find_class(name) for name in BASES]
    paths = set()
    for cls in unrealsdk.find_all("Class", exact=True):
        if not any(cls._inherits(base) for base in bases):
            continue
        for field in cls._fields():
            if field.Class.Name == "Function" and field.Outer == cls:
                paths.add(field._path_name())
    return sorted(paths)


def enable():
    folder = trace_dir()
    folder.mkdir(parents=True, exist_ok=True)
    name = time.strftime("uitrace_%Y%m%d_%H%M%S.jsonl")
    state.update(file=(folder / name).open("w", encoding="utf-8"), seq=0,
                 start=time.perf_counter(), calls=Counter(), last_detailed={})
    functions = ui_functions()
    for path in functions:
        add_hook(path, Type.PRE, HOOK_ID, on_call)
        add_hook(path, Type.POST, HOOK_ID, on_return)
    state["hooked"] = functions
    write({"phase": "start", "hooked": len(functions)})
    logging.info(f"[OpenWillow UI trace] {len(functions)} functions hooked, writing {folder / name}")


def disable():
    for path in state["hooked"]:
        remove_hook(path, Type.PRE, HOOK_ID)
        remove_hook(path, Type.POST, HOOK_ID)
    write({"phase": "stop", "counts": dict(state["calls"].most_common())})
    if state["file"] is not None:
        state["file"].close()
    state.update(file=None, hooked=[])


mod = build_mod(
    name="OpenWillow UI Trace",
    author="OpenWillow",
    description="Logs Borderlands 2 UI calls (menus, HUD, Scaleform bridge) for OpenWillow golden files. "
                "Changes nothing in game. Enable, use the menus, then disable to close the trace file.",
    version=__version__,
    supported_games=Game.BL2,
    auto_enable=False,
    inject_version_from_pyproject=False,
    on_enable=enable,
    on_disable=disable,
)
