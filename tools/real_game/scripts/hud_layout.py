"""HUD layout ground truth: one-shot dump of the HUD movie's placement for the openwillow_realgame command channel.

Our own code; what it writes is game data and goes under RG_OUT (normally the ignored local/realgame/hud_layout).
Run through Invoke-GamePyFile, then call hl_dump(label, clips) with the clip paths to read (e.g. "p1.health").

The movie is the player's WillowHUDGFxMovie (PlayerController.myHUD.HUDMovie). Written to <label>.json:
- movie: every property of the movie object (plain() values) and the result of a few viewport getters;
- stage: Stage.width/height/scaleMode/align and the Scaleform Stage rect extensions, read as variables;
- clips: per path _x, _y, _xscale, _yscale, _width, _height, _rotation, _currentframe, _visible, _alpha, _target,
  plus GFxObject reads (GetDisplayInfo, GetPosition, GetDisplayMatrix) where they answer, and "global": the clip's
  origin and scale in stage units, composed from its parents' _x/_y/_xscale/_yscale/_rotation (AS2 transform order).
"""
import json
import math
from pathlib import Path

HL_PROPS_NUM = ("_x", "_y", "_xscale", "_yscale", "_width", "_height", "_rotation", "_currentframe", "_totalframes",
                "_alpha", "_z", "_zscale", "_xrotation", "_yrotation", "_perspfov")
HL_STAGE_NUM = ("Stage.width", "Stage.height", "Stage.visibleRect.x", "Stage.visibleRect.y", "Stage.visibleRect.width",
                "Stage.visibleRect.height", "Stage.originalRect.x", "Stage.originalRect.y", "Stage.originalRect.width",
                "Stage.originalRect.height", "Stage.safeRect.x", "Stage.safeRect.y", "Stage.safeRect.width",
                "Stage.safeRect.height", "_global.gfxExtensions")
HL_STAGE_STR = ("Stage.scaleMode", "Stage.align", "Stage.displayState", "_root._target", "_level0._target")
HL_MOVIE_CALLS = ("GetVisibleFrameRect", "GetGameViewportClient", "GetPC", "GetLP", "GetViewportSize")


def _hl_try(fn):
    try:
        return fn()
    except Exception as error:
        return f"<{type(error).__name__}: {str(error)[:80]}>"


def _hl_movie():
    hud = get_pc().myHUD
    return getattr(hud, "HUDMovie", None) if hud is not None else None


def _hl_clip(movie, path):
    row = {p: _hl_try(lambda: movie.GetVariableNumber(f"{path}.{p}")) for p in HL_PROPS_NUM}
    row["_visible"] = _hl_try(lambda: movie.GetVariableBool(f"{path}._visible"))
    row["_target"] = _hl_try(lambda: movie.GetVariableString(f"{path}._target"))
    obj = _hl_try(lambda: movie.GetVariableObject(path))
    if hasattr(obj, "Class"):
        row["GetDisplayInfo"] = _hl_try(lambda: plain(obj.GetDisplayInfo()))
        row["GetPosition"] = _hl_try(lambda: plain(obj.GetPosition()))
        row["GetDisplayMatrix"] = _hl_try(lambda: plain(obj.GetDisplayMatrix()))
    else:
        row["GetVariableObject"] = str(obj)
    return row


def _hl_global(clips, path):
    """Origin and scale of `path` in its root's units: apply each ancestor's translate * rotate * scale in turn."""
    parts = path.split(".")
    a, b, c, d, tx, ty = 1.0, 0.0, 0.0, 1.0, 0.0, 0.0   # x' = a x + c y + tx, y' = b x + d y + ty
    for i in range(1, len(parts) + 1):
        row = clips.get(".".join(parts[:i]))
        if row is None:
            return None
        try:
            sx, sy = row["_xscale"] / 100.0, row["_yscale"] / 100.0
            r = math.radians(row["_rotation"])
            la, lb, lc, ld = math.cos(r) * sx, math.sin(r) * sx, -math.sin(r) * sy, math.cos(r) * sy
            ltx, lty = row["_x"], row["_y"]
        except (TypeError, KeyError):
            return None
        a, b, c, d, tx, ty = (a * la + c * lb, b * la + d * lb, a * lc + c * ld, b * lc + d * ld,
                              a * ltx + c * lty + tx, b * ltx + d * lty + ty)
    return {"x": tx, "y": ty, "xscale": math.hypot(a, b) * 100.0, "yscale": math.hypot(c, d) * 100.0}


def hl_dump(label, clips):
    movie = _hl_movie()
    if movie is None:
        return {"error": "no HUD movie"}
    out = {"movie": movie._path_name(), "class": movie.Class.Name, "props": {}, "calls": {}}
    for field in movie.Class._fields():
        if field.Class.Name.endswith("Property"):
            out["props"][field.Name] = {"type": field.Class.Name,
                                        "value": _hl_try(lambda: plain(getattr(movie, field.Name)))}
    for name in HL_MOVIE_CALLS:
        out["calls"][name] = _hl_try(lambda: plain(getattr(movie, name)()))
    definition = getattr(movie, "MyDefinition", None)
    if definition is not None:
        out["definition"] = {"name": definition._path_name(), "props": {
            f.Name: _hl_try(lambda: plain(getattr(definition, f.Name))) for f in definition.Class._fields()
            if f.Class.Name.endswith("Property")}}
    out["stage"] = {k: _hl_try(lambda: movie.GetVariableNumber(k)) for k in HL_STAGE_NUM}
    out["stage"].update({k: _hl_try(lambda: movie.GetVariableString(k)) for k in HL_STAGE_STR})
    rows = {}
    for path in ["_root", "_level0", "p1"] + [c for c in clips if c not in ("_root", "_level0", "p1")]:
        rows[path] = _hl_clip(movie, path)
    out["clips"] = rows
    out["global"] = {path: _hl_global(rows, path) for path in rows if path.startswith("p1")}
    vp = _hl_try(lambda: get_pc().Player.ViewportClient)
    out["viewport"] = _hl_try(lambda: {"class": vp.Class.Name, "size": plain(vp.GetViewportSize())})
    Path(RG_OUT).mkdir(parents=True, exist_ok=True)
    (Path(RG_OUT) / f"{label}.json").write_text(json.dumps(out, indent=1, default=str), encoding="utf-8")
    return {"movie": out["movie"], "clips": len(rows), "stage": out["stage"].get("Stage.width")}
