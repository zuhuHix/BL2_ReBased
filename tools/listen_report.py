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
FIXTURE_MAG = re.compile(r'OWSLICEGATE note test fixture: magazine of (\S+) set to (\d+) rounds')
FIXTURE_REFUSED = re.compile(r'OWSLICEGATE note test fixture: -owslicegatemag=(\d+) not applied, (.*)')
SILENT_DBFS = -60.0     # a window quieter than this counts as silence (host choice)
STEP_WINDOW = 1.0       # seconds measured after a step cue that is not itself a sound
TAIL = 0.4              # the mixer starts a sound up to ~0.15 s after its cue and stops it 0.25 s after it drains: skipped before a gap is measured

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
    'equip': ('a gun drawn: from the slot keys, the inventory, or the first gun at the start of a run', 'manifest weapon:equip (the lent pistol\'s pickup-and-equip sound)',
              'a short pickup-and-equip sound for the gun in hand. The host plays it for every gun, so another gun sounding like the lent pistol\'s is the known gap; whether the game plays this sound on a plain draw is UNVERIFIED'),
    'reload_out': ('the reload\'s clip-out notify, on the lent pistol only', 'manifest reload notify, clip out (the play map\'s timing)',
                   'the magazine coming out. When it plays within the reload is UNVERIFIED against the game: the host\'s timing is the notify offset, scaled to the reload'),
    'reload_in': ('the reload\'s clip-in notify, on the lent pistol only', 'manifest reload notify, clip in (the play map\'s timing)',
                  'the new magazine going in, later in the same reload. Its timing is UNVERIFIED against the game, as for clip out'),
    'door_open': ('the slice mover door starts to open', 'manifest door:open (one of the door\'s two sound keys)',
                  'a wooden door opening, from the door: quieter the farther Maya stands (engine default falloff, UNVERIFIED against the game). Only the one mover door has a sound; which of the two keys is the opening sound is UNVERIFIED'),
    'door_close': ('the slice mover door starts to close', 'manifest door:close (the door\'s other sound key)',
                   'a wooden door closing, from the door, like the opening sound. The host starts it when the door starts to close, not at the key\'s own time in the game data (UNVERIFIED that the game does the same)'),
    'line:<event tag>': ('a mission line or a Marcus bark, when the stock dialog (run in the VM) starts it; the next line waits for its end',
                         'manifest dialog: or marcus_group: entry of that event tag',
                         'a voice line (the cue names its event tag). Which line plays and when is the stock dialog\'s choice in the VM; a bark with several takes is picked '
                         'at random (the container\'s rule is UNVERIFIED). From Marcus when he is the talker (an echo caller stays 2D); no radio (echo) filter, no ducking of the other sounds'),
}
LINE_ROLE = 'line:<event tag>'


def describe(role):
    """What a listener should hear for a cue's role; every line:<tag> cue shares one description."""
    if role.startswith('line:'):
        return ROLES[LINE_ROLE][2]
    return ROLES.get(role, ('', '', 'a sound the guide does not describe'))[2]

# The same step when the magazine fixture (-owslicegatemag) was applied: the first shot empties the magazine, the objective completes, and the
# fixture then asks for one reload before the turn-in step.
STEPS_WITH_FIXTURE = {'ShootDummy': 'Maya shoots the dummy with the lent pistol; the magazine fixture empties it on the first shot, and after the objective she reloads before turning in'}

