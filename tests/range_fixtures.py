"""Synthetic range-log lines for the leap and throw rows of tools/check_range.py (AI-assisted, Claude Code, 2026-11-13).

Shared by tests/range_tools_test.py and tests/range_kind_melee_test.py: a log in which every expected step passes needs leap and throw lines
that follow the note's rule, with the spawn records they are tied to. Invented enemy names and numbers; the arc numbers are worked out with
the checker's own solver, so a line is consistent by construction. Not a test file itself.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_range  # noqa: E402


def attack_spawn(enemy, kind):
    """The spawn record the host prints for an enemy of a kind with a row (AOpenWillowEnemy::SetupKind), from the checker's ATTACKS."""
    row = check_range.ATTACKS[kind]
    leap = row['leap'] or (0.0, 0.0, 0.0, 0.0, 0.0, False, 0.0, 0.0)
    throw = row['throw'] or (0.0, 0.0, 0.0, 0.0, 0.0, False, False, ())
    return ('OWRANGE enemy %s attacks kind=%s close=%.0f weights=%.2f:%.2f:%.2f leap=%.0f-%.0f leap_speed=%.0f leap_angle_pct=%.2f '
            'leap_lead_cap=%.0f leap_resource=%d leap_clips=%.3f:%.3f throw=%.0f-%.0f throw_speed=%.0f throw_angle_pct=%.2f throw_lead_cap=%.0f '
            'throw_front=%d throw_resource=%d throw_clips=%d anim=idle_stand-in hand=stand-in source=note'
            % ((enemy, kind, row['close']) + row['weights'] + leap[:2] + leap[2:5] + (leap[5],) + leap[6:8] + throw[:2] + throw[2:5]
               + (throw[5], throw[6], len(throw[7]))))


def leap_detail(kind='bullymong', enemy='E_0', distance=1500.0, dz=10.0, lead=0.0, windup=0.840, late=0.010, max_frame=0.017, **override):
    """A leap PASS detail that follows the note's rule; override replaces fields by name (a value of None drops the field)."""
    low, high, speed, angle_pct, lead_cap, _, launch, _ = check_range.ATTACKS[kind]['leap']
    d1 = distance + lead
    _, _, flight = check_range.solve_arc(distance, dz, d1, speed, angle_pct)
    fields = {'kind': kind, 'enemy': enemy, 'forced': '1', 'start_distance': '%.0f' % distance, 'band_min': '%.0f' % low, 'band_max': '%.0f' % high,
              'windup': '%.3f' % windup, 'launch_clip': '%.3f' % launch, 'd0': '%.1f' % distance, 'dz': '%.1f' % dz, 'd1': '%.1f' % d1,
              'lead': '%.1f' % lead, 'speed': '%.0f' % speed, 'angle_pct': '%.3f' % angle_pct, 'lead_cap': '%.0f' % lead_cap,
              'flight_predicted': '%.4f' % flight, 'flight_measured': '%.4f' % (flight + late), 'attack_to_landed': '%.3f' % (windup + flight + late),
              'land_error': '0.0', 'hit': '1', 'expected': '24.98', 'applied': '24.98', 'pools_lost': '24.98', 'knock': '1000',
              'max_frame': '%.3f' % max_frame}
    return _join(fields, override)


def throw_detail(kind='bullymong_adult', enemy='E_3', distance=1500.0, dz=-100.0, release=1.070, release_due=1.064, early=0.030, max_frame=0.017,
                 **override):
    """A throw PASS detail that follows the note's rule (a rock that touched a Maya standing on its aim point)."""
    low, high, speed, angle_pct = check_range.ATTACKS[kind]['throw'][:4]
    d0 = distance - 60.0   # the hand is ahead of the thrower's origin
    _, _, flight = check_range.solve_arc(d0, dz, d0, speed, angle_pct)
    fields = {'kind': kind, 'enemy': enemy, 'forced': '1', 'start_distance': '%.0f' % distance, 'band_min': '%.0f' % low, 'band_max': '%.0f' % high,
              'release': '%.3f' % release, 'release_due': '%.3f' % release_due, 'd0': '%.1f' % d0, 'dz': '%.1f' % dz, 'd1': '%.1f' % d0, 'lead': '0.0',
              'speed': '%.0f' % speed, 'angle_pct': '%.3f' % angle_pct, 'flight_predicted': '%.4f' % flight,
              'flight_measured': '%.4f' % (flight - early), 'what': 'maya', 'miss': '55.0', 'hit': '1', 'expected': '73.70', 'applied': '73.70',
              'pools_lost': '73.70', 'knock': '500', 'max_frame': '%.3f' % max_frame}
    return _join(fields, override)


def _join(fields, override):
    fields = dict(fields)
    for key, value in override.items():
        if value is None:
            fields.pop(key, None)
        else:
            fields[key] = value
    return ' '.join('%s=%s' % pair for pair in fields.items())


# The default range: three regulars (E_0 to E_2) and one adult (E_3). Both kinds leap; the adult throws.
DEFAULT_SPAWNS = [attack_spawn('E_%d' % i, 'bullymong') for i in range(3)] + [attack_spawn('E_3', 'bullymong_adult')]


def ranged_passing(step):
    """The passing lines of the leap or throw step on the default range, with the spawn records they are tied to."""
    if step == 'leap':
        lines = ['OWRANGETEST step=leap result=PASS ' + leap_detail(),
                 'OWRANGETEST step=leap result=PASS ' + leap_detail('bullymong_adult', 'E_3', expected='73.70', applied='73.70', pools_lost='73.70')]
    else:
        lines = ['OWRANGETEST step=throw result=PASS ' + throw_detail()]
    return '\n'.join(DEFAULT_SPAWNS + lines)
