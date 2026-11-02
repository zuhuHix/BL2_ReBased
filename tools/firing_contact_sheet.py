"""Contact sheets: real-game firing frames (row 1) over host firing frames (row 2), aligned by frame offset from the shot.

Inputs (both under the ignored local/ tree; game-derived images are never written to the repository):
  real: local/realgame/firing/r3/frames/<gun>_NN.png and <gun>_index.json (the 'click' or 'down' event marks the shot)
  host: local/firing/frames/host/<gun>_NN.png from tools/capture_firing_frames.ps1 (the shot is frame 04 by construction)

Outputs: local/firing/frames/sheets/<gun>.png (one per gun) and local/firing/frames/sheets/overview.png.
Offsets are frame numbers relative to the shot frame; the default -2..10 covers two frames before the trigger to the
projectile and impact frames. Missing frames are drawn as labelled grey cells, so a short capture shows up on the sheet.

Usage: python tools/firing_contact_sheet.py [--offsets=-2:10] [--thumb 256]
(the = form is needed for negative LOW values; argparse reads a bare -2:10 as an option)
"""
import argparse
import glob
import json
import os
import re

from PIL import Image, ImageDraw, ImageFont

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REAL_DIR = os.path.join(REPO, 'local', 'realgame', 'firing', 'r3', 'frames')
HOST_DIR = os.path.join(REPO, 'local', 'firing', 'frames', 'host')
OUT_DIR = os.path.join(REPO, 'local', 'firing', 'frames', 'sheets')
HOST_SHOT_FRAME = 4          # tools/capture_firing_frames.ps1: the trigger is pulled with frame 04 (RunFiringShots)
LABEL_W = 80
HEAD_H = 36
PAD = 6
BG = (24, 24, 28)
FG = (235, 235, 235)
MISSING = (70, 70, 74)


def gun_names():
    names = []
    for path in sorted(glob.glob(os.path.join(REAL_DIR, '*_pose.json'))):
        names.append(os.path.basename(path)[:-len('_pose.json')])
    return names


def real_shot_frame(gun):
    """The frame whose timestamp is nearest the index's shot event (click or down)."""
    path = os.path.join(REAL_DIR, gun + '_index.json')
    if not os.path.exists(path):
        return None
    with open(path, encoding='utf-8') as handle:
        index = json.load(handle)
    events = [e for e in index.get('events', []) if e.get('label') in ('click', 'down')]
    if not events:
        return None
    shot_t = events[0]['t']
    frames = index.get('frames', [])
    if not frames:
        return None
    return min(frames, key=lambda f: abs(f['t'] - shot_t))['n']


def frame_path(folder, gun, number):
    return os.path.join(folder, f'{gun}_{number:02d}.png')


