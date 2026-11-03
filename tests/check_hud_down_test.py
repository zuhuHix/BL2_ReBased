"""Synthetic tests for tools/check_hud_down.py (no game or engine needed; the logs are generated here).

Every value is invented: an injury at t=10.000, Maya's maximum health 400, a strings file with 5 strings, a title
"TEST TITLE" (also the strings file's), a mission "Test Mission" with the objectives "Toy step one", "Toy side step"
(optional) and "Toy step two", captures drawn here (grey frames, with a red bar where the down panel would be), and run ids
made up for these tests. The expected times are
worked out by hand from the host's constants (2.28 s, a 12 s first bleed-out, 5.76 s), not computed by the checker:
bled out at 10.000 + 2.28 + 12 = 24.280, respawn at 24.280 + 5.76 = 30.040.
"""
import atexit
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_hud_down as chk  # noqa: E402
from check_hud_shots_test import png  # noqa: E402  (the synthetic PNG writer)

STRINGS = tempfile.NamedTemporaryFile('w', suffix='.json', delete=False, encoding='utf-8')
json.dump({'injured': 'TEST TITLE'}, STRINGS)
STRINGS.close()
atexit.register(lambda: Path(STRINGS.name).unlink(missing_ok=True))

PREFIX = 'LogTemp: Display: '
CONSOLE = 'LogTemp: Display: OpenWillow Flash HUD console: '
OWID = 'A1B2C3D4E5F60718293A4B5C6D7E8F90'
ENDING = f' (http://127.0.0.1:8795/?owid={OWID}:140)'
HEAD = [PREFIX + f'OpenWillow Flash HUD overlay: http://127.0.0.1:8795/?owid={OWID}',
        CONSOLE + 'OpenWillow HUD strings 5' + ENDING,
        CONSOLE + f'OpenWillow HUD movie ready id={OWID} pages=C:\\Test\\tools\\hud_overlay' + ENDING,
        PREFIX + 'OpenWillow HUD path: movie (page reported ready)']
TITLE = 'TEST\\u0020TITLE'


def down_state(t, phase, bleed, wind, page=True, bar=None, dying=None):
    """A host OWHUDDOWN line and, unless page is False, the page's echo with its frames (bar frame 1 + round((1-b)*99))."""
    lines = [PREFIX + f'OWHUDDOWN t={t:.3f} phase={phase} bleed={bleed:.2f} secondWind={wind}']
    if page:
        injured = phase == 'injured'
        frame = bar if bar is not None else (1 + round((1 - bleed) * 99) if injured else '-')
        lines.append(CONSOLE + f'OWHUDDOWNPAGE phase={phase} bleed={bleed:.2f} secondWind={wind} '
                     f'dying={dying or ("2" if injured else "1")} bleedout={frame} title={TITLE if injured else "-"} '
                     f'windFrame={"12" if wind else "1"}' + ENDING)
    return lines


def down_log(bled_at=24.280, respawn_at=30.040, wind_health=100, skip_page_at=None, bad_bar=False):
    lines = HEAD + down_state(0.0, 'up', 1.0, 0)
    lines.append(PREFIX + 'OpenWillow Maya down t=10.000 bleedout=12.00s successive=0')
    lines += down_state(10.0, 'injured', 1.0, 0)
    # The bar falls from 1.00 to 0.01 over the 12 s after the 2.28 s delay: one state per hundredth.
    for step in range(1, 100):
        bleed = 1 - step / 100
        lines += down_state(12.28 + step * 0.12, 'injured', bleed, 0, page=step != skip_page_at,
                            bar=50 if bad_bar and step == 40 else None)
    lines.append(PREFIX + f'OpenWillow Maya bled out t={bled_at:.3f}')
    lines += down_state(bled_at, 'dead', 0.0, 0)
    lines.append(PREFIX + 'OpenWillow Maya died and respawned')
    lines += down_state(respawn_at, 'up', 1.0, 0)
    lines.append(PREFIX + 'OpenWillow Maya down t=33.000 bleedout=12.00s successive=0')
    lines += down_state(33.0, 'injured', 1.0, 0)
    lines.append(PREFIX + f'OpenWillow Maya second wind t=36.000 health {wind_health}/400')
    lines += down_state(36.0, 'up', 1.0, 1)
    return '\n'.join(f'[2026.01.01-00.00.00:000][  1]{line}' for line in lines)


