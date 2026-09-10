import json,sys,re
rows=[json.loads(l) for l in open(sys.argv[1],encoding="utf-8") if l.strip()]
rows.sort(key=lambda r:-r.get("votes",0))
n=int(sys.argv[2]) if len(sys.argv)>2 else 15
maxlen=int(sys.argv[3]) if len(sys.argv)>3 else 700
for r in rows[:n]:
    t=re.sub(r"\s+"," ",r["text"]).strip()
    print(f"[+{r.get('votes',0)} | {r.get('hours',0)//60}h] {t[:maxlen]}")
    print("-"*80)
