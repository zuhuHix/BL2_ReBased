"""Numbers for a real-game gun frame against a host frame of the same gun (lane C review aid; AI-assisted).

Both images are side views of the same gun mesh (the real game's Inspect view and the host's SaveFrame/preview capture),
so after cutting out the gun and scaling both to the same box, cell (i, j) of one is the same part of the gun as cell (i, j)
of the other. For each cell it prints the median sRGB colour of both and the linear-light ratio, and a summary
(median luminance ratio, median absolute hue difference over saturated cells, mean colour difference in CIE-Lab-like
units). Segmentation is by distance from a heavily blurred copy of the image (the Inspect background is a smooth
gradient; the gun has ink outlines) plus an optional crop box to exclude the item card.

It measures; it does not judge. Frames are game-derived and stay under ignored local/. Needs numpy and Pillow.

  python tools/weapon_visual_compare.py --real real.png --host host.png [--real-crop x0 y0 x1 y1] [--host-crop ...]
         [--grid 12 5] [--out comparison.png]
"""
import argparse
import colorsys
import sys

import numpy as np
from PIL import Image, ImageFilter


def srgb_to_linear(v):
    v = np.asarray(v, dtype=np.float64) / 255.0
    return np.where(v <= 0.04045, v / 12.92, ((v + 0.055) / 1.055) ** 2.4)


def gun_box(image, crop, threshold):
    """Bounding box (x0, y0, x1, y1) of the gun inside `crop`, found against a blurred copy of the frame."""
    rgb = image.convert('RGB')
    blurred = rgb.filter(ImageFilter.GaussianBlur(radius=max(rgb.size) / 14))
    diff = np.abs(np.asarray(rgb, dtype=np.int32) - np.asarray(blurred, dtype=np.int32)).sum(axis=2)
    mask = diff > threshold
    x0, y0, x1, y1 = crop
    keep = np.zeros_like(mask)
    keep[y0:y1, x0:x1] = True
    mask &= keep
    # Ignore sparse specks: a row/column counts when more than 0.5 % of its crop span is set.
    cols = np.where(mask.sum(axis=0) > 0.005 * (y1 - y0))[0]
    rows = np.where(mask.sum(axis=1) > 0.005 * (x1 - x0))[0]
    if len(cols) == 0 or len(rows) == 0:
        raise SystemExit('no gun found; adjust --threshold or the crop')
    return int(cols.min()), int(rows.min()), int(cols.max()) + 1, int(rows.max()) + 1


def host_mask(image, background, tolerance=10):
    """True where the host frame differs from its known clear colour (the gun)."""
    arr = np.asarray(image.convert('RGB'), dtype=np.int32)
    return np.abs(arr - np.asarray(background, dtype=np.int32)).sum(axis=2) > tolerance


def cells(image, box, grid, mask=None):
    """Median colour per grid cell of the gun box; `mask` (cells x 16 x 16 booleans) limits it to gun pixels."""
    gx, gy = grid
    crop = image.convert('RGB').crop(box).resize((gx * 16, gy * 16), Image.LANCZOS)
    arr = np.asarray(crop, dtype=np.float64)
    out = np.full((gy, gx, 3), np.nan)
    for j in range(gy):
        for i in range(gx):
            block = arr[j * 16:(j + 1) * 16, i * 16:(i + 1) * 16].reshape(-1, 3)
            if mask is not None:
                inside = mask[j * 16:(j + 1) * 16, i * 16:(i + 1) * 16].reshape(-1)
                if inside.mean() < 0.6:
                    continue  # mostly background in the host frame
                block = block[inside]
            lum = block.mean(axis=1)
            keep = block[(lum > np.percentile(lum, 20)) & (lum < np.percentile(lum, 95))]  # drop ink outline and glints
            out[j, i] = np.median(keep if len(keep) else block, axis=0)
    return out


