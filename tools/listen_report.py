"""Measure one tools/record_audio_listen.ps1 recording and write the listening guide for it.

Reads the run's UE log (OWAUDIOREC and OWSLICEGATE lines) and the WAV the host's audio mixer wrote, then writes into the run folder:
  cues.json  - every cue with its time from the recording start, peak and RMS in its window, and the frame saved for it
  LISTEN.md  - step by step: what is on screen, which sounds the host played, what to listen for, and what it measured
  ANSWERS.md - a sheet to fill in per step: OK / wrong / missing, and notes
Everything it writes is game-derived (event names come from the local manifest) and belongs under ignored local/.

The measurements say what the host sent to the mixer and how loud the mix was. They do not say a sound is the right one: nobody here can hear.

Usage: python tools/listen_report.py <log> <wav> <out dir> [--manifest local/slice/audio.json] [--play-map local/slice/audio_play.json]
Exit 0 when the WAV is there, long enough and not silent; 1 otherwise (the guide is still written, with the problem at the top).
"""
from __future__ import annotations

import argparse
import array
import json
import math
import re
import sys
import wave
from pathlib import Path

CUE = re.compile(r'OWAUDIOREC cue t=(-?[\d.]+) label=(.*?) frame=(\S+)')
START = re.compile(r'OWAUDIOREC start file=(.*?) map=(\S+)')
STOP = re.compile(r'OWAUDIOREC stop t=([\d.]+)')
WRITTEN = re.compile(r'OWAUDIOREC written=(\d) bytes=(-?\d+)')
LINK = re.compile(r'OWSLICEGATE link=(\S+) result=(\S+) fixtures=\[(.*?)\] detail=(.*) \(t=')
SUMMARY = re.compile(r'OWSLICEGATE SUMMARY pass=(\d+) fail=(\d+) na=(\d+)')
SILENT_DBFS = -60.0     # a window quieter than this counts as silence (host choice)
STEP_WINDOW = 1.0       # seconds measured after a step cue that is not itself a sound

# What the driver does on screen at each slice gate step, in our words. Steps with no sound of their own are still listed so a gap is visible.
STEPS = {
    'Start': 'the map has loaded; the driver reads its settings',
    'Spawn': 'Maya spawns in Sanctuary holding her starting gun',
    'Arena': 'Maya is moved to the fight area facing an enemy',
    'Fight': 'Maya waits for the enemy to hit her, then shoots it until it dies (one shot at a time until it is wounded, then faster)',
    'Recover': 'after the kill the test puts Maya down and restores her (the revive stand-in), and the other enemies go passive',
    'Loot': 'the driver looks for the dropped gun',
    'Pickup': 'Maya aims at the dropped gun and presses use to pick it up',
    'PickupDecoy': 'two more guns are dropped; Maya picks up the one she aims at, not the nearest',
    'Equip': 'the picked-up gun is equipped and drawn',
    'Phaselock': 'Maya casts Phaselock on an enemy; it lifts and is held, then released when the lock runs out',
    'Accept': 'Maya is moved to Marcus and presses use: the Fire mission is accepted',
    'WaitWalk': 'Marcus walks to his spot; the objective marker appears',
    'Trigger': 'Maya touches the mission trigger; the target dummy appears',
    'ShootDummy': 'Maya shoots the dummy with the lent pistol',
    'TurnIn': 'Maya turns the mission in at Marcus',
    'Confirm': 'the driver checks the reward and that the lent pistol was returned',
    'Down': 'the test applies lethal damage: Maya enters Fight For Your Life',
    'Bleed': 'Maya bleeds out (nobody to kill) and respawns',
    'Done': 'the run is over; the recording stops',
}

# The host's sound roles (OpenWillowAudio::Play callers), in our words. Event names come from the local manifest at run time.
ROLES = {
    'fire': ('every shot of every gun', 'manifest weapon:fire',
             'a pistol shot. The host plays the lent pistol\'s shot for every gun, so a rifle or another pistol sounding like that pistol is the known gap, not a new fault'),
    'hit': ('a bullet landing on a target (at most one sound per 50 ms)', 'manifest role:hit',
            'a bullet impact. The host picks one of many surface variants at random, so it may sound like metal, dirt or flesh at random; that is expected for now'),
    'phaselock_cast': ('the moment Phaselock lifts its target', 'manifest role:phaselock_cast',
                       'the Phaselock activation. Only the cast has a sound: the hold, the release and the bubble are silent on our build'),
    'pickup': ('an item added by the pickup key', 'no sound identified (play map: unresolved)', 'nothing: the host has no pickup sound yet'),
}