def results(text, mode='down', strings=STRINGS.name):
    lines = text.splitlines()
    found = [chk.check_path(lines), chk.check_identity(lines)]
    found += chk.check_down(lines, strings) if mode == 'down' else chk.check_tracker(lines)
    return {name: (ok, detail) for name, ok, detail in found}


def failed(found):
    return {name for name, (ok, _) in found.items() if not ok}


def track_state(rest, page_modules):
    lines = [PREFIX + f'OWHUDTRACK t=1.000 {rest}']
    if page_modules is not None:
        lines.append(CONSOLE + f'OWHUDTRACKPAGE {page_modules}' + ENDING)
    return lines


# Modules as the page reports them: <module frame>/<inner frame>/<escaped text>; 1/... is a module off.
OFF = ' '.join(f'm{k}=1/1/-' for k in range(1, 7))


def modules(*shown):
    parts = [f'm{k}=5/{frame}/{text}' for k, (frame, text) in enumerate(shown, 1)]
    parts += [f'm{k}=1/1/-' for k in range(len(shown) + 1, 7)]
    return ' '.join(parts)


def tracker_log(second_text='Toy\\u0020step\\u0020two', side_frame=23):
    """Objective one, then one done with an optional side step and objective two, then the turn-in. side_frame is the inner
    frame the page reports for the optional line (23 `Optional`; 8 would be `NotComplete`)."""
    name = (29, 'Test\\u0020Mission')
    one = 'Toy\\u0020step\\u0020one'
    lines = HEAD[:1] + HEAD[2:]
    lines += track_state('none', 'mission=- turnIn=0 lines=0 ' + OFF)
    lines += track_state('mission=Test Mission turnIn=0 lines=[ ]Toy step one',
                         'mission=Test\\u0020Mission turnIn=0 lines=1 ' + modules(name, (8, one)))
    lines += track_state('mission=Test Mission turnIn=0 lines=[x]Toy step one|[o]Toy side step|[ ]Toy step two',
                         'mission=Test\\u0020Mission turnIn=0 lines=3 '
                         + modules(name, (1, one), (side_frame, 'Toy\\u0020side\\u0020step'), (8, second_text)))
    lines += track_state('mission=Test Mission turnIn=1 lines=',
                         'mission=Test\\u0020Mission turnIn=1 lines=0 ' + modules(name, (51, 'Turn\\u0020in')))
    return '\n'.join(lines)


def down_shots(folder, mid_bar=(450, 710), up_bar=None):
    """The six down captures: grey frames; down_start's bar spans x 450-840, down_mid's mid_bar (None: no panel);
    up_bar draws a bar into the `up` capture too (a panel where none should be)."""
    bars = {'down_start': (450, 840), 'down_mid': mid_bar, 'up': up_bar}
    for name in chk.DOWN_SHOTS:
        span = bars.get(name)
        png(Path(folder) / f'OWHudDown_{name}.png',
            boxes=[(span[0], 490, span[1], 550, (210, 30, 30))] if span else [], shade=False)