def hue(rgb):
    r, g, b = (float(c) / 255.0 for c in rgb)
    h, s, v = colorsys.rgb_to_hsv(r, g, b)
    return h * 360.0, s, v


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--real', required=True)
    parser.add_argument('--host', required=True)
    parser.add_argument('--real-box', type=int, nargs=4, help='gun box in the real frame, overriding the detection')
    parser.add_argument('--real-crop', type=int, nargs=4, default=None)
    parser.add_argument('--host-crop', type=int, nargs=4, default=None)
    parser.add_argument('--grid', type=int, nargs=2, default=[12, 5])
    parser.add_argument('--threshold', type=int, default=60)
    parser.add_argument('--real-left-from-aspect', action='store_true',
                        help='real left edge = right edge - host aspect x real height (card covers the barrel)')
    parser.add_argument('--host-bg', type=int, nargs=3, help='host clear colour: the gun is where the host frame differs from it')
    parser.add_argument('--out')
    args = parser.parse_args()

    real, host = Image.open(args.real), Image.open(args.host)
    real_crop = args.real_crop or [360, 0, real.width, 620]
    host_crop = args.host_crop or [0, 0, host.width, host.height]
    rb = tuple(args.real_box) if args.real_box else gun_box(real, real_crop, args.threshold)
    hmask = host_mask(host, args.host_bg) if args.host_bg else None
    if hmask is not None:
        ys, xs = np.where(hmask)
        hb = (int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1)
    else:
        hb = gun_box(host, host_crop, args.threshold)
    if hmask is not None and args.real_left_from_aspect and not args.real_box:
        # The item card overlaps the barrel in the Inspect view, so the left edge is rebuilt from the host mesh's aspect.
        aspect = (hb[2] - hb[0]) / (hb[3] - hb[1])
        rb = (int(round(rb[2] - aspect * (rb[3] - rb[1]))), rb[1], rb[2], rb[3])
    print(f'real box {rb} ({rb[2]-rb[0]}x{rb[3]-rb[1]}), host box {hb} ({hb[2]-hb[0]}x{hb[3]-hb[1]}); '
          f'aspect real {(rb[2]-rb[0])/(rb[3]-rb[1]):.2f} host {(hb[2]-hb[0])/(hb[3]-hb[1]):.2f}')
    gx, gy = args.grid
    cell_mask = None
    if hmask is not None:
        cell_mask = np.asarray(Image.fromarray((hmask[hb[1]:hb[3], hb[0]:hb[2]] * 255).astype(np.uint8)).resize(
            (gx * 16, gy * 16), Image.NEAREST)) > 127
    rc, hc = cells(real, rb, args.grid, cell_mask), cells(host, hb, args.grid, cell_mask)
    ratios, hue_diffs, lab = [], [], []
    print('cell  real(sRGB)        host(sRGB)        lin ratio host/real  hue real/host')
    for j in range(gy):
        for i in range(gx):
            r, h = rc[j, i], hc[j, i]
            if np.isnan(r).any() or np.isnan(h).any():
                continue
            lr, lh = srgb_to_linear(r).mean(), srgb_to_linear(h).mean()
            if lr < 0.01:
                continue  # empty or black cell in the real frame
            ratio = lh / lr
            ratios.append(ratio)
            hr, sr, vr = hue(r)
            hh, sh, vh = hue(h)
            if sr > 0.25 and sh > 0.25:
                d = abs(hr - hh)
                hue_diffs.append(min(d, 360 - d))
            lab.append(np.linalg.norm(np.asarray(r) - np.asarray(h)))
            print(f'({i:2d},{j}) {tuple(int(x) for x in r)!s:16} {tuple(int(x) for x in h)!s:16} {ratio:6.2f}   {hr:5.0f}/{hh:5.0f}')
    if ratios:
        print(f'SUMMARY cells={len(ratios)} median lin ratio host/real={np.median(ratios):.2f} '
              f'(p10 {np.percentile(ratios, 10):.2f}, p90 {np.percentile(ratios, 90):.2f}); '
              f'median hue diff={np.median(hue_diffs) if hue_diffs else float("nan"):.1f} deg over {len(hue_diffs)} saturated cells; '
              f'mean sRGB distance={np.mean(lab):.1f}')
    if args.out:
        w = 900
        panels = []
        for img, box in ((real, rb), (host, hb)):
            c = img.convert('RGB').crop(box)
            c = c.resize((w, int(w * c.height / c.width)), Image.LANCZOS)
            panels.append(c)
        sheet = Image.new('RGB', (w, panels[0].height + panels[1].height + 4), (255, 0, 255))
        sheet.paste(panels[0], (0, 0))
        sheet.paste(panels[1], (0, panels[0].height + 4))
        sheet.save(args.out)
        print('wrote', args.out)


if __name__ == '__main__':
    sys.exit(main())
