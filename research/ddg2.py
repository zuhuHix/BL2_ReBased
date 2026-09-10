import urllib.request, urllib.parse, re, html, time, json, random

UAS=["Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0 Safari/537.36",
     "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
     "Mozilla/5.0 (X11; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0"]

def fetch(q, endpoint="lite"):
    if endpoint=="lite":
        url="https://lite.duckduckgo.com/lite/?q="+urllib.parse.quote(q)
    else:
        url="https://html.duckduckgo.com/html/?q="+urllib.parse.quote(q)
    req=urllib.request.Request(url, headers={"User-Agent":random.choice(UAS),
        "Accept":"text/html,application/xhtml+xml","Accept-Language":"en-US,en;q=0.9"})
    with urllib.request.urlopen(req,timeout=30) as r: return r.read().decode("utf-8","replace")

def parse(h):
    clean=lambda s: html.unescape(re.sub(r"<[^>]+>","",s)).strip()
    out=[]
    # lite layout
    links=re.findall(r'class="result-link"[^>]*>(.*?)</a>',h,re.S)
    snips=re.findall(r'class="result-snippet"[^>]*>(.*?)</td>',h,re.S)
    if links:
        for i,t in enumerate(links):
            out.append({"title":clean(t),"snippet":clean(snips[i]) if i<len(snips) else ""})
        return out
    links=re.findall(r'result__a[^>]*>(.*?)</a>',h,re.S)
    snips=re.findall(r'result__snippet[^>]*>(.*?)</a>',h,re.S)
    for i,t in enumerate(links):
        out.append({"title":clean(t),"snippet":clean(snips[i]) if i<len(snips) else ""})
    return out

QUERIES=[l.strip() for l in open("queries.txt",encoding="utf-8") if l.strip()]
allres=[]
for i,q in enumerate(QUERIES):
    got=[]
    for attempt in range(3):
        try:
            got=parse(fetch(q,"lite" if attempt==0 else "html"))
        except Exception as e:
            got=[]
        if got: break
        time.sleep(6+attempt*6)
    print(f"[{i+1}/{len(QUERIES)}] {len(got):>2} :: {q[:70]}")
    for g in got[:10]:
        g["q"]=q; allres.append(g)
    time.sleep(random.uniform(3.5,6.0))
json.dump(allres,open("reddit_ddg.json","w",encoding="utf-8"),indent=1,ensure_ascii=False)
print("TOTAL",len(allres))
