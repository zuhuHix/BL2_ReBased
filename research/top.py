import json,sys,re,os
import corpus_paths
# Usage: top.py FILE [N] [MAXLEN] [--corpus DIR]
# FILE is a review file name in the corpus folder (see corpus_paths.py) or a path.
CORPUS=corpus_paths.corpus_dir()
if len(sys.argv)<2: sys.exit("usage: top.py FILE [N] [MAXLEN] [--corpus DIR]")
path=corpus_paths.resolve(CORPUS,sys.argv[1])
if not os.path.isfile(path): corpus_paths.missing_corpus(CORPUS,[sys.argv[1]],"research/fetch_steam_reviews.py")
rows=[json.loads(l) for l in open(path,encoding="utf-8") if l.strip()]
rows.sort(key=lambda r:-r.get("votes",0))
n=int(sys.argv[2]) if len(sys.argv)>2 else 15
maxlen=int(sys.argv[3]) if len(sys.argv)>3 else 700
for r in rows[:n]:
    t=re.sub(r"\s+"," ",r["text"]).strip()
    print(f"[+{r.get('votes',0)} | {r.get('hours',0)//60}h] {t[:maxlen]}")
    print("-"*80)
