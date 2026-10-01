"""OpenWillow game trace: log gameplay-logic calls and object state, for golden files.

A mod for the community mod SDK (unrealsdk / pyunrealsdk with mods_base, installed
separately by the player; recorded in THIRD_PARTY.md). It is the gameplay-logic sibling of
openwillow_uitrace and, like it, changes nothing in game by itself: while enabled it hooks
every function declared on the classes selected by `gametrace.json` (regular expressions over
class names, next to this file) and writes each call and return as one JSON line.

Native engine/game logic (the mission tracker, population, damage...) cannot be read from
bytecode, so the only way to know what it does is to watch it run. This mod records those
observations (docs/LEGAL.md, clean-room rule 3). Traces contain game data and belong under
the repository's ignored local/ folder; the destination is read from trace_dir.txt.

Developer probe channel (local machine only): when `probe/cmd.py` exists in the trace
directory the mod executes it once, from the game thread, with helpers in scope (see
`run_probe`), then renames it to `cmd.py.done`. This lets a capture script ask the real
engine to run its own logic (e.g. accept a mission) and snapshot the result without driving
menus. It is only active when the mod is enabled by the player.
"""
import json
import re
import time
import traceback
from collections import Counter
from pathlib import Path

import unrealsdk
from mods_base import Game, build_mod
from unrealsdk import logging
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

__version__ = "0.1.0"
__version_info__ = (0, 1, 0)

HOOK_ID = "openwillow_gametrace"
POLL_HOOK = "WillowGame.WillowPlayerController:PlayerTick"
MAX_DEPTH = 4

state = {"file": None, "seq": 0, "start": 0.0, "hooked": [], "calls": Counter(),
         "last_detailed": {}, "max_detailed": 400, "tick": 0}


def trace_dir():
    configured = Path(__file__).with_name("trace_dir.txt")
    if configured.is_file():
        return Path(configured.read_text(encoding="utf-8").strip())
    return Path(__file__).with_name("traces")


def config():
    path = Path(__file__).with_name("gametrace.json")
    if path.is_file():
        return json.loads(path.read_text(encoding="utf-8"))
    return {"include": [r"^MissionTracker$"], "exclude": [], "max_detailed": 400}


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
        return [plain(v, depth + 1) for v in list(value)[:128]]
    return repr(value)[:200]


def write(record):
    if state["file"] is None:
        return
    state["seq"] += 1
    record = {"seq": state["seq"], "t": round(time.perf_counter() - state["start"], 4), **record}
    state["file"].write(json.dumps(record, ensure_ascii=False) + "\n")
    state["file"].flush()


def on_call(obj, args, _ret, func):
    try:
        path = func.func._path_name()
        state["calls"][path] += 1
        state["last_detailed"][path] = state["calls"][path] <= state["max_detailed"]
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
            # Out parameters are only complete after the call, so they are re-read here.
            outs = {p.Name: plain(getattr(args, p.Name))
                    for p in args._type._properties()
                    if p.Name != "ReturnValue" and (int(getattr(p, "PropertyFlags", 0)) & 0x100)}
            record = {"phase": "return", "func": path, "obj": plain(obj), "ret": plain(ret)}
            if outs:
                record["out"] = outs
            write(record)
    except Exception as error:
        write({"phase": "error", "error": repr(error)[:300]})


def selected_functions(cfg):
    include = [re.compile(p) for p in cfg.get("include", [])]
    exclude = [re.compile(p) for p in cfg.get("exclude", [])]
    paths = set()
    for cls in unrealsdk.find_all("Class", exact=True):
        name = cls.Name
        if not any(p.search(name) for p in include) or any(p.search(name) for p in exclude):
            continue
        for field in cls._fields():
            if field.Class.Name == "Function" and field.Outer == cls:
                paths.add(field._path_name())
    return sorted(paths)


def snapshot(obj, names=None, depth=0):
    """Selected (or all) properties of an object, rendered with plain()."""
    result = {}
    for field in obj.Class._fields():
        if field.Class.Name.endswith("Property") and (names is None or field.Name in names):
            try:
                result[field.Name] = plain(getattr(obj, field.Name), depth)
            except Exception as error:
                result[field.Name] = f"<unreadable {error!r}>"[:120]
    return result


def run_probe():
    """Run probe/cmd.py once. Helpers: unrealsdk, plain, snapshot, rec(label, value), find(cls, path)."""
    folder = trace_dir() / "probe"
    cmd = folder / "cmd.py"
    if not cmd.is_file():
        return
    done = folder / "cmd.py.done"
    code = cmd.read_text(encoding="utf-8")
    cmd.replace(done)

    def rec(label, value):
        write({"phase": "probe", "label": label, "value": plain(value)})

    scope = {"unrealsdk": unrealsdk, "plain": plain, "snapshot": snapshot, "rec": rec,
             "find": unrealsdk.find_object, "all": unrealsdk.find_all}
    write({"phase": "probe_begin", "bytes": len(code)})
    try:
        exec(compile(code, str(cmd), "exec"), scope)
        write({"phase": "probe_end", "ok": True})
    except Exception:
        write({"phase": "probe_end", "ok": False, "error": traceback.format_exc()[-1500:]})


def on_tick(_obj, _args, _ret, _func):
    state["tick"] += 1
    if state["tick"] % 20 == 0 and state["file"] is not None:
        try:
            run_probe()
        except Exception as error:
            write({"phase": "error", "error": repr(error)[:300]})


def enable():
    folder = trace_dir()
    (folder / "probe").mkdir(parents=True, exist_ok=True)
    cfg = config()
    name = time.strftime("gametrace_%Y%m%d_%H%M%S.jsonl")
    state.update(file=(folder / name).open("w", encoding="utf-8"), seq=0, start=time.perf_counter(),
                 calls=Counter(), last_detailed={}, max_detailed=cfg.get("max_detailed", 400), tick=0)
    functions = selected_functions(cfg)
    for path in functions:
        add_hook(path, Type.PRE, HOOK_ID, on_call)
        add_hook(path, Type.POST, HOOK_ID, on_return)
    add_hook(POLL_HOOK, Type.POST, HOOK_ID + "_poll", on_tick)
    state["hooked"] = functions
    write({"phase": "start", "hooked": len(functions), "config": cfg})
    logging.info(f"[OpenWillow game trace] {len(functions)} functions hooked, writing {folder / name}")


def disable():
    for path in state["hooked"]:
        remove_hook(path, Type.PRE, HOOK_ID)
        remove_hook(path, Type.POST, HOOK_ID)
    remove_hook(POLL_HOOK, Type.POST, HOOK_ID + "_poll")
    write({"phase": "stop", "counts": dict(state["calls"].most_common())})
    if state["file"] is not None:
        state["file"].close()
    state.update(file=None, hooked=[])


mod = build_mod(
    name="OpenWillow Game Trace",
    author="OpenWillow",
    description="Logs Borderlands 2 gameplay-logic calls (missions, behaviors, Kismet) for OpenWillow golden "
                "files. Changes nothing by itself. Enable, play, then disable to close the trace file.",
    version=__version__,
    supported_games=Game.BL2,
    auto_enable=Path(__file__).with_name("autostart.txt").is_file(),
    inject_version_from_pyproject=False,
    on_enable=enable,
    on_disable=disable,
)
