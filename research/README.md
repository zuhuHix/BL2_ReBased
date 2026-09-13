# Research corpus

The data behind [`docs/BL2_REMASTER_ANALYSIS.md`](../docs/BL2_REMASTER_ANALYSIS.md)
and the measurements in [`docs/OPENWILLOW_ENGINE_PLAN.md`](../docs/OPENWILLOW_ENGINE_PLAN.md).
Everything here is re-runnable; nothing here is a game file.

## Community demand (collected 2026-09-09)

| File | What | How |
|---|---|---|
| `reviews_bl2_recent.jsonl` (1,500), `reviews_bl2_helpful.jsonl` (477), `reviews_bl2_negative.jsonl` (126) | Borderlands 2 Steam reviews: review id, helpful votes, hours played, timestamp, text | Public Steam `appreviews` API via `fetch_steam_reviews.py` / `fetch_neg.py` |
| `reviews_bl3_helpful.jsonl` (242), `reviews_bl4_recent.jsonl` (1,000) | Comparison baselines | Same |
| `steam_topics_bl2.json` (247) | Steam Discussions thread titles and reply counts | `steam_forum.py` |
| `reddit_ddg.json`, `reddit_ddg_all.json` (226) | Search-index titles and snippets for Reddit threads | `ddg2.py`, `ddg3.py`, `ddg_reddit.py` with `queries.txt` / `queries_full.txt` |
| `analyze.py`, `topic_themes.py`, `aggregate.py`, `top.py` | Theme-frequency analysis and top-review listing | — |

Review records carry no usernames or profile identifiers. The Reddit corpus
is query-seeded and therefore measures which topics exist, not how often they
are raised — see the analysis document's methodology section before quoting
percentages from it.

## Engine measurements

| File | What |
|---|---|
| `native_count.py` | Pure-Python UE3 package reader for version 832: LZO1X decompressor, compressed-chunk container, name/import/export tables, `UFunction` flag census. Parsed every code package with zero errors and matched Gearbox's own `.uncompressed_size` values byte-for-byte. It is the seed of the C++ reader and remains its independent comparison oracle (`tools/verify_packages.py`). |
| `native_by_class.json` | Per-class `[native, script, event]` function counts for the nine code packages — the numbers behind the 20,119 / 12,978 / 7,141 split. |

**Provenance caveat.** `native_count.py` describes its decompressor as a
faithful port of miniLZO's `lzo1x_decompress`. miniLZO is GPL-2.0-or-later,
and the exact upstream version and authorship of that translation are not
recorded. It is never linked into the C++ reader (which uses the MIT lzokay)
and is used only as a local comparison oracle. Its status is tracked in
[`THIRD_PARTY.md`](../THIRD_PARTY.md) and is the open item gating the
project-wide license choice.