class DownTest(unittest.TestCase):
    def test_matching_run_passes(self):
        found = results(down_log())
        self.assertEqual(failed(found), set(), found)

    def test_late_bleed_out_fails_timing(self):
        # Bled out 0.3 s late (24.580 against 24.280): more than the 0.1 s allowed.
        self.assertIn('down_timing', failed(results(down_log(bled_at=24.580, respawn_at=30.340))))

    def test_late_respawn_fails_timing(self):
        self.assertIn('down_timing', failed(results(down_log(respawn_at=30.300))))

    def test_second_wind_health_off_fails(self):
        # 25 % of 400 is 100; 120 is more than 1 point off.
        self.assertEqual(failed(results(down_log(wind_health=120))), {'second_wind_health'})

    def test_missing_page_echo_fails(self):
        self.assertEqual(failed(results(down_log(skip_page_at=50))), {'down_page_echo'})

    def test_wrong_bar_frame_fails(self):
        # bleed 0.60 is frame 1 + round(0.4 * 99) = 41, not 50.
        found = results(down_log(bad_bar=True))
        self.assertEqual(failed(found), {'down_page_echo'})
        self.assertIn('expected 41', found['down_page_echo'][1])

    def test_without_strings_file_fails_texts_as_missing_data(self):
        found = results(down_log().replace('OpenWillow HUD strings 5', 'OpenWillow HUD strings none'))
        self.assertEqual(failed(found), {'down_page_texts'})
        self.assertIn('local data missing', found['down_page_texts'][1])

    def test_no_second_wind_fails_sequence(self):
        text = '\n'.join(l for l in down_log().splitlines() if 'second wind' not in l and 'secondWind=1' not in l)
        self.assertTrue({'down_sequence', 'second_wind_health', 'down_states', 'second_wind_banner'} <= failed(results(text)))

    def test_title_not_the_strings_files_fails(self):
        # The page shows another title than the strings file's `injured` value.
        found = results(down_log().replace(TITLE, 'OTHER\\u0020TITLE'))
        self.assertEqual(failed(found), {'down_page_texts'})
        self.assertIn('NOT equal', found['down_page_texts'][1])

    def test_unreadable_strings_file_fails_as_missing_data(self):
        found = results(down_log(), strings=str(Path(STRINGS.name).with_suffix('.missing')))
        self.assertEqual(failed(found), {'down_page_texts'})
        self.assertIn('local data missing', found['down_page_texts'][1])

    def test_canvas_path_fails(self):
        text = down_log().replace('HUD path: movie (page reported ready)', 'HUD path: canvas (forced)')
        self.assertIn('movie_path', failed(results(text)))


class DownShotsTest(unittest.TestCase):
    def shots(self, **bars):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        down_shots(folder.name, **bars)
        return chk.check_down_shots(folder.name)

    def test_panel_where_expected_passes(self):
        name, ok, detail = self.shots()
        self.assertTrue(ok, detail)

    def test_valid_png_without_the_panel_fails(self):
        name, ok, detail = self.shots(mid_bar=None)
        self.assertFalse(ok)
        self.assertIn('down_mid', detail)

    def test_panel_in_the_up_capture_fails(self):
        name, ok, detail = self.shots(up_bar=(450, 840))
        self.assertFalse(ok)
        self.assertIn('up:', detail)

    def test_bar_not_draining_fails(self):
        name, ok, detail = self.shots(mid_bar=(450, 840))
        self.assertFalse(ok)
        self.assertIn('not shorter', detail)

    def test_missing_capture_fails(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        down_shots(folder.name)
        (Path(folder.name) / 'OWHudDown_dead.png').unlink()
        self.assertFalse(chk.check_down_shots(folder.name)[1])


class TrackerTest(unittest.TestCase):
    def test_matching_run_passes(self):
        found = results(tracker_log(), 'tracker')
        self.assertEqual(failed(found), set(), found)

    def test_optional_shown_as_not_complete_fails(self):
        self.assertEqual(failed(results(tracker_log(side_frame=8), 'tracker')), {'tracker_page'})

    def test_wrong_objective_text_fails(self):
        found = results(tracker_log(second_text='Toy\\u0020other'), 'tracker')
        self.assertEqual(failed(found), {'tracker_page'})

    def test_no_turn_in_fails_progress(self):
        text = '\n'.join(tracker_log().splitlines()[:-2])
        self.assertIn('tracker_progress', failed(results(text, 'tracker')))


if __name__ == '__main__':
    unittest.main()