# Asked for, but not played by our build: what exists for each so a listener knows a silence there is expected.
NOT_PLAYED = [
    ('Reload of guns other than the lent pistol', 'no reload sound mapped for them: the lent pistol\'s reload clip is the only one with notify times in the manifest'),
    ('Phaselock hold, release and bubble', 'no sound mapped'),
    ('Pickups: cash, ammo, items', 'no sound identified; cash and ammo take other paths and are not hooked'),
    ('Shield hit, break, recharge', 'no sound mapped'),
    ('Maya taking damage, pain', 'no sound mapped'),
    ('Fight For Your Life, revive, respawn', 'no sound mapped'),
    ('Level-up', 'no sound mapped'),
    ('Marcus\'s turn-in and dismissal lines, his lingering-in-menu bark', 'not played: the stock data gives the turn-in tag no line, the dismissal line is not in the manifest, and the lingering timer is not modelled (section 5c)'),
    ('A mission line whose talker the VM does not resolve', 'silent: the run log reports it as "no talker"; no speaker is guessed'),
    ('Mission accept and complete sounds other than voice lines', 'no sound identified'),
    ('Other doors, and the door\'s opening or closing in the middle of a move', 'no sound: the mover door\'s sound plays only when a motion starts (section 5b)'),
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
    fixture = FIXTURE_MAG.search(text)
    refused = FIXTURE_REFUSED.search(text)
    return {'magazine_fixture': (fixture.group(1), int(fixture.group(2))) if fixture else None,
            'magazine_refused': (int(refused.group(1)), refused.group(2).strip()) if refused else None, 'cues': cues, 'links': links, 'started': bool(START.search(text)), 'stop_t': float(stop.group(1)) if stop else None,
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
    # First frame louder than the silence floor, as a delay from the cue. The cue's clock (the host's real time from its start call) and the
    # file's (the mixer's first recorded buffer) are not calibrated against each other, so this mixes render delay with that offset. Only when the cue starts in
    # silence (the 0.2 s before it are quiet); over another sound it would measure that sound.
    floor = 10 ** (SILENT_DBFS / 20)
    before = mono[max(0, a - int(0.2 * rate)):a]
    quiet_before = not before or max(before) <= floor
    onset = next((i for i, v in enumerate(window) if v > floor), None) if quiet_before else None
    return {'peak_dbfs': round(dbfs(peak), 1), 'rms_dbfs': round(dbfs(rms), 1), 'onset_s': None if onset is None else round(onset / rate, 3),
            'clipped': peak >= 0.999}


PLAYED = re.compile(r'OWAUDIO play role=(\S+) channels=\d+ rate=\d+ bytes=\d+(?: gain_db=(-?[\d.]+))?(?: file=(\S+))?.*?( at=\S+)?\s+\(the host called play')
REFERENCE_ROLE = 'fire'
FIT_SEARCH_S = 0.4      # how late after its cue a sound may start in the recording (the mixer starts about 0.13 s late)
FIT_LEAD_S = 0.05       # how early before its cue a sound may start (the two clocks are not calibrated)
FIT_MIN_CORR = 0.3      # below this the fit has not found the file in the recording: no applied gain is reported
FIT_MIN_FRAMES = 32     # a file shorter than this is not fitted
FIT_MAX_CANDIDATES = 8  # without a logged file, a role with at most this many files has each tried


def read_channels(path: Path):
    """(rate, frames x channels float array in -1..1) of a 16-bit PCM WAV, with numpy. Raises ImportError without numpy."""
    import numpy
    with wave.open(str(path), 'rb') as w:
        rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        if width != 2:
            raise ValueError(f'{width * 8}-bit WAV, expected 16-bit')
        data = numpy.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(numpy.float64) / 32768.0
    return rate, data.reshape(-1, channels)


def fit_gain(recording, rate, t, source_rate, source):
    """The gain at which `source` (frames x channels) sits in `recording`, starting between FIT_LEAD_S before t and FIT_SEARCH_S after it
    (the returned lag is relative to t): the least-squares scale of the source against the recording at the best-correlated lag, per recording channel (a mono source against each), combined as the RMS
    of the channel gains. A different sound overlapping it adds little to the fit, which is why this, not a window's RMS, measures what the
    host applied. Returns (gain dB, correlation, lag s, clipped) or None."""
    import numpy
    n = int(len(source) * rate / source_rate)
    if n < FIT_MIN_FRAMES:
        return None
    grid = numpy.arange(n) * source_rate / rate
    x = numpy.stack([numpy.interp(grid, numpy.arange(len(source)), source[:, c]) for c in range(source.shape[1])], 1)
    # The search starts FIT_LEAD_S before the cue: the first equip of both listening runs started 2 and 8 ms before its cue, so a search
    # from the cue on missed it (why it comes early is UNVERIFIED; the clocks of cues and recording are assumed, not shown, to differ).
    a = int(t * rate)
    if a < 0:
        return None   # a cue before the recording started: nothing of it can be in the file
    first = max(0, a - int(FIT_LEAD_S * rate))   # a cue in the first FIT_LEAD_S searches from the file's start
    segment = recording[first:a + int(FIT_SEARCH_S * rate) + n]
    if len(segment) < n:
        return None
    xm, sm = x.mean(1), segment.mean(1)

    def correlation(lag):
        y = sm[lag:lag + n]
        return float(numpy.dot(xm, y) / (math.sqrt(float(numpy.dot(xm, xm)) * float(numpy.dot(y, y))) + 1e-12))

    # Every lag at full rate, by FFT: a broadband sound (a shot, an impact) correlates only at its exact lag, so a search on a decimated
    # signal can miss it entirely (review r1 on PR #221).
    count = len(sm) - n + 1
    size = 1 << (len(sm) + n - 1).bit_length()
    raw = numpy.fft.irfft(numpy.fft.rfft(sm, size) * numpy.conj(numpy.fft.rfft(xm, size)), size)[:count]
    running = numpy.concatenate([[0.0], numpy.cumsum(sm * sm)])
    energy = numpy.sqrt(numpy.maximum(running[n:n + count] - running[:count], 0.0)) + 1e-12
    best = int(numpy.argmax(raw / energy))
    y = segment[best:best + n]
    gains = []
    for c in range(y.shape[1]):
        # A mono source fits each recording channel; a stereo source fits channel for channel. A stereo source against a mono recording
        # would fit only its left channel (the recorder writes stereo, so this does not arise today).
        xc = x[:, min(c, x.shape[1] - 1)]
        gains.append(float(numpy.dot(xc, y[:, c])) / (float(numpy.dot(xc, xc)) + 1e-12))
    # A clipped recording flattens the peaks the fit scales against, which biases the gain low: flagged, not corrected.
    clipped = bool(numpy.max(numpy.abs(y)) >= 0.999)
    return dbfs(math.sqrt(sum(g * g for g in gains) / len(gains))), correlation(best), (first + best - a) / rate, clipped


def fit_cues(cues, log_text: str, wav: Path, play_map: dict | None):
    """Per sound cue (by its index in `cues`): {'file', 'fit_gain_db', 'fit_corr', 'file_logged', 'file_guessed', 'clipped'}. Each cue line
    is paired with the play line of its role just before it (Play logs its line, then its cue), so a play outside the recording, which logs
    no cue, shifts nothing. The file is the one that play line's file= names among the role's files; without one, a role's only file, or
    each of a small role's files tried and the best kept (file_guessed). None when numpy is missing (the report then says so)."""
    try:
        rate, recording = read_channels(wav)
    except ImportError:
        return None
    roles = (play_map or {}).get('roles', {})
    logged_files = cue_play_files(log_text)
    if len(logged_files) != len(cues):
        logged_files = [None] * len(cues)   # cues not taken from this log: nothing to pair them with
    sources, found = {}, {}
    for index, cue in enumerate(cues):
        if cue['kind'] != 'sound':
            continue
        name = logged_files[index]
        files = roles.get(cue['name'], [])
        # Another cue of the same role within the search window would be found too (two identical sounds a few ms apart even add up),
        # so such a cue gets no fit rather than a wrong one.
        if any(j != index and other['kind'] == 'sound' and other['name'] == cue['name'] and abs(other['t'] - cue['t']) < FIT_SEARCH_S + FIT_LEAD_S
               for j, other in enumerate(cues)):
            continue
        named = next((f for f in files if name and Path(f).name == name), None)
        # A log without file= (recordings before it was logged): try each of a small role's files and keep the best fit.
        candidates = [named] if named else (files if len(files) <= FIT_MAX_CANDIDATES else [])
        best = None
        for path in candidates:
            if path not in sources:
                try:
                    sources[path] = read_channels(Path(path))
                except (OSError, ValueError, wave.Error, EOFError):
                    sources[path] = None
            fitted = fit_gain(recording, rate, cue['t'], *sources[path]) if sources[path] else None
            if fitted and (best is None or fitted[1] > best[1][1]):
                best = (path, fitted)
        if best and best[1][1] >= FIT_MIN_CORR:
            found[index] = {'file': best[0], 'fit_gain_db': round(best[1][0], 2), 'fit_corr': round(best[1][1], 3), 'file_logged': bool(named), 'file_guessed': not named and len(candidates) > 1,
                            'clipped': best[1][3]}
    return found


def cue_play_files(log_text: str):
    """For each OWAUDIOREC cue line, in order: the file= of the play line of its role logged after that role's previous cue and before this
    one (the last such), or None (a step cue, or no file logged)."""
    events = sorted([(m.start(), 'play', m.group(1), m.group(3)) for m in PLAYED.finditer(log_text)]
                    + [(m.start(), 'cue', m.group(2).strip(), None) for m in CUE.finditer(log_text)])
    pending, files = {}, []
    for _, kind, label, file in events:
        if kind == 'play':
            pending[label] = file
            continue
        role = re.sub(r' seconds=[\d.]+$', '', label)
        files.append(pending.pop(role, None))
    return files


def file_level(path: Path):
    """RMS in dBFS of one played file as the recording's cue window would see it: per frame the loudest channel (as read_wav), over the
    file's length plus the 0.1 s the cue window adds, so the two are measured alike. None when the file cannot be read."""
    try:
        rate, _, mono = read_wav(path)
    except (OSError, ValueError, wave.Error, EOFError):
        return None
    if not mono:
        return None
    frames = len(mono) + int(0.1 * rate)
    return dbfs(math.sqrt(sum(v * v for v in mono) / frames))


def level_table(cues, log_text: str, play_map: dict | None, fits: dict | None = None):
    """Per role: the measured RMS of its cues (median), and what the game's mix predicts: each file's level plus that file's own gain from
    the play map, then the median of those. A file whose gain is null (no game value) is left out, and a role with none left gets no
    prediction: a fallback 0 dB is not the game's balance. Positioned roles (a play line with at=) depend on where Maya stood, so they get
    no prediction either. The balance column puts both next to the shot (REFERENCE_ROLE), which cancels any offset the whole mix shares.

    With `fits` (fit_cues), each role also gets the gain the host applied, measured by fitting the played file in the recording, and that
    minus the file's map gain. "Off by" then compares that offset with the shot's. A window's RMS also holds whatever else sounds at the
    time (a shot's own impact, a second shot, a line), which made the shot's RMS reference several dB too loud in the first recordings."""
    positioned = {m.group(1) for m in PLAYED.finditer(log_text) if m.group(4)}
    roles = (play_map or {}).get('roles', {})
    gains = (play_map or {}).get('gains', {})
    notes = (play_map or {}).get('gain_notes', {})
    rows = {}
    for cue in cues:
        if cue['kind'] != 'sound' or not cue.get('level') or cue['level']['rms_dbfs'] == float('-inf'):
            continue
        rows.setdefault(cue['name'], []).append(cue['level']['rms_dbfs'])
    median = lambda values: sorted(values)[len(values) // 2] if values else None
    table = []
    for role, measured in rows.items():
        files = roles.get(role, [])
        role_gains = gains.get(role) or []
        pairs = []   # (file level, its gain) for files with a game gain
        for index, path in enumerate(files):
            gain = role_gains[index] if index < len(role_gains) else None
            level = file_level(Path(path))
            if isinstance(gain, (int, float)) and level is not None and level != float('-inf'):
                pairs.append((level, float(gain)))
        expected = median([level + gain for level, gain in pairs]) if pairs and role not in positioned else None
        if role in positioned:
            prediction = 'positioned'
        elif not pairs:
            prediction = 'no game prediction'
        else:
            prediction = 'from %d of %d file(s)' % (len(pairs), len(files)) if len(pairs) < len(files) else 'game data'
        file_gain = {path: role_gains[i] for i, path in enumerate(files) if i < len(role_gains)}
        fitted = [fit for i, fit in (fits or {}).items() if cues[i]['name'] == role]
        offsets = [fit['fit_gain_db'] - file_gain[fit['file']] for fit in fitted if isinstance(file_gain.get(fit['file']), (int, float))]
        table.append({'fit_gain_db': None if not fitted else round(median([f['fit_gain_db'] for f in fitted]), 1),
                      'fit_corr': None if not fitted else round(median([f['fit_corr'] for f in fitted]), 2), 'fitted_cues': len(fitted),
                      'applied_minus_map_db': None if not offsets else round(median(offsets), 1),
                      'file_guessed': any(f.get('file_guessed') for f in fitted), 'fit_clipped': any(f.get('clipped') for f in fitted),
                      'role': role, 'cues': sum(1 for c in cues if c['kind'] == 'sound' and c['name'] == role),
                      'measured_cues': len(measured), 'measured_rms_dbfs': round(median(measured), 1),
                      'source_rms_dbfs': None if not pairs else round(median([level for level, _ in pairs]), 1),
                      'gain_db': median([gain for _, gain in pairs]), 'positioned': role in positioned, 'prediction': prediction,
                      'gain_note': notes.get(role), 'expected_rms_dbfs': None if expected is None else round(expected, 1)})
    reference = next((r for r in table if r['role'] == REFERENCE_ROLE), None)
    for row in table:
        row['measured_vs_shot_db'] = round(row['measured_rms_dbfs'] - reference['measured_rms_dbfs'], 1) if reference else None
        both = reference and row['expected_rms_dbfs'] is not None and reference['expected_rms_dbfs'] is not None
        row['game_vs_shot_db'] = round(row['expected_rms_dbfs'] - reference['expected_rms_dbfs'], 1) if both else None
        row['off_by_db'] = round(row['measured_vs_shot_db'] - row['game_vs_shot_db'], 1) if both else None
        row['off_by_from'] = 'rms' if row['off_by_db'] is not None else None
        if fits is not None:
            # With fits the RMS balance is known to be several dB off (overlapping sounds), so a role without a fit shows none.
            row['off_by_db'], row['off_by_from'] = None, None
            if reference and not row['positioned'] and row['applied_minus_map_db'] is not None and reference['applied_minus_map_db'] is not None:
                row['off_by_db'] = round(row['applied_minus_map_db'] - reference['applied_minus_map_db'], 1)
                row['off_by_from'] = 'fit'
    return table


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
            if key in ('weapon:fire', 'weapon:equip'):
                names[key[7:]] = name
            elif key in ('door:open', 'door:close'):
                names['door_' + key[5:]] = name
            elif key.startswith('role:'):
                names[key[5:]] = name
            elif key.startswith('weapon:reload_anim_') and name:
                # Same rule as tools/prepare_slice_audio.py: the AkEvent's name says clip out or clip in.
                for role, suffix in (('reload_out', 'Clip_Out'), ('reload_in', 'Clip_In')):
                    if name.endswith(suffix):
                        names.setdefault(role, name)
    return names


LOCAL = Path(__file__).resolve().parent.parent / 'local'


def checked_out(out: Path, local_root: Path = LOCAL) -> Path:
    """The output folder, resolved through links, if it lies inside local_root (resolved the same way); ValueError otherwise.
    The report carries event names from the local manifest, so it may only be written under the checkout's ignored local/."""
    root = local_root.resolve()
    target = out.resolve()
    if target != root and root not in target.parents:
        raise ValueError(f'refusing to write the report outside {root}: {out} resolves to {target}')
    return target


def build(log_text: str, wav: Path, out: Path, manifest: Path | None = None, play_map: Path | None = None, local_root: Path = LOCAL):
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
    if run['magazine_refused']:
        # The gate asked for a magazine it could not set (no magazine-fed gun in hand): the fixture did nothing, and the report says so.
        requested, why = run['magazine_refused']
        problems.append(f'the magazine fixture (-owslicegatemag={requested}) was not applied: {why}. No fixture reload is expected from this run.')
    elif run['magazine_fixture']:
        # The run set the lent gun's magazine (-owslicegatemag): a reload is expected after its first shot at the dummy.
        missing = [r for r in ('reload_out', 'reload_in') if not any(c['kind'] == 'sound' and c['name'] == r for c in run['cues'])]
        if missing:
            problems.append('the magazine fixture was set but no ' + ' or '.join(missing) + ' sound was logged: look for "OpenWillow reload started" in the run log')
    if mono and dbfs(overall_peak) < SILENT_DBFS:
        problems.append(f'WAV is silent (peak {fmt_db(dbfs(overall_peak))} dBFS)')

    for cue in run['cues']:
        span = cue['seconds'] if cue['seconds'] else STEP_WINDOW
        cue['level'] = measure(mono, rate, cue['t'], span + 0.1) if mono else None

    # Steps: from one step cue to the next. Each holds the sound cues that fall inside it.
    steps = []
    for cue in run['cues']:
        # A step that restarts its own clock logs its name again (the gate's Accept does): one step for the listener.
        if cue['kind'] == 'step' and steps and steps[-1]['name'] == cue['name']:
            continue
        if cue['kind'] == 'step':
            steps.append({'name': cue['name'], 't': cue['t'], 'frame': cue['frame'], 'sounds': []})
        elif steps:
            steps[-1]['sounds'].append(cue)
    for i, step in enumerate(steps):
        step['end'] = steps[i + 1]['t'] if i + 1 < len(steps) else (run['stop_t'] or length)
        # The pause before the next step should be quiet: measured over its last 1.5 s, which the driver's pause leaves empty.
        last_sound_end = max([s['t'] + (s['seconds'] or 0) for s in step['sounds']], default=step['t'])
        quiet_from = max(last_sound_end + TAIL if step['sounds'] else last_sound_end, step['end'] - 1.5)
        step['gap'] = measure(mono, rate, quiet_from, step['end'] - quiet_from) if mono and step['end'] - quiet_from > 0.2 else None

    names = load_names(manifest)
    unresolved, play = {}, None
    if play_map and play_map.exists():
        play = json.loads(play_map.read_text(encoding='utf-8'))
        unresolved = play.get('unresolved', {})
    fits = fit_cues(run['cues'], log_text, wav, play) if mono else None
    for index, fit in (fits or {}).items():
        run['cues'][index]['fit'] = fit
    levels = level_table(run['cues'], log_text, play, fits) if mono else []
    onsets = sorted(c['level']['onset_s'] for c in run['cues'] if c['kind'] == 'sound' and c['level'] and c['level']['onset_s'] is not None)
    median_onset = onsets[len(onsets) // 2] if onsets else None

    out = checked_out(out, local_root)
    out.mkdir(parents=True, exist_ok=True)
    (out / 'cues.json').write_text(json.dumps({'wav': str(wav), 'rate': rate, 'channels': channels, 'length_s': round(length, 3),
                                               'peak_dbfs': round(dbfs(overall_peak), 1) if mono else None, 'median_onset_s': median_onset,
                                               'problems': problems, 'links': run['links'], 'cues': run['cues'], 'levels': levels}, indent=1), encoding='utf-8')

    lines = ['# Listening guide: our host\'s sounds over one slice gate run', '',
             'AI-assisted (Claude). Written by `tools/listen_report.py` from the run\'s log and the WAV the host\'s audio mixer recorded. '
             'Game-derived: keep under ignored `local/`.', '',
             f'- Recording: `{wav.name}` ({length:.1f} s, {rate} Hz, {channels} channel(s), overall peak {fmt_db(dbfs(overall_peak)) if mono else "-"} dBFS)',
             f'- Slice gate: {"pass %s, fail %s, n/a %s" % run["summary"] if run["summary"] else "no summary in the log"}',
             f'- Times are minutes:seconds from the recording start (the first {steps[0]["t"]:.0f} s are the map loading, silent). Sounds that start in silence: onset relative to the cue, median '
             f'{median_onset if median_onset is not None else "?"} s ({len(onsets)} measured; clocks not calibrated, so this is not a latency).' if steps else '- No step cues in the log.',
             '- A shot and its bullet impact start within a few milliseconds of each other, so their levels below are of the two together, not each alone.',
             '- Frames (`frames/`) are saved at most every 1.5 s, at the first cue that allows one, so a step shows its first moment.',
             '- Nothing here was listened to. The levels are measurements of the mix, not a judgement that a sound is right.', '']
    if run['magazine_fixture']:
        item, rounds = run['magazine_fixture']
        lines.append(f'- Test fixture: the held gun ({item}) had its magazine set to {rounds} round(s) before the dummy shots (`-owslicegatemag`), so it reloads '
                     'after its first shot there. The reload itself is the game code; the magazine size and the first shot belong to the fixture.')
    else:
        lines.append('- No magazine fixture in this run: a reload sound appears only if the gun ran dry on its own.')
    lines.append('')
    if problems:
        lines += ['**Problems with this recording:**', ''] + [f'- {p}' for p in problems] + ['']
    lines += ['## What our host plays', '', '| Role | When | Mapped to | What you should hear |', '|---|---|---|---|']
    for role, (when, mapped, hear) in ROLES.items():
        mapped_text = f'`{names[role]}` ({mapped})' if role in names else (unresolved.get(role) or mapped)
        lines.append(f'| `{role}` | {when} | {mapped_text} | {hear} |')
    if levels:
        lines += ['', "## Levels against the game's mix", '',
                  "Median RMS of each role's cues, next to what the game's static mix predicts: the median level of the role's files (measured like a cue window) "
                  'plus its gain from the play map (SLICE_AUDIO_CHAIN.md section 7). "vs shot" is the difference to the `fire` cues, which cancels an offset the whole mix '
                  "shares; \"off by\" is our balance minus the game's. A shot's cue window also holds its bullet impact, and a line can overlap other sounds, so a few dB "
                  'are noise. Positioned roles depend on where Maya stood and get no prediction, and neither does a role whose files have no game gain (null in the play map: the host played them at 0 dB). File RMS and Gain are medians; Game RMS applies to each file its own gain before its median. Measurements of the mix, not a listen.', '',
                  '"Applied" is the gain at which the played file itself sits in the recording: a least-squares fit of the file against the recording at the best lag '
                  '(median over the cues it found, with the median correlation). It is not moved by other sounds playing at the same time, as a window\'s RMS is. '
                  '"Applied - map" is that minus the file\'s gain in the play map: the same number for every 2D role means the host kept the game\'s balance. '
                  'When fits exist, "Off by" is a role\'s "Applied - map" minus the shot\'s (marked fit); a role without a fit shows none, and when the shot itself has no fit '
                  '(for example two shots inside one 0.4 s search window, as under automatic fire) no role does. Without numpy it is the RMS balance (marked rms). '
                  '"(file guessed)": the log named no file, so each of the role\'s files was tried and the best fit kept. "(clipped)": the recording clips there, which biases the fit low.'
                  + ('' if fits is not None else ' **No fit in this report: numpy is not installed in the Python that ran it.**'), '',
                  '| Role | Cues | Measured RMS | File RMS | Gain | Game RMS | Measured vs shot | Game vs shot | Applied (corr) | Applied - map | Off by |',
                  '|---|---|---|---|---|---|---|---|---|---|---|']
        for row in sorted(levels, key=lambda r: (r['role'].startswith('line:'), r['role'])):
            cell = lambda v, unit=' dB': '-' if v is None else f'{v:.1f}{unit}'
            counted = row['cues'] if row['measured_cues'] == row['cues'] else f'{row["cues"]} ({row["measured_cues"]} measured)'
            lines.append(f'| `{row["role"]}` | {counted} | {cell(row["measured_rms_dbfs"], " dBFS")} | {cell(row["source_rms_dbfs"], " dBFS")} | {cell(row["gain_db"])} | '
                         + (cell(row['expected_rms_dbfs'], ' dBFS') + ('' if row['prediction'] == 'game data' else f' ({row["prediction"]})')
                            if row['expected_rms_dbfs'] is not None else row['prediction'])
                         + f' | {cell(row["measured_vs_shot_db"])} | {cell(row["game_vs_shot_db"])} | '
                         + (f'{row["fit_gain_db"]:.1f} dB ({row["fit_corr"]:.2f}, {row["fitted_cues"]} cue(s))'
                            + (' (file guessed)' if row['file_guessed'] else '') + (' (clipped)' if row['fit_clipped'] else '')
                            if row['fit_gain_db'] is not None else '-')
                         + f' | {cell(row["applied_minus_map_db"])} | '
                         + (f'{cell(row["off_by_db"])} ({row["off_by_from"]})' if row['off_by_db'] is not None else '-') + ' |')
        reference = next((r for r in levels if r['role'] == REFERENCE_ROLE), None)
        if fits is not None and (reference is None or reference['applied_minus_map_db'] is None):
            shots = sum(1 for c in run['cues'] if c['kind'] == 'sound' and c['name'] == REFERENCE_ROLE)
            if not shots:
                why = 'there is no cue of it'
            elif reference is not None and reference['fit_gain_db'] is not None:
                why = 'it fitted, but its file has no game gain (null in the play map), so there is nothing to compare'
            else:
                why = (f'none of its {shots} cue(s) fitted: another of its cues within {FIT_SEARCH_S + FIT_LEAD_S:.2f} s, no level measured, '
                       'no file matched, or a correlation under the floor')
            lines += ['', f'**No "Off by" in this table: the reference role `{REFERENCE_ROLE}` gives no balance in this recording** ({why}).'
                          ' "Applied - map" per role still holds.']
    lines += ['', "Maya's own sounds play 2D at the game's static level for each file (the play map's gains; 0 dB when it has none). The door and Marcus's lines come from where they are, with the engine's default falloff (UNVERIFIED against the game), and the recording is the mix at Maya's camera, so their levels depend on her distance. No RTPC, no ducking, no reverb. If something sounds too loud or too quiet next to the game, that is part of what to note.", '',
              '## Step by step', '']
    for n, step in enumerate(steps, 1):
        lines.append(f'### {n}. {mmss(step["t"])} to {mmss(step["end"])}: {step["name"]}')
        lines.append('')
        shown = STEPS_WITH_FIXTURE.get(step['name']) if run['magazine_fixture'] else None
        lines.append(f'On screen: {shown or STEPS.get(step["name"], "(no description)")}.' + (f' Frame: `frames/{step["frame"]}`.' if step['frame'] else ''))
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
                hear = describe(role)
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
    try:
        checked_out(args.out)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
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
