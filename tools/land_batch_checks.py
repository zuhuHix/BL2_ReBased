"""The verdict on one required check for tools/land_batch.sh, from GitHub's raw JSON.

AI-assisted (Claude Code), 2026-11-12. The landing script fetches, for the chain's tip,
  gh api "repos/<repo>/commits/<sha>/check-runs?check_name=<name>&filter=latest&per_page=100"
  gh api "repos/<repo>/actions/runs?head_sha=<sha>&per_page=100"
saves both answers to files and asks this script, which prints one line, "<verdict><TAB><detail><TAB><link>":

  success  every run counted below has completed with success, and at least one of them came from the pull_request
           event (the run GitHub attaches to the PR's head). UNVERIFIED: that a ruleset counts that run and not one
           started by hand is a reading of GitHub's documentation, not tried; if it is wrong, demanding a
           pull_request run is only stricter than GitHub.
  failure  a counted run completed with any other conclusion (failure, cancelled, timed_out, action_required, neutral,
           skipped, stale): main must not move, whatever the other runs say
  pending  a counted run has not completed yet (queued or in progress, with or without a start time)
  none     no run is counted yet, or only runs that do not come from the pull_request event
  error    an answer could not be read or judged (any exception, with its message)

Counted: check runs with this exact name, from the GitHub Actions app, on this exact commit, and not completed more
than seven days ago. UNVERIFIED: seven days as GitHub's limit for a required check is a reading of its documentation,
not tried; if GitHub accepts older runs, leaving them out only makes the script wait where GitHub would not. A run's
event comes from the workflow run of its check suite. Older runs are left out, and the detail says how many.
"""
import argparse
import datetime
import json
import sys

MAX_AGE = datetime.timedelta(days=7)


def parse_time(text):
    return datetime.datetime.strptime(text, '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=datetime.timezone.utc)


def verdict(check_runs, workflow_runs, sha, name, now):
    """(verdict, detail, link) for the parsed JSON answers."""
    try:
        runs = check_runs['check_runs']
        events = {w['check_suite_id']: w.get('event') for w in workflow_runs['workflow_runs']}
    except (KeyError, TypeError) as error:
        return 'error', 'unexpected answer from GitHub (%s missing)' % error, ''
    counted, old = [], 0
    for run in runs:
        if run.get('name') != name or run.get('head_sha') != sha or (run.get('app') or {}).get('slug') != 'github-actions':
            continue
        when = run.get('completed_at') or run.get('started_at')
        if run.get('status') == 'completed' and when and now - parse_time(when) > MAX_AGE:
            old += 1
            continue
        run['event'] = events.get((run.get('check_suite') or {}).get('id'), 'unknown')
        counted.append(run)
    left_out = ' (%d run(s) older than 7 days left out)' % old if old else ''
    for run in counted:
        if run.get('status') == 'completed' and run.get('conclusion') != 'success':
            return ('failure', '%s, event %s%s' % (run.get('conclusion') or 'no conclusion', run['event'], left_out),
                    run.get('html_url') or '')
    for run in counted:
        if run.get('status') != 'completed':
            return 'pending', '%s, event %s%s' % (run.get('status'), run['event'], left_out), run.get('html_url') or ''
    from_pr = [run for run in counted if run['event'] == 'pull_request']
    if not from_pr:
        events_seen = ', '.join(sorted(set(run['event'] for run in counted))) or 'no run'
        return 'none', 'no completed run from the pull_request event (seen: %s)%s' % (events_seen, left_out), ''
    return ('success', '%d run(s), %d from pull_request%s' % (len(counted), len(from_pr), left_out),
            from_pr[0].get('html_url') or '')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--sha', required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--check-runs', required=True, help='file with the check-runs answer')
    parser.add_argument('--workflow-runs', required=True, help='file with the actions/runs answer')
    parser.add_argument('--now', help='the time to judge ages against, YYYY-MM-DDTHH:MM:SSZ (default: now)')
    args = parser.parse_args(argv)
    now = parse_time(args.now) if args.now else datetime.datetime.now(datetime.timezone.utc)
    try:
        with open(args.check_runs, encoding='utf-8') as one, open(args.workflow_runs, encoding='utf-8') as two:
            result = verdict(json.load(one), json.load(two), args.sha, args.name, now)
    except (OSError, ValueError) as error:
        result = ('error', 'unreadable answer from GitHub: %s' % error, '')
    except Exception as error:  # an answer of an unexpected shape: say so now rather than wait for the deadline
        result = ('error', 'unexpected answer from GitHub (%s: %s)' % (type(error).__name__, error), '')
    sys.stdout.write('\t'.join(part.replace('\t', ' ').replace('\n', ' ') for part in result) + '\n')
    return 0


if __name__ == '__main__':
    sys.exit(main())