# Asked for, but not played by our build: what exists for each so a listener knows a silence there is expected.
NOT_PLAYED = [
    ('Reload (clip out / clip in)', 'decoded for the lent pistol, not hooked: the host has no reload notify yet'),
    ('Equip / draw a gun', 'decoded for the lent pistol, not hooked'),
    ('Phaselock hold, release and bubble', 'no sound mapped'),
    ('Pickups: cash, ammo, items', 'no sound identified; cash and ammo take other paths and are not hooked'),
    ('Shield hit, break, recharge', 'no sound mapped'),
    ('Maya taking damage, pain', 'no sound mapped'),
    ('Fight For Your Life, revive, respawn', 'no sound mapped'),
    ('Level-up', 'no sound mapped'),
    ('Mission accept / complete, Marcus\'s lines', 'mission dialog and Marcus\'s barks are decoded, not hooked (dialog is not played by the host)'),
    ('Door open / close', 'decoded, not hooked'),
    ('Vendor and menu sounds', 'no sound mapped (the menus are the Flash overlay, which plays no sound)'),
    ('Footsteps, jump, land', 'no sound mapped'),
    ('Ambient sound, music', 'none'),
]


def parse_log(text: str):
    cues = []
    for m in CUE.finditer(text):
        label = m.group(2).strip()
        seconds = None
        sm = re.search(r' seconds=([\d.]+)$', label)
        if sm:
            seconds = float(sm.group(1))
            label = label[:sm.start()]
        kind, name = ('step', label[5:]) if label.startswith('step ') else ('sound', label)
        cues.append({'t': float(m.group(1)), 'kind': kind, 'name': name, 'seconds': seconds, 'frame': None if m.group(3) == '-' else m.group(3)})
    links = [{'link': m.group(1), 'result': m.group(2), 'fixtures': m.group(3), 'detail': m.group(4)} for m in LINK.finditer(text)]
    stop = STOP.search(text)
    written = WRITTEN.search(text)
    summary = SUMMARY.search(text)
    return {'cues': cues, 'links': links, 'started': bool(START.search(text)), 'stop_t': float(stop.group(1)) if stop else None,
            'written': (written.group(1) == '1', int(written.group(2))) if written else None,
            'summary': summary.groups() if summary else None}


def read_wav(path: Path):
    """Returns (rate, channels, per-frame peak magnitude as a list of floats in 0..1). 16-bit PCM only (what the mixer's writer emits)."""
    with wave.open(str(path), 'rb') as w:
        rate, channels, width, frames = w.getframerate(), w.getnchannels(), w.getsampwidth(), w.getnframes()
        if width != 2:
            raise ValueError(f'{width * 8}-bit WAV, expected 16-bit')
        data = array.array('h')
        data.frombytes(w.readframes(frames))
    if sys.byteorder != 'little':
        data.byteswap()
    # Per frame, the loudest channel: a sound panned to one side still counts.
    mono = [max(abs(data[i + c]) for c in range(channels)) / 32768.0 for i in range(0, len(data) - channels + 1, channels)] if channels > 1 \
        else [abs(v) / 32768.0 for v in data]
    return rate, channels, mono


def dbfs(value: float) -> float:
    return 20 * math.log10(value) if value > 0 else float('-inf')


def measure(mono, rate, start, length):
    a, b = max(0, int(start * rate)), min(len(mono), int((start + length) * rate))
    window = mono[a:b]
    if not window:
        return None
    peak = max(window)
    rms = math.sqrt(sum(v * v for v in window) / len(window))
    # First frame louder than the silence floor, as a delay from the cue: how late the mixer started the sound.
    floor = 10 ** (SILENT_DBFS / 20)
    onset = next((i for i, v in enumerate(window) if v > floor), None)
    return {'peak_dbfs': round(dbfs(peak), 1), 'rms_dbfs': round(dbfs(rms), 1), 'onset_s': None if onset is None else round(onset / rate, 3),
            'clipped': peak >= 0.999}


def fmt_db(v):
    return '-inf' if v is None or v == float('-inf') else f'{v:.1f}'


def mmss(t):
    return f'{int(t // 60)}:{t % 60:06.3f}'


def load_names(manifest: Path | None):
    """Our role -> the Wwise event name the local manifest maps it to (game-derived; only ever written under local/)."""
    names = {}
    if manifest and manifest.exists():
        for entry in json.loads(manifest.read_text(encoding='utf-8')).get('entries', []):
            key, name = entry.get('key', ''), (entry.get('ak_event') or {}).get('wwise_name')
            if key == 'weapon:fire':
                names['fire'] = name
            elif key.startswith('role:'):
                names[key[5:]] = name
    return names


