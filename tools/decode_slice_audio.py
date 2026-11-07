#!/usr/bin/env python3
"""Decode the slice's sounds to PCM WAV with vgmstream-cli (AI-assisted, clean room). See docs/verification/SLICE_AUDIO_CHAIN.md section 3.

Reads the ow-audio-v1 manifest (tools/audio_slice_chain.py), runs the decoder on each distinct .wem it names, and writes <source>.wav to an ignored
folder, where tools/prepare_slice_audio.py looks for them. The bytes come from the player's own install and stay on the player's machine.

  python tools/decode_slice_audio.py --vgmstream <path to vgmstream-cli.exe>      (or set OPENWILLOW_VGMSTREAM)

vgmstream is an external tool: this repository holds no copy, and this script never downloads one (THIRD_PARTY.md). A file is kept only when the decoder
exited 0, the WAV passes the same rules as the host's parser, and its length matches the .wem header. That is a length check: it does not show the sound
is the right one, and nothing here means anyone has listened to it.
"""
import argparse
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import prepare_slice_audio as psa  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECODE_TIMEOUT_S = 120


def run_decoder(exe, wem, wav):
    """(ok, detail). -i plays the stream once: without it a looped file comes out longer than its header says. The default output is 16-bit PCM."""
    try:
        done = subprocess.run([exe, '-i', '-o', wav, wem], capture_output=True, text=True, errors='replace', timeout=DECODE_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as error:
        return False, str(error)
    if done.returncode:
        return False, 'exit %d: %s' % (done.returncode, (done.stderr or done.stdout).strip()[:200])
    return True, ''


def distinct_media(manifest):
    """Each source once, whichever entries share it. A source with no .wem cannot be decoded and is reported."""
    media = {}
    for entry in manifest['entries']:
        for item in entry['media']:
            media.setdefault(item['source'], item)
    return media


def decode_one(exe, source, item, out_dir, decoder=run_decoder):
    """None when <source>.wav is in place and checked, else why not. The file appears under its final name only after the checks, so a bad decode leaves nothing."""
    wem = item.get('wem')
    if not wem or not os.path.isfile(wem):
        return 'no .wem at %s' % wem
    final = os.path.join(out_dir, '%s.wav' % source)
    partial = os.path.join(out_dir, '%s.partial.wav' % source)
    if os.path.exists(final):
        os.remove(final)
    ok, detail = decoder(exe, wem, partial)
    reason = detail if not ok else psa.check_wav(partial, item.get('duration_s'))
    if reason:
        if os.path.exists(partial):
            os.remove(partial)
        return reason
    os.replace(partial, final)
    return None


def inside_repo_but_not_local(path):
    """True for a folder in this repository outside ignored local/: decoded game audio must never land where it could be committed."""
    path, root = os.path.normcase(os.path.abspath(path)), os.path.normcase(ROOT)
    if not (path == root or path.startswith(root + os.sep)):
        return False
    local = os.path.join(root, 'local')
    return not (path == local or path.startswith(local + os.sep))


def decode_all(exe, manifest, out_dir, decoder=run_decoder):
    media = distinct_media(manifest)
    os.makedirs(out_dir, exist_ok=True)
    failures = {}
    started = time.perf_counter()
    for source, item in sorted(media.items()):
        reason = decode_one(exe, source, item, out_dir, decoder)
        if reason:
            failures[str(source)] = reason
    elapsed = time.perf_counter() - started
    kept = [os.path.join(out_dir, '%s.wav' % s) for s in media if str(s) not in failures]
    return {'distinct_sources': len(media), 'decoded': len(kept), 'failed': len(failures), 'failures': failures, 'elapsed_s': round(elapsed, 3),
            'output_bytes': sum(os.path.getsize(p) for p in kept)}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--vgmstream', default=os.environ.get('OPENWILLOW_VGMSTREAM'), help='path to vgmstream-cli.exe (default: OPENWILLOW_VGMSTREAM)')
    ap.add_argument('--manifest', default=os.path.join(ROOT, 'local', 'slice', 'audio.json'))
    ap.add_argument('--out-dir', default=os.path.join(ROOT, 'local', 'audio', 'decoded'))
    ap.add_argument('--report', default=os.path.join(ROOT, 'local', 'audio', 'decode_report.json'))
    args = ap.parse_args(argv)
    if not args.vgmstream or not os.path.isfile(args.vgmstream):
        sys.exit('vgmstream-cli not found (%s): pass --vgmstream or set OPENWILLOW_VGMSTREAM. See THIRD_PARTY.md; this script does not download it.' % args.vgmstream)
    if inside_repo_but_not_local(args.out_dir) or inside_repo_but_not_local(args.report):
        sys.exit('decoded audio stays under ignored local/: refusing %s' % args.out_dir)
    if not os.path.isfile(args.manifest):
        sys.exit('no manifest at %s: run tools/audio_slice_chain.py first' % args.manifest)
    with open(args.manifest, encoding='utf-8') as fh:
        manifest = json.load(fh)
    if manifest.get('schema') != 'ow-audio-v1':
        sys.exit('%s is not an ow-audio-v1 manifest' % args.manifest)
    report = decode_all(args.vgmstream, manifest, args.out_dir)
    with open(args.report, 'w', encoding='utf-8') as fh:
        json.dump(report, fh, indent=1)
    print('decoded %d of %d sources in %.1f s, %d bytes (length checked only; nothing has been listened to)'
          % (report['decoded'], report['distinct_sources'], report['elapsed_s'], report['output_bytes']))
    for source, reason in sorted(report['failures'].items()):
        print('FAILED %s: %s' % (source, reason))
    return 1 if report['failed'] else 0


if __name__ == '__main__':
    sys.exit(main())
