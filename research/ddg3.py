import urllib.request, urllib.parse, re, html, time, json, random, sys
UAS=["Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0 Safari/537.36",
     "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
     "Mozilla/5.0 (X11; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0"]
def fetch(q, lite=True):
    url=("https://lite.duckduckgo.com/lite/?q=" if lite else "https://html.duckduckgo.com/html/?q=")+urllib.parse.quote(q)
    req=urllib.request.Request(url, headers={"User-Agent":random.choice(UAS),"Accept":"text/html","Accept-Language":"en-US,en;q=0.9"})
    with urllib.request.urlopen(req,timeout=30) as r: return r.read().decode("utf-8","replace")
def parse(h):
    clean=lambda s: html.unescape(re.sub(r"<[^>]+>","",s)).strip()
    out=[]
    L=re.findall(r'class="result-link"[^>]*>(.*?)</a>',h,re.S); S=re.findall(r'class="result-snippet"[^>]*>(.*?)</td>',h,re.S)
    if not L:
        L=re.findall(r'result__a[^>]*>(.*?)</a>',h,re.S); S=re.findall(r'result__snippet[^>]*>(.*?)</a>',h,re.S)
    for i,t in enumerate(L): out.append({"title":clean(t),"snippet":clean(S[i]) if i<len(S) else ""})
    return out
qs=[l.strip() for l in open(sys.argv[1],encoding="utf-8") if l.strip()]
allres=[]
try: allres=json.load(open("reddit_ddg_all.json",encoding="utf-8"))
except: pass
done={r["q"] for r in allres}
for q in qs:
    if q in done: print("skip",q[:50]); continue
    got=[]
    for a in range(3):
        try: got=parse(fetch(q, a%2==0))
        except Exception as e: got=[]
        if got: break
        time.sleep(8+a*8)
    print(f"{len(got):>2} :: {q[:72]}", flush=True)
    for g in got[:10]: g["q"]=q; allres.append(g)
    json.dump(allres,open("reddit_ddg_all.json","w",encoding="utf-8"),indent=1,ensure_ascii=False)
    time.sleep(random.uniform(6,10))
print("TOTAL",len(allres))