def build(log_text: str, wav: Path, out: Path, manifest: Path | None = None, play_map: Path | None = None):
    run = parse_log(log_text)
    problems = []
    if not run['started']:
        problems.append('the log has no OWAUDIOREC start line: the host never started recording')
    rate = channels = 0
    mono = []
    if not wav.exists():
        problems.append(f'no WAV at {wav}')
    else:
        try:
            rate, channels, mono = read_wav(wav)
        except (ValueError, wave.Error, EOFError) as error:
            problems.append(f'WAV unreadable: {error}')
    length = len(mono) / rate if rate else 0.0
    if mono and run['stop_t'] and length < run['stop_t'] - 1.0:
        problems.append(f'WAV is {length:.1f} s but the recording ran {run["stop_t"]:.1f} s: the file looks cut short')
    overall_peak = max(mono) if mono else 0.0
    if mono and dbfs(overall_peak) < SILENT_DBFS:
        problems.append(f'WAV is silent (peak {fmt_db(dbfs(overall_peak))} dBFS)')

    for cue in run['cues']:
        span = cue['seconds'] if cue['seconds'] else STEP_WINDOW
        cue['level'] = measure(mono, rate, cue['t'], span + 0.1) if mono else None

    # Steps: from one step cue to the next. Each holds the sound cues that fall inside it.
    steps = []
    for cue in run['cues']:
        if cue['kind'] == 'step':
            steps.append({'name': cue['name'], 't': cue['t'], 'frame': cue['frame'], 'sounds': []})
        elif steps:
            steps[-1]['sounds'].append(cue)
    for i, step in enumerate(steps):
        step['end'] = steps[i + 1]['t'] if i + 1 < len(steps) else (run['stop_t'] or length)
        # The pause before the next step should be quiet: measured over its last 1.5 s, which the driver's pause leaves empty.
        last_sound_end = max([s['t'] + (s['seconds'] or 0) for s in step['sounds']], default=step['t'])
        quiet_from = max(last_sound_end, step['end'] - 1.5)
        step['gap'] = measure(mono, rate, quiet_from, step['end'] - quiet_from) if mono and step['end'] - quiet_from > 0.2 else None

    names = load_names(manifest)
    unresolved = {}
    if play_map and play_map.exists():
        unresolved = json.loads(play_map.read_text(encoding='utf-8')).get('unresolved', {})
    onsets = sorted(c['level']['onset_s'] for c in run['cues'] if c['kind'] == 'sound' and c['level'] and c['level']['onset_s'] is not None)
    median_onset = onsets[len(onsets) // 2] if onsets else None

    out.mkdir(parents=True, exist_ok=True)
    (out / 'cues.json').write_text(json.dumps({'wav': str(wav), 'rate': rate, 'channels': channels, 'length_s': round(length, 3),
                                               'peak_dbfs': round(dbfs(overall_peak), 1) if mono else None, 'median_onset_s': median_onset,
                                               'problems': problems, 'links': run['links'], 'cues': run['cues']}, indent=1), encoding='utf-8')

    lines = ['# Listening guide: our host\'s sounds over one slice gate run', '',
             'AI-assisted (Claude). Written by `tools/listen_report.py` from the run\'s log and the WAV the host\'s audio mixer recorded. '
             'Game-derived: keep under ignored `local/`.', '',
             f'- Recording: `{wav.name}` ({length:.1f} s, {rate} Hz, {channels} channel(s), overall peak {fmt_db(dbfs(overall_peak)) if mono else "-"} dBFS)',
             f'- Slice gate: {"pass %s, fail %s, n/a %s" % run["summary"] if run["summary"] else "no summary in the log"}',
             f'- Times are seconds from the recording start. The mixer started sounds a median {median_onset if median_onset is not None else "?"} s after their cue.',
             '- Frames (`frames/`) are saved at most every 1.5 s, at the first cue that allows one, so a step shows its first moment.',
             '- Nothing here was listened to. The levels are measurements of the mix, not a judgement that a sound is right.', '']
    if problems:
        lines += ['**Problems with this recording:**', ''] + [f'- {p}' for p in problems] + ['']
    lines += ['## What our host plays', '', '| Role | When | Mapped to | What you should hear |', '|---|---|---|---|']
    for role, (when, mapped, hear) in ROLES.items():
        mapped_text = f'`{names[role]}` ({mapped})' if role in names else (unresolved.get(role) or mapped)
        lines.append(f'| `{role}` | {when} | {mapped_text} | {hear} |')
    lines += ['', 'Every sound plays 2D at full volume: no distance, no bus mix, no ducking. If something sounds too loud next to the game, that is part of what to note.', '',
              '## Step by step', '']
    for n, step in enumerate(steps, 1):
        lines.append(f'### {n}. {mmss(step["t"])} to {mmss(step["end"])}: {step["name"]}')
        lines.append('')
        lines.append(f'On screen: {STEPS.get(step["name"], "(no description)")}.' + (f' Frame: `frames/{step["frame"]}`.' if step['frame'] else ''))
        lines.append('')
        if not step['sounds']:
            lines.append('Our build plays nothing here. **You should hear silence.**')
        else:
            by_role = {}
            for s in step['sounds']:
                by_role.setdefault(s['name'], []).append(s)
            for role, cues in by_role.items():
                peaks = [c['level']['peak_dbfs'] for c in cues if c['level']]
                rmss = [c['level']['rms_dbfs'] for c in cues if c['level']]
                level = f'peak {fmt_db(min(peaks))} to {fmt_db(max(peaks))} dBFS, RMS {fmt_db(min(rmss))} to {fmt_db(max(rmss))} dBFS' if peaks else 'not measured'
                clipped = sum(1 for c in cues if c['level'] and c['level']['clipped'])
                hear = ROLES.get(role, ('', '', 'a sound the guide does not describe'))[2]
                lines.append(f'- **{role}** x{len(cues)}, first at {mmss(cues[0]["t"])}' + (f', last at {mmss(cues[-1]["t"])}' if len(cues) > 1 else '')
                             + f' ({level}' + (f', {clipped} clipped' if clipped else '') + f'). You should hear {hear}.')
                frames = [c['frame'] for c in cues if c['frame']]
                if frames:
                    lines.append(f'  Frames: ' + ', '.join(f'`frames/{f}`' for f in frames[:6]) + (' ...' if len(frames) > 6 else ''))
        gap = step['gap']
        if gap:
            lines.append('')
            lines.append(f'Gap before the next step: peak {fmt_db(gap["peak_dbfs"])} dBFS' + (' (silent, as it should be).' if gap['peak_dbfs'] < SILENT_DBFS else ' (**not silent**: something is still sounding).'))
        lines.append('')
    lines += ['## If it sounds wrong, note which', '',
              '- **Wrong sound**: a different sound from the game for this moment (say what it sounds like instead).',
              '- **Missing**: silence where the game makes a sound (the table below lists the ones we know are missing).',
              '- **Too loud / too quiet** next to the others, or next to the game.',
              '- **Cut off**: it stops early, or its tail is clipped.',
              '- **Wrong pitch or speed**: higher, lower, faster or slower than the game (a sample-rate fault sounds like this).',
              '- **Doubled / stacked**: the same sound twice at once, or a pile-up on fast fire.', '',
              '## Asked for, not played by our build (silence there is expected)', '', '| Sound | State |', '|---|---|']
    lines += [f'| {what} | {state} |' for what, state in NOT_PLAYED]
    (out / 'LISTEN.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')

    sheet = ['# Answer sheet', '', f'Recording `{wav.name}`. Mark each step OK, wrong or missing, and add a note (see LISTEN.md, "If it sounds wrong").', '',
             '| # | Time | Step | Sounds played | OK / wrong / missing | Notes |', '|---|---|---|---|---|---|']
    for n, step in enumerate(steps, 1):
        played = ', '.join(sorted({s['name'] for s in step['sounds']})) or 'none (expect silence)'
        sheet.append(f'| {n} | {mmss(step["t"])} | {step["name"]} | {played} |  |  |')
    sheet += ['', 'Overall: loudness of our sounds next to each other (OK / too loud / too quiet): ', '',
              'Anything the game plays in these moments that is not on the list in LISTEN.md: ', '']
    (out / 'ANSWERS.md').write_text('\n'.join(sheet) + '\n', encoding='utf-8')
    return problems, steps, run


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('log', type=Path)
    parser.add_argument('wav', type=Path)
    parser.add_argument('out', type=Path)
    parser.add_argument('--manifest', type=Path)
    parser.add_argument('--play-map', type=Path)
    args = parser.parse_args(argv)
    problems, steps, run = build(args.log.read_text(encoding='utf-8', errors='replace'), args.wav, args.out, args.manifest, args.play_map)
    for step in steps:
        roles = {}
        for s in step['sounds']:
            roles.setdefault(s['name'], []).append(s)
        text = '; '.join(f'{r} x{len(c)} peak max {fmt_db(max((x["level"]["peak_dbfs"] for x in c if x["level"]), default=None))} dBFS' for r, c in roles.items()) or 'no sound'
        gap = step['gap']
        print(f'{mmss(step["t"])} {step["name"]}: {text}' + (f' | gap peak {fmt_db(gap["peak_dbfs"])} dBFS' if gap else ''))
    for p in problems:
        print('PROBLEM', p)
    print(f'Wrote {args.out / "LISTEN.md"}, {args.out / "ANSWERS.md"}, {args.out / "cues.json"}')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
