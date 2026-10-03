"""OpenWillow value watch: log when selected object properties change, from the moment the SDK loads.

A Library mod for the community mod SDK (THIRD_PARTY.md). It reads, on every rendered frame once a
second has passed, a few live properties and writes a JSON line whenever one differs from the last
reading (and one line at the first successful read), with time.perf_counter(). Used on 2026-10-02 to
find out whether the live weapon-type values differ from the cooked packages from the start or change
after the game goes online. Local developer use only; the output is game data and stays under local/.
The output folder is read from out_dir.txt next to this file.
"""
import json
import time
from pathlib import Path

import unrealsdk
from mods_base import Library, build_mod
from unrealsdk.hooks import Type, add_hook

WATCH = (
    ("WeaponTypeDefinition", "GD_Weap_Pistol.A_Weapons.WeaponType_Bandit_Pistol", "ClipSize"),
    ("WeaponTypeDefinition", "GD_Weap_Pistol.A_Weapons.WeaponType_Dahl_Pistol", "ClipSize"),
    ("WeaponTypeDefinition", "GD_Weap_Shotgun.A_Weapons.WT_Bandit_Shotgun", "ClipSize"),
    ("WeaponTypeDefinition", "GD_Weap_Shotgun.A_Weapons.WT_Bandit_Shotgun", "ReloadTime"),
)
state = {"last": {}, "next": 0.0, "start": time.perf_counter(), "micropatch": None}


def out_file():
    return Path(Path(__file__).with_name("out_dir.txt").read_text(encoding="utf-8").strip()) / "valuewatch.jsonl"


def log(**row):
    with out_file().open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({"t": time.perf_counter(), "since_load": round(time.perf_counter() - state["start"], 2), **row}) + "\n")


def micropatch_count():
    for config in unrealsdk.find_all("SparkServiceConfiguration"):
        if config.ServiceName == "Micropatch":
            return len(list(config.Keys))
    return None


def on_tick(*_args):
    now = time.perf_counter()
    if now < state["next"]:
        return
    state["next"] = now + 1.0
    try:
        for cls, path, prop in WATCH:
            key = (path, prop)
            try:
                value = getattr(unrealsdk.find_object(cls, path), prop)
            except Exception:
                value = "unavailable"
            if state["last"].get(key) != value:
                log(event="value", path=path, prop=prop, value=value, was=state["last"].get(key))
                state["last"][key] = value
        count = micropatch_count()
        if count != state["micropatch"]:
            log(event="micropatch_entries", count=count, was=state["micropatch"])
            state["micropatch"] = count
    except Exception as error:
        log(event="error", error=repr(error)[:200])


log(event="loaded")
add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "openwillow_valuewatch", on_tick)
mod = build_mod(cls=Library, name="OpenWillow Value Watch", author="OpenWillow",
                description="Logs live value changes for OpenWillow ground-truth checks.", version="0.1.0",
                inject_version_from_pyproject=False)
