#!/usr/bin/env python3
"""Decode the slice's sounds to PCM WAV with vgmstream-cli (AI-assisted, clean room). See docs/verification/SLICE_AUDIO_CHAIN.md section 3.

Reads the ow-audio-v1 manifest (tools/audio_slice_chain.py), runs the decoder on each distinct .wem it names, and writes <source>.wav to an ignored
folder, where tools/prepare_slice_audio.py looks for them. The bytes come from the player's own install and stay on the player's machine.

  python tools/decode_slice_audio.py --vgmstream <path to vgmstream-cli.exe>      (or set OPENWILLOW_VGMSTREAM)

vgmstream is an external tool: this repository holds no copy, and this script never downloads one (THIRD_PARTY.md). A file is kept only when the decoder
exited 0, the WAV passes the same rules as the host's parser, its sample count, channels and rate equal the .wem header's exactly, and it is not all zeros.
That shows the decode's shape and that it holds signal: it does not show the sound is the right one, and nothing here means anyone has listened to it.
"""
import argparse
import json
import os
import subprocess
import sys
import time
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audio_census as ac  # noqa: E402
import prepare_slice_audio as psa  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECODE_TIMEOUT_S = 120
STATUS_DLL_NOT_FOUND = 0xC0000135


def run_decoder(exe, wem, wav):
    """(ok, detail). -i plays the stream once: without it a looped file comes out longer than its header says. The default output is 16-bit PCM."""
    try:
        done = subprocess.run([exe, '-i', '-o', wav, wem], capture_output=True, text=True, errors='replace', timeout=DECODE_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as error:
        return False, str(error)
    if done.returncode & 0xFFFFFFFF == STATUS_DLL_NOT_FOUND:
        return False, 'exit 0xC0000135: a DLL is missing; vgmstream-cli needs the DLLs bundled with it in its own folder (THIRD_PARTY.md)'
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


def output_paths(out_dir, source):
    """(final, partial) for a source, or None if either would not be directly inside the resolved out_dir: a source comes from a file, and must not choose where it writes."""
    base = os.path.realpath(out_dir)
    paths = tuple(os.path.realpath(os.path.join(base, name % source)) for name in ('%s.wav', '%s.partial.wav'))
    same = all(os.path.normcase(os.path.dirname(path)) == os.path.normcase(base) for path in paths)
    return paths if same else None


def check_against_header(wav, wem):
    """None when the WAV has exactly the .wem header's sample count, channels and rate and is not all zeros, else why not. Run after the parser rules have passed."""
    with open(wem, 'rb') as fh:
        header = ac.riff_info(fh.read(256))
    samples = header.get('vorbis_samples')
    if samples is None:
        return 'the .wem header gives no sample count'
    with wave.open(wav, 'rb') as decoded:
        frames, channels, rate = decoded.getnframes(), decoded.getnchannels(), decoded.getframerate()
        pcm = decoded.readframes(frames)
    if frames != samples:
        return '%d frames, the .wem header says %d' % (frames, samples)
    if (channels, rate) != (header.get('channels'), header.get('sample_rate')):
        return '%d channels at %d Hz, the .wem header says %s at %s' % (channels, rate, header.get('channels'), header.get('sample_rate'))
    if not pcm.strip(b'\0'):
        return 'decoded to silence'
    return None


def decode_one(exe, source, item, out_dir, decoder=run_decoder):
    """None when <source>.wav is in place and checked, else why not. The file appears under its final name only after the checks, so a bad decode leaves nothing."""
    wem = item.get('wem')
    if not wem or not os.path.isfile(wem):
        return 'no .wem at %s' % wem
    paths = output_paths(out_dir, source)
    if paths is None:
        return 'source %r would write outside the output folder' % (source,)
    final, partial = paths
    for stale in paths:     # a stale partial would otherwise be promoted by a decoder that exits 0 and writes nothing
        if os.path.exists(stale):
            os.remove(stale)
    ok, detail = decoder(exe, wem, partial)
    reason = detail if not ok else (psa.check_wav(partial, item.get('duration_s')) or check_against_header(partial, wem))
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
    kept = [output_paths(out_dir, s)[0] for s in media if str(s) not in failures]
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
    print('decoded %d of %d sources in %.1f s, %d bytes (shape and signal checked only; nothing has been listened to)'
          % (report['decoded'], report['distinct_sources'], report['elapsed_s'], report['output_bytes']))
    for source, reason in sorted(report['failures'].items()):
        print('FAILED %s: %s' % (source, reason))
    return 1 if report['failed'] else 0


if __name__ == '__main__':
    sys.exit(main())
