import urllib.request, urllib.parse, re, html, time, json, sys

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36"
def get(url):
    req = urllib.request.Request(url, headers={"User-Agent":UA,"Accept-Language":"en-US,en;q=0.9"})
    with urllib.request.urlopen(req, timeout=35) as r:
        return r.read().decode("utf-8","replace")

def parse(h):
    names=[m[1] for m in re.finditer(r'forum_topic_name[^>]*>([\s\S]*?)</div>',h)]
    reps=[m[1] for m in re.finditer(r'forum_topic_reply_count[^>]*>([\s\S]*?)</div>',h)]
    clean=lambda s: html.unescape(re.sub(r"<[^>]+>","",s)).replace("PINNED:","").strip()
    out=[]
    for i,n in enumerate(names):
        c=clean(n)
        rc=clean(reps[i]) if i<len(reps) else "0"
        try: rcn=int(rc.replace(",",""))
        except: rcn=0
        if c: out.append({"title":c,"replies":rcn})
    return out

APP = sys.argv[1] if len(sys.argv)>1 else "49520"
PAGES = int(sys.argv[2]) if len(sys.argv)>2 else 40
OUT = sys.argv[3] if len(sys.argv)>3 else "steam_topics_bl2.json"

topics=[]; seen=set()
for p in range(1, PAGES+1):
    url=f"https://steamcommunity.com/app/{APP}/discussions/0/?fp={p}"
    try: h=get(url)
    except Exception as e:
        print("ERR",p,e); time.sleep(3); continue
    got=parse(h)
    new=0
    for g in got:
        k=g["title"].lower()
        if k in seen: continue
        seen.add(k); topics.append(g); new+=1
    print(f"page {p}: {len(got)} topics ({new} new), total {len(topics)}")
    if not got: break
    time.sleep(1.2)
json.dump(topics, open(OUT,"w",encoding="utf-8"), indent=1, ensure_ascii=False)
print("SAVED", len(topics))
