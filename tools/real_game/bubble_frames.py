"""Pick burst frames at chosen times after a Phaselock cast and measure the bubble rim.

Usage: python tools/real_game/bubble_frames.py <run folder> [--times 0.25 0.5 ...] [--centre X Y] [--rim-row Y]

The run folder holds `burst/frames.json` + `burst/f*.jpg` (tools/real_game/realgame.ps1 Invoke-Burst) and
`samples.jsonl` (tools/real_game/scripts/phaselock.py pl_start). The cast time is the skill's SkillStartTime
placed on the frames' clock: the first sample whose skill state is not 0 gives both clocks. Writes
`frames_at/t<time>.jpg` and `frames_at/index.json` beside the burst, and prints a rim-to-rim width per
frame. The rim is the brightest blue-minus-red ridge on each side of the centre along a short band of rows:
a rough measure on JPEG frames (a few pixels), meant for comparing sizes, not for pixel-exact work.
Pillow and numpy are required; frames and their numbers are game data and stay under local/.
"""
import argparse
import json
import shutil
from pathlib import Path

import numpy as np
from PIL import Image


def cast_time(run):
    rows = [json.loads(line) for line in (run / "samples.jsonl").open(encoding="utf-8") if line.strip()]
    first = next(r for r in rows if r.get("state") not in (None, "0"))
    # perf time of SkillStartTime = this sample's perf time minus the game time elapsed since the skill started
    return first["t"] - (first["game_t"] - first["skill_start"])


def rim_width(path, centre, band=6):
    im = np.asarray(Image.open(path).convert("RGB")).astype(float)
    score = (im[..., 2] - im[..., 0]) * (im[..., 2] > 170)
    cx, cy = centre
    row = score[cy - band:cy + band, :].mean(axis=0)
    left, right = int(np.argmax(row[:cx])), int(cx + np.argmax(row[cx:]))
    return left, right, right - left


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run")
    parser.add_argument("--times", type=float, nargs="+", default=[0.25, 0.5, 0.8, 1.5, 3.0, 4.5, 4.8, 5.0])
    parser.add_argument("--centre", type=int, nargs=2, default=[640, 300])
    args = parser.parse_args()
    run = Path(args.run)
    frames = json.loads((run / "burst" / "frames.json").read_text(encoding="utf-8"))
    cast = cast_time(run)
    out = run / "frames_at"
    out.mkdir(exist_ok=True)
    index = []
    for want in args.times:
        frame = min(frames, key=lambda f: abs((f["t"] - cast) - want))
        name = f"t{want:.2f}.jpg"
        shutil.copy(run / "burst" / frame["file"], out / name)
        entry = {"want": want, "file": frame["file"], "actual": round(frame["t"] - cast, 3), "saved_as": name}
        if want in (1.5, 3.0, 4.5):
            entry["rim_x_left_right_width"] = rim_width(out / name, tuple(args.centre))
        index.append(entry)
        print(entry)
    (out / "index.json").write_text(json.dumps({"cast_perf_t": cast, "frames": index}, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
