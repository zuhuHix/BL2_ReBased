"""Synthetic checks for the research scripts' corpus folder (research/corpus_paths.py).

The corpus itself is not tracked. These checks write a few invented reviews into a temporary folder, run the
analysis scripts against it, and check that an empty folder fails with a message naming the fetch script.
No fetched text and no game data.
"""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]
RESEARCH = REPO / 'research'

# Invented reviews: not copied from any source.
REVIEWS = [
    {'id': '1', 'up': True, 'votes': 9, 'funny': 0, 'hours': 600, 'ts': 1700000000,
     'text': 'Invented review one: the loot and the co-op are great.'},
    {'id': '2', 'up': False, 'votes': 3, 'funny': 0, 'hours': 120, 'ts': 1700000100,
     'text': 'Invented review two: it crashes and the inventory is a mess.'},
]

failures = []


def check(name, cond, detail=''):
    if not cond:
        failures.append(f'{name}: {detail}')
    print(('ok   ' if cond else 'FAIL ') + name)


def run(script, *args, env_dir=None, cwd):
    env = dict(os.environ)
    env.pop('OW_RESEARCH_CORPUS', None)
    env['PYTHONIOENCODING'] = 'utf-8'
    if env_dir is not None:
        env['OW_RESEARCH_CORPUS'] = str(env_dir)
    return subprocess.run([sys.executable, str(RESEARCH / script)] + list(args), capture_output=True,
                          text=True, encoding='utf-8', env=env, cwd=str(cwd))


def write_reviews(folder, name):
    with open(folder / name, 'w', encoding='utf-8') as f:
        for r in REVIEWS:
            f.write(json.dumps(r) + '\n')


def output_checks():
    """Fetchers refuse to write inside the repository outside local/ (no network: zero pages)."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        stray = RESEARCH / 'local'
        existed = stray.exists()
        r = run('fetch_steam_reviews.py', '49520', 'x.jsonl', 'recent', '0', '--corpus', 'local/x', cwd=RESEARCH)
        check('relative --corpus inside research/ is refused',
              r.returncode != 0 and 'Refusing' in r.stderr and (existed or not stray.exists()), r.stdout + r.stderr)
        if stray.exists() and not existed:
            import shutil
            shutil.rmtree(stray)
        r = run('fetch_steam_reviews.py', '49520', 'x.jsonl', 'recent', '0', '--corpus=', cwd=tmp)
        check('empty --corpus= is refused', r.returncode != 0 and 'needs a folder' in r.stderr, r.stdout + r.stderr)
        target = RESEARCH / 'research_corpus_test_out.jsonl'
        r = run('fetch_steam_reviews.py', '49520', str(target), 'recent', '0', cwd=tmp)
        check('absolute output path in the tree is refused',
              r.returncode != 0 and 'Refusing' in r.stderr and not target.exists(), r.stdout + r.stderr)
        if target.exists():
            target.unlink()
        if os.name == 'nt':
            r = run('fetch_steam_reviews.py', '49520', '\\\\?\\' + str(target), 'recent', '0', cwd=tmp)
            check('output path with the \\\\?\\ prefix in the tree is refused',
                  r.returncode != 0 and 'Refusing' in r.stderr and not target.exists(), r.stdout + r.stderr)
            if target.exists():
                target.unlink()
        r = run('fetch_steam_reviews.py', '49520', 'x.jsonl', 'recent', '0', '--corpus', str(tmp / 'out'), cwd=tmp)
        check('a folder outside the repository is accepted',
              r.returncode == 0 and (tmp / 'out' / 'x.jsonl').exists(), r.stdout + r.stderr)
        a_file = tmp / 'not_a_folder'
        a_file.write_text('', encoding='utf-8')
        r = run('analyze.py', env_dir=a_file, cwd=tmp)
        check('a corpus path that is a file is named as such',
              r.returncode != 0 and 'is a file, not a folder' in r.stderr, r.stdout + r.stderr)


def main():
    output_checks()
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        full = tmp / 'full'
        empty = tmp / 'empty'
        work = tmp / 'work'
        for d in (full, empty, work):
            d.mkdir()
        for name in ('reviews_bl2_recent.jsonl', 'reviews_bl2_helpful.jsonl', 'reviews_bl2_negative.jsonl'):
            write_reviews(full, name)

        r = run('analyze.py', '--corpus', str(full), cwd=work)
        check('analyze reads --corpus', r.returncode == 0 and '(n=2)' in r.stdout, r.stdout + r.stderr)
        r = run('analyze.py', env_dir=full, cwd=work)
        check('analyze reads OW_RESEARCH_CORPUS', r.returncode == 0 and '(n=2)' in r.stdout, r.stderr)
        r = run('analyze.py', '--corpus', str(empty), cwd=work)
        check('analyze fails on an empty folder',
              r.returncode != 0 and 'research/fetch_steam_reviews.py' in r.stderr, r.stdout + r.stderr)

        r = run('aggregate.py', f'--corpus={full}', cwd=work)
        check('aggregate reads --corpus=', r.returncode == 0 and 'n=2' in r.stdout, r.stdout + r.stderr)
        r = run('aggregate.py', env_dir=empty, cwd=work)
        check('aggregate fails on an empty folder',
              r.returncode != 0 and 'research/fetch_steam_reviews.py' in r.stderr, r.stdout + r.stderr)

        r = run('top.py', 'reviews_bl2_recent.jsonl', '1', '--corpus', str(full), cwd=work)
        check('top lists the most-voted invented review', r.returncode == 0 and 'Invented review one' in r.stdout
              and 'Invented review two' not in r.stdout, r.stdout + r.stderr)
        r = run('top.py', 'reviews_bl2_recent.jsonl', '--corpus', str(empty), cwd=work)
        check('top fails on an empty folder',
              r.returncode != 0 and 'research/fetch_steam_reviews.py' in r.stderr, r.stdout + r.stderr)

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        full = tmp / 'full'
        empty = tmp / 'empty'
        for d in (full, empty):
            d.mkdir()
        # Invented thread titles.
        topics = [{'title': 'Invented thread: co-op will not connect', 'replies': 12},
                  {'title': 'Invented thread: best build for the skill tree', 'replies': 4}]
        (full / 'steam_topics_bl2.json').write_text(json.dumps(topics), encoding='utf-8')
        r = run('topic_themes.py', '--corpus', str(full), cwd=tmp)
        check('topic_themes reads the corpus folder', r.returncode == 0 and 'recent 2 threads' in r.stdout,
              r.stdout + r.stderr)
        r = run('topic_themes.py', env_dir=empty, cwd=tmp)
        check('topic_themes fails on an empty folder',
              r.returncode != 0 and 'research/steam_forum.py' in r.stderr, r.stdout + r.stderr)

    if failures:
        print(f'{len(failures)} failure(s)')
        for f in failures:
            print('  ' + f)
        return 1
    print('all research corpus checks passed')
    return 0


if __name__ == '__main__':
    sys.exit(main())
