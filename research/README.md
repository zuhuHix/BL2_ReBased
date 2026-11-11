# BL2_ReBased research scripts

The scripts behind [`docs/BL2_REMASTER_ANALYSIS.md`](../docs/BL2_REMASTER_ANALYSIS.md)
and the measurements in [`docs/OPENWILLOW_ENGINE_PLAN.md`](../docs/OPENWILLOW_ENGINE_PLAN.md).
Everything here is re-runnable; nothing here is a game file.

## Community demand (collected 2026-09-09)

### Where the corpus lives

The Steam reviews are other people's writing, so they are not tracked in the
repository (decision of 2026-11-11, `DECISIONS.md`). The fetch scripts write
them, and the analysis scripts read them, in one folder:

1. `--corpus DIR` on the command line, if given;
2. otherwise the `OW_RESEARCH_CORPUS` environment variable;
3. otherwise `local/research_corpus/` under the repository root, which git ignores.

`corpus_paths.py` implements that order. A reader given an empty folder stops
and names `fetch_steam_reviews.py`. To fetch the review files again (run from
`research/`; the counts below were what came back on 2026-09-09, and a new
fetch returns current reviews, not the same ones):

```
python fetch_steam_reviews.py 49520 reviews_bl2_recent.jsonl recent 15
python fetch_steam_reviews.py 49520 reviews_bl2_helpful.jsonl all 5
python fetch_neg.py
python fetch_steam_reviews.py 397540 reviews_bl3_helpful.jsonl all 3
python fetch_steam_reviews.py 1285190 reviews_bl4_recent.jsonl recent 10
```

The page counts and the BL3 and BL4 app ids are UNVERIFIED reconstructions:
the original command lines were not recorded. Commits made before 2026-11-11
still contain the 2026-09-09 files under `research/`.

| File | What | How |
|---|---|---|
| `reviews_bl2_recent.jsonl` (1,500), `reviews_bl2_helpful.jsonl` (477), `reviews_bl2_negative.jsonl` (126) | Borderlands 2 Steam reviews: review id, helpful votes, hours played, timestamp, text. Corpus folder, not tracked | Public Steam `appreviews` API via `fetch_steam_reviews.py` / `fetch_neg.py` |
| `reviews_bl3_helpful.jsonl` (242), `reviews_bl4_recent.jsonl` (1,000) | Comparison baselines. Corpus folder, not tracked | Same |
| `steam_topics_bl2.json` (247) | Steam Discussions thread titles and reply counts | `steam_forum.py` |
| `reddit_ddg.json`, `reddit_ddg_all.json` (226) | Search-index titles and snippets for Reddit threads | `ddg2.py`, `ddg3.py`, `ddg_reddit.py` with `queries.txt` / `queries_full.txt` |
| `analyze.py`, `topic_themes.py`, `aggregate.py`, `top.py` | Theme-frequency analysis and top-review listing. `analyze.py`, `aggregate.py` and `top.py` read the reviews from the corpus folder | n/a |

Review records carry no usernames or profile identifiers. The Reddit corpus
is query-seeded and therefore measures which topics exist, not how often they
are raised; see the analysis document's methodology section before quoting
percentages from it.

## Engine measurements

| File | What |
|---|---|
| `native_count.py` | Pure-Python UE3 package reader for version 832: LZO1X decompressor, compressed-chunk container, name/import/export tables, `UFunction` flag census. Parsed every code package with zero errors and matched Gearbox's own `.uncompressed_size` values byte-for-byte. It is the seed of the C++ reader and remains its independent comparison oracle (`tools/verify_packages.py`). |
| `native_by_class.json` | Per-class `[native, script, event]` function counts for the nine code packages: the numbers behind the 20,119 / 12,978 / 7,141 split. |

**Provenance.** The LZO1X decompressor in `native_count.py` is an
independent implementation written from the public instruction-format
description; an earlier miniLZO-derived version was removed on 2026-09-13
(record in [`THIRD_PARTY.md`](../THIRD_PARTY.md)). It is never linked into
the C++ reader, which uses the vendored MIT lzokay, and serves only as a
local comparison oracle.