def load_thumb(path, size):
    if not os.path.exists(path):
        cell = Image.new('RGB', size, MISSING)
        ImageDraw.Draw(cell).text((6, size[1] // 2 - 6), 'missing', fill=FG)
        return cell
    with Image.open(path) as image:
        return image.convert('RGB').resize(size, Image.LANCZOS)


def offset_label(offset):
    if offset == 0:
        return 'shot'
    return f'{offset:+d}'


def build_sheet(gun, offsets, thumb_w, font, title, real_shot, host_shot, rows):
    """rows: list of (row_label, folder, shot_frame). Returns a PIL image."""
    thumb_h = int(thumb_w * 9 / 16)
    cols = len(offsets)
    width = LABEL_W + cols * (thumb_w + PAD) + PAD
    height = HEAD_H + len(rows) * (thumb_h + PAD) + PAD + 22
    sheet = Image.new('RGB', (width, height), BG)
    draw = ImageDraw.Draw(sheet)
    draw.text((PAD, 8), title, fill=FG, font=font)
    for col, offset in enumerate(offsets):
        x = LABEL_W + PAD + col * (thumb_w + PAD)
        draw.text((x + 4, HEAD_H - 16), offset_label(offset), fill=FG, font=font)
    for row, (label, folder, shot) in enumerate(rows):
        y = HEAD_H + PAD + row * (thumb_h + PAD)
        draw.text((PAD, y + thumb_h // 2 - 6), label, fill=FG, font=font)
        for col, offset in enumerate(offsets):
            x = LABEL_W + PAD + col * (thumb_w + PAD)
            sheet.paste(load_thumb(frame_path(folder, gun, shot + offset), (thumb_w, thumb_h)), (x, y))
    note = (f'real shot = frame {real_shot:02d}, host shot = frame {host_shot:02d}; offsets are frames from the shot; '
            'host frames ~46 ms apart (real timing from index.json)')
    draw.text((PAD, height - 18), note, fill=FG, font=font)
    return sheet


def parse_offsets(text):
    low, high = (int(part) for part in text.split(':'))
    return list(range(low, high + 1))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--offsets', default='-2:10', help='frame offsets from the shot as LOW:HIGH (default -2:10)')
    parser.add_argument('--thumb', type=int, default=256, help='thumbnail width in pixels (default 256)')
    parser.add_argument('--overview-offsets', default='-2:2,4,6,10',
                        help='offsets for the overview sheet, LOW:HIGH or a comma list (default -2:2,4,6,10)')
    args = parser.parse_args()
    offsets = parse_offsets(args.offsets)
    overview_offsets = []
    for part in args.overview_offsets.split(','):
        overview_offsets += parse_offsets(part) if ':' in part else [int(part)]
    try:
        font = ImageFont.truetype('arial.ttf', 14)
    except OSError:
        font = ImageFont.load_default()
    os.makedirs(OUT_DIR, exist_ok=True)
    guns = gun_names()
    if not guns:
        raise SystemExit(f'No real firing references under {REAL_DIR}')
    summary = []
    for gun in guns:
        real_shot = real_shot_frame(gun) or HOST_SHOT_FRAME
        host_present = len(glob.glob(os.path.join(HOST_DIR, f'{gun}_??.png')))
        real_present = len(glob.glob(os.path.join(REAL_DIR, f'{gun}_??.png')))
        rows = [('real', REAL_DIR, real_shot), ('host', HOST_DIR, HOST_SHOT_FRAME)]
        title = f'{gun}: real {real_present} frames (shot {real_shot:02d}), host {host_present} frames (shot {HOST_SHOT_FRAME:02d})'
        sheet = build_sheet(gun, offsets, args.thumb, font, title, real_shot, HOST_SHOT_FRAME, rows)
        path = os.path.join(OUT_DIR, gun + '.png')
        sheet.save(path)
        summary.append((gun, real_present, host_present, path))
    # Overview: one real/host row pair per gun, with the shot and the few offsets that show it.
    thumb_w = 160
    thumb_h = int(thumb_w * 9 / 16)
    cols = len(overview_offsets)
    width = LABEL_W + cols * (thumb_w + PAD) + PAD
    height = HEAD_H + len(guns) * 2 * (thumb_h + PAD) + PAD + 22
    overview = Image.new('RGB', (width, height), BG)
    draw = ImageDraw.Draw(overview)
    draw.text((PAD, 8), 'Firing frames: real (top of each pair) vs host, aligned on the shot frame', fill=FG, font=font)
    for col, offset in enumerate(overview_offsets):
        draw.text((LABEL_W + PAD + col * (thumb_w + PAD) + 4, HEAD_H - 16), offset_label(offset), fill=FG, font=font)
    for index, gun in enumerate(guns):
        real_shot = real_shot_frame(gun) or HOST_SHOT_FRAME
        for side, (label, folder, shot) in enumerate((('real', REAL_DIR, real_shot), ('host', HOST_DIR, HOST_SHOT_FRAME))):
            row = index * 2 + side
            y = HEAD_H + PAD + row * (thumb_h + PAD)
            draw.text((PAD, y + thumb_h // 2 - 6), f'{gun} {label}' if side == 0 else label, fill=FG, font=font)
            for col, offset in enumerate(overview_offsets):
                x = LABEL_W + PAD + col * (thumb_w + PAD)
                overview.paste(load_thumb(frame_path(folder, gun, shot + offset), (thumb_w, thumb_h)), (x, y))
    overview_path = os.path.join(OUT_DIR, 'overview.png')
    overview.save(overview_path)
    for gun, real_present, host_present, path in summary:
        print(f'{gun:22s} real {real_present:2d}  host {host_present:2d}  -> {os.path.relpath(path, REPO)}')
    print(f'overview -> {os.path.relpath(overview_path, REPO)}')


if __name__ == '__main__':
    main()
