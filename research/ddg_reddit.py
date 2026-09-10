import urllib.request, urllib.parse, re, html, time, json, sys, collections

QUERIES = [
 "site:reddit.com borderlands 2 remaster what would you want",
 "site:reddit.com borderlands 2 remake what should change",
 "site:reddit.com borderlands 2 quality of life changes needed",
 "site:reddit.com why is borderlands 2 the best borderlands",
 "site:reddit.com borderlands 2 worst thing about the game",
 "site:reddit.com borderlands 2 slag UVHM bad design",
 "site:reddit.com borderlands 2 OP levels endgame problem",
 "site:reddit.com borderlands 2 aged badly what aged worst",
 "site:reddit.com borderlands 2 unofficial community patch worth it",
 "site:reddit.com borderlands 2 best mods overhaul reborn exodus",
 "site:reddit.com borderlands 2 graphics mod reshade 4k textures",
 "site:reddit.com borderlands 2 multiplayer broken shift servers",
 "site:reddit.com borderlands 2 inventory bank space annoying",
 "site:reddit.com borderlands 2 fast travel backtracking complaint",
 "site:reddit.com borderlands 2 skill trees best designed",
 "site:reddit.com borderlands 2 handsome jack best villain writing",
 "site:reddit.com borderlands 2 dlc ranking tiny tina",
 "site:reddit.com borderlands 2 vs borderlands 3 gunplay movement",
 "site:reddit.com borderlands 2 would a remaster ruin it",
 "site:reddit.com borderlands 2 60fps ultrawide fov mod",
 "site:reddit.com borderlands 2 modding unreal engine 5 remake fan project",
 "site:reddit.com borderlands 2 what do you hate about borderlands 2",
 "site:reddit.com borderlands 2 replay 2025 still worth playing",
 "site:reddit.com borderlands 2 crossplay console pc",
 "site:reddit.com borderlands 2 loot drop rates farming tedious",
]

def ddg(q):
    url = "https://html.duckduckgo.com/html/?q=" + urllib.parse.quote(q)
    req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122 Safari/537.36"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read().decode("utf-8","replace")

results=[]
for q in QUERIES:
    try:
        h = ddg(q)
    except Exception as e:
        print("ERR", q, e); time.sleep(3); continue
    titles = re.findall(r'result__a[^>]*>(.*?)</a>', h, re.S)
    snips  = re.findall(r'result__snippet[^>]*>(.*?)</a>', h, re.S)
    clean = lambda s: html.unescape(re.sub(r"<[^>]+>","",s)).strip()
    for i,t in enumerate(titles[:10]):
        sn = clean(snips[i]) if i < len(snips) else ""
        results.append({"q":q,"title":clean(t),"snippet":sn})
    time.sleep(2.0)

with open("reddit_ddg.json","w",encoding="utf-8") as f:
    json.dump(results,f,indent=1,ensure_ascii=False)
print("COLLECTED",len(results),"results")
