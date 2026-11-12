## 2026-11-11: Third-party text for research is fetched locally, not tracked (lane-research-corpus)

AI-assisted (Claude Opus 5.5). Repository contents and research scripts only: no tool, parser or host behaviour changes. Maintainer decision of 2026-11-11.

- **Policy.** Text other people wrote (Steam reviews and similar) is fetched into an ignored local folder and is not tracked. The scripts that fetch and analyse it stay in `research/`. History is not rewritten, so commits before this one still contain the earlier files.
- **Why.** An audit pointed out that no rule in the repository covered third-party personal data (review ids, playtime) or the licence of the text, and nothing in the build uses it.
- **What.** The five Steam review files (3,345 reviews) left `research/`. `research/corpus_paths.py` picks the folder: `--corpus DIR`, then `OW_RESEARCH_CORPUS`, then `local/research_corpus/` under the repository root. `analyze.py`, `aggregate.py` and `top.py` read the reviews from there and stop with a message naming `research/fetch_steam_reviews.py` when the folder has none; `fetch_steam_reviews.py` and `fetch_neg.py` write there. `research/README.md` says how to fetch again; the page counts and app ids in those commands are UNVERIFIED, because the original command lines were not recorded.
- **Verified (automated):** `tests/research_corpus_test.py` (new, invented reviews, registered as `py-research_corpus`). Each changed reader was run once against the moved files and once against an empty folder.
- **Not verified:** that a fresh fetch reproduces the 2026-09-09 counts; it will not, because Steam returns current reviews.
- **Forum and search snippets (separate commit).** The same for `steam_topics_bl2.json`, `reddit_ddg.json` and `reddit_ddg_all.json`: `topic_themes.py` and `aggregate.py` read them from the corpus folder (`topic_themes.py` names `research/steam_forum.py` when the file is missing); `steam_forum.py`, `ddg2.py`, `ddg3.py` and `ddg_reddit.py` write there. The query lists `queries.txt` and `queries_full.txt` are the project's own and stay tracked.

