"""Grade one tools/test_audio.ps1 run: the exact expected rows, each once and PASS, the tone's play line, a matching summary, and a clean editor exit.

A summary of "result=PASS" alone would pass a run that lost a row or renamed one, and a crash during teardown comes after every row. A pass says the host called play and the mixer pulled
the queued bytes of a synthetic tone. It does not say anyone heard it, or anything about a game sound.

Usage: python tools/check_audio.py <log> <editor exit code, or "timeout">. Exit 0 pass, 1 fail.
"""
from pathlib import Path
import re
import sys

EXPECTED = ('parse_good_wav', 'parse_odd_chunk_padded', 'refuse_truncated_data', 'refuse_riff_size_zero', 'refuse_chunk_past_riff_size',
            'refuse_non_pcm', 'refuse_zero_rate', 'refuse_extreme_rate', 'refuse_short_header', 'refuse_duplicate_fmt', 'refuse_duplicate_data',
            'refuse_chunk_after_data_past_riff_size', 'refuse_partial_chunk_at_riff_end', 'parse_odd_final_chunk_padded', 'refuse_odd_final_chunk_no_pad',
            'accept_at_the_limits', 'refuse_file_over_limit',
            'refuse_total_over_limit', 'unknown_role_plays_nothing', 'null_world_plays_nothing', 'tone_accepted_by_engine', 'mixer_pulled_all_bytes')
ROW = re.compile(r'OWAUDIOTEST (PASS|FAIL) (\S+)(.*)')
SUMMARY = re.compile(r'OWAUDIOTEST SUMMARY result=(\S+) passed=(\d+) failed=(\d+)')
PLAY = re.compile(r'OWAUDIO play role=test_tone channels=\d+ rate=\d+ bytes=(\d+)')
DRAINED = re.compile(r'queued=(-?\d+) left=(-?\d+)')


def grade(text, editor):
    """Returns (rows, problems). rows: (name, result, detail) as logged, in order. editor: the editor's exit code, or 'timeout'."""
    rows = [(m.group(2), m.group(1), m.group(3).strip()) for m in ROW.finditer(text)]
    problems = []
    names = [name for name, _, _ in rows]
    for name in EXPECTED:
        count = names.count(name)
        if count != 1:
            problems.append(f'row {name} appears {count} times, expected once')
    for name in sorted(set(names) - set(EXPECTED)):
        problems.append(f'unexpected row {name}')
    for name, result, detail in rows:
        if result != 'PASS':
            problems.append(f'row {name} {result}: {detail}')
    plays = PLAY.findall(text)
    if len(plays) != 1:
        problems.append(f'{len(plays)} tone play lines, expected one')
    # The row's own numbers, so a pass with nothing queued cannot slip through.
    detail = next((detail for name, _, detail in rows if name == 'mixer_pulled_all_bytes'), '')
    drained = DRAINED.search(detail)
    if not drained or int(drained.group(1)) <= 0 or int(drained.group(2)) != 0:
        problems.append(f'mixer row does not show queued>0 and left=0: {detail!r}')
    if editor == 'timeout':
        problems.append('editor did not exit within the limit after the summary')
    elif editor != '0':
        problems.append(f'editor exited with {editor}, expected 0')
    summaries = SUMMARY.findall(text)
    if len(summaries) != 1:
        problems.append(f'{len(summaries)} summaries, expected one')
    elif summaries[0] != ('PASS', str(len(EXPECTED)), '0'):
        problems.append(f'summary result={summaries[0][0]} passed={summaries[0][1]} failed={summaries[0][2]}, expected result=PASS passed={len(EXPECTED)} failed=0')
    return rows, problems


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    rows, problems = grade(Path(sys.argv[1]).read_text(encoding='utf-8', errors='replace'), sys.argv[2])
    for name, result, detail in rows:
        print(f'{result:<5} {name:<30} {detail}')
    for problem in problems:
        print(f'PROBLEM {problem}')
    print(f'CHECK_AUDIO {"PASS" if not problems else "FAIL"} rows={len(rows)} expected={len(EXPECTED)} editor={sys.argv[2]} problems={len(problems)} (nothing was listened to)')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
